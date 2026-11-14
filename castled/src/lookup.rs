//! Finding out about a film, a show, an episode or an album, from open sources on the
//! internet. Each lookup sends only the title (and year, or artist) being looked up.
//!
//!   films     Wikipedia for the summary and poster; Wikidata for cast, director, genres,
//!             running time. No account or key.
//!   shows     TVmaze for the summary, poster, genres, cast and each episode. No key.
//!   albums    MusicBrainz for the release; the Cover Art Archive for its cover. No key.
//!   subtitles OpenSubtitles, which needs a free API key (Settings > Theater).
//!
//! Lookups run only when asked for (a details card is opened, or "Fetch details" is
//! pressed). The caller keeps every answer, so a title is asked about once.

use std::time::Duration;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::wiki::{self, with_query};

const WIKIDATA: &str = "https://www.wikidata.org/w/api.php";
const TVMAZE: &str = "https://api.tvmaze.com";
const MUSICBRAINZ: &str = "https://musicbrainz.org/ws/2/release-group/";
const COVER_ART: &str = "https://coverartarchive.org/release-group";
const OPENSUBTITLES: &str = "https://api.opensubtitles.com/api/v1";
const MAX_CAST: usize = 12;
const MAX_GENRES: usize = 5;
/// Pictures bigger than this are not kept.
pub const MAX_ART_BYTES: usize = 6 * 1024 * 1024;
const MAX_SUBTITLE_BYTES: usize = 2 * 1024 * 1024;
/// MusicBrainz asks for no more than one request a second from each client.
const MUSICBRAINZ_PAUSE: Duration = Duration::from_millis(1100);

/// What is known about a title, from whichever source had it.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Details {
    pub title: String,
    pub year: u32,
    pub overview: String,
    /// Where the picture can be fetched; the box downloads it once and keeps it.
    pub poster_url: Option<String>,
    pub cast: Vec<String>,
    pub director: String,
    pub genres: Vec<String>,
    pub runtime_minutes: u32,
    /// The page the details came from, to read more.
    pub url: String,
    /// Who supplied them, to credit: "Wikipedia and Wikidata", "TVmaze", "MusicBrainz".
    pub source: String,
    /// TVmaze's number for a show, so its episodes can be asked for.
    pub tvmaze_id: Option<u64>,
}

/// One episode's own name and summary.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Episode {
    pub name: String,
    pub overview: String,
    pub aired: String,
    pub runtime_minutes: u32,
    pub still_url: Option<String>,
}

fn get_json(http: &Client, url: &str, what: &str) -> Result<serde_json::Value, String> {
    let res = http.get(url).send().map_err(|_| format!("{what} did not answer"))?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(serde_json::Value::Null);
    }
    res.error_for_status().and_then(|r| r.json()).map_err(|_| format!("{what} did not answer"))
}

/// Text from a page fragment ("<p>A <b>show</b></p>" -> "A show").
pub fn plain_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&quot;", "\"").replace("&#39;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&nbsp;", " ").trim().to_string()
}

// ---------------------------------------------------------------------------------------
// Films: Wikipedia, then Wikidata
// ---------------------------------------------------------------------------------------

/// The ids of a Wikidata item's values for one property ("P161" cast member, ...).
fn claim_ids(entity: &serde_json::Value, property: &str) -> Vec<String> {
    entity["claims"][property]
        .as_array()
        .map(|claims| claims.iter().filter_map(|c| c["mainsnak"]["datavalue"]["value"]["id"].as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

fn claim_amount(entity: &serde_json::Value, property: &str) -> Option<f64> {
    entity["claims"][property][0]["mainsnak"]["datavalue"]["value"]["amount"].as_str()?.trim_start_matches('+').parse().ok()
}

fn claim_year(entity: &serde_json::Value, property: &str) -> u32 {
    entity["claims"][property][0]["mainsnak"]["datavalue"]["value"]["time"]
        .as_str()
        .and_then(|t| t.trim_start_matches('+').get(..4))
        .and_then(|y| y.parse().ok())
        .unwrap_or(0)
}

/// English names of Wikidata items, in the order asked.
fn labels(http: &Client, ids: &[String]) -> Vec<String> {
    if ids.is_empty() {
        return Vec::new();
    }
    let joined = ids.join("|");
    let url = with_query(WIKIDATA, &[("action", "wbgetentities"), ("format", "json"), ("props", "labels"), ("languages", "en"), ("ids", &joined)]);
    let Ok(answer) = get_json(http, &url, "Wikidata") else { return Vec::new() };
    ids.iter().filter_map(|id| answer["entities"][id]["labels"]["en"]["value"].as_str().map(str::to_string)).collect()
}

pub fn film(http: &Client, title: &str, year: u32) -> Result<Option<Details>, String> {
    let search = if year > 0 { format!("{title} {year} film") } else { format!("{title} film") };
    let Some(article) = wiki::find(http, &search)?.filter(|a| wiki::is_about(title, a)) else { return Ok(None) };
    let mut details = Details {
        title: article.title.trim_end_matches(')').split(" (").next().unwrap_or(&article.title).to_string(),
        year,
        overview: article.extract.clone(),
        poster_url: article.image.clone(),
        url: article.url.clone(),
        source: "Wikipedia".into(),
        ..Details::default()
    };
    if let Some(qid) = &article.wikidata {
        let url = with_query(WIKIDATA, &[("action", "wbgetentities"), ("format", "json"), ("props", "claims"), ("ids", qid)]);
        if let Ok(answer) = get_json(http, &url, "Wikidata") {
            let entity = &answer["entities"][qid];
            let mut cast = claim_ids(entity, "P161");
            cast.truncate(MAX_CAST);
            let mut genres = claim_ids(entity, "P136");
            genres.truncate(MAX_GENRES);
            let directors = claim_ids(entity, "P57");
            let names = labels(http, &[cast.clone(), genres.clone(), directors.clone()].concat());
            let (cast_names, rest) = names.split_at(cast.len().min(names.len()));
            let (genre_names, director_names) = rest.split_at(genres.len().min(rest.len()));
            details.cast = cast_names.to_vec();
            details.genres = genre_names.iter().map(|g| g.trim_end_matches(" film").to_string()).collect();
            details.director = director_names.join(", ");
            details.runtime_minutes = claim_amount(entity, "P2047").map(|m| m.round() as u32).unwrap_or(0);
            if details.year == 0 {
                details.year = claim_year(entity, "P577");
            }
            details.source = "Wikipedia and Wikidata".into();
        }
    }
    Ok(Some(details))
}

// ---------------------------------------------------------------------------------------
// Shows: TVmaze
// ---------------------------------------------------------------------------------------

pub fn show(http: &Client, name: &str) -> Result<Option<Details>, String> {
    let url = with_query(&format!("{TVMAZE}/singlesearch/shows"), &[("q", name), ("embed", "cast")]);
    let found = get_json(http, &url, "TVmaze")?;
    if found.is_null() || !wiki::is_about(name, &wiki::Article { title: found["name"].as_str().unwrap_or("").into(), extract: String::new(), url: String::new(), image: None, wikidata: None }) {
        return Ok(None);
    }
    Ok(Some(Details {
        title: found["name"].as_str().unwrap_or(name).to_string(),
        year: found["premiered"].as_str().and_then(|d| d.get(..4)).and_then(|y| y.parse().ok()).unwrap_or(0),
        overview: plain_text(found["summary"].as_str().unwrap_or("")),
        poster_url: found["image"]["original"].as_str().or(found["image"]["medium"].as_str()).map(str::to_string),
        cast: found["_embedded"]["cast"]
            .as_array()
            .map(|cast| {
                cast.iter()
                    .take(MAX_CAST)
                    .filter_map(|c| {
                        let person = c["person"]["name"].as_str()?;
                        Some(match c["character"]["name"].as_str() {
                            Some(role) => format!("{person} as {role}"),
                            None => person.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        genres: found["genres"].as_array().map(|g| g.iter().filter_map(|x| x.as_str().map(str::to_string)).take(MAX_GENRES).collect()).unwrap_or_default(),
        runtime_minutes: found["averageRuntime"].as_u64().or(found["runtime"].as_u64()).unwrap_or(0) as u32,
        url: found["url"].as_str().unwrap_or("").to_string(),
        source: "TVmaze".into(),
        tvmaze_id: found["id"].as_u64(),
        ..Details::default()
    }))
}

pub fn episode(http: &Client, show_id: u64, season: u32, number: u32) -> Result<Option<Episode>, String> {
    let url = with_query(&format!("{TVMAZE}/shows/{show_id}/episodebynumber"), &[("season", &season.to_string()), ("number", &number.to_string())]);
    let found = get_json(http, &url, "TVmaze")?;
    if found.is_null() {
        return Ok(None);
    }
    Ok(Some(Episode {
        name: found["name"].as_str().unwrap_or("").to_string(),
        overview: plain_text(found["summary"].as_str().unwrap_or("")),
        aired: found["airdate"].as_str().unwrap_or("").to_string(),
        runtime_minutes: found["runtime"].as_u64().unwrap_or(0) as u32,
        still_url: found["image"]["original"].as_str().or(found["image"]["medium"].as_str()).map(str::to_string),
    }))
}

// ---------------------------------------------------------------------------------------
// Albums: MusicBrainz and the Cover Art Archive
// ---------------------------------------------------------------------------------------

/// A search term with Lucene's special characters taken out.
fn lucene(text: &str) -> String {
    text.chars().filter(|c| !r#"+-&|!(){}[]^"~*?:\/"#.contains(*c)).collect::<String>().trim().to_string()
}

pub fn album(http: &Client, artist: &str, album: &str) -> Result<Option<Details>, String> {
    let query = if artist.is_empty() {
        format!("releasegroup:\"{}\"", lucene(album))
    } else {
        format!("releasegroup:\"{}\" AND artist:\"{}\"", lucene(album), lucene(artist))
    };
    let url = with_query(MUSICBRAINZ, &[("query", &query), ("fmt", "json"), ("limit", "1")]);
    let found = get_json(http, &url, "MusicBrainz")?;
    let Some(group) = found["release-groups"].as_array().and_then(|g| g.first()) else { return Ok(None) };
    if group["score"].as_u64().unwrap_or(0) < 80 {
        return Ok(None);
    }
    let Some(id) = group["id"].as_str() else { return Ok(None) };
    std::thread::sleep(MUSICBRAINZ_PAUSE);
    let credit = group["artist-credit"]
        .as_array()
        .map(|parts| parts.iter().filter_map(|p| Some(format!("{}{}", p["name"].as_str()?, p["joinphrase"].as_str().unwrap_or("")))).collect::<String>())
        .unwrap_or_default();
    Ok(Some(Details {
        title: group["title"].as_str().unwrap_or(album).to_string(),
        year: group["first-release-date"].as_str().and_then(|d| d.get(..4)).and_then(|y| y.parse().ok()).unwrap_or(0),
        poster_url: Some(format!("{COVER_ART}/{id}/front-500")),
        director: credit,
        genres: group["tags"].as_array().map(|t| t.iter().filter_map(|x| x["name"].as_str().map(str::to_string)).take(MAX_GENRES).collect()).unwrap_or_default(),
        url: format!("https://musicbrainz.org/release-group/{id}"),
        source: "MusicBrainz and the Cover Art Archive".into(),
        ..Details::default()
    }))
}

// ---------------------------------------------------------------------------------------
// Pictures and subtitles
// ---------------------------------------------------------------------------------------

/// Downloads a picture. None when it is missing, not a picture, or too big.
pub fn fetch_picture(http: &Client, url: &str) -> Option<Vec<u8>> {
    let res = http.get(url).send().ok()?.error_for_status().ok()?;
    let is_image = res.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|t| t.starts_with("image/"));
    if !is_image || res.content_length().is_some_and(|n| n as usize > MAX_ART_BYTES) {
        return None;
    }
    let bytes = res.bytes().ok()?;
    (bytes.len() <= MAX_ART_BYTES).then(|| bytes.to_vec())
}

/// OpenSubtitles' fingerprint of a video: its size plus the 64-bit sums of its first and
/// last 64 KB. Finds subtitles made for this exact file.
pub fn movie_hash(path: &std::path::Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    const CHUNK: u64 = 65536;
    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    if size < CHUNK * 2 {
        return None;
    }
    let mut sum = size;
    let mut buf = vec![0u8; CHUNK as usize];
    for start in [0, size - CHUNK] {
        file.seek(SeekFrom::Start(start)).ok()?;
        file.read_exact(&mut buf).ok()?;
        for word in buf.chunks_exact(8) {
            sum = sum.wrapping_add(u64::from_le_bytes(word.try_into().ok()?));
        }
    }
    Some(format!("{sum:016x}"))
}

/// The best English subtitles OpenSubtitles has for a video, as SubRip text.
pub fn subtitles(http: &Client, key: &str, query: &str, hash: Option<&str>, language: &str) -> Result<Option<String>, String> {
    let mut pairs = vec![("languages", language), ("query", query)];
    if let Some(hash) = hash {
        pairs.push(("moviehash", hash));
    }
    let url = with_query(&format!("{OPENSUBTITLES}/subtitles"), &pairs);
    let found: serde_json::Value = http
        .get(url)
        .header("Api-Key", key)
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json())
        .map_err(|_| "OpenSubtitles did not answer. Check the API key in Settings > Theater.".to_string())?;
    let Some(file_id) = found["data"][0]["attributes"]["files"][0]["file_id"].as_u64() else { return Ok(None) };
    let link: serde_json::Value = http
        .post(format!("{OPENSUBTITLES}/download"))
        .header("Api-Key", key)
        .json(&serde_json::json!({ "file_id": file_id, "sub_format": "srt" }))
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json())
        .map_err(|_| "OpenSubtitles would not give the file (the daily limit may be used up).".to_string())?;
    let Some(url) = link["link"].as_str() else { return Ok(None) };
    let text = http.get(url).send().and_then(|r| r.error_for_status()).and_then(|r| r.text()).map_err(|_| "The subtitle file did not download.".to_string())?;
    Ok((text.len() <= MAX_SUBTITLE_BYTES).then_some(text))
}

/// SubRip (.srt) to WebVTT, which browsers show: a header, and dots for commas in times.
pub fn srt_to_vtt(srt: &str) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for line in srt.trim_start_matches('\u{feff}').lines() {
        if line.contains("-->") {
            out.push_str(&line.replace(',', "."));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_fragments_become_text() {
        assert_eq!(plain_text("<p>A <b>chemistry</b> teacher &amp; his partner.</p>"), "A chemistry teacher & his partner.");
    }

    #[test]
    fn subrip_becomes_webvtt() {
        let vtt = srt_to_vtt("1\r\n00:00:01,000 --> 00:00:02,500\r\nHello, there.\r\n");
        assert!(vtt.starts_with("WEBVTT\n\n"));
        assert!(vtt.contains("00:00:01.000 --> 00:00:02.500"));
        assert!(vtt.contains("Hello, there."), "commas in the words stay");
    }

    #[test]
    fn search_terms_lose_special_characters() {
        assert_eq!(lucene("AC/DC: Back in Black!"), "ACDC Back in Black");
    }

    #[test]
    fn the_movie_hash_matches_the_published_example() {
        // A file of 131072 zero bytes hashes to its size alone.
        let path = std::env::temp_dir().join(format!("castled-hash-{}.bin", crate::random_hex(6)));
        std::fs::write(&path, vec![0u8; 131072]).unwrap();
        assert_eq!(movie_hash(&path).as_deref(), Some("0000000000020000"));
        std::fs::remove_file(path).unwrap();
    }
}
