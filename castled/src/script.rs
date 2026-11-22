//! Castle script: the language automations are written in, and its compiler.
//!
//! A script is plain lines, one step each, read top to bottom:
//!
//! ```text
//! castle 1
//! website walmart.com
//!
//! element search box    = input[name="q"]
//! list results          = [data-item-id]
//!   field title = [data-automation-id="product-title"]
//!   field price = [data-automation-id="product-price"]
//!
//! type "motor oil" into search box
//! press enter
//! wait for results
//! read results
//! keep results where title has "0W-20"
//! average results price as average price
//! post results to castle
//! ```
//!
//! Names (`search box`, `average price`) are declared before they are used and may have
//! spaces in them: the compiler matches the longest name it knows. What comes after `=` in
//! a declaration is a selector, passed to the browser as it is.
//!
//! The compiler turns a script into one Python file that calls the `castle` library
//! (clonewars/automation/castle). Every step of it carries its script line, so while it runs
//! Castle shows which line it is on, and a failure names the line, not the Python. The script
//! is checked as a whole before anything runs: every problem comes back with its line.

use std::collections::{BTreeMap, HashSet};

use serde::Serialize;
use serde_json::json;

/// The language version this compiler speaks (`castle 1` at the top of a script).
pub const VERSION: u32 = 1;
const MAX_SCRIPT_BYTES: usize = 64 * 1024;
const MAX_LINE: usize = 2000;
const MAX_NAME: usize = 60;
const MAX_WAIT_SECONDS: f64 = 600.0;
/// Words that join the parts of a step, so a name cannot be made of them alone.
const JOINERS: [&str; 2] = ["as", "where"];
/// The units `measure` understands, by the names a script may use.
const UNITS: [(&str, &str); 18] = [
    ("quart", "quart"), ("quarts", "quart"), ("qt", "quart"),
    ("gallon", "gallon"), ("gallons", "gallon"),
    ("liter", "liter"), ("liters", "liter"), ("litre", "liter"), ("litres", "liter"),
    ("milliliters", "milliliter"),
    ("fluid ounces", "fluid ounce"), ("fl oz", "fluid ounce"),
    ("pound", "pound"), ("pounds", "pound"), ("ounces", "ounce"),
    ("grams", "gram"), ("kilograms", "kilogram"), ("items", "item"),
];
/// Keys `press` knows by a friendly name; anything else goes to the browser as it is.
const KEYS: [(&str, &str); 14] = [
    ("enter", "Enter"), ("return", "Enter"), ("tab", "Tab"), ("escape", "Escape"), ("esc", "Escape"),
    ("space", " "), ("backspace", "Backspace"), ("delete", "Delete"), ("up", "ArrowUp"), ("down", "ArrowDown"),
    ("left", "ArrowLeft"), ("right", "ArrowRight"), ("page down", "PageDown"), ("page up", "PageUp"),
];

/// One problem with a script, on the line it is on (counted from 1).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub line: usize,
    pub message: String,
}

/// A script that compiled: the Python it became, and what it needs and makes.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Compiled {
    pub python: String,
    /// It drives a web browser (`website`), so the machine needs one.
    pub web: bool,
    /// Datasets its `post` steps write, in the order they first appear.
    pub datasets: Vec<String>,
    /// Values its `track` steps follow over time, with which way is better.
    pub tracked: Vec<Tracked>,
    /// Fields (`{NAME}` in its text) it reads from the bot's sign-in.
    pub fields: Vec<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Tracked {
    pub name: String,
    /// "lower", "higher" or "" when the script does not say.
    pub better: String,
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Word(String),
    Text(String),
}

impl Tok {
    fn word(&self) -> Option<&str> {
        match self {
            Tok::Word(w) => Some(w),
            Tok::Text(_) => None,
        }
    }
}

/// A list's fields: (name, selector inside one item, attribute to read or "").
#[derive(Clone, Default)]
struct ListDef {
    selector: String,
    fields: Vec<(String, String, String)>,
    /// Fields worked out from others (`measure`, `divide`), known once that step is seen.
    derived: Vec<String>,
    read: bool,
}

#[derive(Default)]
struct Scope {
    elements: BTreeMap<String, String>,
    lists: BTreeMap<String, ListDef>,
    /// Single values: numbers, text and rows made by earlier steps.
    values: HashSet<String>,
    web: bool,
    website_line: usize,
}

struct Out {
    lines: Vec<String>,
    errors: Vec<Diagnostic>,
    datasets: Vec<String>,
    tracked: Vec<Tracked>,
    fields: Vec<String>,
}

fn py(text: &str) -> String {
    // A JSON string is a Python string literal (the file is UTF-8, so nothing needs \u).
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into())
}

/// A name as the compiler compares it: lower case, single spaces.
fn norm(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn words_of(toks: &[Tok]) -> Option<Vec<String>> {
    toks.iter().map(|t| t.word().map(str::to_lowercase)).collect()
}

/// Splits a step into words and quoted text. `\"` inside quotes is a quote.
fn tokenize(line: &str) -> Result<Vec<Tok>, String> {
    let mut toks = Vec::new();
    let mut chars = line.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' || c == '\u{201c}' {
            chars.next();
            let mut text = String::new();
            let mut closed = false;
            while let Some(c) = chars.next() {
                match c {
                    '\\' if chars.peek().is_some_and(|n| *n == '"' || *n == '\\') => text.push(chars.next().unwrap_or('"')),
                    '"' | '\u{201d}' => {
                        closed = true;
                        break;
                    }
                    _ => text.push(c),
                }
            }
            if !closed {
                return Err("a quote is opened and never closed".into());
            }
            toks.push(Tok::Text(text));
        } else {
            let mut word = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == '"' {
                    break;
                }
                word.push(c);
                chars.next();
            }
            toks.push(Tok::Word(word));
        }
    }
    Ok(toks)
}

/// The longest known name at the start of `toks`, and how many words it took.
fn take_name<'a, I: IntoIterator<Item = &'a String>>(toks: &[Tok], names: I) -> Option<(String, usize)> {
    let mut best: Option<(String, usize)> = None;
    for name in names {
        let parts: Vec<&str> = name.split(' ').collect();
        if parts.len() > toks.len() {
            continue;
        }
        let fits = parts.iter().zip(toks).all(|(p, t)| t.word().is_some_and(|w| w.eq_ignore_ascii_case(p)));
        if fits && best.as_ref().is_none_or(|(_, n)| parts.len() > *n) {
            best = Some((name.clone(), parts.len()));
        }
    }
    best
}

/// A new name made of the words that are left. Letters, digits, spaces and dashes.
fn new_name(toks: &[Tok]) -> Result<String, String> {
    let words = words_of(toks).ok_or("a name is plain words, not quoted text")?;
    if words.is_empty() {
        return Err("a name is missing".into());
    }
    let name = words.join(" ");
    if name.len() > MAX_NAME {
        return Err(format!("a name can be {MAX_NAME} characters at most"));
    }
    if !name.chars().next().is_some_and(|c| c.is_alphabetic()) || !name.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == '_') {
        return Err(format!("\"{name}\" cannot be a name: use letters, digits, spaces and dashes, starting with a letter"));
    }
    if words.iter().any(|w| JOINERS.contains(&w.as_str())) {
        return Err(format!("\"{name}\" cannot be a name: \"as\" and \"where\" join the parts of a step"));
    }
    Ok(name)
}

/// The part of a declaration after `=`: a selector. `label "Search"` and friends stay as
/// they are; the library turns them into the browser's own ways of finding things.
fn selector(text: &str) -> Result<String, String> {
    let s = text.trim();
    if s.is_empty() {
        return Err("put a selector after the =, like #search or button[type=submit]".into());
    }
    Ok(s.to_string())
}

/// `{NAME}` pieces of a text: values of the script or fields of the bot.
fn placeholders(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else { break };
        out.push(after[..close].trim().to_string());
        rest = &after[close + 1..];
    }
    out
}

fn is_number(word: &str) -> Option<f64> {
    word.replace(',', "").parse::<f64>().ok().filter(|n| n.is_finite())
}

fn address(text: &str) -> Result<String, String> {
    let a = text.trim();
    if a.is_empty() || a.contains(char::is_whitespace) {
        return Err("give a web address, like walmart.com".into());
    }
    if a.starts_with("https://") || a.starts_with("http://") {
        return Ok(a.to_string());
    }
    if a.contains("://") {
        return Err("a web address starts with https:// or with the site's name".into());
    }
    Ok(format!("https://{a}"))
}

impl Out {
    fn err(&mut self, line: usize, message: impl Into<String>) {
        self.errors.push(Diagnostic { line, message: message.into() });
    }

    /// One step: its line, what it says, and the Python that does it.
    fn step(&mut self, line: usize, said: &str, body: &[String]) {
        self.lines.push(format!("with run.step({line}, {}):", py(said.trim())));
        for b in body {
            self.lines.push(format!("    {b}"));
        }
    }
}

impl Scope {
    fn list(&self, name: &str) -> Option<&ListDef> {
        self.lists.get(name)
    }

    /// Fields a list has now: declared ones, then worked-out ones.
    fn fields_of(&self, list: &str) -> Vec<String> {
        self.list(list).map(|l| l.fields.iter().map(|f| f.0.clone()).chain(l.derived.iter().cloned()).collect()).unwrap_or_default()
    }

    fn taken(&self, name: &str) -> bool {
        self.elements.contains_key(name) || self.lists.contains_key(name) || self.values.contains(name)
    }

    fn need_web(&self, line: usize, out: &mut Out) -> bool {
        if !self.web {
            out.err(line, "say which website first: website example.com");
        }
        self.web
    }
}

/// Every name a step could start with: elements, lists and values.
fn all_names(scope: &Scope) -> Vec<String> {
    scope.elements.keys().chain(scope.lists.keys()).chain(scope.values.iter()).cloned().collect()
}

/// A list that has been read, at the start of `toks`, or the reason there is none.
fn read_list(scope: &Scope, toks: &[Tok]) -> Result<(String, usize), String> {
    let (name, used) = take_name(toks, scope.lists.keys()).ok_or_else(|| {
        let said = words_of(toks).map(|w| w.join(" ")).unwrap_or_default();
        format!("\"{said}\" does not start with a list. Declare one first: list results = .item")
    })?;
    if !scope.list(&name).is_some_and(|l| l.read) {
        return Err(format!("read {name} first"));
    }
    Ok((name, used))
}

/// A field of `list` at the start of `toks`.
fn field_of(scope: &Scope, list: &str, toks: &[Tok]) -> Result<(String, usize), String> {
    let fields = scope.fields_of(list);
    take_name(toks, fields.iter()).ok_or_else(|| {
        let said = words_of(toks).map(|w| w.join(" ")).unwrap_or_default();
        if fields.is_empty() {
            format!("{list} has no fields. Give it some: field title = .title")
        } else {
            format!("{list} has no field \"{said}\". It has: {}", fields.join(", "))
        }
    })
}

/// `as <new name>` at the start of `toks`.
fn as_name(scope: &Scope, toks: &[Tok]) -> Result<String, String> {
    match toks.first().and_then(Tok::word) {
        Some(w) if w.eq_ignore_ascii_case("as") => {
            let name = new_name(&toks[1..])?;
            if scope.elements.contains_key(&name) || scope.lists.contains_key(&name) {
                return Err(format!("{name} is already an element or a list"));
            }
            Ok(name)
        }
        _ => Err("end it with: as <a name for the answer>".into()),
    }
}

fn word_is(toks: &[Tok], at: usize, word: &str) -> bool {
    toks.get(at).and_then(Tok::word).is_some_and(|w| w.eq_ignore_ascii_case(word))
}

/// Compiles a script into the Python that runs it, or every problem it has.
pub fn compile(src: &str) -> Result<Compiled, Vec<Diagnostic>> {
    if src.len() > MAX_SCRIPT_BYTES {
        return Err(vec![Diagnostic { line: 1, message: format!("a script can be {} KB at most", MAX_SCRIPT_BYTES / 1024) }]);
    }
    let mut scope = Scope::default();
    let mut out = Out { lines: Vec::new(), errors: Vec::new(), datasets: Vec::new(), tracked: Vec::new(), fields: Vec::new() };
    let lines: Vec<&str> = src.lines().collect();
    let mut i = 0;
    let mut seen_step = false;
    let mut open_list: Option<String> = None;

    while i < lines.len() {
        let n = i + 1;
        let raw = lines[i].trim_end();
        i += 1;
        let text = raw.trim();
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        if raw.len() > MAX_LINE {
            out.err(n, format!("a line can be {MAX_LINE} characters at most"));
            continue;
        }
        let indented = raw.starts_with(char::is_whitespace);
        let lower = text.to_lowercase();

        // Fields belong to the list above them, one indent in.
        if indented && lower.starts_with("field ") {
            let Some(list) = open_list.clone() else {
                out.err(n, "a field goes under a list, indented");
                continue;
            };
            let Some((left, right)) = text[6..].split_once('=') else {
                out.err(n, "a field is: field <name> = <selector inside one item>");
                continue;
            };
            let name = match tokenize(left).and_then(|t| new_name(&t)) {
                Ok(name) => name,
                Err(e) => {
                    out.err(n, e);
                    continue;
                }
            };
            // "a @href" reads an attribute; "@data-id" reads one of the item itself.
            let right = right.trim();
            let (sel, attr) = match right.rsplit_once('@') {
                Some((sel, attr)) if !attr.is_empty() && attr.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':') => (sel.trim().to_string(), attr.to_string()),
                _ => (right.to_string(), String::new()),
            };
            if sel.is_empty() && attr.is_empty() {
                out.err(n, "put a selector after the =, like .title, or @href for one of the item's own attributes");
                continue;
            }
            if let Some(l) = scope.lists.get_mut(&list) {
                if l.fields.iter().any(|f| f.0 == name) {
                    out.err(n, format!("{list} already has a field called {name}"));
                    continue;
                }
                l.fields.push((name, sel, attr));
            }
            continue;
        }
        if indented && open_list.is_none() {
            out.err(n, "only a list's fields are indented");
            continue;
        }
        open_list = None;

        // The version line, if there is one, comes before any step.
        if let Some(rest) = lower.strip_prefix("castle ") {
            if seen_step {
                out.err(n, "\"castle 1\" goes on the first line");
            } else if rest.trim() != VERSION.to_string() {
                out.err(n, format!("this Castle reads version {VERSION} scripts"));
            }
            seen_step = true;
            continue;
        }
        seen_step = true;

        // A block of Python, for what the language cannot say yet.
        if lower == "python:" {
            let mut body = Vec::new();
            while i < lines.len() && (lines[i].trim().is_empty() || lines[i].starts_with(char::is_whitespace)) {
                body.push(lines[i]);
                i += 1;
            }
            while body.last().is_some_and(|l| l.trim().is_empty()) {
                body.pop();
                i -= 1;
            }
            let pad = body.iter().filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
            let code: Vec<String> = body.iter().map(|l| if l.trim().is_empty() { String::new() } else { l[pad.min(l.len())..].replace('\t', "    ") }).collect();
            if code.iter().all(|l| l.trim().is_empty()) {
                out.err(n, "indent the Python under python:");
                continue;
            }
            out.step(n, "python", &code);
            continue;
        }

        // Declarations: name = selector.
        for (kw, is_list) in [("element ", false), ("list ", true)] {
            if !lower.starts_with(kw) {
                continue;
            }
            let Some((left, right)) = text[kw.len()..].split_once('=') else {
                out.err(n, format!("write it as: {kw}<name> = <selector>"));
                break;
            };
            let name = match tokenize(left).and_then(|t| new_name(&t)) {
                Ok(name) => name,
                Err(e) => {
                    out.err(n, e);
                    break;
                }
            };
            if scope.taken(&name) {
                out.err(n, format!("{name} is already declared"));
                break;
            }
            match selector(right) {
                Ok(sel) if is_list => {
                    scope.lists.insert(name.clone(), ListDef { selector: sel, ..Default::default() });
                    open_list = Some(name);
                }
                Ok(sel) => {
                    scope.elements.insert(name, sel);
                }
                Err(e) => out.err(n, e),
            }
            break;
        }
        if lower.starts_with("element ") || lower.starts_with("list ") {
            continue;
        }

        let toks = match tokenize(text) {
            Ok(t) => t,
            Err(e) => {
                out.err(n, e);
                continue;
            }
        };
        for t in &toks {
            if let Tok::Text(s) = t {
                for p in placeholders(s) {
                    if !scope.values.contains(&norm(&p)) && !scope.lists.contains_key(&norm(&p)) {
                        if p.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') && !p.is_empty() {
                            if !out.fields.contains(&p) {
                                out.fields.push(p);
                            }
                        } else {
                            out.err(n, format!("{{{p}}} is not a value yet. Values come from steps that end in \"as <name>\"; sign-in fields are written in capitals, like {{API_KEY}}"));
                        }
                    }
                }
            }
        }
        if let Err(e) = statement(n, text, &toks, &mut scope, &mut out) {
            out.err(n, e);
        }
    }

    if out.errors.is_empty() && out.lines.is_empty() {
        out.err(1, "the script has no steps yet");
    }
    if !out.errors.is_empty() {
        out.errors.sort_by_key(|d| d.line);
        return Err(out.errors);
    }

    let mut python = vec![
        "# Made by Castle from automation.castle. Change the script, not this file:".to_string(),
        "# Castle writes it again from the script on every start.".to_string(),
        "import castle".to_string(),
        String::new(),
        "run = castle.start()".to_string(),
        "v = run.values".to_string(),
    ];
    if !scope.elements.is_empty() {
        python.push(format!("ELEMENTS = {}", json!(scope.elements)));
    }
    if !scope.lists.is_empty() {
        let lists: serde_json::Map<String, serde_json::Value> = scope
            .lists
            .iter()
            .map(|(name, l)| (name.clone(), json!({ "selector": l.selector, "fields": l.fields.iter().map(|(f, s, a)| json!([f, s, a])).collect::<Vec<_>>() })))
            .collect();
        python.push(format!("LISTS = {}", serde_json::Value::Object(lists)));
    }
    python.push(String::new());
    python.extend(out.lines);
    python.push(String::new());
    python.push("run.finish()".to_string());
    Ok(Compiled { python: python.join("\n") + "\n", web: scope.web, datasets: out.datasets, tracked: out.tracked, fields: out.fields })
}

/// One step that is not a declaration.
fn statement(n: usize, text: &str, toks: &[Tok], scope: &mut Scope, out: &mut Out) -> Result<(), String> {
    let verb = toks.first().and_then(Tok::word).map(str::to_lowercase).unwrap_or_default();
    let rest = &toks[1.min(toks.len())..];
    let el = |scope: &Scope, toks: &[Tok]| -> Result<String, String> {
        let names: Vec<String> = scope.elements.keys().cloned().collect();
        match take_name(toks, names.iter()) {
            Some((name, used)) if used == toks.len() => Ok(name),
            Some((name, _)) => Err(format!("there is more after {name} than this step understands")),
            None => {
                let said = words_of(toks).map(|w| w.join(" ")).unwrap_or_default();
                Err(format!("\"{said}\" is not an element. Declare it first: element {said} = <selector>"))
            }
        }
    };
    match verb.as_str() {
        "website" => {
            if scope.web {
                return Err(format!("the website is already set on line {}. Use \"go to\" to open another page", scope.website_line));
            }
            let url = address(&text[7..])?;
            scope.web = true;
            scope.website_line = n;
            out.step(n, text, &[format!("run.website({})", py(&url))]);
        }
        "go" => {
            if !word_is(toks, 1, "to") {
                return Err("write it as: go to example.com/page".into());
            }
            if !scope.need_web(n, out) {
                return Ok(());
            }
            let at = text.to_lowercase().find(" to ").map(|p| p + 4).unwrap_or(text.len());
            let url = address(&text[at..])?;
            out.step(n, text, &[format!("run.web.go({})", py(&url))]);
        }
        "type" => {
            if !scope.need_web(n, out) {
                return Ok(());
            }
            let Some(Tok::Text(what)) = rest.first() else {
                return Err("put what to type in quotes: type \"motor oil\" into search box".into());
            };
            if !word_is(rest, 1, "into") {
                return Err("write it as: type \"what\" into <element>".into());
            }
            let target = el(scope, &rest[2..])?;
            out.step(n, text, &[format!("run.web.type(ELEMENTS[{}], run.text({}))", py(&target), py(what))]);
        }
        "click" => {
            if !scope.need_web(n, out) {
                return Ok(());
            }
            let target = el(scope, rest)?;
            out.step(n, text, &[format!("run.web.click(ELEMENTS[{}])", py(&target))]);
        }
        "press" => {
            if !scope.need_web(n, out) {
                return Ok(());
            }
            let words = words_of(rest).ok_or("name the key, like: press enter")?;
            let at = words.iter().position(|w| w == "in");
            let key_words = &words[..at.unwrap_or(words.len())];
            if key_words.is_empty() {
                return Err("name the key, like: press enter".into());
            }
            let said = key_words.join(" ");
            let key = KEYS.iter().find(|(k, _)| *k == said).map(|(_, v)| v.to_string()).unwrap_or_else(|| rest[0].word().unwrap_or("").to_string());
            let target = match at {
                Some(at) => format!("ELEMENTS[{}]", py(&el(scope, &rest[at + 1..])?)),
                None => "None".into(),
            };
            out.step(n, text, &[format!("run.web.press({}, {target})", py(&key))]);
        }
        "wait" => {
            if word_is(toks, 1, "for") {
                if !scope.need_web(n, out) {
                    return Ok(());
                }
                let what = &toks[2..];
                let names: Vec<String> = scope.elements.keys().chain(scope.lists.keys()).cloned().collect();
                let (name, used) = take_name(what, names.iter()).ok_or_else(|| "wait for an element or a list that is declared".to_string())?;
                if used != what.len() {
                    return Err(format!("there is more after {name} than this step understands"));
                }
                let sel = if scope.elements.contains_key(&name) { format!("ELEMENTS[{}]", py(&name)) } else { format!("LISTS[{}][\"selector\"]", py(&name)) };
                out.step(n, text, &[format!("run.web.wait_for({sel})")]);
            } else {
                let secs = toks.get(1).and_then(Tok::word).and_then(is_number).ok_or("write it as: wait 5 seconds, or wait for <element>")?;
                if !(0.0..=MAX_WAIT_SECONDS).contains(&secs) || !toks.get(2).and_then(Tok::word).is_some_and(|w| w.to_lowercase().starts_with("second")) || toks.len() > 3 {
                    return Err(format!("write it as: wait 5 seconds (up to {MAX_WAIT_SECONDS} seconds)"));
                }
                out.step(n, text, &[format!("run.wait({secs})")]);
            }
        }
        "scroll" => {
            if !scope.need_web(n, out) {
                return Ok(());
            }
            let words = words_of(rest).unwrap_or_default().join(" ");
            let call = match words.as_str() {
                "down" => "run.web.scroll_down()",
                "to bottom" | "to the bottom" => "run.web.scroll_to_bottom()",
                _ => return Err("write it as: scroll down, or scroll to the bottom".into()),
            };
            out.step(n, text, &[call.to_string()]);
        }
        "read" => {
            if !scope.need_web(n, out) {
                return Ok(());
            }
            if let Some((list, used)) = take_name(rest, scope.lists.keys()) {
                if used != rest.len() {
                    return Err(format!("read a list as it is: read {list}"));
                }
                if scope.list(&list).is_some_and(|l| l.fields.is_empty()) {
                    return Err(format!("{list} has no fields to read. Indent some under it: field title = .title"));
                }
                if let Some(l) = scope.lists.get_mut(&list) {
                    l.read = true;
                    l.derived.clear();
                }
                out.step(n, text, &[format!("v[{0}] = run.web.read_list(LISTS[{0}][\"selector\"], LISTS[{0}][\"fields\"])", py(&list)), format!("run.say(str(len(v[{0}])) + \" {1} found\")", py(&list), list.replace('"', ""))]);
            } else {
                let (name, used) = take_name(rest, scope.elements.keys()).ok_or("read a list, or read <element> as <name>")?;
                let value = as_name(scope, &rest[used..])?;
                scope.values.insert(value.clone());
                out.step(n, text, &[format!("v[{}] = run.web.read_text(ELEMENTS[{}])", py(&value), py(&name))]);
            }
        }
        "keep" | "drop" => {
            let (list, used) = read_list(scope, rest)?;
            // keep <list> unique by <field>: the first of each, so a listing shown twice counts once.
            if verb == "keep" && word_is(rest, used, "unique") && word_is(rest, used + 1, "by") {
                let (field, fused) = field_of(scope, &list, &rest[used + 2..])?;
                if used + 2 + fused != rest.len() {
                    return Err(format!("write it as: keep {list} unique by {field}"));
                }
                out.step(n, text, &[format!("v[{0}] = castle.unique(v[{0}], {1})", py(&list), py(&field))]);
                return Ok(());
            }
            if !word_is(rest, used, "where") {
                return Err(format!("write it as: {verb} {list} where <field> has \"text\", or keep {list} unique by <field>"));
            }
            let after = &rest[used + 1..];
            let (field, fused) = field_of(scope, &list, after)?;
            let cond = &after[fused..];
            let words: Vec<String> = cond.iter().filter_map(|t| t.word().map(str::to_lowercase)).collect();
            let quoted = cond.iter().find_map(|t| if let Tok::Text(s) = t { Some(s.clone()) } else { None });
            let (op, arg) = match (words.iter().map(String::as_str).collect::<Vec<_>>().as_slice(), quoted) {
                (["has"], Some(s)) => ("has", py(&s)),
                (["matches"], Some(s)) => {
                    if regex_problem(&s) {
                        return Err("that pattern has a bracket or parenthesis that is never closed".into());
                    }
                    ("matches", py(&s))
                }
                (["is", "empty"], None) => ("empty", "None".into()),
                (["is", "not", "empty"], None) => ("not empty", "None".into()),
                (["is", "below", num], None) | (["is", "under", num], None) => ("below", is_number(num).map(|x| x.to_string()).ok_or("below takes a number")?),
                (["is", "above", num], None) | (["is", "over", num], None) => ("above", is_number(num).map(|x| x.to_string()).ok_or("above takes a number")?),
                _ => return Err(format!("say how to {verb}: has \"text\", matches \"pattern\", is empty, is not empty, is below 10, is above 10")),
            };
            out.step(n, text, &[format!("v[{0}] = castle.{verb}(v[{0}], {1}, {2}, {arg})", py(&list), py(&field), py(op))]);
        }
        "measure" => {
            let (list, used) = read_list(scope, rest)?;
            let after = &rest[used..];
            let (field, fused) = field_of(scope, &list, after)?;
            let tail = &after[fused..];
            if !word_is(tail, 0, "in") {
                return Err(format!("write it as: measure {list} {field} in quarts as quarts"));
            }
            let as_at = tail.iter().position(|t| t.word().is_some_and(|w| w.eq_ignore_ascii_case("as"))).ok_or("end it with: as <a name for the amount>")?;
            let unit_said = words_of(&tail[1..as_at]).unwrap_or_default().join(" ");
            let unit = UNITS.iter().find(|(said, _)| *said == unit_said).map(|(_, u)| *u).ok_or_else(|| format!("measure knows quarts, gallons, liters, milliliters, fluid ounces, pounds, ounces, grams, kilograms and items, not \"{unit_said}\""))?;
            let name = as_name(scope, &tail[as_at..])?;
            add_field(scope, &list, &name)?;
            out.step(n, text, &[format!("v[{0}] = castle.measure(v[{0}], {1}, {2}, {3})", py(&list), py(&field), py(unit), py(&name))]);
        }
        "divide" | "multiply" => {
            let (list, used) = read_list(scope, rest)?;
            let after = &rest[used..];
            let (field, fused) = field_of(scope, &list, after)?;
            if !word_is(after, fused, "by") {
                return Err(format!("write it as: {verb} {list} {field} by <field or number> as <name>"));
            }
            let other_toks = &after[fused + 1..];
            let (other, oused) = match other_toks.first().and_then(Tok::word).and_then(is_number) {
                Some(x) => (x.to_string(), 1),
                None => {
                    let (f, u) = field_of(scope, &list, other_toks)?;
                    (py(&f), u)
                }
            };
            let name = as_name(scope, &other_toks[oused..])?;
            add_field(scope, &list, &name)?;
            out.step(n, text, &[format!("v[{0}] = castle.{verb}(v[{0}], {1}, {other}, {2})", py(&list), py(&field), py(&name))]);
        }
        "sort" => {
            let (list, used) = read_list(scope, rest)?;
            if !word_is(rest, used, "by") {
                return Err(format!("write it as: sort {list} by <field>"));
            }
            let after = &rest[used + 1..];
            let (field, fused) = field_of(scope, &list, after)?;
            let tail = words_of(&after[fused..]).unwrap_or_default().join(" ");
            let descending = match tail.as_str() {
                "" | "ascending" | "lowest first" => false,
                "descending" | "highest first" => true,
                _ => return Err("end it with nothing, \"lowest first\" or \"highest first\"".into()),
            };
            let py_bool = if descending { "True" } else { "False" };
            out.step(n, text, &[format!("v[{0}] = castle.sort(v[{0}], {1}, {py_bool})", py(&list), py(&field))]);
        }
        "average" | "sum" | "min" | "max" | "median" | "lowest" | "highest" => {
            let (list, used) = read_list(scope, rest)?;
            let after = &rest[used..];
            let (field, fused) = field_of(scope, &list, after)?;
            let name = as_name(scope, &after[fused..])?;
            scope.values.insert(name.clone());
            out.step(n, text, &[format!("v[{}] = castle.{verb}(v[{}], {})", py(&name), py(&list), py(&field))]);
        }
        "count" => {
            let (list, used) = read_list(scope, rest)?;
            let name = as_name(scope, &rest[used..])?;
            scope.values.insert(name.clone());
            out.step(n, text, &[format!("v[{}] = len(v[{}])", py(&name), py(&list))]);
        }
        "post" => {
            // post <list or value> to castle [as "dataset"]
            let names = all_names(scope);
            let (name, used) = take_name(rest, names.iter()).ok_or("post a list or a value that earlier steps made")?;
            if scope.elements.contains_key(&name) {
                return Err(format!("{name} is an element. Read it into a value first: read {name} as <name>"));
            }
            if scope.lists.contains_key(&name) && !scope.list(&name).is_some_and(|l| l.read) {
                return Err(format!("read {name} first"));
            }
            if !(word_is(rest, used, "to") && word_is(rest, used + 1, "castle")) {
                return Err(format!("write it as: post {name} to castle"));
            }
            let dataset = match &rest[used + 2..] {
                [] => name.clone(),
                [Tok::Word(w), Tok::Text(d)] if w.eq_ignore_ascii_case("as") && !d.trim().is_empty() && d.len() <= MAX_NAME => norm(d),
                _ => return Err("end it with nothing, or with as \"a dataset name\"".into()),
            };
            if !out.datasets.contains(&dataset) {
                out.datasets.push(dataset.clone());
            }
            out.step(n, text, &[format!("run.post({}, v[{}])", py(&dataset), py(&name))]);
        }
        "track" => {
            let names: Vec<String> = scope.values.iter().cloned().collect();
            let (name, used) = take_name(rest, names.iter()).ok_or("track a number an earlier step made, like: average results price as average price")?;
            let tail = words_of(&rest[used..]).unwrap_or_default().join(" ");
            let better = match tail.as_str() {
                "" => "",
                "lower is better" | "(lower is better)" => "lower",
                "higher is better" | "(higher is better)" => "higher",
                _ => return Err("end it with nothing, \"lower is better\" or \"higher is better\"".into()),
            };
            if out.tracked.iter().any(|t| t.name == name) {
                return Err(format!("{name} is tracked already"));
            }
            out.tracked.push(Tracked { name: name.clone(), better: better.into() });
            out.step(n, text, &[format!("run.track({}, v[{}], {})", py(&name), py(&name), py(better))]);
        }
        "say" => {
            let Some(Tok::Text(what)) = rest.first().filter(|_| rest.len() == 1) else {
                return Err("put what to say in quotes: say \"found {count} listings\"".into());
            };
            out.step(n, text, &[format!("run.say(run.text({}))", py(what))]);
        }
        "screenshot" => {
            let label = match rest {
                [] => String::new(),
                [Tok::Text(l)] => l.clone(),
                _ => return Err("write it as: screenshot, or screenshot \"what it shows\"".into()),
            };
            if !scope.need_web(n, out) {
                return Ok(());
            }
            out.step(n, text, &[format!("run.screenshot({})", py(&label))]);
        }
        // pause whenever <element> is shown ["message"]: a person answers it, then the run goes on.
        // fail whenever <element> is shown ["message"]: the run ends. Both hold from here to the end.
        "pause" | "fail" if verb == "pause" || word_is(toks, 1, "whenever") => {
            if !word_is(toks, 1, "whenever") {
                return Err("write it as: pause whenever <element> is shown \"what to do\"".into());
            }
            let after = &toks[2..];
            let (name, used) = take_name(after, scope.elements.keys()).ok_or_else(|| format!("{verb} whenever an element is shown: declare it first, like element robot check = text \"Robot or human?\""))?;
            if !(word_is(after, used, "is") && (word_is(after, used + 1, "shown") || word_is(after, used + 1, "visible"))) {
                return Err(format!("write it as: {verb} whenever {name} is shown \"what to do about it\""));
            }
            let message = match &after[used + 2..] {
                [] => format!("the page shows {name}"),
                [Tok::Text(m)] => m.clone(),
                _ => return Err("end it with nothing, or a message in quotes".into()),
            };
            if !scope.need_web(n, out) {
                return Ok(());
            }
            let pause = if verb == "pause" { "True" } else { "False" };
            out.step(n, text, &[format!("run.web.guard(ELEMENTS[{}], run.text({}), pause={pause})", py(&name), py(&message))]);
        }
        "fail" => {
            // fail if <list or value> is empty ["message"]
            if !word_is(toks, 1, "if") {
                return Err("write it as: fail if results is empty \"why that is a problem\", or fail whenever <element> is shown".into());
            }
            let after = &toks[2..];
            // fail if <element> is shown ["message"]: a page the script cannot go on from.
            if let Some((name, used)) = take_name(after, scope.elements.keys()) {
                if !(word_is(after, used, "is") && (word_is(after, used + 1, "shown") || word_is(after, used + 1, "visible"))) {
                    return Err(format!("write it as: fail if {name} is shown \"what to do about it\""));
                }
                let message = match &after[used + 2..] {
                    [] => format!("the page shows {name}"),
                    [Tok::Text(m)] => m.clone(),
                    _ => return Err("end it with nothing, or a message in quotes".into()),
                };
                if !scope.need_web(n, out) {
                    return Ok(());
                }
                out.step(n, text, &[format!("run.web.fail_if_shown(ELEMENTS[{}], run.text({}))", py(&name), py(&message))]);
                return Ok(());
            }
            let names: Vec<String> = scope.lists.keys().chain(scope.values.iter()).cloned().collect();
            let (name, used) = take_name(after, names.iter()).ok_or("fail if a list or a value is empty, or if an element is shown")?;
            if scope.lists.contains_key(&name) && !scope.list(&name).is_some_and(|l| l.read) {
                return Err(format!("read {name} first"));
            }
            if !(word_is(after, used, "is") && word_is(after, used + 1, "empty")) {
                return Err(format!("write it as: fail if {name} is empty"));
            }
            let message = match &after[used + 2..] {
                [] => format!("{name} is empty"),
                [Tok::Text(m)] => m.clone(),
                _ => return Err("end it with nothing, or a message in quotes".into()),
            };
            out.step(n, text, &[format!("run.fail_if_empty(v.get({}), run.text({}))", py(&name), py(&message))]);
        }
        "" => return Err("a step is missing".into()),
        other => {
            return Err(format!(
                "Castle does not know \"{other}\". Steps start with: website, go to, type, click, press, wait, scroll, read, keep, drop, measure, divide, multiply, sort, average, sum, min, max, median, lowest, highest, count, post, track, say, screenshot, fail if, pause whenever, fail whenever, python:"
            ))
        }
    }
    Ok(())
}

/// Adds a worked-out field to a list.
fn add_field(scope: &mut Scope, list: &str, name: &str) -> Result<(), String> {
    let fields = scope.fields_of(list);
    if fields.iter().any(|f| f == name) {
        return Err(format!("{list} already has a field called {name}"));
    }
    if let Some(l) = scope.lists.get_mut(list) {
        l.derived.push(name.to_string());
    }
    Ok(())
}

/// A pattern whose brackets do not pair up would only fail once the script runs.
fn regex_problem(pattern: &str) -> bool {
    let mut depth: (i32, i32) = (0, 0);
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '(' => depth.0 += 1,
            ')' => depth.0 -= 1,
            '[' => depth.1 += 1,
            ']' => depth.1 -= 1,
            _ => {}
        }
        if depth.0 < 0 || depth.1 < 0 {
            return true;
        }
    }
    depth != (0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WALMART: &str = include_str!("../../app/clonewars/automation/examples/walmart-motor-oil.castle");

    fn errors(src: &str) -> Vec<Diagnostic> {
        compile(src).err().unwrap_or_default()
    }

    #[test]
    fn the_walmart_example_compiles() {
        let c = compile(WALMART).unwrap_or_else(|e| panic!("{e:?}"));
        assert!(c.web);
        assert!(c.python.contains("run.website(\"https://walmart.com\")"));
        assert!(c.python.contains("castle.measure("));
        assert_eq!(c.tracked.first().map(|t| t.better.as_str()), Some("lower"));
        assert!(c.python.trim_end().ends_with("run.finish()"));
    }

    /// Every whole script the language's pages show (```castle blocks) and every file in
    /// examples/ compiles, so the documentation cannot drift from the language.
    #[test]
    fn the_documentation_and_the_examples_compile() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/clonewars/automation");
        let mut scripts = Vec::new();
        for dir in [root.clone(), root.join("docs")] {
            for path in std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()) {
                if path.extension().is_some_and(|e| e == "md") {
                    let text = std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
                    for (i, block) in text.split("```castle\n").skip(1).enumerate() {
                        scripts.push((format!("{} (script {})", path.display(), i + 1), block.split("```").next().unwrap_or_default().to_string()));
                    }
                }
            }
        }
        for path in std::fs::read_dir(root.join("examples")).unwrap().flatten().map(|e| e.path()) {
            if path.extension().is_some_and(|e| e == "castle") {
                scripts.push((path.display().to_string(), std::fs::read_to_string(&path).unwrap()));
            }
        }
        assert!(scripts.len() >= 6, "found only {} scripts", scripts.len());
        for (name, script) in scripts {
            if let Err(e) = compile(&script) {
                panic!("{name} does not compile: {e:?}");
            }
        }
    }

    #[test]
    fn names_with_spaces_are_matched_longest_first() {
        let c = compile("website example.com\nelement search = #s\nelement search button = #b\nclick search button\nclick search\n").unwrap();
        assert!(c.python.contains("run.web.click(ELEMENTS[\"search button\"])"));
        assert!(c.python.contains("run.web.click(ELEMENTS[\"search\"])"));
    }

    #[test]
    fn every_step_carries_its_line() {
        let c = compile("castle 1\n\nwebsite example.com\n# a comment\nwait 2 seconds\n").unwrap();
        assert!(c.python.contains("with run.step(3, \"website example.com\"):"));
        assert!(c.python.contains("with run.step(5, \"wait 2 seconds\"):"));
    }

    #[test]
    fn problems_come_back_with_their_lines_all_at_once() {
        let e = errors("website example.com\nclick search box\nfrobnicate it\nread results\n");
        let lines: Vec<usize> = e.iter().map(|d| d.line).collect();
        assert_eq!(lines, [2, 3, 4]);
        assert!(e[0].message.contains("element search box ="));
        assert!(e[1].message.contains("does not know \"frobnicate\""));
    }

    #[test]
    fn a_list_must_be_read_before_it_is_used() {
        let src = "website example.com\nlist rows = .r\n  field price = .p\naverage rows price as avg\n";
        assert!(errors(src)[0].message.contains("read rows first"));
        let ok = "website example.com\nlist rows = .r\n  field price = .p\nread rows\naverage rows price as avg\ntrack avg lower is better\npost rows to castle\n";
        let c = compile(ok).unwrap();
        assert_eq!(c.datasets, ["rows"]);
        assert_eq!(c.tracked, [Tracked { name: "avg".into(), better: "lower".into() }]);
    }

    #[test]
    fn fields_read_attributes_and_worked_out_fields_join_the_list() {
        let src = "website example.com\nlist rows = .r\n  field link = a @href\n  field id = @data-id\n  field size = .s\nread rows\nmeasure rows size in quarts as quarts\ndivide rows quarts by 2 as halves\nsort rows by halves highest first\n";
        let c = compile(src).unwrap();
        assert!(c.python.contains("[\"link\",\"a\",\"href\"]"));
        assert!(c.python.contains("[\"id\",\"\",\"data-id\"]"));
        assert!(c.python.contains("castle.divide(v[\"rows\"], \"quarts\", 2, \"halves\")"));
        assert!(c.python.contains("castle.sort(v[\"rows\"], \"halves\", True)"));
    }

    #[test]
    fn quotes_in_text_cannot_break_out_of_the_python() {
        let c = compile("website example.com\nelement box = #q\ntype \"x\\\"); import os #\" into box\n").unwrap();
        assert!(c.python.contains(r#"run.text("x\"); import os #")"#));
    }

    #[test]
    fn placeholders_are_values_or_capital_fields() {
        let c = compile("website example.com\nelement box = #q\ntype \"{EMAIL}\" into box\n").unwrap();
        assert_eq!(c.fields, ["EMAIL"]);
        assert!(errors("website example.com\nelement box = #q\ntype \"{nothing}\" into box\n")[0].message.contains("{nothing} is not a value yet"));
    }

    #[test]
    fn web_steps_need_a_website_and_only_one() {
        assert!(errors("element b = #b\nclick b\n")[0].message.contains("say which website first"));
        assert!(errors("website a.com\nwebsite b.com\n")[0].message.contains("already set on line 1"));
        assert!(errors("website ftp://a.com\n")[0].message.contains("https://"));
    }

    #[test]
    fn python_blocks_keep_their_own_indent() {
        let c = compile("castle 1\npython:\n    x = 1\n    if x:\n        print(x)\nsay \"done\"\n").unwrap();
        assert!(c.python.contains("with run.step(2, \"python\"):\n    x = 1\n    if x:\n        print(x)\n"));
        assert!(c.python.contains("with run.step(6, "));
    }

    #[test]
    fn names_cannot_be_joining_words_or_odd_characters() {
        assert!(errors("website a.com\nelement price as text = #p\n")[0].message.contains("cannot be a name"));
        assert!(errors("website a.com\nelement x\"y = #p\n")[0].message.contains("quote"));
        assert!(errors("website a.com\nelement 1st = #p\n")[0].message.contains("cannot be a name"));
    }

    #[test]
    fn the_version_line_is_first_and_known() {
        assert!(errors("castle 2\nwait 1 seconds\n")[0].message.contains("version 1"));
        assert!(errors("wait 1 seconds\ncastle 1\n")[0].message.contains("first line"));
    }

    #[test]
    fn a_page_the_script_cannot_go_on_from_ends_the_run() {
        let c = compile("website a.com
element robot check = text \"Robot or human?\"
fail if robot check is shown \"press and hold it once\"
").unwrap();
        assert!(c.python.contains("run.web.fail_if_shown(ELEMENTS[\"robot check\"], run.text(\"press and hold it once\"))"));
        assert!(errors("website a.com
element x = #x
fail if x is empty
")[0].message.contains("fail if x is shown"));
    }

    #[test]
    fn a_watch_holds_for_the_rest_of_the_run() {
        let c = compile("website a.com\nelement check = text \"Robot?\"\npause whenever check is shown \"answer it\"\nfail whenever check is visible\n").unwrap();
        assert!(c.python.contains("run.web.guard(ELEMENTS[\"check\"], run.text(\"answer it\"), pause=True)"));
        assert!(c.python.contains("run.web.guard(ELEMENTS[\"check\"], run.text(\"the page shows check\"), pause=False)"));
        assert!(errors("website a.com\npause whenever nothing is shown\n")[0].message.contains("declare it first"));
        assert!(errors("element c = #c\npause whenever c is shown\n")[0].message.contains("say which website first"));
    }

    #[test]
    fn an_empty_script_is_a_problem() {
        assert!(errors("# nothing\n")[0].message.contains("no steps"));
    }
}
