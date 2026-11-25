//! Links: pages from the internet that a person saved, read by the box and kept.
//!
//! A person hands the box an address (pasted in the app, or shared to it from a phone). The
//! box fetches the page once and keeps what it says: the title, who published it, its
//! picture, its text, and, when the page carries one, its recipe (the ingredients and
//! steps that recipe sites publish for search engines, schema.org/Recipe). A video is kept
//! with its description, and the addresses in that description are followed once, because
//! the recipe a video shows is usually on the maker's own site. From then on the link is
//! searched and read from the box, whether or not the page still exists.
//!
//! On disk, one person's links are one folder, `<vault>/.castle/links/<person>/`, with one
//! JSON file per link (`<id>.json`, everything the box read) and the link's picture beside
//! it (`<id>.jpg`). Nothing else: no index and no database, so the folder can be read,
//! copied or backed up as it is.
//!
//! This reaches out to the internet, and only when a link is added or refreshed. It never
//! fetches an address on this computer or on the home network: every name is looked up
//! first, and a page that answers from a private address is refused, as is a redirect to
//! one. Otherwise a saved link would be a way to read the router's admin page.
//!
//! Who sees what: as with Contacts and Comms, the app names the person and the box keeps
//! each person's links apart. Until the box has accounts that is a convention, not a lock.

use std::{
    collections::HashMap,
    io::Read,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::{Url, blocking::Client, header, redirect::Policy};
use serde::{Deserialize, Serialize};

use crate::contacts::{valid_person, write_json};

pub type LinksResult<T> = Result<T, String>;

/// Says what is asking, and where to read about it. Sites that turn this away are not
/// worked around: the link is kept with the reason, and can be opened in a browser.
const USER_AGENT: &str = concat!("Mozilla/5.0 (compatible; Castle/", env!("CARGO_PKG_VERSION"), "; +https://castle.dogs.red)");
const TIMEOUT: Duration = Duration::from_secs(15);
const MAX_REDIRECTS: usize = 5;
const MAX_PAGE: usize = 4 * 1024 * 1024;
const MAX_PICTURE: usize = 5 * 1024 * 1024;
/// How much of a page's text is kept, in bytes.
const MAX_TEXT: usize = 60_000;
const MAX_DESCRIPTION: usize = 5_000;
const MAX_TITLE: usize = 300;
const MAX_NOTE: usize = 2_000;
const MAX_TAGS: usize = 12;
const MAX_TAG: usize = 40;
const MAX_LIST: usize = 200;
const MAX_LINKS: usize = 20_000;
const MAX_URL: usize = 2_000;
/// Addresses in a video's description that are followed, looking for the recipe.
const MAX_FOLLOWED: usize = 4;
const MAX_RELATED: usize = 12;
const SNIPPET_BEFORE: usize = 60;
const SNIPPET_AFTER: usize = 140;
/// Query fields that only track where a visitor came from.
const TRACKING: [&str; 6] = ["fbclid", "gclid", "igshid", "mc_cid", "mc_eid", "ref_src"];
/// Places a video's description links to that are never the recipe.
const NOT_THE_SOURCE: [&str; 20] = [
    "youtube.com", "youtu.be", "instagram.com", "facebook.com", "twitter.com", "x.com", "tiktok.com", "amazon.com", "amzn.to", "patreon.com",
    "discord.gg", "discord.com", "twitch.tv", "pinterest.com", "threads.net", "linktr.ee", "spotify.com", "apple.com", "google.com", "paypal.com",
];
/// Parts of a page that are not its words.
const SKIPPED: [&str; 13] = ["script", "style", "noscript", "svg", "template", "nav", "footer", "header", "aside", "form", "iframe", "button", "select"];
const BLOCKS: [&str; 22] = [
    "p", "div", "br", "li", "h1", "h2", "h3", "h4", "h5", "h6", "tr", "section", "article", "ul", "ol", "blockquote", "pre", "table", "hr", "dd", "dt", "figcaption",
];
const ENTITIES: [(&str, &str); 30] = [
    ("amp", "&"), ("lt", "<"), ("gt", ">"), ("quot", "\""), ("apos", "'"), ("nbsp", " "), ("mdash", "—"), ("ndash", "–"), ("hellip", "…"),
    ("rsquo", "’"), ("lsquo", "‘"), ("rdquo", "”"), ("ldquo", "“"), ("frac12", "½"), ("frac14", "¼"), ("frac34", "¾"), ("deg", "°"),
    ("times", "×"), ("copy", "©"), ("reg", "®"), ("trade", "™"), ("eacute", "é"), ("egrave", "è"), ("aacute", "á"), ("iacute", "í"),
    ("oacute", "ó"), ("uacute", "ú"), ("ntilde", "ñ"), ("uuml", "ü"), ("ccedil", "ç"),
];

fn now_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// `text` cut to at most `max` bytes, on a character.
fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", text[..end].trim_end())
}

// ---------------------------------------------------------------------------------------
// What is kept
// ---------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Recipe {
    pub name: String,
    /// The page the recipe was read from, when that is not the link itself.
    pub source: String,
    pub yields: String,
    pub prep: String,
    pub cook: String,
    pub total: String,
    pub ingredients: Vec<String>,
    pub steps: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Link {
    pub id: String,
    pub url: String,
    pub title: String,
    /// Who published it: "Serious Eats", or the site's name.
    pub site: String,
    pub author: String,
    pub description: String,
    /// "page", "video" or "recipe".
    pub kind: String,
    /// The picture's file name in the person's folder, or empty.
    pub picture: String,
    pub added: u64,
    /// When the page was last read, or 0 when it never could be.
    pub fetched: u64,
    /// Why the page could not be read, in words for the person. Empty when it was.
    pub problem: String,
    pub note: String,
    pub tags: Vec<String>,
    pub recipe: Option<Recipe>,
    /// Addresses found in a video's description.
    pub related: Vec<String>,
    /// The page's own words, for reading and for search.
    pub text: String,
}

/// A link as lists show it: everything but its text.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: String,
    pub url: String,
    pub title: String,
    pub site: String,
    pub author: String,
    pub description: String,
    pub kind: String,
    pub has_picture: bool,
    pub has_recipe: bool,
    pub added: u64,
    pub problem: String,
    pub note: String,
    pub tags: Vec<String>,
    /// The words around what a search found, when it found them in the page's text.
    pub snippet: String,
}

fn card(link: &Link, snippet: String) -> Card {
    Card {
        id: link.id.clone(),
        url: link.url.clone(),
        title: link.title.clone(),
        site: link.site.clone(),
        author: link.author.clone(),
        description: clip(&link.description, 400),
        kind: link.kind.clone(),
        has_picture: !link.picture.is_empty(),
        has_recipe: link.recipe.is_some(),
        added: link.added,
        problem: link.problem.clone(),
        note: link.note.clone(),
        tags: link.tags.clone(),
        snippet,
    }
}

// ---------------------------------------------------------------------------------------
// Addresses
// ---------------------------------------------------------------------------------------

/// The address in what a person pasted or shared. A phone's Share often sends words with
/// the address in them ("Look at this https://..."), and a typed one often has no "https://".
pub fn url_in(text: &str) -> Option<Url> {
    let text = text.trim();
    if text.is_empty() || text.len() > MAX_URL * 2 {
        return None;
    }
    let found = text.split_whitespace().find(|word| {
        let lower = word.to_ascii_lowercase();
        lower.starts_with("http://") || lower.starts_with("https://")
    });
    let candidate = match found {
        Some(word) => word.trim_end_matches(['.', ',', ')', ']', '!', '?', ';', '"', '\'']).to_string(),
        None if !text.contains(char::is_whitespace) && text.contains('.') => format!("https://{text}"),
        None => return None,
    };
    let url = Url::parse(&candidate).ok()?;
    let named = url.host_str().is_some_and(|host| host.contains('.') || host.contains(':'));
    (matches!(url.scheme(), "http" | "https") && named && candidate.len() <= MAX_URL).then_some(url)
}

fn host_of(url: &Url) -> String {
    url.host_str().unwrap_or("").trim_start_matches("www.").to_string()
}

/// The video a YouTube address is for.
fn youtube_id(url: &Url) -> Option<String> {
    let host = host_of(url);
    let host = host.trim_start_matches("m.").trim_start_matches("music.");
    let id = match host {
        "youtu.be" => url.path_segments()?.next()?.to_string(),
        "youtube.com" | "youtube-nocookie.com" => {
            let mut parts = url.path_segments()?;
            match parts.next()? {
                "watch" => url.query_pairs().find(|(key, _)| key == "v")?.1.to_string(),
                "shorts" | "embed" | "live" => parts.next()?.to_string(),
                _ => return None,
            }
        }
        _ => return None,
    };
    (!id.is_empty() && id.len() <= 20 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')).then_some(id)
}

/// One address for one page: no fragment and no tracking fields, and a video by its id.
/// Two shares of the same thing are the same link.
pub fn tidy(url: &Url) -> Url {
    if let Some(id) = youtube_id(url) {
        if let Ok(video) = Url::parse(&format!("https://www.youtube.com/watch?v={id}")) {
            return video;
        }
    }
    let mut out = url.clone();
    out.set_fragment(None);
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| !key.starts_with("utm_") && !TRACKING.contains(&key.as_ref()))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    if kept.is_empty() {
        out.set_query(None);
    } else {
        out.query_pairs_mut().clear().extend_pairs(kept);
    }
    out
}

/// True for an address out on the internet: not this computer, not the home network, not
/// a Tailscale or carrier-private range, not a multicast or reserved one.
pub fn public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                || a == 0
                || (a == 100 && (64..128).contains(&b))
                || (a == 192 && b == 0)
                || (a == 198 && (18..20).contains(&b))
                || a >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return public_address(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || first == 0x2001 && v6.segments()[1] == 0x0db8
                || first == 0x0064)
        }
    }
}

/// Where a name leads, refused unless every answer is out on the internet.
fn public_addresses(url: &Url) -> LinksResult<Vec<SocketAddr>> {
    let addrs = url.socket_addrs(|| None).map_err(|_| format!("{} could not be found", host_of(url)))?;
    if addrs.is_empty() {
        return Err(format!("{} could not be found", host_of(url)));
    }
    if addrs.iter().any(|addr| !public_address(addr.ip())) {
        return Err("that address is on this computer or the home network, which Links does not read".into());
    }
    Ok(addrs)
}

// ---------------------------------------------------------------------------------------
// Fetching
// ---------------------------------------------------------------------------------------

pub struct Fetched {
    /// Where the page really was, after any redirects.
    pub url: Url,
    pub mime: String,
    pub body: Vec<u8>,
}

/// Reads one address from the internet. Redirects are followed here, one at a time, so
/// each stop is checked the way the first address was; and the connection goes to the
/// very addresses that were checked, so a name cannot answer differently the second time.
pub fn fetch(url: &Url, accept: &str, max: usize) -> LinksResult<Fetched> {
    let mut url = url.clone();
    for _ in 0..=MAX_REDIRECTS {
        if !matches!(url.scheme(), "http" | "https") {
            return Err("that is not a web address".into());
        }
        let site = host_of(&url);
        let addrs = public_addresses(&url)?;
        let client = Client::builder()
            .timeout(TIMEOUT)
            .user_agent(USER_AGENT)
            .redirect(Policy::none())
            .resolve_to_addrs(url.host_str().unwrap_or(""), &addrs)
            .build()
            .map_err(|_| "could not start a web request".to_string())?;
        let res = client
            .get(url.clone())
            .header(header::ACCEPT, accept)
            .header(header::ACCEPT_LANGUAGE, "en-US,en;q=0.8")
            .send()
            .map_err(|err| if err.is_timeout() { format!("{site} took too long to answer") } else { format!("{site} did not answer") })?;
        let status = res.status();
        if status.is_redirection() {
            let next = res.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).and_then(|to| url.join(to).ok());
            url = next.ok_or_else(|| format!("{site} sent the request nowhere"))?;
            continue;
        }
        if !status.is_success() {
            return Err(match status.as_u16() {
                401 | 403 | 429 => format!("{site} would not show the page to the box ({})", status.as_u16()),
                404 | 410 => format!("{site} says the page is gone ({})", status.as_u16()),
                code => format!("{site} answered with an error ({code})"),
            });
        }
        let mime = res.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").split(';').next().unwrap_or("").trim().to_ascii_lowercase();
        let mut body = Vec::new();
        res.take(max as u64 + 1).read_to_end(&mut body).map_err(|_| format!("{site} stopped answering part way"))?;
        if body.len() > max {
            return Err(format!("the page at {site} is too big to keep"));
        }
        return Ok(Fetched { url, mime, body });
    }
    Err("the page kept sending the request somewhere else".into())
}

fn fetch_page(url: &Url) -> LinksResult<Fetched> {
    let page = fetch(url, "text/html,application/xhtml+xml;q=0.9,*/*;q=0.5", MAX_PAGE)?;
    if !page.mime.is_empty() && !page.mime.contains("html") && !page.mime.starts_with("text/") {
        return Err(format!("that address is a file ({}), not a page", page.mime));
    }
    Ok(page)
}

/// The extension for a picture, by how its bytes begin. None for anything else.
fn picture_kind(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0xff, 0xd8, 0xff, ..] => Some("jpg"),
        [0x89, b'P', b'N', b'G', ..] => Some("png"),
        [b'G', b'I', b'F', b'8', ..] => Some("gif"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some("webp"),
        _ => None,
    }
}

pub fn picture_mime(name: &str) -> &'static str {
    match name.rsplit('.').next() {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    }
}

// ---------------------------------------------------------------------------------------
// Reading a page
// ---------------------------------------------------------------------------------------

/// `&amp;` and its kin as the characters they stand for.
fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let end = rest.char_indices().skip(1).take(10).find(|(_, c)| *c == ';').map(|(i, _)| i);
        let decoded = end.and_then(|end| {
            let name = &rest[1..end];
            let found = if let Some(code) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                u32::from_str_radix(code, 16).ok().and_then(char::from_u32).map(String::from)
            } else if let Some(code) = name.strip_prefix('#') {
                code.parse::<u32>().ok().and_then(char::from_u32).map(String::from)
            } else {
                ENTITIES.iter().find(|(known, _)| *known == name).map(|(_, to)| to.to_string())
            };
            found.map(|text| (text, end))
        });
        match decoded {
            Some((text, end)) => {
                out.push_str(&text);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// One line of plain words from a fragment that may hold tags and entities.
fn words(fragment: &str) -> String {
    let mut out = String::with_capacity(fragment.len());
    let mut in_tag = false;
    for c in fragment.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    // Twice: recipe sites often store "&amp;frac12;".
    decode_entities(&decode_entities(&out)).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A page, with a lower-case copy of the same length for finding tags whatever their case.
struct Doc<'a> {
    html: &'a str,
    lower: String,
}

struct Tag {
    attrs: HashMap<String, String>,
    /// Where the tag's own text ends: the index after its `>`.
    end: usize,
}

fn parse_attrs(inside: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    let bytes = inside.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'=' && bytes[i] != b'/' {
            i += 1;
        }
        if i == start {
            break;
        }
        let name = inside[start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i];
                let from = i + 1;
                i = from;
                while i < bytes.len() && bytes[i] != quote {
                    i += 1;
                }
                value = decode_entities(&inside[from..i]);
                i += 1;
            } else {
                let from = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                value = decode_entities(&inside[from..i]);
            }
        }
        attrs.entry(name).or_insert(value);
    }
    attrs
}

impl<'a> Doc<'a> {
    fn new(html: &'a str) -> Self {
        Self { html, lower: html.to_ascii_lowercase() }
    }

    /// Every `<name ...>` in the page, in order.
    fn tags(&self, name: &str) -> Vec<Tag> {
        let open = format!("<{name}");
        let bytes = self.lower.as_bytes();
        let mut found = Vec::new();
        let mut from = 0;
        while let Some(at) = self.lower[from..].find(&open).map(|i| i + from) {
            let after = at + open.len();
            from = after;
            if !bytes.get(after).is_some_and(|b| b.is_ascii_whitespace() || *b == b'>' || *b == b'/') {
                continue;
            }
            // The tag ends at the first `>` that is not inside a quoted value.
            let mut quote = 0u8;
            let mut end = None;
            for (i, b) in bytes[after..].iter().enumerate() {
                match (*b, quote) {
                    (b'"' | b'\'', 0) => quote = *b,
                    (b, q) if q != 0 && b == q => quote = 0,
                    (b'>', 0) => {
                        end = Some(after + i);
                        break;
                    }
                    _ => {}
                }
            }
            let Some(end) = end else { break };
            found.push(Tag { attrs: parse_attrs(&self.html[after..end]), end: end + 1 });
            from = end + 1;
        }
        found
    }

    /// What is between a tag and its closing tag.
    fn inside(&self, tag: &Tag, name: &str) -> &'a str {
        let close = self.lower[tag.end..].find(&format!("</{name}")).map_or(self.html.len(), |i| i + tag.end);
        &self.html[tag.end..close]
    }

    /// `<meta property="og:title" content="...">`: the first that has something to say.
    fn meta(&self, metas: &[Tag], names: &[&str]) -> String {
        for wanted in names {
            for tag in metas {
                let key = tag.attrs.get("property").or_else(|| tag.attrs.get("name")).or_else(|| tag.attrs.get("itemprop"));
                if key.is_some_and(|key| key.eq_ignore_ascii_case(wanted)) {
                    let content = tag.attrs.get("content").map(|c| words(c)).unwrap_or_default();
                    if !content.is_empty() {
                        return content;
                    }
                }
            }
        }
        String::new()
    }

    /// The page's own words: its article when it marks one, without menus, scripts and
    /// forms, one paragraph to a line.
    fn text(&self) -> String {
        let region = ["article", "main", "body"].iter().find_map(|name| {
            let start = self.tags(name).into_iter().next()?.end;
            let end = self.lower.rfind(&format!("</{name}")).filter(|end| *end > start).unwrap_or(self.html.len());
            Some((start, end))
        });
        let (start, end) = region.unwrap_or((0, self.html.len()));
        let (html, lower) = (&self.html[start..end], &self.lower[start..end]);
        let mut raw = String::with_capacity(html.len() / 2);
        let mut i = 0;
        while let Some(at) = html[i..].find('<').map(|n| n + i) {
            raw.push_str(&html[i..at]);
            if html[at..].starts_with("<!--") {
                i = html[at..].find("-->").map_or(html.len(), |n| at + n + 3);
                continue;
            }
            let Some(close) = html[at..].find('>').map(|n| n + at) else {
                i = html.len();
                break;
            };
            let name: String = lower[at + 1..close].trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            let closing = lower[at + 1..].starts_with('/');
            i = close + 1;
            if !closing && SKIPPED.contains(&name.as_str()) {
                i = lower[i..].find(&format!("</{name}")).and_then(|n| lower[i + n..].find('>').map(|m| i + n + m + 1)).unwrap_or(html.len());
            } else if BLOCKS.contains(&name.as_str()) {
                raw.push('\n');
            }
        }
        raw.push_str(&html[i.min(html.len())..]);
        let lines: Vec<String> = decode_entities(&raw).lines().map(|line| line.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|line| !line.is_empty()).collect();
        clip(&lines.join("\n"), MAX_TEXT)
    }
}

/// "PT1H30M" (how recipe pages write a time) as "1 h 30 min".
fn duration(iso: &str) -> String {
    let Some(rest) = iso.trim().strip_prefix('P') else { return String::new() };
    let (mut minutes, mut number, mut in_time) = (0f64, String::new(), false);
    for c in rest.chars() {
        match c {
            'T' => in_time = true,
            '0'..='9' | '.' => number.push(c),
            unit => {
                let n: f64 = number.parse().unwrap_or(0.0);
                number.clear();
                minutes += match (unit, in_time) {
                    ('D', false) => n * 1440.0,
                    ('H', true) => n * 60.0,
                    ('M', true) => n,
                    ('S', true) => n / 60.0,
                    _ => 0.0,
                };
            }
        }
    }
    let minutes = minutes.round() as u64;
    match (minutes / 60, minutes % 60) {
        (0, 0) => String::new(),
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// A name out of the shapes schema.org allows: "Ana", {"name": "Ana"} or a list of those.
fn name_of(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => words(text),
        serde_json::Value::Object(fields) => fields.get("name").map(name_of).unwrap_or_default(),
        serde_json::Value::Array(items) => items.iter().map(name_of).filter(|name| !name.is_empty()).collect::<Vec<_>>().join(", "),
        _ => String::new(),
    }
}

/// A recipe's steps, flattened: a step may be words, a step object, or a named section
/// holding more steps.
fn steps_of(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => out.extend(text.lines().map(words).filter(|line| !line.is_empty())),
        serde_json::Value::Array(items) => items.iter().for_each(|item| steps_of(item, out)),
        serde_json::Value::Object(fields) => {
            if let Some(inner) = fields.get("itemListElement") {
                let heading = fields.get("name").map(name_of).unwrap_or_default();
                if !heading.is_empty() {
                    out.push(format!("{heading}:"));
                }
                steps_of(inner, out);
            } else if let Some(text) = fields.get("text").or_else(|| fields.get("name")) {
                steps_of(text, out);
            }
        }
        _ => {}
    }
}

fn is_type(value: &serde_json::Value, wanted: &str) -> bool {
    match &value["@type"] {
        serde_json::Value::String(kind) => kind == wanted,
        serde_json::Value::Array(kinds) => kinds.iter().any(|kind| kind == wanted),
        _ => false,
    }
}

/// The first recipe anywhere in a page's structured data.
fn find_recipe(value: &serde_json::Value) -> Option<&serde_json::Value> {
    match value {
        serde_json::Value::Array(items) => items.iter().find_map(find_recipe),
        serde_json::Value::Object(fields) => {
            if is_type(value, "Recipe") {
                return Some(value);
            }
            fields.get("@graph").and_then(find_recipe).or_else(|| fields.get("mainEntity").and_then(find_recipe))
        }
        _ => None,
    }
}

fn recipe_from(data: &serde_json::Value) -> Option<(Recipe, String, String)> {
    let found = find_recipe(data)?;
    let text = |key: &str| found.get(key).map(name_of).unwrap_or_default();
    let yields = match &found["recipeYield"] {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Array(items) => items.iter().map(name_of).max_by_key(String::len).unwrap_or_default(),
        other => name_of(other),
    };
    let ingredients = match &found["recipeIngredient"] {
        serde_json::Value::Array(items) => items.iter().map(name_of).filter(|line| !line.is_empty()).collect(),
        serde_json::Value::String(text) => text.lines().map(words).filter(|line| !line.is_empty()).collect(),
        _ => Vec::new(),
    };
    let mut steps = Vec::new();
    steps_of(&found["recipeInstructions"], &mut steps);
    if ingredients.is_empty() && steps.is_empty() {
        return None;
    }
    let picture = match &found["image"] {
        serde_json::Value::String(url) => url.clone(),
        serde_json::Value::Array(items) => items.first().map(|first| first.as_str().map(str::to_string).unwrap_or_else(|| first["url"].as_str().unwrap_or("").to_string())).unwrap_or_default(),
        serde_json::Value::Object(fields) => fields.get("url").and_then(|url| url.as_str()).unwrap_or("").to_string(),
        _ => String::new(),
    };
    let recipe = Recipe {
        name: text("name"),
        source: String::new(),
        yields,
        prep: duration(&text("prepTime")),
        cook: duration(&text("cookTime")),
        total: duration(&text("totalTime")),
        ingredients,
        steps,
    };
    Some((recipe, text("author"), picture))
}

/// A quoted JSON string's value, found by the name before it: `"shortDescription":"..."`.
/// This is how a video page carries its full description.
fn embedded_string(html: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\":\"");
    let start = html.find(&marker)? + marker.len() - 1;
    let bytes = html.as_bytes();
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return serde_json::from_str(&html[start..=i]).ok(),
            _ => i += 1,
        }
    }
    None
}

/// The web addresses in a piece of writing, in order, each once.
fn urls_in(text: &str) -> Vec<Url> {
    let mut found: Vec<Url> = Vec::new();
    for word in text.split_whitespace() {
        let Some(at) = word.find("https://").or_else(|| word.find("http://")) else { continue };
        let candidate = word[at..].trim_end_matches(['.', ',', ')', ']', '!', '?', ';', '"', '\'']);
        if let Ok(url) = Url::parse(candidate) {
            if url.host_str().is_some() && !found.contains(&url) {
                found.push(url);
            }
        }
    }
    found
}

/// What one page says, before anything is fetched on its account.
#[derive(Default, Debug)]
struct Reading {
    title: String,
    site: String,
    author: String,
    description: String,
    kind: String,
    picture: String,
    recipe: Option<Recipe>,
    related: Vec<String>,
    text: String,
}

fn read_page(url: &Url, html: &str) -> Reading {
    let doc = Doc::new(html);
    let metas = doc.tags("meta");
    let video = youtube_id(url).is_some();
    let mut reading = Reading {
        title: doc.meta(&metas, &["og:title", "twitter:title"]),
        site: doc.meta(&metas, &["og:site_name", "application-name"]),
        author: doc.meta(&metas, &["author", "article:author"]),
        description: doc.meta(&metas, &["og:description", "description", "twitter:description"]),
        picture: doc.meta(&metas, &["og:image", "og:image:url", "twitter:image", "twitter:image:src"]),
        kind: "page".into(),
        ..Reading::default()
    };
    if reading.title.is_empty() {
        reading.title = doc.tags("title").first().map(|tag| words(doc.inside(tag, "title"))).unwrap_or_default();
    }
    // An author given as an address (Facebook's convention) is not a name.
    if reading.author.starts_with("http") {
        reading.author.clear();
    }
    if doc.meta(&metas, &["og:type"]).starts_with("video") || video {
        reading.kind = "video".into();
    }

    for script in doc.tags("script") {
        if !script.attrs.get("type").is_some_and(|kind| kind.eq_ignore_ascii_case("application/ld+json")) {
            continue;
        }
        let Ok(data) = serde_json::from_str::<serde_json::Value>(doc.inside(&script, "script").trim()) else { continue };
        if let Some((recipe, author, picture)) = recipe_from(&data) {
            if reading.author.is_empty() {
                reading.author = author;
            }
            if reading.picture.is_empty() {
                reading.picture = picture;
            }
            if reading.title.is_empty() {
                reading.title = recipe.name.clone();
            }
            reading.recipe = Some(recipe);
            reading.kind = "recipe".into();
            break;
        }
    }

    if video {
        // The page's own description is cut short; the full one is in the player's data.
        if let Some(full) = embedded_string(html, "shortDescription").filter(|text| !text.trim().is_empty()) {
            reading.description = full;
        }
        if let Some(channel) = embedded_string(html, "ownerChannelName") {
            reading.author = channel;
        }
        reading.related = urls_in(&reading.description).iter().take(MAX_RELATED).map(Url::to_string).collect();
        reading.text = reading.description.clone();
    } else {
        reading.text = doc.text();
    }

    if reading.site.is_empty() {
        reading.site = host_of(url);
    }
    reading.picture = url.join(&reading.picture).ok().filter(|_| !reading.picture.is_empty()).map(|u| u.to_string()).unwrap_or_default();
    reading.title = clip(&reading.title, MAX_TITLE);
    reading.description = clip(&reading.description, MAX_DESCRIPTION);
    reading.text = clip(&reading.text, MAX_TEXT);
    reading
}

/// Whether a video's description might be pointing at its recipe here.
fn worth_following(url: &Url) -> bool {
    let host = host_of(url);
    matches!(url.scheme(), "http" | "https") && !NOT_THE_SOURCE.iter().any(|not| host == *not || host.ends_with(&format!(".{not}")))
}

/// Reads a link's page, and for a video the pages its description points at, through
/// `get` (the internet, or a test's stand-in). Returns what was read and the picture's
/// address.
fn read_link(url: &Url, get: &dyn Fn(&Url) -> LinksResult<Fetched>) -> LinksResult<Reading> {
    let page = get(url)?;
    let html = String::from_utf8_lossy(&page.body);
    let mut reading = read_page(&page.url, &html);
    if reading.kind == "video" && reading.recipe.is_none() {
        let candidates: Vec<Url> = reading.related.iter().filter_map(|text| Url::parse(text).ok()).filter(worth_following).take(MAX_FOLLOWED).collect();
        for candidate in candidates {
            let Ok(other) = get(&candidate) else { continue };
            let found = read_page(&other.url, &String::from_utf8_lossy(&other.body));
            if let Some(mut recipe) = found.recipe {
                recipe.source = other.url.to_string();
                reading.recipe = Some(recipe);
                break;
            }
        }
    }
    if reading.title.is_empty() {
        reading.title = host_of(&page.url);
    }
    Ok(reading)
}

// ---------------------------------------------------------------------------------------
// Searching
// ---------------------------------------------------------------------------------------

fn search_words(query: &str) -> Vec<String> {
    query.split_whitespace().map(str::to_lowercase).filter(|word| !word.is_empty()).take(12).collect()
}

/// What a person would call the link: found here, it ranks above a match in the page's text.
fn heading(link: &Link) -> String {
    format!("{} {} {} {} {}", link.title, link.site, link.author, link.tags.join(" "), link.note).to_lowercase()
}

fn body(link: &Link) -> String {
    let recipe = link.recipe.as_ref().map(|r| format!("{} {} {}", r.name, r.ingredients.join(" "), r.steps.join(" "))).unwrap_or_default();
    format!("{} {} {} {}", link.url, link.description, recipe, link.text).to_lowercase()
}

/// The words around the first place a search word appears, from the recipe or the text.
fn snippet(link: &Link, wanted: &[String]) -> String {
    let lines = link.recipe.iter().flat_map(|r| r.ingredients.iter().chain(r.steps.iter())).map(String::as_str);
    for source in lines.chain([link.text.as_str(), link.description.as_str()]) {
        let lower = source.to_ascii_lowercase();
        let Some(at) = wanted.iter().find_map(|word| lower.find(&word.to_ascii_lowercase())) else { continue };
        let mut start = at.saturating_sub(SNIPPET_BEFORE);
        while !source.is_char_boundary(start) {
            start -= 1;
        }
        let mut end = (at + SNIPPET_AFTER).min(source.len());
        while !source.is_char_boundary(end) {
            end += 1;
        }
        let cut = source[start..end].split_whitespace().collect::<Vec<_>>().join(" ");
        return format!("{}{cut}{}", if start > 0 { "…" } else { "" }, if end < source.len() { "…" } else { "" });
    }
    String::new()
}

// ---------------------------------------------------------------------------------------
// The shelf
// ---------------------------------------------------------------------------------------

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.chars().all(|c| c.is_ascii_hexdigit())
}

fn clean_tags(tags: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let tag = clip(&tag.trim().trim_start_matches('#').to_lowercase(), MAX_TAG);
        if !tag.is_empty() && !out.contains(&tag) && out.len() < MAX_TAGS {
            out.push(tag);
        }
    }
    out
}

pub struct Links {
    dir: PathBuf,
    /// Each person's links, read from their folder the first time they are asked for.
    shelves: Mutex<HashMap<String, Vec<Link>>>,
}

impl Links {
    pub fn open(dir: PathBuf) -> Self {
        Self { dir, shelves: Mutex::new(HashMap::new()) }
    }

    fn person_dir(&self, person: &str) -> LinksResult<PathBuf> {
        if !valid_person(person) {
            return Err("that person is not on this box".into());
        }
        Ok(self.dir.join(person))
    }

    fn read_folder(dir: &Path) -> Vec<Link> {
        let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
        entries
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .filter_map(|entry| serde_json::from_slice::<Link>(&std::fs::read(entry.path()).ok()?).ok())
            .filter(|link| valid_id(&link.id))
            .collect()
    }

    /// Runs `action` on one person's links.
    fn with<T>(&self, person: &str, action: impl FnOnce(&mut Vec<Link>, &Path) -> LinksResult<T>) -> LinksResult<T> {
        let dir = self.person_dir(person)?;
        let mut shelves = self.shelves.lock().map_err(|_| "links are unavailable".to_string())?;
        let shelf = shelves.entry(person.to_string()).or_insert_with(|| Self::read_folder(&dir));
        action(shelf, &dir)
    }

    /// A person's links, newest first; with `query`, only those holding every word of it,
    /// the ones that hold them in their title, tags or note before the rest.
    pub fn list(&self, person: &str, query: &str) -> LinksResult<Vec<Card>> {
        let wanted = search_words(query);
        self.with(person, |shelf, _| {
            let mut found: Vec<(bool, &Link)> = shelf
                .iter()
                .filter_map(|link| {
                    if wanted.is_empty() {
                        return Some((true, link));
                    }
                    let (heading, body) = (heading(link), body(link));
                    let named = wanted.iter().all(|word| heading.contains(word.as_str()));
                    (named || wanted.iter().all(|word| heading.contains(word.as_str()) || body.contains(word.as_str()))).then_some((named, link))
                })
                .collect();
            found.sort_by_key(|(named, link)| (!named, std::cmp::Reverse(link.added)));
            let limit = if wanted.is_empty() { usize::MAX } else { MAX_LIST };
            Ok(found.into_iter().take(limit).map(|(_, link)| card(link, if wanted.is_empty() { String::new() } else { snippet(link, &wanted) })).collect())
        })
    }

    pub fn get(&self, person: &str, id: &str) -> LinksResult<Link> {
        self.with(person, |shelf, _| shelf.iter().find(|link| link.id == id).cloned().ok_or_else(|| "no such link".to_string()))
    }

    /// Where a link's picture is, and what kind it is.
    pub fn picture(&self, person: &str, id: &str) -> LinksResult<(PathBuf, &'static str)> {
        let link = self.get(person, id)?;
        if link.picture.is_empty() {
            return Err("no picture".into());
        }
        Ok((self.person_dir(person)?.join(&link.picture), picture_mime(&link.picture)))
    }

    /// Saves what a person shared: reads the page and keeps it. A page that cannot be read
    /// is kept anyway, with the reason, so a share is never lost. The same address twice
    /// is one link.
    pub fn add(&self, person: &str, shared: &str, note: &str) -> LinksResult<Link> {
        self.add_through(person, shared, note, &fetch_page, &|url| fetch(url, "image/*", MAX_PICTURE))
    }

    fn add_through(&self, person: &str, shared: &str, note: &str, get: &dyn Fn(&Url) -> LinksResult<Fetched>, get_picture: &dyn Fn(&Url) -> LinksResult<Fetched>) -> LinksResult<Link> {
        let url = tidy(&url_in(shared).ok_or("that does not look like a web address")?);
        let known = self.with(person, |shelf, _| {
            if shelf.len() >= MAX_LINKS {
                return Err("this box keeps no more links for one person".into());
            }
            Ok(shelf.iter().find(|link| link.url == url.as_str()).cloned())
        })?;
        if let Some(known) = known {
            // Shared again: if it could not be read the first time, this is the second try.
            return if known.problem.is_empty() { Ok(known) } else { self.refresh_through(person, &known.id, get, get_picture) };
        }
        let mut link = Link { id: crate::random_hex(8), url: url.to_string(), added: now_millis(), note: clip(note, MAX_NOTE), kind: "page".into(), ..Link::default() };
        let dir = self.person_dir(person)?;
        fill(&mut link, &url, &dir, get, get_picture);
        self.with(person, |shelf, dir| {
            write_json(&dir.join(format!("{}.json", link.id)), &link)?;
            shelf.push(link.clone());
            Ok(link)
        })
    }

    /// Reads a link's page again, keeping the person's note and tags.
    pub fn refresh(&self, person: &str, id: &str) -> LinksResult<Link> {
        self.refresh_through(person, id, &fetch_page, &|url| fetch(url, "image/*", MAX_PICTURE))
    }

    fn refresh_through(&self, person: &str, id: &str, get: &dyn Fn(&Url) -> LinksResult<Fetched>, get_picture: &dyn Fn(&Url) -> LinksResult<Fetched>) -> LinksResult<Link> {
        let mut link = self.get(person, id)?;
        let url = Url::parse(&link.url).map_err(|_| "that link's address is broken".to_string())?;
        let dir = self.person_dir(person)?;
        let before = link.clone();
        fill(&mut link, &url, &dir, get, get_picture);
        // A page that has gone since it was saved does not take the saved copy with it.
        if !link.problem.is_empty() && before.fetched > 0 {
            link = Link { problem: link.problem, ..before };
        }
        self.save(person, link)
    }

    fn save(&self, person: &str, link: Link) -> LinksResult<Link> {
        self.with(person, |shelf, dir| {
            let slot = shelf.iter_mut().find(|known| known.id == link.id).ok_or("no such link")?;
            write_json(&dir.join(format!("{}.json", link.id)), &link)?;
            *slot = link.clone();
            Ok(link)
        })
    }

    /// Changes what the person wrote on a link: its title, note and tags.
    pub fn update(&self, person: &str, id: &str, title: Option<String>, note: Option<String>, tags: Option<Vec<String>>) -> LinksResult<Link> {
        let mut link = self.get(person, id)?;
        if let Some(title) = title.filter(|title| !title.trim().is_empty()) {
            link.title = clip(&title, MAX_TITLE);
        }
        if let Some(note) = note {
            link.note = clip(&note, MAX_NOTE);
        }
        if let Some(tags) = tags {
            link.tags = clean_tags(tags);
        }
        self.save(person, link)
    }

    pub fn delete(&self, person: &str, id: &str) -> LinksResult<()> {
        self.with(person, |shelf, dir| {
            let at = shelf.iter().position(|link| link.id == id).ok_or("no such link")?;
            let link = shelf.remove(at);
            let _ = std::fs::remove_file(dir.join(format!("{}.json", link.id)));
            if !link.picture.is_empty() {
                let _ = std::fs::remove_file(dir.join(&link.picture));
            }
            Ok(())
        })
    }
}

/// Reads `url` into `link`. On failure the link keeps what it had and gains the reason.
fn fill(link: &mut Link, url: &Url, dir: &Path, get: &dyn Fn(&Url) -> LinksResult<Fetched>, get_picture: &dyn Fn(&Url) -> LinksResult<Fetched>) {
    let reading = match read_link(url, get) {
        Ok(reading) => reading,
        Err(problem) => {
            link.problem = problem;
            if link.title.is_empty() {
                link.title = url.to_string();
                link.site = host_of(url);
                if youtube_id(url).is_some() {
                    link.kind = "video".into();
                }
            }
            return;
        }
    };
    link.problem.clear();
    link.fetched = now_millis();
    link.title = reading.title;
    link.site = reading.site;
    link.author = reading.author;
    link.description = reading.description;
    link.kind = reading.kind;
    link.recipe = reading.recipe;
    link.related = reading.related;
    link.text = reading.text;
    // The picture is copied to the box: the app shows nothing from other sites.
    let picture = Url::parse(&reading.picture).ok().and_then(|at| get_picture(&at).ok()).and_then(|got| Some((picture_kind(&got.body)?, got.body)));
    if let Some((ext, bytes)) = picture {
        let name = format!("{}.{ext}", link.id);
        if std::fs::create_dir_all(dir).is_ok() && std::fs::write(dir.join(&name), bytes).is_ok() {
            if !link.picture.is_empty() && link.picture != name {
                let _ = std::fs::remove_file(dir.join(&link.picture));
            }
            link.picture = name;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECIPE_PAGE: &str = r#"<!DOCTYPE html><html><head>
        <TITLE>Butternut Pie &amp; Custard | Field Kitchen</TITLE>
        <meta property="og:title" content="Butternut Squash Pie">
        <meta property="og:site_name" content="Field Kitchen">
        <meta name="description" content="A silky pie that isn&#39;t pumpkin.">
        <meta property="og:image" content="/img/pie.jpg">
        <script>var tracking = "<p>not text</p>";</script>
        <script type="application/ld+json">{"@context":"https://schema.org","@graph":[{"@type":"WebSite","name":"Field Kitchen"},
          {"@type":["Recipe","NewsArticle"],"name":"Butternut Squash Pie","author":{"@type":"Person","name":"Ana López"},
           "recipeYield":["8","8 slices"],"prepTime":"PT25M","cookTime":"PT1H","totalTime":"PT1H25M",
           "recipeIngredient":["1 butternut squash (about 2 lb)","&amp;frac12; cup brown sugar","3 eggs"],
           "recipeInstructions":[{"@type":"HowToSection","name":"Filling","itemListElement":[{"@type":"HowToStep","text":"Roast the squash at 400&deg;F until soft."},{"@type":"HowToStep","text":"<p>Blend with sugar and eggs.</p>"}]},
             {"@type":"HowToStep","text":"Bake 50 minutes."}]}]}</script>
        </head><body><nav><a href="/">Home</a> Subscribe</nav>
        <article><h1>Butternut Squash Pie</h1><p>My grandmother made this every <b>November</b>.</p>
        <!-- an ad --><p>It keeps for three days.</p><aside>Related: pumpkin pie</aside></article>
        <footer>Copyright</footer></body></html>"#;

    fn page(url: &Url, html: &str) -> LinksResult<Fetched> {
        Ok(Fetched { url: url.clone(), mime: "text/html".into(), body: html.as_bytes().to_vec() })
    }

    fn no_picture(_: &Url) -> LinksResult<Fetched> {
        Err("no picture".into())
    }

    fn shelf(name: &str) -> (Links, PathBuf) {
        let dir = std::env::temp_dir().join(format!("castle-links-{name}-{}", crate::random_hex(6)));
        (Links::open(dir.clone()), dir)
    }

    #[test]
    fn a_recipe_page_is_read_whole() {
        let url = Url::parse("https://fieldkitchen.example/pie").unwrap();
        let reading = read_page(&url, RECIPE_PAGE);
        assert_eq!(reading.title, "Butternut Squash Pie");
        assert_eq!(reading.site, "Field Kitchen");
        assert_eq!(reading.author, "Ana López");
        assert_eq!(reading.description, "A silky pie that isn't pumpkin.");
        assert_eq!(reading.kind, "recipe");
        assert_eq!(reading.picture, "https://fieldkitchen.example/img/pie.jpg");
        let recipe = reading.recipe.unwrap();
        assert_eq!(recipe.yields, "8 slices");
        assert_eq!((recipe.prep.as_str(), recipe.cook.as_str(), recipe.total.as_str()), ("25 min", "1 h", "1 h 25 min"));
        assert_eq!(recipe.ingredients, ["1 butternut squash (about 2 lb)", "½ cup brown sugar", "3 eggs"]);
        assert_eq!(recipe.steps, ["Filling:", "Roast the squash at 400°F until soft.", "Blend with sugar and eggs.", "Bake 50 minutes."]);
        assert_eq!(reading.text, "Butternut Squash Pie\nMy grandmother made this every November.\nIt keeps for three days.");
    }

    #[test]
    fn a_page_with_nothing_marked_still_has_a_title_and_words() {
        let url = Url::parse("https://www.plain.example/a/b").unwrap();
        let reading = read_page(&url, "<html><head><title>  Plain\n page </title><style>p{color:red}</style></head><body><p>One.</p><div>Two &mdash; three</div></body></html>");
        assert_eq!((reading.title.as_str(), reading.site.as_str(), reading.kind.as_str()), ("Plain page", "plain.example", "page"));
        assert_eq!(reading.text, "One.\nTwo — three");
        assert!(reading.recipe.is_none() && reading.picture.is_empty());
    }

    #[test]
    fn a_video_brings_the_recipe_its_description_points_at() {
        let video = r#"<html><head><meta property="og:title" content="The best butternut pie"><meta property="og:description" content="Cut short...">
            <script>var ytInitialPlayerResponse = {"videoDetails":{"shortDescription":"Full recipe: https://fieldkitchen.example/pie.\nMy knives: https://amzn.to/abc\n\"Thanks\" for watching","ownerChannelName":"Field Kitchen"}};</script></head><body></body></html>"#;
        let get = |url: &Url| match url.host_str() {
            Some("www.youtube.com") => page(url, video),
            Some("fieldkitchen.example") => page(url, RECIPE_PAGE),
            _ => Err("should not be fetched".into()),
        };
        let (links, dir) = shelf("video");
        let link = links.add_through("owner", "Look at this https://youtu.be/dQw4w9WgXcQ?si=tracker", "for thanksgiving", &get, &no_picture).unwrap();
        assert_eq!(link.url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        assert_eq!((link.kind.as_str(), link.author.as_str(), link.note.as_str()), ("video", "Field Kitchen", "for thanksgiving"));
        assert!(link.description.contains("\"Thanks\" for watching"));
        assert_eq!(link.related, ["https://fieldkitchen.example/pie", "https://amzn.to/abc"]);
        let recipe = link.recipe.clone().unwrap();
        assert_eq!(recipe.source, "https://fieldkitchen.example/pie");
        assert_eq!(recipe.ingredients.len(), 3);

        // Found by an ingredient, which only the recipe's own site had.
        let found = links.list("owner", "brown SUGAR").unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].has_recipe && found[0].snippet.contains("brown sugar"));
        assert!(links.list("owner", "sugar anchovies").unwrap().is_empty());
        // Sharing it again is the same link, and it is there after a restart.
        assert_eq!(links.add_through("owner", "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=42s", "", &get, &no_picture).unwrap().id, link.id);
        assert_eq!(Links::open(dir.clone()).list("owner", "").unwrap().len(), 1);
        assert!(Links::open(dir.clone()).list("someone-else", "").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_page_that_cannot_be_read_is_kept_and_read_later() {
        let (links, dir) = shelf("later");
        let down = |_: &Url| -> LinksResult<Fetched> { Err("fieldkitchen.example did not answer".into()) };
        let link = links.add_through("owner", "fieldkitchen.example/pie?utm_source=mail&page=2#top", "", &down, &no_picture).unwrap();
        assert_eq!(link.url, "https://fieldkitchen.example/pie?page=2");
        assert_eq!((link.problem.as_str(), link.fetched), ("fieldkitchen.example did not answer", 0));

        let up = |url: &Url| page(url, RECIPE_PAGE);
        let picture = |url: &Url| Ok(Fetched { url: url.clone(), mime: "image/png".into(), body: vec![0x89, b'P', b'N', b'G', 1, 2, 3] });
        let again = links.add_through("owner", "https://fieldkitchen.example/pie?page=2", "", &up, &picture).unwrap();
        assert_eq!((again.id.as_str(), again.problem.as_str(), again.title.as_str()), (link.id.as_str(), "", "Butternut Squash Pie"));
        assert_eq!(again.picture, format!("{}.png", link.id));
        assert!(links.picture("owner", &link.id).unwrap().0.is_file());

        // The page goes away: the saved copy stays, with the reason beside it.
        let tagged = links.update("owner", &link.id, None, Some("mine".into()), Some(vec!["#Pie".into(), "pie".into(), " dessert ".into()])).unwrap();
        assert_eq!(tagged.tags, ["pie", "dessert"]);
        let gone = links.refresh_through("owner", &link.id, &down, &no_picture).unwrap();
        assert_eq!((gone.title.as_str(), gone.note.as_str(), gone.problem.is_empty()), ("Butternut Squash Pie", "mine", false));
        assert!(gone.recipe.is_some());

        links.delete("owner", &link.id).unwrap();
        assert!(links.list("owner", "").unwrap().is_empty());
        assert_eq!(std::fs::read_dir(dir.join("owner")).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn only_addresses_out_on_the_internet_are_read() {
        for private in ["127.0.0.1", "10.0.0.5", "192.168.1.1", "172.16.4.4", "169.254.169.254", "100.100.1.1", "0.0.0.0", "::1", "fe80::1", "fd00::1", "::ffff:192.168.1.1"] {
            assert!(!public_address(private.parse().unwrap()), "{private}");
        }
        for public in ["1.1.1.1", "142.250.72.14", "2606:4700:4700::1111"] {
            assert!(public_address(public.parse().unwrap()), "{public}");
        }
        assert!(fetch(&Url::parse("http://127.0.0.1:8787/api/v1/hello").unwrap(), "*/*", 1000).is_err());
        assert!(fetch(&Url::parse("http://[::1]/").unwrap(), "*/*", 1000).is_err());
        assert!(fetch(&Url::parse("http://localhost/").unwrap(), "*/*", 1000).is_err());
    }

    #[test]
    fn what_a_person_shares_becomes_one_address() {
        assert_eq!(url_in("see https://example.com/a).").unwrap().as_str(), "https://example.com/a");
        assert_eq!(url_in("example.com/pie").unwrap().as_str(), "https://example.com/pie");
        assert!(url_in("just words").is_none() && url_in("file:///etc/passwd").is_none() && url_in("").is_none());
        let tidy_of = |text: &str| tidy(&Url::parse(text).unwrap()).to_string();
        assert_eq!(tidy_of("https://m.youtube.com/shorts/abc_-123?feature=share"), "https://www.youtube.com/watch?v=abc_-123");
        assert_eq!(tidy_of("https://example.com/a?utm_medium=x&fbclid=1#frag"), "https://example.com/a");
        assert_eq!(duration("P1DT2H"), "26 h");
        assert_eq!(duration("soon"), "");
        assert_eq!(decode_entities("a &amp; b &#233; &#x2014; &unknown; & c"), "a & b é — &unknown; & c");
    }
}
