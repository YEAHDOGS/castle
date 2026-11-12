//! Looking a title up on Wikipedia: what a game or a film is, in a paragraph, with the
//! article's picture.
//!
//! This reaches out to the internet, and what it sends is the title being looked up. It
//! runs only when something asks for it (a game's card is opened, or Theater is told to
//! fetch details), never on its own. Answers are kept by the caller so a title is asked
//! about once.
//!
//! Wikipedia needs no account and no key. Its text is free to reuse (CC BY-SA); a film's
//! poster or a game's box art in an article is usually there under fair use, so pictures
//! are shown from Wikipedia's own address and are not copied into the vault as the user's.

use std::{collections::HashMap, path::PathBuf, sync::Mutex, time::Duration};

use serde::{Deserialize, Serialize};

const API: &str = "https://en.wikipedia.org/w/api.php";
/// Wikimedia asks every client to say what it is. No address or name of the owner is sent.
const USER_AGENT: &str = concat!("Castle/", env!("CARGO_PKG_VERSION"), " (home media server; castled)");
const TIMEOUT: Duration = Duration::from_secs(12);
/// How much of an article's opening is kept.
const EXTRACT_CHARS: usize = 1200;
const IMAGE_WIDTH: u32 = 600;
/// Words too common to tell one title from another.
const NOISE: [&str; 12] = ["the", "a", "an", "of", "and", "in", "to", "for", "on", "at", "no", "de"];
/// A found article must share this much of the wanted title's words to count.
const MATCH_SHARE: f32 = 0.6;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Article {
    pub title: String,
    /// The opening of the article, in plain text.
    pub extract: String,
    pub url: String,
    /// The article's main picture, as an address on Wikipedia's own servers.
    pub image: Option<String>,
    /// The matching Wikidata item ("Q12345"), which is where cast and dates come from.
    pub wikidata: Option<String>,
}

pub fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|_| "could not start a web request".to_string())
}

/// The words of a title that carry its meaning, lower-cased: letters and digits only,
/// without the common small words.
pub fn key_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|word| !word.is_empty() && !NOISE.contains(&word.as_str()))
        .collect()
}

/// True when the article is plausibly about `wanted`: most of the wanted words appear in
/// its title or the start of its text. A search always returns something; this is what
/// stops a wrong answer being shown as the right one.
pub fn is_about(wanted: &str, article: &Article) -> bool {
    let wanted = key_words(wanted);
    if wanted.is_empty() {
        return false;
    }
    let lead: String = article.extract.chars().take(300).collect();
    let found = key_words(&format!("{} {lead}", article.title));
    let hits = wanted.iter().filter(|word| found.contains(word)).count();
    hits as f32 / wanted.len() as f32 >= MATCH_SHARE
}

/// `base?name=value&...`, with every value made safe for an address.
pub fn with_query(base: &str, pairs: &[(&str, &str)]) -> String {
    let mut url = base.to_string();
    for (i, (name, value)) in pairs.iter().enumerate() {
        url.push(if i == 0 { '?' } else { '&' });
        url.push_str(name);
        url.push('=');
        url.push_str(&encode(value));
    }
    url
}

/// Percent-encodes everything but letters, digits and `-._~`.
pub fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The best article for a search, or None when Wikipedia has nothing for it.
pub fn find(http: &reqwest::blocking::Client, search: &str) -> Result<Option<Article>, String> {
    let width = IMAGE_WIDTH.to_string();
    let chars = EXTRACT_CHARS.to_string();
    let query = [
        ("action", "query"),
        ("format", "json"),
        ("formatversion", "2"),
        ("redirects", "1"),
        ("generator", "search"),
        ("gsrsearch", search),
        ("gsrlimit", "1"),
        ("prop", "extracts|pageimages|pageprops|info"),
        ("exintro", "1"),
        ("explaintext", "1"),
        ("exchars", chars.as_str()),
        ("piprop", "thumbnail"),
        ("pithumbsize", width.as_str()),
        ("pilicense", "any"),
        ("ppprop", "wikibase_item"),
        ("inprop", "url"),
    ];
    let answer: serde_json::Value = http
        .get(with_query(API, &query))
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json())
        .map_err(|_| "Wikipedia did not answer".to_string())?;
    Ok(article_from(&answer))
}

/// Reads the one page out of a search answer.
fn article_from(answer: &serde_json::Value) -> Option<Article> {
    let page = answer["query"]["pages"].as_array()?.first()?;
    let title = page["title"].as_str()?.to_string();
    let extract = page["extract"].as_str().unwrap_or("").trim().to_string();
    if extract.is_empty() {
        return None;
    }
    Some(Article {
        url: page["fullurl"].as_str().map(str::to_string).unwrap_or_else(|| format!("https://en.wikipedia.org/wiki/{}", title.replace(' ', "_"))),
        image: page["thumbnail"]["source"].as_str().map(str::to_string),
        wikidata: page["pageprops"]["wikibase_item"].as_str().map(str::to_string),
        title,
        extract,
    })
}

/// Answers already fetched, kept in one file in the vault so each title is asked about
/// once. "Wikipedia has nothing for this" is kept too, as an empty answer.
pub struct Shelf {
    file: PathBuf,
    kept: Mutex<HashMap<String, Option<Article>>>,
}

impl Shelf {
    pub fn open(file: PathBuf) -> Self {
        let kept = std::fs::read(&file).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default();
        Self { file, kept: Mutex::new(kept) }
    }

    /// None: never asked. Some(None): asked, and there was nothing.
    pub fn get(&self, key: &str) -> Option<Option<Article>> {
        self.kept.lock().ok()?.get(key).cloned()
    }

    pub fn put(&self, key: &str, answer: Option<Article>) {
        let Ok(mut kept) = self.kept.lock() else { return };
        kept.insert(key.to_string(), answer);
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.file.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec(&*kept).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(tmp, &self.file);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn article(title: &str, extract: &str) -> Article {
        Article { title: title.into(), extract: extract.into(), url: String::new(), image: None, wikidata: None }
    }

    #[test]
    fn a_search_answer_becomes_an_article() {
        let answer = serde_json::json!({ "query": { "pages": [{
            "title": "Final Fantasy IX",
            "extract": "Final Fantasy IX is a 2000 role-playing video game.",
            "fullurl": "https://en.wikipedia.org/wiki/Final_Fantasy_IX",
            "thumbnail": { "source": "https://upload.wikimedia.org/x/600px-Ffixbox.jpg" },
            "pageprops": { "wikibase_item": "Q474573" },
        }] } });
        let found = article_from(&answer).unwrap();
        assert_eq!(found.title, "Final Fantasy IX");
        assert_eq!(found.wikidata.as_deref(), Some("Q474573"));
        assert_eq!(found.image.as_deref(), Some("https://upload.wikimedia.org/x/600px-Ffixbox.jpg"));
        // No results, or a page with no text, is nothing.
        assert_eq!(article_from(&serde_json::json!({ "batchcomplete": true })), None);
        assert_eq!(article_from(&serde_json::json!({ "query": { "pages": [{ "title": "Stub" }] } })), None);
    }

    #[test]
    fn a_wrong_article_is_not_taken_for_the_right_one() {
        let ff9 = article("Final Fantasy IX", "Final Fantasy IX is a 2000 role-playing video game developed by Square.");
        assert!(is_about("Final Fantasy IX", &ff9));
        assert!(is_about("The Legend of Zelda: Ocarina of Time", &article("The Legend of Zelda: Ocarina of Time", "An action-adventure game.")));
        // A search for an obscure title falls back to whatever is nearest; that is refused.
        assert!(!is_about("Sky Runner 3", &ff9));
        assert!(!is_about("", &ff9));
    }

    #[test]
    fn answers_are_kept_between_runs() {
        let file = std::env::temp_dir().join(format!("castled-shelf-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&file);
        let shelf = Shelf::open(file.clone());
        assert_eq!(shelf.get("a"), None);
        shelf.put("a", Some(article("A", "About A.")));
        shelf.put("nothing", None);
        let reopened = Shelf::open(file.clone());
        assert_eq!(reopened.get("a").unwrap().unwrap().title, "A");
        assert_eq!(reopened.get("nothing"), Some(None));
        std::fs::remove_file(file).unwrap();
    }

    #[test]
    fn values_in_an_address_are_made_safe() {
        assert_eq!(with_query("https://x/api", &[("a", "1"), ("q", "Ratchet & Clank: 100%")]), "https://x/api?a=1&q=Ratchet%20%26%20Clank%3A%20100%25");
        assert_eq!(encode("Pok\u{e9}mon"), "Pok%C3%A9mon");
    }

    #[test]
    fn small_words_do_not_count_as_a_match() {
        assert_eq!(key_words("The Legend of Zelda: A Link to the Past"), ["legend", "zelda", "link", "past"]);
    }
}
