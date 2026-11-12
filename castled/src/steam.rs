//! A Steam cache for the home: a game downloads from Valve to the box once, and every PC
//! in the house then gets it from the box over the home network.
//!
//! How Steam finds it: the Steam client looks up `lancache.steamcontent.com`. When that
//! name points at an address in the home (the box's DNS answers it, see dns.rs, or a line
//! in a PC's hosts file), Steam downloads through it, with no setting in Steam to change.
//! This is the same arrangement LANCache uses, so Steam's own downloads page shows the
//! download as usual, only faster the second time.
//!
//! What it keeps: Steam downloads a game as "chunks", each named by the hash of what is in
//! it, so a chunk never changes and can be kept for good. A game's list of chunks (its
//! manifest) is kept too. An update is new chunks, which the first PC to update brings in.
//!
//! Games: a cached depot (a part of a game) is matched to its game from the Steam
//! libraries on this computer and from api.steamcmd.net, which also says when the game was
//! last updated and which manifest is current. Names, art and descriptions come from the
//! Steam store. Purging a game deletes its chunks.
//!
//! The cache answers only Steam's own content requests and forwards only to Valve's
//! content hosts, so it cannot be used to reach anything else.

use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

/// The name Steam looks up to find a cache in the home.
pub const LANCACHE_NAME: &str = "lancache.steamcontent.com";
/// Every answer says who handled it; Steam's check for a cache looks for this header.
const PROCESSED_BY: &str = "X-LanCache-Processed-By";
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_CHUNK_BYTES: usize = 64 * 1024 * 1024;
/// Recent requests kept for the "downloading now" list.
const ACTIVITY_KEPT: usize = 2000;
const ACTIVITY_WINDOW_MS: u64 = 30_000;
/// Game details from the store and steamcmd.net are asked for again after this long.
const APP_INFO_AGE: u64 = 6 * 60 * 60 * 1000;
const STATS_SAVE_EVERY: Duration = Duration::from_secs(30);
const DOH: &str = "https://cloudflare-dns.com/dns-query";
const STEAMCMD_INFO: &str = "https://api.steamcmd.net/v1/info";
const STORE_DETAILS: &str = "https://store.steampowered.com/api/appdetails";
const STORE_SEARCH: &str = "https://store.steampowered.com/api/storesearch/";
const MAX_SEARCH: usize = 10;

fn now_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// A host the cache may fetch from: Valve's content servers only.
pub fn allowed_upstream(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == "steamcontent.com" || host.ends_with(".steamcontent.com") || host.ends_with(".steampowered.com")
}

// ---------------------------------------------------------------------------------------
// What a request asks for
// ---------------------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub enum Wanted {
    Chunk { depot: u32, sha: String },
    Manifest { depot: u32, gid: u64 },
    /// Another depot path (patches, keys...): passed through, not kept.
    Other { depot: u32 },
    Heartbeat,
}

pub fn wanted(path: &str) -> Option<Wanted> {
    if path == "/lancache-heartbeat" {
        return Some(Wanted::Heartbeat);
    }
    let rest = path.strip_prefix("/depot/")?;
    let mut parts = rest.split('/');
    let depot: u32 = parts.next()?.parse().ok()?;
    match (parts.next(), parts.next()) {
        (Some("chunk"), Some(sha)) if sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()) => {
            Some(Wanted::Chunk { depot, sha: sha.to_ascii_lowercase() })
        }
        (Some("manifest"), Some(gid)) => Some(Wanted::Manifest { depot, gid: gid.parse().ok()? }),
        _ => Some(Wanted::Other { depot }),
    }
}

// ---------------------------------------------------------------------------------------
// What is kept about each depot and game
// ---------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DepotStats {
    pub bytes: u64,
    pub chunks: u64,
    pub manifests: Vec<String>,
    pub hits: u64,
    pub misses: u64,
    /// Bytes sent to PCs from what was already kept: downloads the internet did not carry.
    pub saved: u64,
    pub first: u64,
    pub last: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DepotInfo {
    /// The manifest Steam's public branch has now.
    pub manifest: String,
    pub size: u64,
    pub oslist: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AppInfo {
    pub name: String,
    pub description: String,
    pub developers: Vec<String>,
    pub genres: Vec<String>,
    pub released: String,
    pub header: String,
    /// Steam's public build and when it last changed (seconds since 1970).
    pub buildid: String,
    pub updated: u64,
    pub depots: BTreeMap<String, DepotInfo>,
    pub fetched: u64,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Book {
    depots: BTreeMap<u32, DepotStats>,
    apps: BTreeMap<u32, AppInfo>,
    /// Which game each depot belongs to.
    owners: BTreeMap<u32, u32>,
    /// Games someone chose to keep, even before any PC has downloaded them.
    kept: Vec<u32>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Event {
    at: u64,
    client: String,
    depot: u32,
    hit: bool,
    bytes: u64,
}

/// A game installed on this computer, from Steam's own files.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct LocalGame {
    pub app: u32,
    pub name: String,
    pub size: u64,
    /// When Steam last updated it here, milliseconds since 1970.
    pub updated: u64,
    pub buildid: String,
    pub depots: BTreeMap<String, String>,
    pub library: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppView {
    pub app: u32,
    pub name: String,
    pub header: bool,
    pub cached: u64,
    /// What the depots on hand add up to on Steam now.
    pub full: u64,
    /// cached, partial, update, empty
    pub state: &'static str,
    pub updated: u64,
    pub buildid: String,
    pub last_served: u64,
    pub kept: bool,
    pub installed_here: bool,
    pub saved: u64,
    pub depots: Vec<DepotView>,
    pub description: String,
    pub developers: Vec<String>,
    pub genres: Vec<String>,
    pub released: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepotView {
    pub depot: u32,
    pub cached: u64,
    pub chunks: u64,
    pub manifests: usize,
    pub current: String,
    pub up_to_date: bool,
    pub size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Downloading {
    pub client: String,
    pub app: u32,
    pub name: String,
    pub bytes: u64,
    pub hit_percent: u32,
    pub mbps: f64,
}

// ---------------------------------------------------------------------------------------
// Steam's own files on this computer
// ---------------------------------------------------------------------------------------

/// Steam's text format ("key" "value", "key" { ... }), as nested pairs.
#[derive(Debug, Clone, PartialEq)]
pub enum Vdf {
    Text(String),
    Map(Vec<(String, Vdf)>),
}

impl Vdf {
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Map(pairs) => pairs.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            Vdf::Text(_) => None,
        }
    }
    pub fn text(&self, key: &str) -> &str {
        match self.get(key) {
            Some(Vdf::Text(t)) => t,
            _ => "",
        }
    }
    pub fn pairs(&self) -> &[(String, Vdf)] {
        match self {
            Vdf::Map(pairs) => pairs,
            Vdf::Text(_) => &[],
        }
    }
}

pub fn parse_vdf(text: &str) -> Vdf {
    fn tokens(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    let mut s = String::new();
                    while let Some(c) = chars.next() {
                        match c {
                            '\\' => {
                                if let Some(next) = chars.next() {
                                    s.push(match next {
                                        'n' => '\n',
                                        't' => '\t',
                                        other => other,
                                    });
                                }
                            }
                            '"' => break,
                            _ => s.push(c),
                        }
                    }
                    out.push(s);
                }
                '{' => out.push("{".into()),
                '}' => out.push("}".into()),
                '/' if chars.peek() == Some(&'/') => {
                    for c in chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }
    fn map(tokens: &[String], at: &mut usize) -> Vec<(String, Vdf)> {
        let mut pairs = Vec::new();
        while *at < tokens.len() {
            let key = &tokens[*at];
            if key == "}" {
                *at += 1;
                break;
            }
            *at += 1;
            match tokens.get(*at).map(String::as_str) {
                Some("{") => {
                    *at += 1;
                    pairs.push((key.clone(), Vdf::Map(map(tokens, at))));
                }
                Some(value) => {
                    pairs.push((key.clone(), Vdf::Text(value.to_string())));
                    *at += 1;
                }
                None => break,
            }
        }
        pairs
    }
    let tokens = tokens(text);
    let mut at = 0;
    Vdf::Map(map(&tokens, &mut at))
}

/// Where Steam is installed on this computer, if it is.
fn steam_dirs() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::env::var_os("CASTLE_STEAM_DIR").map(PathBuf::from).into_iter().collect();
    if cfg!(windows) {
        found.extend([r"C:\Program Files (x86)\Steam", r"C:\Program Files\Steam"].map(PathBuf::from));
    } else if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        found.extend([home.join(".steam/steam"), home.join(".local/share/Steam")]);
    }
    found.into_iter().filter(|d| d.join("steamapps").is_dir()).collect()
}

/// The games installed on this computer, from every Steam library it has.
pub fn local_games() -> Vec<LocalGame> {
    let mut libraries: Vec<PathBuf> = Vec::new();
    for dir in steam_dirs() {
        libraries.push(dir.clone());
        if let Ok(text) = std::fs::read_to_string(dir.join("steamapps").join("libraryfolders.vdf")) {
            let vdf = parse_vdf(&text);
            for (_, folder) in vdf.get("libraryfolders").map(Vdf::pairs).unwrap_or(&[]) {
                let path = folder.text("path");
                if !path.is_empty() {
                    libraries.push(PathBuf::from(path));
                }
            }
        }
    }
    let mut seen = HashSet::new();
    let mut games = Vec::new();
    for library in libraries {
        let key = library.to_string_lossy().to_lowercase();
        if !seen.insert(key) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(library.join("steamapps")) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else { continue };
            let vdf = parse_vdf(&text);
            let Some(state) = vdf.get("AppState") else { continue };
            let Ok(app) = state.text("appid").parse::<u32>() else { continue };
            let depots = state.get("InstalledDepots").map(Vdf::pairs).unwrap_or(&[]).iter().map(|(d, v)| (d.clone(), v.text("manifest").to_string())).collect();
            games.push(LocalGame {
                app,
                name: state.text("name").to_string(),
                size: state.text("SizeOnDisk").parse().unwrap_or(0),
                updated: state.text("LastUpdated").parse::<u64>().unwrap_or(0) * 1000,
                buildid: state.text("buildid").to_string(),
                depots,
                library: library.to_string_lossy().to_string(),
            });
        }
    }
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    games
}

// ---------------------------------------------------------------------------------------
// The cache
// ---------------------------------------------------------------------------------------

pub struct SteamCache {
    dir: PathBuf,
    book: Mutex<Book>,
    activity: Mutex<VecDeque<Event>>,
    dirty: AtomicBool,
    saved_at: Mutex<Instant>,
    /// Upstream connections, one per Valve host, each pinned to a public address.
    clients: Mutex<HashMap<String, (reqwest::Client, Instant)>>,
    /// Where the cache is listening, once it is.
    pub listening: Mutex<Option<SocketAddr>>,
    refreshing: AtomicBool,
}

impl SteamCache {
    pub fn open(dir: PathBuf) -> Self {
        let book: Book = std::fs::read(dir.join("book.json")).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default();
        Self {
            dir,
            book: Mutex::new(book),
            activity: Mutex::new(VecDeque::new()),
            dirty: AtomicBool::new(false),
            saved_at: Mutex::new(Instant::now()),
            clients: Mutex::new(HashMap::new()),
            listening: Mutex::new(None),
            refreshing: AtomicBool::new(false),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn chunk_path(&self, depot: u32, sha: &str) -> PathBuf {
        self.dir.join("depots").join(depot.to_string()).join("chunks").join(&sha[..2]).join(sha)
    }

    fn manifest_path(&self, depot: u32, gid: u64) -> PathBuf {
        self.dir.join("depots").join(depot.to_string()).join("manifests").join(gid.to_string())
    }

    fn save(&self, book: &Book) {
        let file = self.dir.join("book.json");
        let _ = std::fs::create_dir_all(&self.dir);
        let tmp = file.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec(book).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(tmp, file);
        }
        self.dirty.store(false, Ordering::Relaxed);
        if let Ok(mut at) = self.saved_at.lock() {
            *at = Instant::now();
        }
    }

    /// Writes the counts down every so often, not on every chunk.
    pub fn save_if_due(&self) {
        let due = self.saved_at.lock().map(|at| at.elapsed() >= STATS_SAVE_EVERY).unwrap_or(false);
        if self.dirty.load(Ordering::Relaxed) && due {
            if let Ok(book) = self.book.lock() {
                self.save(&book);
            }
        }
    }

    /// Counts what is on disk, for a cache made before the counts were kept or changed by hand.
    pub fn recount(&self) {
        let root = self.dir.join("depots");
        let Ok(entries) = std::fs::read_dir(&root) else { return };
        let mut counted: BTreeMap<u32, (u64, u64, Vec<String>)> = BTreeMap::new();
        for entry in entries.flatten() {
            let Ok(depot) = entry.file_name().to_string_lossy().parse::<u32>() else { continue };
            let mut bytes = 0;
            let mut chunks = 0;
            if let Ok(prefixes) = std::fs::read_dir(entry.path().join("chunks")) {
                for prefix in prefixes.flatten() {
                    for chunk in std::fs::read_dir(prefix.path()).into_iter().flatten().flatten() {
                        if let Ok(meta) = chunk.metadata() {
                            bytes += meta.len();
                            chunks += 1;
                        }
                    }
                }
            }
            let manifests = std::fs::read_dir(entry.path().join("manifests"))
                .into_iter()
                .flatten()
                .flatten()
                .map(|m| m.file_name().to_string_lossy().to_string())
                .filter(|n| !n.ends_with(".tmp"))
                .collect();
            counted.insert(depot, (bytes, chunks, manifests));
        }
        if let Ok(mut book) = self.book.lock() {
            book.depots.retain(|depot, _| counted.contains_key(depot));
            for (depot, (bytes, chunks, manifests)) in counted {
                let stats = book.depots.entry(depot).or_default();
                stats.bytes = bytes;
                stats.chunks = chunks;
                stats.manifests = manifests;
            }
            self.save(&book);
        }
    }

    fn note(&self, client: IpAddr, depot: u32, hit: bool, bytes: u64, kept_new: Option<&str>) {
        let now = now_millis();
        if let Ok(mut book) = self.book.lock() {
            let stats = book.depots.entry(depot).or_default();
            if stats.first == 0 {
                stats.first = now;
            }
            stats.last = now;
            if hit {
                stats.hits += 1;
                stats.saved += bytes;
            } else {
                stats.misses += 1;
            }
            match kept_new {
                Some("chunk") => {
                    stats.bytes += bytes;
                    stats.chunks += 1;
                }
                Some(gid) if !stats.manifests.iter().any(|m| m == gid) => stats.manifests.push(gid.to_string()),
                _ => {}
            }
        }
        self.dirty.store(true, Ordering::Relaxed);
        if let Ok(mut activity) = self.activity.lock() {
            activity.push_back(Event { at: now, client: client.to_string(), depot, hit, bytes });
            while activity.len() > ACTIVITY_KEPT {
                activity.pop_front();
            }
        }
    }

    /// An upstream client for a Valve host. When this computer's own DNS says the host is
    /// in the home (the box answering for itself), the real address is asked of a public
    /// resolver, so the cache never fetches from itself.
    async fn upstream(&self, host: &str) -> Result<reqwest::Client, String> {
        if let Some((client, made)) = self.clients.lock().ok().and_then(|c| c.get(host).cloned()) {
            if made.elapsed() < Duration::from_secs(600) {
                return Ok(client);
            }
        }
        let local = tokio::net::lookup_host((host, 80)).await.map(|addrs| addrs.collect::<Vec<_>>()).unwrap_or_default();
        let public = local.iter().find(|a| is_public(a.ip())).copied();
        let addr = match public {
            Some(addr) => addr,
            None => SocketAddr::new(doh_lookup(host).await?, 80),
        };
        let client = reqwest::Client::builder()
            .timeout(UPSTREAM_TIMEOUT)
            .resolve(host, addr)
            .build()
            .map_err(|_| "could not start a web request".to_string())?;
        if let Ok(mut clients) = self.clients.lock() {
            clients.insert(host.to_string(), (client.clone(), Instant::now()));
        }
        Ok(client)
    }

    /// The games in the cache or chosen to be kept, with what is on hand for each.
    pub fn apps(&self, local: &[LocalGame]) -> Vec<AppView> {
        let Ok(book) = self.book.lock() else { return Vec::new() };
        let installed: HashSet<u32> = local.iter().map(|g| g.app).collect();
        let mut by_app: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for depot in book.depots.keys() {
            let owner = book.owners.get(depot).copied().unwrap_or(0);
            by_app.entry(owner).or_default().push(*depot);
        }
        // A game chosen to keep lists what it needs even before any of it is here: the
        // depots installed on this computer, or else the Windows ones Steam lists.
        for app in &book.kept {
            let depots = by_app.entry(*app).or_default();
            let installed: Vec<u32> = local.iter().filter(|g| g.app == *app).flat_map(|g| g.depots.keys().filter_map(|d| d.parse().ok())).collect();
            let listed: Vec<u32> = book
                .apps
                .get(app)
                .map(|i| i.depots.iter().filter(|(_, d)| d.size > 0 && (d.oslist.is_empty() || d.oslist.contains("windows"))).filter_map(|(id, _)| id.parse().ok()).collect())
                .unwrap_or_default();
            for depot in if installed.is_empty() { listed } else { installed } {
                if !depots.contains(&depot) {
                    depots.push(depot);
                }
            }
        }
        by_app
            .into_iter()
            .map(|(app, depots)| {
                let info = book.apps.get(&app).cloned().unwrap_or_default();
                let local_name = local.iter().find(|g| g.app == app).map(|g| g.name.clone());
                let mut views = Vec::new();
                let (mut cached, mut full, mut saved, mut last) = (0, 0, 0, 0);
                let mut stale = false;
                for depot in &depots {
                    let stats = book.depots.get(depot).cloned().unwrap_or_default();
                    let current = info.depots.get(&depot.to_string()).cloned().unwrap_or_default();
                    let up_to_date = current.manifest.is_empty() || stats.manifests.iter().any(|m| *m == current.manifest);
                    stale |= !up_to_date && stats.chunks > 0;
                    cached += stats.bytes;
                    full += current.size;
                    saved += stats.saved;
                    last = last.max(stats.last);
                    views.push(DepotView { depot: *depot, cached: stats.bytes, chunks: stats.chunks, manifests: stats.manifests.len(), current: current.manifest, up_to_date, size: current.size });
                }
                let state = if cached == 0 {
                    "empty"
                } else if stale {
                    "update"
                } else if full > 0 && cached * 10 < full * 9 {
                    "partial"
                } else {
                    "cached"
                };
                AppView {
                    app,
                    name: if !info.name.is_empty() { info.name.clone() } else { local_name.unwrap_or_else(|| if app == 0 { "Not matched to a game yet".into() } else { format!("App {app}") }) },
                    header: self.dir.join("art").join(format!("{app}.jpg")).is_file(),
                    cached,
                    full,
                    state,
                    updated: info.updated * 1000,
                    buildid: info.buildid.clone(),
                    last_served: last,
                    kept: book.kept.contains(&app),
                    installed_here: installed.contains(&app),
                    saved,
                    depots: views,
                    description: info.description,
                    developers: info.developers,
                    genres: info.genres,
                    released: info.released,
                }
            })
            .collect()
    }

    /// Who is downloading right now, by computer and game.
    pub fn downloading(&self) -> Vec<Downloading> {
        let now = now_millis();
        let events: Vec<Event> = self.activity.lock().map(|a| a.iter().filter(|e| now - e.at <= ACTIVITY_WINDOW_MS).cloned().collect()).unwrap_or_default();
        let book = self.book.lock();
        let owner = |depot: u32| book.as_ref().ok().and_then(|b| b.owners.get(&depot).copied()).unwrap_or(0);
        let name = |app: u32| book.as_ref().ok().and_then(|b| b.apps.get(&app).map(|a| a.name.clone())).filter(|n| !n.is_empty()).unwrap_or_else(|| format!("App {app}"));
        let mut groups: BTreeMap<(String, u32), (u64, u64, u64, u64)> = BTreeMap::new();
        for e in &events {
            let g = groups.entry((e.client.clone(), owner(e.depot))).or_insert((0, 0, 0, u64::MAX));
            g.0 += e.bytes;
            if e.hit {
                g.1 += e.bytes;
            }
            g.2 = g.2.max(e.at);
            g.3 = g.3.min(e.at);
        }
        groups
            .into_iter()
            .map(|((client, app), (bytes, hit_bytes, last, first))| {
                let secs = ((last.saturating_sub(first)) as f64 / 1000.0).max(1.0);
                Downloading { client, app, name: name(app), bytes, hit_percent: (hit_bytes * 100 / bytes.max(1)) as u32, mbps: ((bytes as f64 / secs / 1e6) * 10.0).round() / 10.0 }
            })
            .collect()
    }

    pub fn totals(&self) -> serde_json::Value {
        let Ok(book) = self.book.lock() else { return serde_json::json!({}) };
        let (mut bytes, mut chunks, mut hits, mut misses, mut saved) = (0, 0, 0, 0, 0);
        for stats in book.depots.values() {
            bytes += stats.bytes;
            chunks += stats.chunks;
            hits += stats.hits;
            misses += stats.misses;
            saved += stats.saved;
        }
        serde_json::json!({ "bytes": bytes, "chunks": chunks, "hits": hits, "misses": misses, "saved": saved, "free": fs4::available_space(&self.dir).unwrap_or(0) })
    }

    pub fn keep(&self, app: u32, keep: bool) {
        if let Ok(mut book) = self.book.lock() {
            book.kept.retain(|a| *a != app);
            if keep {
                book.kept.push(app);
            }
            self.save(&book);
        }
    }

    /// Deletes every chunk and manifest of a game, and forgets it was chosen.
    pub fn purge(&self, app: u32, local: &[LocalGame]) -> Result<u64, String> {
        let mut book = self.book.lock().map_err(|_| "The cache is unavailable".to_string())?;
        let mut depots: HashSet<u32> = book.owners.iter().filter(|(_, a)| **a == app).map(|(d, _)| *d).collect();
        if let Some(info) = book.apps.get(&app) {
            depots.extend(info.depots.keys().filter_map(|d| d.parse::<u32>().ok()));
        }
        if let Some(game) = local.iter().find(|g| g.app == app) {
            depots.extend(game.depots.keys().filter_map(|d| d.parse::<u32>().ok()));
        }
        let mut freed = 0;
        for depot in depots {
            if let Some(stats) = book.depots.remove(&depot) {
                freed += stats.bytes;
            }
            let _ = std::fs::remove_dir_all(self.dir.join("depots").join(depot.to_string()));
        }
        book.kept.retain(|a| *a != app);
        self.save(&book);
        Ok(freed)
    }

    /// Matches cached depots to games and refreshes game details that are old. Runs in
    /// the background; asks the internet about each game at most every few hours.
    pub fn refresh(&self, local: &[LocalGame]) {
        if self.refreshing.swap(true, Ordering::SeqCst) {
            return;
        }
        let Ok(http) = crate::wiki::client() else {
            self.refreshing.store(false, Ordering::SeqCst);
            return;
        };
        // Depots of games installed here are known without asking.
        if let Ok(mut book) = self.book.lock() {
            for game in local {
                for depot in game.depots.keys().filter_map(|d| d.parse::<u32>().ok()) {
                    book.owners.entry(depot).or_insert(game.app);
                }
            }
        }
        let (unmatched, wanted): (Vec<u32>, Vec<u32>) = match self.book.lock() {
            Ok(book) => {
                let unmatched = book.depots.keys().filter(|d| !book.owners.contains_key(d)).copied().collect();
                let mut apps: HashSet<u32> = book.owners.iter().filter(|(d, _)| book.depots.contains_key(d)).map(|(_, a)| *a).collect();
                apps.extend(book.kept.iter().copied());
                let now = now_millis();
                let wanted = apps.into_iter().filter(|a| *a != 0 && book.apps.get(a).is_none_or(|i| now.saturating_sub(i.fetched) > APP_INFO_AGE)).collect();
                (unmatched, wanted)
            }
            Err(_) => (Vec::new(), Vec::new()),
        };
        // A depot usually belongs to the game whose number it rounds down to.
        let mut asked: HashMap<u32, Option<AppInfo>> = HashMap::new();
        for depot in unmatched {
            let guess = depot / 10 * 10;
            let info = asked.entry(guess).or_insert_with(|| fetch_app(&http, guess)).clone();
            if let Some(info) = info.filter(|i| i.depots.contains_key(&depot.to_string())) {
                if let Ok(mut book) = self.book.lock() {
                    book.owners.insert(depot, guess);
                    book.apps.insert(guess, info);
                }
            }
        }
        for app in wanted {
            if asked.contains_key(&app) {
                continue;
            }
            if let Some(info) = fetch_app(&http, app) {
                if let Ok(mut book) = self.book.lock() {
                    book.apps.insert(app, info);
                }
            }
        }
        // Art for every game that has none yet.
        let apps: Vec<(u32, String)> = self.book.lock().map(|b| b.apps.iter().map(|(a, i)| (*a, i.header.clone())).collect()).unwrap_or_default();
        let art = self.dir.join("art");
        let _ = std::fs::create_dir_all(&art);
        for (app, header) in apps {
            let file = art.join(format!("{app}.jpg"));
            if file.is_file() || header.is_empty() {
                continue;
            }
            if let Some(bytes) = crate::lookup::fetch_picture(&http, &header) {
                let _ = std::fs::write(file, bytes);
            }
        }
        if let Ok(book) = self.book.lock() {
            self.save(&book);
        }
        self.refreshing.store(false, Ordering::SeqCst);
    }

    pub fn art(&self, app: u32) -> Option<PathBuf> {
        let file = self.dir.join("art").join(format!("{app}.jpg"));
        file.is_file().then_some(file)
    }
}

fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => !(v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_unspecified() || v4.is_broadcast()),
        IpAddr::V6(v6) => !(v6.is_loopback() || v6.is_unspecified() || (v6.segments()[0] & 0xfe00) == 0xfc00),
    }
}

/// A host's public address, asked of a public resolver over HTTPS.
async fn doh_lookup(host: &str) -> Result<IpAddr, String> {
    let url = crate::wiki::with_query(DOH, &[("name", host), ("type", "A")]);
    let answer: serde_json::Value = reqwest::Client::new()
        .get(url)
        .header(header::ACCEPT, "application/dns-json")
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| "Valve's address could not be found".to_string())?
        .json()
        .await
        .map_err(|_| "Valve's address could not be found".to_string())?;
    answer["Answer"]
        .as_array()
        .and_then(|records| records.iter().filter(|r| r["type"] == 1).find_map(|r| r["data"].as_str()?.parse().ok()))
        .ok_or_else(|| "Valve's address could not be found".to_string())
}

/// What Steam says about a game: its depots and current build (steamcmd.net) and its
/// name, description and art (the store).
fn fetch_app(http: &reqwest::blocking::Client, app: u32) -> Option<AppInfo> {
    let info: serde_json::Value = http.get(format!("{STEAMCMD_INFO}/{app}")).send().ok()?.error_for_status().ok()?.json().ok()?;
    let data = &info["data"][app.to_string()];
    if data.is_null() {
        return None;
    }
    let mut out = AppInfo { name: data["common"]["name"].as_str().unwrap_or("").to_string(), fetched: now_millis(), ..AppInfo::default() };
    let depots = &data["depots"];
    out.buildid = depots["branches"]["public"]["buildid"].as_str().unwrap_or("").to_string();
    out.updated = depots["branches"]["public"]["timeupdated"]
        .as_str()
        .or(depots["branches"]["public"]["timebuildupdated"].as_str())
        .and_then(|t| t.parse().ok())
        .unwrap_or(0);
    if let Some(map) = depots.as_object() {
        for (id, depot) in map {
            if !id.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let public = &depot["manifests"]["public"];
            // Newer answers give an object per branch; older ones the id alone.
            let (manifest, size) = match public {
                serde_json::Value::Object(m) => (m.get("gid").and_then(|g| g.as_str()).unwrap_or("").to_string(), m.get("size").and_then(|s| s.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0)),
                serde_json::Value::String(gid) => (gid.clone(), depot["maxsize"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0)),
                _ => continue,
            };
            out.depots.insert(id.clone(), DepotInfo { manifest, size, oslist: depot["config"]["oslist"].as_str().unwrap_or("").to_string() });
        }
    }
    if let Ok(store) = http.get(crate::wiki::with_query(STORE_DETAILS, &[("appids", &app.to_string()), ("l", "english")])).send().and_then(|r| r.json::<serde_json::Value>()) {
        let d = &store[app.to_string()]["data"];
        if !d.is_null() {
            if out.name.is_empty() {
                out.name = d["name"].as_str().unwrap_or("").to_string();
            }
            out.description = crate::lookup::plain_text(d["short_description"].as_str().unwrap_or(""));
            out.developers = d["developers"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
            out.genres = d["genres"].as_array().map(|a| a.iter().filter_map(|g| g["description"].as_str().map(str::to_string)).collect()).unwrap_or_default();
            out.released = d["release_date"]["date"].as_str().unwrap_or("").to_string();
            out.header = d["header_image"].as_str().unwrap_or("").to_string();
        }
    }
    Some(out)
}

#[derive(Serialize)]
pub struct Found {
    pub app: u32,
    pub name: String,
    pub image: String,
}

/// Games on the Steam store matching a search, to choose one to keep.
pub fn search(query: &str) -> Result<Vec<Found>, String> {
    let http = crate::wiki::client()?;
    let answer: serde_json::Value = http
        .get(crate::wiki::with_query(STORE_SEARCH, &[("term", query), ("l", "english"), ("cc", "us")]))
        .send()
        .and_then(|r| r.json())
        .map_err(|_| "The Steam store did not answer".to_string())?;
    Ok(answer["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .take(MAX_SEARCH)
                .filter_map(|i| Some(Found { app: i["id"].as_u64()? as u32, name: i["name"].as_str()?.to_string(), image: i["tiny_image"].as_str().unwrap_or("").to_string() }))
                .collect()
        })
        .unwrap_or_default())
}

/// Where `lancache.steamcontent.com` points from this computer, and whether that is a
/// cache in the home rather than Valve.
pub async fn steam_points_here() -> (Vec<String>, bool) {
    let addrs: Vec<IpAddr> = tokio::net::lookup_host((LANCACHE_NAME, 80)).await.map(|a| a.map(|s| s.ip()).collect()).unwrap_or_default();
    let home = !addrs.is_empty() && addrs.iter().all(|ip| !is_public(*ip));
    (addrs.iter().map(IpAddr::to_string).collect(), home)
}

// ---------------------------------------------------------------------------------------
// Serving Steam
// ---------------------------------------------------------------------------------------

fn with_mark(mut res: Response) -> Response {
    res.headers_mut().insert(PROCESSED_BY, HeaderValue::from_static("Castle"));
    res
}

fn plain(status: StatusCode) -> Response {
    with_mark(status.into_response())
}

/// Every request Steam sends to the cache.
pub async fn serve(State(cache): State<Arc<SteamCache>>, ConnectInfo(peer): ConnectInfo<SocketAddr>, req: Request) -> Response {
    if req.method() != Method::GET && req.method() != Method::HEAD {
        return plain(StatusCode::METHOD_NOT_ALLOWED);
    }
    let path = req.uri().path().to_string();
    let Some(what) = wanted(&path) else { return plain(StatusCode::NOT_FOUND) };
    if what == Wanted::Heartbeat {
        return plain(StatusCode::NO_CONTENT);
    }
    let host = req.headers().get(header::HOST).and_then(|v| v.to_str().ok()).map(|h| h.rsplit_once(':').map_or(h, |(name, _)| name).to_string());
    // Steam names Valve's host; a bare address (or anything else) goes to the cache name.
    let host = host.filter(|h| allowed_upstream(h)).unwrap_or_else(|| LANCACHE_NAME.to_string());

    let (depot, kept_file, kind) = match &what {
        Wanted::Chunk { depot, sha } => (*depot, Some(cache.chunk_path(*depot, sha)), "chunk".to_string()),
        Wanted::Manifest { depot, gid } => (*depot, Some(cache.manifest_path(*depot, *gid)), gid.to_string()),
        Wanted::Other { depot } => (*depot, None, String::new()),
        Wanted::Heartbeat => unreachable!(),
    };

    if let Some(file) = &kept_file {
        if let Ok(bytes) = tokio::fs::read(file).await {
            cache.note(peer.ip(), depot, true, bytes.len() as u64, None);
            let mut res = Response::new(Body::from(bytes));
            res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
            return with_mark(res);
        }
    }

    let client = match cache.upstream(&host).await {
        Ok(client) => client,
        Err(_) => return plain(StatusCode::BAD_GATEWAY),
    };
    let target = format!("http://{host}{}", req.uri().path_and_query().map(|p| p.as_str()).unwrap_or(&path));
    let mut ask = client.get(target);
    if let Some(agent) = req.headers().get(header::USER_AGENT) {
        ask = ask.header(header::USER_AGENT, agent.clone());
    }
    let answer = match ask.send().await {
        Ok(answer) => answer,
        Err(_) => return plain(StatusCode::BAD_GATEWAY),
    };
    let status = StatusCode::from_u16(answer.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = answer.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| HeaderValue::from_bytes(v.as_bytes()).ok());
    let bytes = match answer.bytes().await {
        Ok(bytes) if bytes.len() <= MAX_CHUNK_BYTES => bytes,
        _ => return plain(StatusCode::BAD_GATEWAY),
    };
    let mut newly_kept = None;
    if status == StatusCode::OK && !bytes.is_empty() {
        if let Some(file) = &kept_file {
            if let Some(dir) = file.parent() {
                let _ = tokio::fs::create_dir_all(dir).await;
            }
            let tmp = file.with_extension(format!("{}.tmp", crate::random_hex(4)));
            if tokio::fs::write(&tmp, &bytes).await.is_ok() && tokio::fs::rename(&tmp, file).await.is_ok() {
                newly_kept = Some(kind.as_str());
            } else {
                let _ = tokio::fs::remove_file(&tmp).await;
            }
        }
    }
    // Only what Valve actually sent counts; a refusal is passed on and not recorded.
    if status.is_success() {
        cache.note(peer.ip(), depot, false, bytes.len() as u64, newly_kept);
    }
    let mut res = Response::new(Body::from(bytes));
    *res.status_mut() = status;
    if let Some(content_type) = content_type {
        res.headers_mut().insert(header::CONTENT_TYPE, content_type);
    }
    with_mark(res)
}

pub fn router(cache: Arc<SteamCache>) -> axum::Router {
    axum::Router::new().fallback(serve).with_state(cache)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_requests_are_recognised() {
        assert_eq!(wanted("/lancache-heartbeat"), Some(Wanted::Heartbeat));
        assert_eq!(
            wanted("/depot/214831/chunk/0123456789ABCDEF0123456789abcdef01234567"),
            Some(Wanted::Chunk { depot: 214831, sha: "0123456789abcdef0123456789abcdef01234567".into() })
        );
        assert_eq!(wanted("/depot/214831/manifest/2364079169036956309/5/123456"), Some(Wanted::Manifest { depot: 214831, gid: 2364079169036956309 }));
        assert_eq!(wanted("/depot/214831/patch/abc/def"), Some(Wanted::Other { depot: 214831 }));
        assert_eq!(wanted("/depot/214831/chunk/../../etc"), Some(Wanted::Other { depot: 214831 }));
        assert_eq!(wanted("/admin"), None);
        assert_eq!(wanted("/depot/notanumber/chunk/x"), None);
    }

    #[test]
    fn only_valves_content_hosts_are_fetched_from() {
        assert!(allowed_upstream("lancache.steamcontent.com"));
        assert!(allowed_upstream("cache1-fra1.steamcontent.com"));
        assert!(allowed_upstream("Content1.SteamPowered.com"));
        for bad in ["example.com", "steamcontent.com.evil.net", "evilsteamcontent.com", "127.0.0.1", ""] {
            assert!(!allowed_upstream(bad), "{bad}");
        }
    }

    #[test]
    fn steams_files_are_read() {
        let vdf = parse_vdf(
            "\"AppState\"\n{\n\t\"appid\"\t\t\"214830\"\n\t\"name\"\t\t\"Half \\\"Minute\\\" Hero\"\n\t\"InstalledDepots\"\n\t{\n\t\t\"214831\"\n\t\t{\n\t\t\t\"manifest\"\t\t\"2364079169036956309\"\n\t\t}\n\t}\n}\n",
        );
        let state = vdf.get("AppState").unwrap();
        assert_eq!(state.text("appid"), "214830");
        assert_eq!(state.text("name"), "Half \"Minute\" Hero");
        assert_eq!(state.get("InstalledDepots").unwrap().get("214831").unwrap().text("manifest"), "2364079169036956309");
    }

    #[test]
    fn private_addresses_are_not_public() {
        for ip in ["10.0.0.5", "192.168.1.2", "172.16.4.4", "127.0.0.1", "169.254.1.1"] {
            assert!(!is_public(ip.parse().unwrap()), "{ip}");
        }
        assert!(is_public("23.45.67.89".parse().unwrap()));
    }

    #[test]
    fn games_are_built_from_kept_depots_and_purged() {
        let dir = std::env::temp_dir().join(format!("castled-steam-{}", crate::random_hex(6)));
        let cache = SteamCache::open(dir.clone());
        let ip: IpAddr = "192.168.1.20".parse().unwrap();
        // Two chunks and a manifest of depot 214831 arrive, then one chunk is asked for again.
        std::fs::create_dir_all(cache.chunk_path(214831, "ab00000000000000000000000000000000000000").parent().unwrap()).unwrap();
        std::fs::write(cache.chunk_path(214831, "ab00000000000000000000000000000000000000"), vec![1u8; 1000]).unwrap();
        cache.note(ip, 214831, false, 1000, Some("chunk"));
        cache.note(ip, 214831, false, 500, Some("chunk"));
        cache.note(ip, 214831, false, 20, Some("2364079169036956309"));
        cache.note("192.168.1.30".parse().unwrap(), 214831, true, 1000, None);
        {
            let mut book = cache.book.lock().unwrap();
            book.owners.insert(214831, 214830);
            book.apps.insert(214830, AppInfo {
                name: "Half Minute Hero".into(),
                depots: [("214831".to_string(), DepotInfo { manifest: "2364079169036956309".into(), size: 1500, oslist: "windows".into() })].into(),
                ..AppInfo::default()
            });
        }
        let apps = cache.apps(&[]);
        assert_eq!(apps.len(), 1);
        let game = &apps[0];
        assert_eq!((game.name.as_str(), game.cached, game.full, game.state, game.saved), ("Half Minute Hero", 1500, 1500, "cached", 1000));
        assert_eq!(cache.downloading().len(), 2, "two PCs downloading");
        // A new build on Steam makes it an update waiting.
        cache.book.lock().unwrap().apps.get_mut(&214830).unwrap().depots.get_mut("214831").unwrap().manifest = "999".into();
        assert_eq!(cache.apps(&[])[0].state, "update");
        assert_eq!(cache.purge(214830, &[]).unwrap(), 1500);
        assert!(cache.apps(&[]).is_empty());
        assert!(!dir.join("depots").join("214831").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_chunk_is_fetched_once_then_served_from_the_cache() {
        use axum::extract::connect_info::MockConnectInfo;
        use tower::ServiceExt;
        let dir = std::env::temp_dir().join(format!("castled-steam-{}", crate::random_hex(6)));
        let cache = Arc::new(SteamCache::open(dir.clone()));
        let sha = "cd00000000000000000000000000000000000000";
        // Put the chunk in place as if fetched, so no request leaves the computer.
        let file = cache.chunk_path(7, sha);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"chunk bytes").unwrap();
        let app = router(cache.clone()).layer(MockConnectInfo(SocketAddr::from(([192, 168, 1, 9], 5000))));
        let req = Request::builder().uri(format!("/depot/7/chunk/{sha}")).header(header::HOST, LANCACHE_NAME).body(Body::empty()).unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[PROCESSED_BY], "Castle");
        assert_eq!(&axum::body::to_bytes(res.into_body(), 100).await.unwrap()[..], b"chunk bytes");
        let beat = app.clone().oneshot(Request::builder().uri("/lancache-heartbeat").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(beat.status(), StatusCode::NO_CONTENT);
        let other = app.oneshot(Request::builder().uri("/etc/passwd").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(other.status(), StatusCode::NOT_FOUND);
        assert_eq!(cache.book.lock().unwrap().depots[&7].hits, 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
