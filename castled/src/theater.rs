//! Theater: films, shows, music, photos and home videos from folders on this computer.
//!
//! A library is a folder and what it holds (films, shows, music, photos, videos). The box
//! walks each library, names what it finds from the files themselves (a film's title and
//! year from its file name, an episode's season and number, a song's tags), and keeps the
//! list in the vault's private folder. Pictures come from the files (a frame of a video, a
//! photo made small, a song's embedded cover) and, when asked, from open sources on the
//! internet (see lookup.rs), downloaded once and kept.
//!
//! Playing: a browser's video player cannot send the box token, so the app asks for a play
//! link first. The link names a random ticket that opens that one file, for a few hours.
//!
//! Who sees what: each library is for everyone, the family (no guests), adults, or only the
//! box's admins, and any item can be marked adults only. The box does not yet know which
//! person is asking (every device holds the same token), so the app applies these limits.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use crate::lookup::{self, Details, Episode};

pub type TheaterResult<T> = Result<T, String>;

const VIDEO: [&str; 13] = ["mp4", "m4v", "mkv", "webm", "mov", "avi", "wmv", "mpg", "mpeg", "ts", "m2ts", "flv", "3gp"];
const AUDIO: [&str; 10] = ["mp3", "flac", "m4a", "aac", "ogg", "oga", "opus", "wav", "wma", "aiff"];
const PHOTO: [&str; 9] = ["jpg", "jpeg", "png", "gif", "webp", "bmp", "heic", "heif", "avif"];
const SUBTITLE: [&str; 2] = ["srt", "vtt"];
/// Words a release name carries after the title: everything from the first of these on
/// is dropped when there is no year to cut at.
const RELEASE_WORDS: [&str; 24] = [
    "2160p", "1080p", "1080i", "720p", "480p", "4k", "uhd", "hdr", "bluray", "blu-ray", "brrip", "bdrip", "webrip", "web-dl", "webdl", "web",
    "hdtv", "dvdrip", "x264", "x265", "h264", "h265", "hevc", "remux",
];
pub const KINDS: [&str; 5] = ["movies", "shows", "music", "photos", "videos"];
pub const AUDIENCES: [&str; 4] = ["everyone", "family", "adults", "admins"];
/// A scan stops here so one enormous folder cannot stall the box.
const MAX_ITEMS: usize = 200_000;
const MAX_DEPTH: usize = 16;
/// How long a play link works.
const TICKET_LIFE: Duration = Duration::from_secs(8 * 60 * 60);
const MAX_TICKETS: usize = 400;
const MAX_TAGS: usize = 24;
const MAX_TAG_LEN: usize = 40;
const MAX_TITLE_LEN: usize = 200;
/// ffmpeg runs at once, at most; a grid of new thumbnails would otherwise start dozens.
const FFMPEG_AT_ONCE: usize = 3;
const THUMB_WIDTH: u32 = 480;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn now_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn short_hash(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn extension(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

// ---------------------------------------------------------------------------------------
// Reading names
// ---------------------------------------------------------------------------------------

/// Dots and underscores as spaces, brackets' contents gone, spaces single.
fn tidy(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' => depth = (depth - 1).max(0),
            '.' | '_' if depth == 0 => out.push(' '),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ").trim_matches(|c: char| c == '-' || c.is_whitespace()).to_string()
}

fn is_year(word: &str) -> Option<u32> {
    let digits = word.trim_matches(|c: char| c == '(' || c == ')');
    if digits.len() != 4 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let year: u32 = digits.parse().ok()?;
    (1888..=2100).contains(&year).then_some(year)
}

/// "[2016]" -> "(2016)", so a year in square brackets survives `tidy`.
fn bracketed_years(stem: &str) -> String {
    let mut out = stem.to_string();
    for (at, _) in stem.match_indices('[') {
        let inner = stem.get(at + 1..at + 5).unwrap_or("");
        if stem.get(at + 5..at + 6) == Some("]") && is_year(inner).is_some() {
            out.replace_range(at..at + 6, &format!("({inner})"));
        }
    }
    out
}

/// "Blade.Runner.1982.1080p.BluRay" or "Blade Runner (1982)" -> ("Blade Runner", 1982).
pub fn film_name(stem: &str) -> (String, u32) {
    let tidied = tidy(&bracketed_years(stem));
    let words: Vec<&str> = tidied.split(' ').filter(|w| !w.is_empty()).collect();
    // The last year that is not the first word ("2001 A Space Odyssey 1968" -> 1968).
    if let Some(at) = (1..words.len()).rev().find(|&i| is_year(words[i]).is_some()) {
        let year = is_year(words[at]).unwrap_or(0);
        return (words[..at].join(" ").trim_end_matches(['(', '-', ' ']).to_string(), year);
    }
    let cut = words.iter().position(|w| RELEASE_WORDS.contains(&w.to_ascii_lowercase().as_str())).unwrap_or(words.len());
    (words[..cut.max(1).min(words.len())].join(" "), 0)
}

/// "The.Office.S02E03.Office.Olympics" -> ("The Office", 2, 3, "Office Olympics"). Also "2x03".
pub fn episode_name(stem: &str) -> Option<(String, u32, u32, String)> {
    let text = tidy(stem);
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    for i in 0..bytes.len() {
        let boundary = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
        if !boundary {
            continue;
        }
        // S01E02
        if bytes[i] == b's' {
            let season: String = lower[i + 1..].chars().take_while(char::is_ascii_digit).collect();
            let after = i + 1 + season.len();
            if !season.is_empty() && season.len() <= 2 && bytes.get(after) == Some(&b'e') {
                let number: String = lower[after + 1..].chars().take_while(char::is_ascii_digit).collect();
                if !number.is_empty() && number.len() <= 3 {
                    let end = after + 1 + number.len();
                    let show = text[..i].trim_end_matches([' ', '-']).to_string();
                    let title = tidy(&text[end..]);
                    return Some((show, season.parse().ok()?, number.parse().ok()?, strip_release(&title)));
                }
            }
        }
        // 2x03
        if bytes[i].is_ascii_digit() {
            let season: String = lower[i..].chars().take_while(char::is_ascii_digit).collect();
            let after = i + season.len();
            if season.len() <= 2 && bytes.get(after) == Some(&b'x') {
                let number: String = lower[after + 1..].chars().take_while(char::is_ascii_digit).collect();
                if number.len() == 2 {
                    let end = after + 1 + number.len();
                    let show = text[..i].trim_end_matches([' ', '-']).to_string();
                    return Some((show, season.parse().ok()?, number.parse().ok()?, strip_release(&tidy(&text[end..]))));
                }
            }
        }
    }
    None
}

fn strip_release(title: &str) -> String {
    let words: Vec<&str> = title.split(' ').collect();
    let cut = words.iter().position(|w| RELEASE_WORDS.contains(&w.to_ascii_lowercase().as_str())).unwrap_or(words.len());
    words[..cut].join(" ")
}

/// "Season 2" or "S02" -> 2.
fn season_folder(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    let digits = lower.strip_prefix("season").or_else(|| lower.strip_prefix('s'))?.trim();
    digits.parse().ok()
}

/// "03 - Title" or "03. Title" -> (3, "Title").
fn track_name(stem: &str) -> (u32, String) {
    let digits: String = stem.chars().take_while(char::is_ascii_digit).collect();
    if !digits.is_empty() && digits.len() <= 3 {
        let rest = stem[digits.len()..].trim_start_matches([' ', '-', '.', '_']).trim();
        if !rest.is_empty() {
            return (digits.parse().unwrap_or(0), rest.to_string());
        }
    }
    (0, stem.to_string())
}

// ---------------------------------------------------------------------------------------
// What is kept
// ---------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub id: String,
    pub name: String,
    /// movies, shows, music, photos, videos
    pub kind: String,
    pub path: String,
    /// everyone, family, adults, admins
    pub audience: String,
    pub added: u64,
    #[serde(default)]
    pub scanned: u64,
    #[serde(default)]
    pub count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Subtitle {
    pub label: String,
    pub file: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Item {
    pub id: String,
    pub library: String,
    /// movie, episode, track, photo, video
    pub kind: String,
    pub title: String,
    pub year: u32,
    pub show: String,
    pub season: u32,
    pub episode: u32,
    pub artist: String,
    pub album: String,
    pub track: u32,
    pub genre: String,
    /// The folder it is in, by name: a photo's album, a video's group.
    pub folder: String,
    pub path: String,
    pub size: u64,
    pub modified: u64,
    pub added: u64,
    /// The file carries its own cover picture (a song's).
    pub cover: bool,
    pub subtitles: Vec<Subtitle>,
    // Set by people, kept separately, joined in when the list is sent.
    pub tags: Vec<String>,
    pub adults_only: bool,
}

/// What a person has changed about an item.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Edit {
    pub title: Option<String>,
    pub year: Option<u32>,
    pub tags: Vec<String>,
    pub adults_only: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditBody {
    pub id: String,
    pub title: Option<String>,
    pub year: Option<u32>,
    pub tags: Option<Vec<String>>,
    pub adults_only: Option<bool>,
}

/// What the video or song file itself says: length, picture size, how it is encoded.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Probe {
    pub modified: u64,
    pub seconds: f64,
    pub width: u32,
    pub height: u32,
    pub video: String,
    pub audio: String,
    pub bitrate: u64,
}

struct Ticket {
    path: PathBuf,
    subtitles: Vec<PathBuf>,
    made: Instant,
}

#[derive(Default, Serialize, Deserialize)]
struct Index {
    libraries: Vec<Library>,
    items: Vec<Item>,
}

/// One file of kept answers, keyed: details, episodes, probes.
struct Kept<T> {
    file: PathBuf,
    map: Mutex<HashMap<String, T>>,
}

impl<T: Clone + Serialize + DeserializeOwned> Kept<T> {
    fn open(file: PathBuf) -> Self {
        let map = std::fs::read(&file).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default();
        Self { file, map: Mutex::new(map) }
    }
    fn get(&self, key: &str) -> Option<T> {
        self.map.lock().ok()?.get(key).cloned()
    }
    fn put(&self, key: &str, value: T) {
        let Ok(mut map) = self.map.lock() else { return };
        map.insert(key.to_string(), value);
        save_json(&self.file, &*map);
    }
}

fn save_json<T: Serialize + ?Sized>(file: &Path, value: &T) {
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = file.with_extension("json.tmp");
    if std::fs::write(&tmp, serde_json::to_vec(value).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(tmp, file);
    }
}

/// Lets a few ffmpeg runs go at once and makes the rest wait their turn.
struct Gate {
    busy: Mutex<usize>,
    freed: Condvar,
}

impl Gate {
    fn run<T>(&self, work: impl FnOnce() -> T) -> T {
        let mut busy = self.busy.lock().unwrap_or_else(|e| e.into_inner());
        while *busy >= FFMPEG_AT_ONCE {
            busy = self.freed.wait(busy).unwrap_or_else(|e| e.into_inner());
        }
        *busy += 1;
        drop(busy);
        let out = work();
        *self.busy.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
        self.freed.notify_one();
        out
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub libraries: Vec<Library>,
    pub scanning: bool,
    /// ffmpeg is installed: video frames and running times are available.
    pub ffmpeg: bool,
    /// Folders that look like media, for a first library.
    pub suggestions: Vec<Suggestion>,
}

#[derive(Serialize)]
pub struct Suggestion {
    pub path: String,
    pub kind: &'static str,
    pub name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailsAnswer {
    pub details: Option<Details>,
    pub episode: Option<Episode>,
    pub probe: Option<Probe>,
    /// A poster or cover has been kept and can be asked for.
    pub poster: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayLink {
    pub url: String,
    pub mime: String,
    pub subtitles: Vec<SubtitleLink>,
}

#[derive(Serialize)]
pub struct SubtitleLink {
    pub label: String,
    pub url: String,
}

pub struct Theater {
    dir: PathBuf,
    index: Mutex<Index>,
    edits: Kept<Edit>,
    details: Kept<Option<Details>>,
    episodes: Kept<Option<Episode>>,
    probes: Kept<Probe>,
    tickets: Mutex<HashMap<String, Ticket>>,
    scanning: AtomicBool,
    ffmpeg: Option<(PathBuf, PathBuf)>,
    gate: Gate,
}

/// ffmpeg and ffprobe, when they can be started: CASTLE_FFMPEG_DIR, then the system's path.
fn find_ffmpeg() -> Option<(PathBuf, PathBuf)> {
    let name = |tool: &str| if cfg!(windows) { format!("{tool}.exe") } else { tool.to_string() };
    let dirs: Vec<PathBuf> = std::env::var_os("CASTLE_FFMPEG_DIR")
        .map(PathBuf::from)
        .into_iter()
        .chain(std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default())
        .collect();
    let ffmpeg = dirs.iter().map(|d| d.join(name("ffmpeg"))).find(|p| p.is_file())?;
    let ffprobe = dirs.iter().map(|d| d.join(name("ffprobe"))).find(|p| p.is_file())?;
    Some((ffmpeg, ffprobe))
}

fn quiet(mut cmd: Command) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    cmd
}

impl Theater {
    pub fn open(dir: PathBuf) -> Self {
        let index = std::fs::read(dir.join("index.json")).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default();
        Self {
            edits: Kept::open(dir.join("edits.json")),
            details: Kept::open(dir.join("details.json")),
            episodes: Kept::open(dir.join("episodes.json")),
            probes: Kept::open(dir.join("probes.json")),
            index: Mutex::new(index),
            tickets: Mutex::new(HashMap::new()),
            scanning: AtomicBool::new(false),
            ffmpeg: find_ffmpeg(),
            gate: Gate { busy: Mutex::new(0), freed: Condvar::new() },
            dir,
        }
    }

    fn save_index(&self, index: &Index) {
        save_json(&self.dir.join("index.json"), index);
    }

    pub fn overview(&self) -> Overview {
        let libraries = self.index.lock().map(|i| i.libraries.clone()).unwrap_or_default();
        let taken: HashSet<String> = libraries.iter().map(|l| l.path.to_lowercase()).collect();
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
        let mut suggestions = Vec::new();
        if let Some(home) = home {
            for (folder, kind) in [("Videos", "videos"), ("Movies", "movies"), ("Music", "music"), ("Pictures", "photos")] {
                let path = home.join(folder);
                let text = path.to_string_lossy().to_string();
                if path.is_dir() && !taken.contains(&text.to_lowercase()) {
                    suggestions.push(Suggestion { path: text, kind, name: folder.to_string() });
                }
            }
        }
        Overview { libraries, scanning: self.scanning.load(Ordering::Relaxed), ffmpeg: self.ffmpeg.is_some(), suggestions }
    }

    // ---- libraries ----

    pub fn add_library(&self, path: &Path, kind: &str, name: &str, audience: &str) -> TheaterResult<Library> {
        if !KINDS.contains(&kind) {
            return Err("A library holds movies, shows, music, photos or videos.".into());
        }
        if !AUDIENCES.contains(&audience) {
            return Err("unknown audience".into());
        }
        if !path.is_dir() {
            return Err("That folder is not there.".into());
        }
        let text = crate::plain_path(path).to_string_lossy().to_string();
        let mut index = self.index.lock().map_err(|_| "Theater is unavailable".to_string())?;
        if index.libraries.iter().any(|l| l.path.eq_ignore_ascii_case(&text)) {
            return Err("That folder is already a library.".into());
        }
        let name = name.trim();
        let name = if name.is_empty() { path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| text.clone()) } else { name.chars().take(80).collect() };
        let library = Library { id: crate::random_hex(6), name, kind: kind.into(), path: text, audience: audience.into(), added: now_millis(), scanned: 0, count: 0 };
        index.libraries.push(library.clone());
        self.save_index(&index);
        Ok(library)
    }

    pub fn update_library(&self, id: &str, name: Option<&str>, audience: Option<&str>) -> TheaterResult<Library> {
        let mut index = self.index.lock().map_err(|_| "Theater is unavailable".to_string())?;
        let library = index.libraries.iter_mut().find(|l| l.id == id).ok_or("No such library.")?;
        if let Some(audience) = audience {
            if !AUDIENCES.contains(&audience) {
                return Err("unknown audience".into());
            }
            library.audience = audience.into();
        }
        if let Some(name) = name.map(str::trim).filter(|n| !n.is_empty()) {
            library.name = name.chars().take(80).collect();
        }
        let out = library.clone();
        self.save_index(&index);
        Ok(out)
    }

    /// Forgets a library and what was found in it. The folder and its files are not touched.
    pub fn remove_library(&self, id: &str) -> TheaterResult<()> {
        let mut index = self.index.lock().map_err(|_| "Theater is unavailable".to_string())?;
        let before = index.libraries.len();
        index.libraries.retain(|l| l.id != id);
        if index.libraries.len() == before {
            return Err("No such library.".into());
        }
        index.items.retain(|i| i.library != id);
        self.save_index(&index);
        Ok(())
    }

    // ---- scanning ----

    /// Walks every library (or one) and brings the list up to date. One scan at a time;
    /// returns false when one was already running.
    pub fn scan(&self, only: Option<&str>) -> bool {
        if self.scanning.swap(true, Ordering::SeqCst) {
            return false;
        }
        let libraries: Vec<Library> = self.index.lock().map(|i| i.libraries.iter().filter(|l| only.is_none_or(|id| l.id == id)).cloned().collect()).unwrap_or_default();
        for library in libraries {
            let known: HashMap<String, Item> =
                self.index.lock().map(|i| i.items.iter().filter(|it| it.library == library.id).map(|it| (it.id.clone(), it.clone())).collect()).unwrap_or_default();
            let found = scan_library(&library, &known);
            if let Ok(mut index) = self.index.lock() {
                index.items.retain(|it| it.library != library.id);
                let count = found.len();
                index.items.extend(found);
                if let Some(l) = index.libraries.iter_mut().find(|l| l.id == library.id) {
                    l.scanned = now_millis();
                    l.count = count;
                }
                self.save_index(&index);
            }
        }
        self.scanning.store(false, Ordering::SeqCst);
        true
    }

    pub fn is_scanning(&self) -> bool {
        self.scanning.load(Ordering::Relaxed)
    }

    // ---- items ----

    fn with_edits(&self, mut item: Item) -> Item {
        if let Some(edit) = self.edits.get(&item.id) {
            if let Some(title) = edit.title {
                item.title = title;
            }
            if let Some(year) = edit.year {
                item.year = year;
            }
            item.tags = edit.tags;
            item.adults_only = edit.adults_only;
        }
        item
    }

    pub fn items(&self, library: Option<&str>) -> Vec<Item> {
        let items: Vec<Item> =
            self.index.lock().map(|i| i.items.iter().filter(|it| library.is_none_or(|l| it.library == l)).cloned().collect()).unwrap_or_default();
        items.into_iter().map(|it| self.with_edits(it)).collect()
    }

    fn item(&self, id: &str) -> TheaterResult<Item> {
        let item = self.index.lock().ok().and_then(|i| i.items.iter().find(|it| it.id == id).cloned()).ok_or("That is no longer in Theater. Scan again.")?;
        Ok(self.with_edits(item))
    }

    pub fn edit(&self, body: EditBody) -> TheaterResult<Item> {
        self.item(&body.id)?;
        let mut edit = self.edits.get(&body.id).unwrap_or_default();
        if let Some(title) = body.title {
            let title = title.trim().chars().take(MAX_TITLE_LEN).collect::<String>();
            edit.title = (!title.is_empty()).then_some(title);
        }
        if let Some(year) = body.year {
            edit.year = (year > 0).then_some(year);
        }
        if let Some(tags) = body.tags {
            let mut clean: Vec<String> = Vec::new();
            for tag in tags {
                let tag: String = tag.trim().chars().filter(|c| !c.is_control()).take(MAX_TAG_LEN).collect();
                if !tag.is_empty() && !clean.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
                    clean.push(tag);
                }
            }
            clean.truncate(MAX_TAGS);
            edit.tags = clean;
        }
        if let Some(adults_only) = body.adults_only {
            edit.adults_only = adults_only;
        }
        self.edits.put(&body.id, edit);
        self.item(&body.id)
    }

    // ---- what the files say ----

    pub fn probe(&self, item: &Item) -> Option<Probe> {
        let (_, ffprobe) = self.ffmpeg.as_ref()?;
        if item.kind == "photo" {
            return None;
        }
        if let Some(kept) = self.probes.get(&item.id).filter(|p| p.modified == item.modified) {
            return Some(kept);
        }
        let mut cmd = quiet(Command::new(ffprobe));
        cmd.args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"]).arg(&item.path);
        let out = self.gate.run(|| cmd.output()).ok().filter(|o| o.status.success())?;
        let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        let streams = json["streams"].as_array().cloned().unwrap_or_default();
        let video = streams.iter().find(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"] != 1);
        let audio = streams.iter().find(|s| s["codec_type"] == "audio");
        let probe = Probe {
            modified: item.modified,
            seconds: json["format"]["duration"].as_str().and_then(|d| d.parse().ok()).unwrap_or(0.0),
            width: video.and_then(|v| v["width"].as_u64()).unwrap_or(0) as u32,
            height: video.and_then(|v| v["height"].as_u64()).unwrap_or(0) as u32,
            video: video.and_then(|v| v["codec_name"].as_str()).unwrap_or("").to_string(),
            audio: audio.and_then(|a| a["codec_name"].as_str()).unwrap_or("").to_string(),
            bitrate: json["format"]["bit_rate"].as_str().and_then(|b| b.parse().ok()).unwrap_or(0),
        };
        self.probes.put(&item.id, probe.clone());
        Some(probe)
    }

    /// A small picture of an item: a frame of a video, a photo made small, a song's cover.
    /// Made once and kept. Returns the file and its type.
    pub fn thumb(&self, id: &str) -> TheaterResult<(PathBuf, String)> {
        let item = self.item(id)?;
        let dir = self.dir.join("thumbs");
        let _ = std::fs::create_dir_all(&dir);
        let out = dir.join(format!("{}-{}.jpg", item.id, item.modified));
        if out.is_file() {
            return Ok((out, "image/jpeg".into()));
        }
        let source = PathBuf::from(&item.path);
        if item.kind == "track" {
            let tags = crate::media_tags::read(&source);
            let (mime, bytes) = tags.picture.ok_or("This song has no cover in it.")?;
            let kept = dir.join(format!("{}-{}.cover", item.id, item.modified));
            std::fs::write(&kept, bytes).map_err(|_| "The cover could not be kept.".to_string())?;
            return Ok((kept, mime));
        }
        let Some((ffmpeg, _)) = &self.ffmpeg else {
            // Without ffmpeg a photo is shown as it is; a video has no picture.
            if item.kind == "photo" {
                let mime = mime_guess::from_path(&source).first_or_octet_stream().to_string();
                return Ok((source, mime));
            }
            return Err("Install ffmpeg for pictures of videos.".into());
        };
        let mut cmd = quiet(Command::new(ffmpeg));
        cmd.args(["-v", "error", "-y"]);
        if item.kind != "photo" {
            // A tenth of the way in, past the opening titles, but no later than two minutes.
            let at = self.probe(&item).map(|p| (p.seconds * 0.1).min(120.0)).filter(|s| *s > 0.0).unwrap_or(5.0);
            cmd.args(["-ss", &format!("{at:.1}")]);
        }
        cmd.arg("-i").arg(&source).args(["-frames:v", "1", "-vf", &format!("scale='min({THUMB_WIDTH},iw)':-2"), "-q:v", "4"]).arg(&out);
        let ok = self.gate.run(|| cmd.status()).is_ok_and(|s| s.success()) && out.is_file();
        if ok {
            Ok((out, "image/jpeg".into()))
        } else if item.kind == "photo" {
            let mime = mime_guess::from_path(&source).first_or_octet_stream().to_string();
            Ok((source, mime))
        } else {
            Err("No picture could be taken from this video.".into())
        }
    }

    // ---- details from the internet ----

    fn lookup_key(item: &Item) -> Option<String> {
        match item.kind.as_str() {
            "movie" => Some(format!("film|{}|{}", item.title.to_lowercase(), item.year)),
            "episode" if !item.show.is_empty() => Some(format!("show|{}", item.show.to_lowercase())),
            "track" if !item.album.is_empty() => Some(format!("album|{}|{}", item.artist.to_lowercase(), item.album.to_lowercase())),
            _ => None,
        }
    }

    fn poster_file(&self, key: &str) -> PathBuf {
        self.dir.join("art").join(format!("{}.img", short_hash(key)))
    }

    /// Details of an item. With `fetch`, asks the internet the first time; without, only
    /// says what is already known.
    pub fn details(&self, id: &str, fetch: bool) -> TheaterResult<DetailsAnswer> {
        let item = self.item(id)?;
        let probe = self.probe(&item);
        let Some(key) = Self::lookup_key(&item) else { return Ok(DetailsAnswer { details: None, episode: None, probe, poster: false }) };
        let details = match self.details.get(&key) {
            Some(kept) => kept,
            None if fetch => {
                let http = crate::wiki::client()?;
                let found = match item.kind.as_str() {
                    "movie" => lookup::film(&http, &item.title, item.year)?,
                    "episode" => lookup::show(&http, &item.show)?,
                    _ => lookup::album(&http, &item.artist, &item.album)?,
                };
                if let Some(url) = found.as_ref().and_then(|d| d.poster_url.clone()) {
                    if let Some(bytes) = lookup::fetch_picture(&http, &url) {
                        let file = self.poster_file(&key);
                        let _ = std::fs::create_dir_all(file.parent().unwrap_or(&self.dir));
                        let _ = std::fs::write(file, bytes);
                    }
                }
                self.details.put(&key, found.clone());
                found
            }
            None => None,
        };
        let mut episode = None;
        if item.kind == "episode" {
            if let Some(show_id) = details.as_ref().and_then(|d| d.tvmaze_id) {
                let ep_key = format!("ep|{show_id}|{}|{}", item.season, item.episode);
                episode = match self.episodes.get(&ep_key) {
                    Some(kept) => kept,
                    None if fetch => {
                        let http = crate::wiki::client()?;
                        let found = lookup::episode(&http, show_id, item.season, item.episode)?;
                        self.episodes.put(&ep_key, found.clone());
                        found
                    }
                    None => None,
                };
            }
        }
        Ok(DetailsAnswer { poster: self.poster_file(&key).is_file(), details, episode, probe })
    }

    /// The kept poster or cover for an item, and its type, read from its first bytes.
    pub fn poster(&self, id: &str) -> TheaterResult<(PathBuf, String)> {
        let item = self.item(id)?;
        let key = Self::lookup_key(&item).ok_or("no poster")?;
        let file = self.poster_file(&key);
        let head = std::fs::read(&file).map_err(|_| "no poster".to_string())?;
        let mime = match head.get(..4) {
            Some([0x89, b'P', b'N', b'G']) => "image/png",
            Some([b'R', b'I', b'F', b'F']) => "image/webp",
            Some([b'G', b'I', b'F', _]) => "image/gif",
            _ => "image/jpeg",
        };
        Ok((file, mime.into()))
    }

    // ---- playing ----

    /// A link that plays one item for the next few hours, with its subtitles.
    pub fn play(&self, id: &str) -> TheaterResult<PlayLink> {
        let item = self.item(id)?;
        let path = PathBuf::from(&item.path);
        if !path.is_file() {
            return Err("That file is not there any more. Is its drive plugged in?".into());
        }
        let mut subtitles: Vec<(String, PathBuf)> =
            item.subtitles.iter().map(|s| (s.label.clone(), PathBuf::from(&s.file))).filter(|(_, p)| p.is_file()).collect();
        let fetched = self.dir.join("subs").join(format!("{}.vtt", item.id));
        if fetched.is_file() {
            subtitles.push(("English (OpenSubtitles)".into(), fetched));
        }
        let ticket = crate::random_hex(16);
        let mut tickets = self.tickets.lock().map_err(|_| "Theater is unavailable".to_string())?;
        tickets.retain(|_, t| t.made.elapsed() < TICKET_LIFE);
        if tickets.len() >= MAX_TICKETS {
            if let Some(oldest) = tickets.iter().min_by_key(|(_, t)| t.made).map(|(k, _)| k.clone()) {
                tickets.remove(&oldest);
            }
        }
        let links = subtitles.iter().enumerate().map(|(n, (label, _))| SubtitleLink { label: label.clone(), url: format!("/api/v1/media/{ticket}/sub/{n}") }).collect();
        tickets.insert(ticket.clone(), Ticket { path, subtitles: subtitles.into_iter().map(|(_, p)| p).collect(), made: Instant::now() });
        Ok(PlayLink { url: format!("/api/v1/media/{ticket}"), mime: media_type(&item.path), subtitles: links })
    }

    /// The file a play link opens, if the link is still good.
    pub fn ticket_file(&self, ticket: &str) -> Option<PathBuf> {
        let tickets = self.tickets.lock().ok()?;
        tickets.get(ticket).filter(|t| t.made.elapsed() < TICKET_LIFE).map(|t| t.path.clone())
    }

    /// A play link's subtitles, as WebVTT.
    pub fn ticket_subtitle(&self, ticket: &str, n: usize) -> Option<String> {
        let path = {
            let tickets = self.tickets.lock().ok()?;
            tickets.get(ticket).filter(|t| t.made.elapsed() < TICKET_LIFE)?.subtitles.get(n)?.clone()
        };
        let text = std::fs::read(&path).ok()?;
        let text = String::from_utf8_lossy(&text);
        Some(if extension(&path) == "vtt" { text.to_string() } else { lookup::srt_to_vtt(&text) })
    }

    /// Asks OpenSubtitles for English subtitles for a video, with the key from Settings.
    pub fn fetch_subtitles(&self, id: &str, key: &str) -> TheaterResult<bool> {
        if key.trim().is_empty() {
            return Err("Add an OpenSubtitles API key in Settings > Theater first. It is free at opensubtitles.com.".into());
        }
        let item = self.item(id)?;
        if !matches!(item.kind.as_str(), "movie" | "episode" | "video") {
            return Err("Subtitles are for videos.".into());
        }
        let query = match item.kind.as_str() {
            "episode" => format!("{} S{:02}E{:02}", item.show, item.season, item.episode),
            _ if item.year > 0 => format!("{} {}", item.title, item.year),
            _ => item.title.clone(),
        };
        let hash = lookup::movie_hash(Path::new(&item.path));
        let http = crate::wiki::client()?;
        let Some(srt) = lookup::subtitles(&http, key.trim(), &query, hash.as_deref(), "en")? else { return Ok(false) };
        let file = self.dir.join("subs").join(format!("{}.vtt", item.id));
        let _ = std::fs::create_dir_all(file.parent().unwrap_or(&self.dir));
        std::fs::write(file, lookup::srt_to_vtt(&srt)).map_err(|_| "The subtitles could not be kept.".to_string())?;
        Ok(true)
    }
}

/// The type a player is told. Matroska is sent as WebM, which browsers play when the
/// streams inside are ones they know.
pub fn media_type(path: &str) -> String {
    match extension(Path::new(path)).as_str() {
        "mkv" => "video/webm".into(),
        "m4a" => "audio/mp4".into(),
        _ => mime_guess::from_path(path).first_or_octet_stream().to_string(),
    }
}

/// Walks one library's folder. Files that have not changed keep what was read before.
fn scan_library(library: &Library, known: &HashMap<String, Item>) -> Vec<Item> {
    let root = PathBuf::from(&library.path);
    let mut out = Vec::new();
    let mut folders = vec![(root.clone(), 0usize)];
    let now = now_millis();
    while let Some((dir, depth)) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut files = Vec::new();
        let mut subs = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name.starts_with('$') {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                if depth < MAX_DEPTH {
                    folders.push((entry.path(), depth + 1));
                }
                continue;
            }
            let ext = extension(&entry.path());
            if SUBTITLE.contains(&ext.as_str()) {
                subs.push(entry.path());
            } else {
                files.push((entry.path(), meta));
            }
        }
        for (path, meta) in files {
            if out.len() >= MAX_ITEMS {
                return out;
            }
            let ext = extension(&path);
            let is_video = VIDEO.contains(&ext.as_str());
            let is_audio = AUDIO.contains(&ext.as_str());
            let is_photo = PHOTO.contains(&ext.as_str());
            let kind = match library.kind.as_str() {
                "movies" if is_video => "movie",
                "shows" if is_video => "episode",
                "music" if is_audio => "track",
                "photos" if is_photo => "photo",
                "photos" | "videos" if is_video => "video",
                "videos" if is_photo => "photo",
                _ => continue,
            };
            let text = crate::plain_path(&path).to_string_lossy().to_string();
            let id = short_hash(&format!("{}|{}", library.id, text));
            let modified = meta.modified().map(crate::millis).unwrap_or(0);
            let size = meta.len();
            if let Some(old) = known.get(&id).filter(|old| old.modified == modified && old.size == size) {
                let mut item = old.clone();
                item.subtitles = subtitles_for(&path, &subs);
                out.push(item);
                continue;
            }
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let parent = path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let mut item = Item {
                id: id.clone(),
                library: library.id.clone(),
                kind: kind.into(),
                folder: if path.parent() == Some(root.as_path()) { String::new() } else { parent.clone() },
                path: text,
                size,
                modified,
                added: known.get(&id).map(|o| o.added).unwrap_or(now),
                ..Item::default()
            };
            match kind {
                "movie" => {
                    let (mut title, mut year) = film_name(&stem);
                    // A file named only by its quality ("movie.1080p.mkv") is named by its folder.
                    if year == 0 && path.parent() != Some(root.as_path()) {
                        let (folder_title, folder_year) = film_name(&parent);
                        if folder_year > 0 {
                            (title, year) = (folder_title, folder_year);
                        }
                    }
                    item.title = title;
                    item.year = year;
                }
                "episode" => {
                    // The show is the first folder under the library, when there is one.
                    let top = path.strip_prefix(&root).ok().and_then(|rel| rel.components().next()).map(|c| c.as_os_str().to_string_lossy().to_string());
                    let top = top.filter(|t| path.parent() != Some(root.as_path()) && t != &stem);
                    match episode_name(&stem) {
                        Some((show, season, number, title)) => {
                            item.show = top.map(|t| film_name(&t).0).filter(|t| !t.is_empty()).unwrap_or(show);
                            item.season = season;
                            item.episode = number;
                            item.title = if title.is_empty() { format!("Episode {number}") } else { title };
                        }
                        None => {
                            item.show = top.map(|t| film_name(&t).0).unwrap_or_else(|| tidy(&parent));
                            item.season = season_folder(&parent).unwrap_or(0);
                            let (number, title) = track_name(&tidy(&stem));
                            item.episode = number;
                            item.title = title;
                        }
                    }
                }
                "track" => {
                    let tags = crate::media_tags::read(&path);
                    let (number, title) = track_name(&stem);
                    // Library/Artist/Album/song: the artist is the folder above the album's.
                    let grand = path
                        .parent()
                        .and_then(Path::parent)
                        .filter(|g| g.starts_with(&root) && *g != root.as_path())
                        .and_then(Path::file_name)
                        .map(|n| n.to_string_lossy().to_string());
                    item.title = if tags.title.is_empty() { title } else { tags.title };
                    item.track = if tags.track > 0 { tags.track } else { number };
                    item.album = if tags.album.is_empty() { item.folder.clone() } else { tags.album };
                    item.artist = if tags.artist.is_empty() { grand.unwrap_or_default() } else { tags.artist };
                    item.year = tags.year;
                    item.genre = tags.genre;
                    item.cover = tags.picture.is_some();
                }
                _ => {
                    item.title = tidy(&stem);
                }
            }
            item.subtitles = subtitles_for(&path, &subs);
            out.push(item);
        }
    }
    out
}

/// Subtitle files beside a video that start with its name: "Film.srt", "Film.en.srt", "Film.English.vtt".
fn subtitles_for(video: &Path, subs: &[PathBuf]) -> Vec<Subtitle> {
    if !VIDEO.contains(&extension(video).as_str()) {
        return Vec::new();
    }
    let stem = video.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mut out: Vec<Subtitle> = subs
        .iter()
        .filter_map(|sub| {
            let sub_stem = sub.file_stem()?.to_string_lossy().to_string();
            let lower = sub_stem.to_lowercase();
            let rest = lower.strip_prefix(&stem)?;
            if !(rest.is_empty() || rest.starts_with('.')) {
                return None;
            }
            let language = sub_stem[stem.len()..].trim_start_matches('.');
            let label = if language.is_empty() { "Subtitles".to_string() } else { language.to_string() };
            Some(Subtitle { label, file: crate::plain_path(sub).to_string_lossy().to_string() })
        })
        .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn film_names_lose_the_release_details() {
        assert_eq!(film_name("Blade.Runner.1982.1080p.BluRay.x264-GRP"), ("Blade Runner".into(), 1982));
        assert_eq!(film_name("Blade Runner (1982)"), ("Blade Runner".into(), 1982));
        assert_eq!(film_name("2001 A Space Odyssey (1968)"), ("2001 A Space Odyssey".into(), 1968));
        assert_eq!(film_name("Arrival [2016] [1080p]"), ("Arrival".into(), 2016));
        assert_eq!(film_name("Heat.1080p.WEBRip"), ("Heat".into(), 0));
        assert_eq!(film_name("home_movie_final"), ("home movie final".into(), 0));
    }

    #[test]
    fn episodes_are_found_in_both_spellings() {
        assert_eq!(episode_name("The.Office.S02E03.Office.Olympics.720p"), Some(("The Office".into(), 2, 3, "Office Olympics".into())));
        assert_eq!(episode_name("Breaking Bad - 1x07 - A No-Rough-Stuff-Type Deal"), Some(("Breaking Bad".into(), 1, 7, "A No-Rough-Stuff-Type Deal".into())));
        assert_eq!(episode_name("s01e01"), Some((String::new(), 1, 1, String::new())));
        assert_eq!(episode_name("Blade Runner 1982"), None);
        assert_eq!(episode_name("Sunset 1080p"), None);
    }

    #[test]
    fn folders_and_tracks_are_read() {
        assert_eq!(season_folder("Season 2"), Some(2));
        assert_eq!(season_folder("S03"), Some(3));
        assert_eq!(season_folder("Extras"), None);
        assert_eq!(track_name("03 - Teardrop"), (3, "Teardrop".into()));
        assert_eq!(track_name("Teardrop"), (0, "Teardrop".into()));
    }

    #[test]
    fn subtitles_beside_a_video_are_found() {
        let dir = Path::new("/films");
        let subs = vec![dir.join("Arrival.srt"), dir.join("Arrival.en.srt"), dir.join("Arrival.Forced.vtt"), dir.join("Arrivals.srt"), dir.join("Other.srt")];
        let found = subtitles_for(&dir.join("Arrival.mkv"), &subs);
        let labels: Vec<&str> = found.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, vec!["Forced", "Subtitles", "en"]);
    }

    fn library_in(dir: &Path, kind: &str) -> Library {
        Library { id: "lib".into(), name: "Test".into(), kind: kind.into(), path: dir.to_string_lossy().to_string(), audience: "everyone".into(), added: 0, scanned: 0, count: 0 }
    }

    #[test]
    fn a_shows_folder_is_read_into_shows_seasons_and_episodes() {
        let dir = std::env::temp_dir().join(format!("castled-theater-{}", crate::random_hex(6)));
        let season = dir.join("The Office (US)").join("Season 2");
        std::fs::create_dir_all(&season).unwrap();
        std::fs::write(season.join("The.Office.S02E03.Office.Olympics.mkv"), b"x").unwrap();
        std::fs::write(season.join("The.Office.S02E03.Office.Olympics.en.srt"), b"1").unwrap();
        std::fs::write(season.join("notes.txt"), b"not media").unwrap();
        let items = scan_library(&library_in(&dir, "shows"), &HashMap::new());
        assert_eq!(items.len(), 1);
        let ep = &items[0];
        assert_eq!((ep.show.as_str(), ep.season, ep.episode, ep.title.as_str()), ("The Office (US)", 2, 3, "Office Olympics"));
        assert_eq!(ep.subtitles.len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn music_without_tags_is_named_by_its_folders() {
        let dir = std::env::temp_dir().join(format!("castled-theater-{}", crate::random_hex(6)));
        let album = dir.join("Massive Attack").join("Mezzanine");
        std::fs::create_dir_all(&album).unwrap();
        std::fs::write(album.join("03 - Teardrop.mp3"), b"no tags here").unwrap();
        let items = scan_library(&library_in(&dir, "music"), &HashMap::new());
        let t = &items[0];
        assert_eq!((t.artist.as_str(), t.album.as_str(), t.track, t.title.as_str()), ("Massive Attack", "Mezzanine", 3, "Teardrop"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn libraries_are_added_scanned_edited_and_forgotten() {
        let dir = std::env::temp_dir().join(format!("castled-theater-{}", crate::random_hex(6)));
        let films = dir.join("Films");
        std::fs::create_dir_all(films.join("Heat (1995)")).unwrap();
        std::fs::write(films.join("Heat (1995)").join("movie.1080p.mkv"), b"x").unwrap();
        std::fs::write(films.join("Arrival.2016.mp4"), b"x").unwrap();
        let theater = Theater::open(dir.join("state"));
        let library = theater.add_library(&films, "movies", "", "family").unwrap();
        assert_eq!(library.name, "Films");
        assert!(theater.add_library(&films, "movies", "", "family").is_err(), "a folder is one library");
        assert!(theater.scan(None));
        let mut titles: Vec<(String, u32)> = theater.items(None).into_iter().map(|i| (i.title, i.year)).collect();
        titles.sort();
        assert_eq!(titles, vec![("Arrival".into(), 2016), ("Heat".into(), 1995)]);
        let heat = theater.items(None).into_iter().find(|i| i.title == "Heat").unwrap();
        let edited = theater.edit(EditBody { id: heat.id.clone(), title: None, year: None, tags: Some(vec!["crime".into(), " Crime ".into(), "".into()]), adults_only: Some(true) }).unwrap();
        assert_eq!(edited.tags, vec!["crime".to_string()]);
        assert!(edited.adults_only);
        // A rescan keeps the edits, and the same id.
        assert!(theater.scan(None));
        assert!(theater.items(None).iter().any(|i| i.id == heat.id && i.adults_only));
        // A play link opens that file, and nothing else.
        let link = theater.play(&heat.id).unwrap();
        let ticket = link.url.rsplit('/').next().unwrap();
        assert_eq!(theater.ticket_file(ticket), Some(PathBuf::from(&heat.path)));
        assert_eq!(theater.ticket_file("0123456789abcdef0123456789abcdef"), None);
        theater.remove_library(&library.id).unwrap();
        assert!(theater.items(None).is_empty());
        assert!(films.join("Arrival.2016.mp4").exists(), "forgetting a library leaves its files alone");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
