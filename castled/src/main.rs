//! castled -- the Castle box daemon.
//!
//! One small HTTP API over one directory (the vault). Every Castle client -- web, desktop,
//! Android, iOS -- talks to this and nothing else, so no client needs Samba or any other
//! file-sharing protocol. Samba can still export the same directory for people who want a
//! mapped drive; that is an extra, not the way the apps work.
//!
//! Safety rules this file keeps:
//!   * Listens on this machine only unless told otherwise (--bind).
//!   * Every call except /api/v1/hello needs the box token (Authorization: Bearer ...).
//!   * A path can never leave the vault: names are validated and symlinks are refused.
//!   * Uploads stream to a temp file and are renamed into place, so a dropped connection
//!     never leaves half a file where a whole one is expected.

mod ai;
mod arcade;
mod clones;
mod dns;
mod drives;
mod host;
mod lookup;
mod media_tags;
mod network;
mod saves;
mod security;
mod steam;
mod stream;
mod system;
mod theater;
mod wiki;

use std::{
    collections::BTreeSet,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Path as UrlPath, Query, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tokio::io::AsyncWriteExt;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
};

const DEFAULT_BIND: &str = "127.0.0.1:8787";
const DEFAULT_DATA_DIR: &str = "./castle-data";
/// Everything the daemon keeps for itself lives here, hidden from every listing.
const META_DIR: &str = ".castle";
const TRASH_PREFIX: &str = "trash:";
const MAX_NAME_LEN: usize = 255;
/// A tree walk stops here so one huge vault cannot stall the box.
const MAX_TREE_NODES: usize = 50_000;
/// Origins the app runs on when the daemon is not the one serving it: the Vite dev server
/// and the Tauri webviews (desktop, Android, iOS).
const APP_ORIGINS: [&str; 5] = [
    "http://localhost:1420",
    "http://127.0.0.1:1420",
    "tauri://localhost",
    "http://tauri.localhost",
    "https://tauri.localhost",
];
const WINDOWS_RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

struct AppState {
    root: PathBuf,
    /// Replaced when the token is rotated from the Security screen.
    token: RwLock<String>,
    security: security::Security,
    stars: Mutex<BTreeSet<String>>,
    /// Set when the daemon was started with --clonewars, or when Clones was turned on later.
    clones: RwLock<Option<Arc<clones::Clones>>>,
    /// Set when the daemon was started with --arcade, or when Arcade was turned on later.
    arcade: RwLock<Option<Arc<arcade::Arcade>>>,
    network: network::Network,
    system: system::System,
    saves: saves::Saves,
    /// What Wikipedia said about each game, so a game is looked up once.
    game_notes: wiki::Shelf,
    /// The rest of this computer, for Storage's "../". None when it is not allowed: the box
    /// can be reached from other machines and was not started with --host-files.
    host: Option<host::Host>,
    /// The computer's drives, and when each was plugged in.
    drives: drives::Drives,
    /// Films, shows, music, photos and videos from folders on this computer.
    theater: theater::Theater,
    /// Steam games kept for every PC in the home.
    steam: Arc<steam::SteamCache>,
    /// Where the box's DNS server listens, once it does.
    dns: Mutex<Option<SocketAddr>>,
    /// DOGS AI's server, which the AI section shows.
    ai: ai::Ai,
    /// The content policy sent with every answer (see harden).
    policy: HeaderValue,
    /// What the daemon was started with, for the app's Settings screen.
    listen: String,
    loopback: bool,
    /// Serialises writes to settings.json.
    settings: Mutex<()>,
}

type Shared = Arc<AppState>;

// ---------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------

#[derive(Debug)]
struct ApiError(StatusCode, String);

impl ApiError {
    fn bad(msg: &str) -> Self {
        Self(StatusCode::BAD_REQUEST, msg.into())
    }
    fn not_found() -> Self {
        Self(StatusCode::NOT_FOUND, "not found".into())
    }
    fn conflict(msg: &str) -> Self {
        Self(StatusCode::CONFLICT, msg.into())
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => Self::not_found(),
            std::io::ErrorKind::PermissionDenied => {
                Self(StatusCode::FORBIDDEN, "This computer will not let Castle open that.".into())
            }
            std::io::ErrorKind::AlreadyExists => Self::conflict("that name is already taken"),
            std::io::ErrorKind::DirectoryNotEmpty => Self::conflict("that folder is not empty"),
            // The detail goes to the box's own log, not to whoever is calling.
            _ => {
                eprintln!("[castled] io error: {e}");
                Self(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into())
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

// ---------------------------------------------------------------------------------------
// Paths: the only way a request turns into a location on disk
// ---------------------------------------------------------------------------------------

/// A file or folder name the vault accepts. Deliberately strict: it has to be safe on
/// Linux (the box) and on Windows (where this is developed and tested).
fn valid_name(name: &str) -> bool {
    if name.is_empty() || name.len() > MAX_NAME_LEN || name.starts_with('.') {
        return false;
    }
    if name.ends_with(' ') || name.ends_with('.') {
        return false;
    }
    if name.chars().any(|c| c.is_control() || r#"/\:*?"<>|"#.contains(c)) {
        return false;
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    !WINDOWS_RESERVED.contains(&stem.as_str())
}

/// Splits a vault id ("Photos/2026/a.jpg", or "" for the top) into validated names.
fn split_id(id: &str) -> ApiResult<Vec<&str>> {
    if id.is_empty() {
        return Ok(Vec::new());
    }
    let parts: Vec<&str> = id.split('/').collect();
    if parts.iter().all(|p| valid_name(p)) {
        Ok(parts)
    } else {
        Err(ApiError::bad("invalid path"))
    }
}

/// Turns a vault id into a real path, refusing any symlink along the way so a link planted
/// in the vault cannot point a request at the rest of the disk.
fn resolve(root: &Path, id: &str) -> ApiResult<PathBuf> {
    let mut path = root.to_path_buf();
    for part in split_id(id)? {
        path.push(part);
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            if meta.file_type().is_symlink() {
                return Err(ApiError::bad("invalid path"));
            }
        }
    }
    Ok(path)
}

fn join_id(parent: &str, name: &str) -> String {
    if parent.is_empty() { name.to_string() } else { format!("{parent}/{name}") }
}

fn parent_of(id: &str) -> &str {
    id.rsplit_once('/').map(|(p, _)| p).unwrap_or("")
}

fn name_of(id: &str) -> &str {
    id.rsplit_once('/').map(|(_, n)| n).unwrap_or(id)
}

/// "report.pdf" -> "report (2).pdf" when the first name is taken. Nothing is ever
/// overwritten by an upload, a new folder, or a restore.
fn free_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    (2..)
        .map(|n| format!("{stem} ({n}){ext}"))
        .find(|candidate| !dir.join(candidate).exists())
        .expect("an unused name exists")
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("the operating system provides randomness");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------------------
// Nodes: what the apps see
// ---------------------------------------------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Node {
    id: String,
    name: String,
    kind: &'static str,
    parent_id: String,
    mime: String,
    size: u64,
    modified: u64,
    starred: bool,
    trashed: bool,
    shared: bool,
}

fn millis(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn node_from(st: &AppState, id: &str, meta: &std::fs::Metadata) -> Node {
    let is_dir = meta.is_dir();
    Node {
        id: id.to_string(),
        name: name_of(id).to_string(),
        kind: if is_dir { "folder" } else { "file" },
        parent_id: parent_of(id).to_string(),
        mime: if is_dir {
            String::new()
        } else {
            mime_guess::from_path(name_of(id)).first_or_octet_stream().to_string()
        },
        size: if is_dir { 0 } else { meta.len() },
        modified: meta.modified().map(millis).unwrap_or(0),
        starred: st.stars.lock().unwrap().contains(id),
        trashed: false,
        shared: false,
    }
}

fn stat_node(st: &AppState, id: &str) -> ApiResult<Node> {
    let meta = std::fs::symlink_metadata(resolve(&st.root, id)?)?;
    Ok(node_from(st, id, &meta))
}

/// Children of one folder. Hidden names (the daemon's own folder included) and symlinks
/// are skipped, never listed.
fn list_dir(st: &AppState, id: &str) -> ApiResult<Vec<Node>> {
    let dir = resolve(&st.root, id)?;
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else { continue };
        if !valid_name(&name) {
            continue;
        }
        let meta = entry.metadata()?;
        if meta.file_type().is_symlink() {
            continue;
        }
        out.push(node_from(st, &join_id(id, &name), &meta));
    }
    Ok(out)
}

fn walk(st: &AppState, id: &str, out: &mut Vec<Node>) -> ApiResult<()> {
    for node in list_dir(st, id)? {
        if out.len() >= MAX_TREE_NODES {
            return Ok(());
        }
        let is_folder = node.kind == "folder";
        let child = node.id.clone();
        out.push(node);
        if is_folder {
            walk(st, &child, out)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Trash: a delete you can take back
// ---------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct TrashRecord {
    /// Vault id the item had before it was trashed.
    path: String,
    deleted: u64,
}

fn trash_dir(st: &AppState) -> PathBuf {
    st.root.join(META_DIR).join("trash")
}

/// A trash key is the random hex this daemon generated; anything else is refused before it
/// touches a path.
fn trash_key(id: &str) -> ApiResult<&str> {
    let key = id.strip_prefix(TRASH_PREFIX).ok_or_else(|| ApiError::bad("not a trash item"))?;
    if key.len() == 32 && key.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(key)
    } else {
        Err(ApiError::bad("invalid trash item"))
    }
}

fn read_trash_record(st: &AppState, key: &str) -> ApiResult<TrashRecord> {
    let raw = std::fs::read(trash_dir(st).join(format!("{key}.json")))?;
    serde_json::from_slice(&raw).map_err(|_| ApiError::bad("damaged trash record"))
}

fn list_trash(st: &AppState) -> ApiResult<Vec<Node>> {
    let dir = trash_dir(st);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.metadata()?.is_dir() {
            continue;
        }
        let Some(key) = entry.file_name().to_str().map(str::to_string) else { continue };
        let Ok(record) = read_trash_record(st, &key) else { continue };
        let Ok(meta) = std::fs::symlink_metadata(entry.path().join(name_of(&record.path))) else {
            continue;
        };
        let mut node = node_from(st, &record.path, &meta);
        node.id = format!("{TRASH_PREFIX}{key}");
        node.trashed = true;
        node.starred = false;
        node.modified = record.deleted;
        out.push(node);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// Stars
// ---------------------------------------------------------------------------------------

fn stars_file(root: &Path) -> PathBuf {
    root.join(META_DIR).join("stars.json")
}

fn save_stars(st: &AppState, stars: &BTreeSet<String>) -> ApiResult<()> {
    let tmp = stars_file(&st.root).with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(stars).unwrap_or_default())?;
    std::fs::rename(tmp, stars_file(&st.root))?;
    Ok(())
}

/// Keeps stars attached when an item (and everything under it) moves, and drops them when
/// it goes away (`to` is None).
fn retarget_stars(st: &AppState, from: &str, to: Option<&str>) -> ApiResult<()> {
    let mut stars = st.stars.lock().unwrap();
    let under = format!("{from}/");
    let hit: Vec<String> =
        stars.iter().filter(|s| *s == from || s.starts_with(&under)).cloned().collect();
    if hit.is_empty() {
        return Ok(());
    }
    for old in hit {
        stars.remove(&old);
        if let Some(to) = to {
            stars.insert(format!("{to}{}", &old[from.len()..]));
        }
    }
    save_stars(st, &stars)
}

// ---------------------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct PathQuery {
    #[serde(default)]
    path: String,
}

#[derive(Deserialize)]
struct IdBody {
    id: String,
}

#[derive(Deserialize)]
struct MkdirBody {
    #[serde(default)]
    parent: String,
    name: String,
}

#[derive(Deserialize)]
struct RenameBody {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct MoveBody {
    id: String,
    #[serde(default)]
    target: String,
}

#[derive(Deserialize)]
struct UploadQuery {
    #[serde(default)]
    parent: String,
    name: String,
}

async fn hello() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "castle",
        "version": env!("CARGO_PKG_VERSION"),
        "auth": "token",
    }))
}

async fn list(State(st): State<Shared>, Query(q): Query<PathQuery>) -> ApiResult<Json<Vec<Node>>> {
    Ok(Json(list_dir(&st, &q.path)?))
}

/// Every item in the vault plus the trash, for search and the Recent / Starred / Trash views.
async fn tree(State(st): State<Shared>) -> ApiResult<Json<Vec<Node>>> {
    let nodes = tokio::task::spawn_blocking(move || -> ApiResult<Vec<Node>> {
        let mut out = Vec::new();
        walk(&st, "", &mut out)?;
        out.extend(list_trash(&st)?);
        Ok(out)
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))??;
    Ok(Json(nodes))
}

async fn stat(State(st): State<Shared>, Query(q): Query<PathQuery>) -> ApiResult<Json<Node>> {
    if q.path.is_empty() {
        return Err(ApiError::bad("invalid path"));
    }
    Ok(Json(stat_node(&st, &q.path)?))
}

async fn mkdir(State(st): State<Shared>, Json(b): Json<MkdirBody>) -> ApiResult<Json<Node>> {
    if !valid_name(&b.name) {
        return Err(ApiError::bad("invalid name"));
    }
    let dir = resolve(&st.root, &b.parent)?;
    if !dir.is_dir() {
        return Err(ApiError::not_found());
    }
    let name = free_name(&dir, &b.name);
    std::fs::create_dir(dir.join(&name))?;
    Ok(Json(stat_node(&st, &join_id(&b.parent, &name))?))
}

/// Shared by rename and move: both are "this item now lives at that id".
fn relocate(st: &AppState, from: &str, to: &str) -> ApiResult<Node> {
    if from.is_empty() {
        return Err(ApiError::bad("invalid path"));
    }
    if to == from {
        return stat_node(st, from);
    }
    if to.starts_with(&format!("{from}/")) {
        return Err(ApiError::bad("a folder cannot be moved into itself"));
    }
    let src = resolve(&st.root, from)?;
    let dst = resolve(&st.root, to)?;
    std::fs::symlink_metadata(&src)?;
    if dst.exists() {
        return Err(ApiError::conflict("that name is already taken"));
    }
    std::fs::rename(src, dst)?;
    retarget_stars(st, from, Some(to))?;
    stat_node(st, to)
}

async fn rename(State(st): State<Shared>, Json(b): Json<RenameBody>) -> ApiResult<Json<Node>> {
    if !valid_name(&b.name) {
        return Err(ApiError::bad("invalid name"));
    }
    Ok(Json(relocate(&st, &b.id, &join_id(parent_of(&b.id), &b.name))?))
}

async fn move_item(State(st): State<Shared>, Json(b): Json<MoveBody>) -> ApiResult<Json<Node>> {
    if !resolve(&st.root, &b.target)?.is_dir() {
        return Err(ApiError::not_found());
    }
    Ok(Json(relocate(&st, &b.id, &join_id(&b.target, name_of(&b.id)))?))
}

async fn star(State(st): State<Shared>, Json(b): Json<IdBody>) -> ApiResult<Json<Node>> {
    if b.id.is_empty() {
        return Err(ApiError::bad("invalid path"));
    }
    std::fs::symlink_metadata(resolve(&st.root, &b.id)?)?;
    {
        let mut stars = st.stars.lock().unwrap();
        if !stars.remove(&b.id) {
            stars.insert(b.id.clone());
        }
        save_stars(&st, &stars)?;
    }
    Ok(Json(stat_node(&st, &b.id)?))
}

async fn trash(State(st): State<Shared>, Json(b): Json<IdBody>) -> ApiResult<StatusCode> {
    if b.id.is_empty() {
        return Err(ApiError::bad("invalid path"));
    }
    let src = resolve(&st.root, &b.id)?;
    std::fs::symlink_metadata(&src)?;
    let key = random_hex(16);
    let slot = trash_dir(&st).join(&key);
    std::fs::create_dir_all(&slot)?;
    let record = TrashRecord { path: b.id.clone(), deleted: millis(SystemTime::now()) };
    std::fs::write(
        trash_dir(&st).join(format!("{key}.json")),
        serde_json::to_vec(&record).unwrap_or_default(),
    )?;
    std::fs::rename(src, slot.join(name_of(&b.id)))?;
    retarget_stars(&st, &b.id, None)?;
    Ok(StatusCode::NO_CONTENT)
}

fn forget_trash(st: &AppState, key: &str) {
    let _ = std::fs::remove_dir_all(trash_dir(st).join(key));
    let _ = std::fs::remove_file(trash_dir(st).join(format!("{key}.json")));
}

/// Puts a trashed item back where it was. If that folder is gone it returns to the top of
/// the vault, and if the name has been reused it comes back as "name (2)".
async fn restore(State(st): State<Shared>, Json(b): Json<IdBody>) -> ApiResult<Json<Node>> {
    let key = trash_key(&b.id)?;
    let record = read_trash_record(&st, key)?;
    let name = name_of(&record.path);
    let mut parent = parent_of(&record.path).to_string();
    if !resolve(&st.root, &parent)?.is_dir() {
        parent.clear();
    }
    let dir = resolve(&st.root, &parent)?;
    let final_name = free_name(&dir, name);
    std::fs::rename(trash_dir(&st).join(key).join(name), dir.join(&final_name))?;
    forget_trash(&st, key);
    Ok(Json(stat_node(&st, &join_id(&parent, &final_name))?))
}

/// Permanent delete. This removes the file from the vault; it is an ordinary filesystem
/// delete, not a guarantee the bytes are unrecoverable from the disk.
async fn delete(State(st): State<Shared>, Json(b): Json<IdBody>) -> ApiResult<StatusCode> {
    if b.id.starts_with(TRASH_PREFIX) {
        let key = trash_key(&b.id)?;
        read_trash_record(&st, key)?;
        forget_trash(&st, key);
        return Ok(StatusCode::NO_CONTENT);
    }
    if b.id.is_empty() {
        return Err(ApiError::bad("invalid path"));
    }
    let path = resolve(&st.root, &b.id)?;
    if std::fs::symlink_metadata(&path)?.is_dir() {
        std::fs::remove_dir_all(&path)?;
    } else {
        std::fs::remove_file(&path)?;
    }
    retarget_stars(&st, &b.id, None)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Streams a request body into `tmp`, all of it on disk before this returns. A dropped
/// connection removes what had arrived.
async fn receive(body: Body, tmp: &Path) -> ApiResult<()> {
    let written: ApiResult<()> = async {
        let mut file = tokio::fs::File::create(tmp).await?;
        let mut stream = body.into_data_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| ApiError::bad("upload interrupted"))?;
            file.write_all(&chunk).await?;
        }
        file.sync_all().await?;
        Ok(())
    }
    .await;
    if written.is_err() {
        let _ = tokio::fs::remove_file(tmp).await;
    }
    written
}

async fn upload(
    State(st): State<Shared>,
    Query(q): Query<UploadQuery>,
    body: Body,
) -> ApiResult<Json<Node>> {
    if !valid_name(&q.name) {
        return Err(ApiError::bad("invalid name"));
    }
    let dir = resolve(&st.root, &q.parent)?;
    if !dir.is_dir() {
        return Err(ApiError::not_found());
    }
    let tmp_dir = st.root.join(META_DIR).join("tmp");
    tokio::fs::create_dir_all(&tmp_dir).await?;
    let tmp = tmp_dir.join(random_hex(16));
    receive(body, &tmp).await?;

    let name = free_name(&dir, &q.name);
    if let Err(e) = tokio::fs::rename(&tmp, dir.join(&name)).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e.into());
    }
    Ok(Json(stat_node(&st, &join_id(&q.parent, &name))?))
}

async fn file(State(st): State<Shared>, Query(q): Query<PathQuery>, headers: HeaderMap) -> ApiResult<Response> {
    if q.path.is_empty() {
        return Err(ApiError::bad("invalid path"));
    }
    let path = resolve(&st.root, &q.path)?;
    if !std::fs::symlink_metadata(&path)?.is_file() {
        return Err(ApiError::not_found());
    }
    let mime = mime_guess::from_path(&path).first_or_octet_stream().to_string();
    Ok(stream::file(&path, headers.get(header::RANGE), &mime, stream::Offer::Download).await?)
}

async fn stats(State(st): State<Shared>) -> ApiResult<Json<serde_json::Value>> {
    let root = st.root.clone();
    let used = tokio::task::spawn_blocking(move || -> ApiResult<u64> {
        let mut out = Vec::new();
        walk(&st, "", &mut out)?;
        Ok(out.iter().map(|n| n.size).sum())
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))??;
    // "quota" is what the vault holds now plus what the disk still has free.
    let free = fs4::available_space(&root).unwrap_or(0);
    Ok(Json(serde_json::json!({ "used": used, "quota": used + free })))
}

// ---------------------------------------------------------------------------------------
// This computer: drives and folders beyond the vault (see host.rs and drives.rs)
// ---------------------------------------------------------------------------------------

const HOST_OFF: &str = "Opening the rest of this computer is off, because the box can be reached from other machines. Start it with --host-files to allow it.";
/// How often the box looks for a drive being plugged in or taken out.
const DRIVE_WATCH: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Deserialize)]
struct HostPathBody {
    path: String,
}

#[derive(Deserialize)]
struct HostMkdirBody {
    parent: String,
    name: String,
}

#[derive(Deserialize)]
struct HostRenameBody {
    path: String,
    name: String,
}

#[derive(Deserialize)]
struct HostMoveBody {
    path: String,
    target: String,
}

#[derive(Deserialize)]
struct SpeedBody {
    path: String,
    /// False for the test that only reads files already on the drive.
    #[serde(default = "yes")]
    write: bool,
}

fn yes() -> bool {
    true
}

/// Runs one action on this computer's files, off the async threads, if that is allowed.
async fn host_call<T, F>(st: Shared, action: F) -> ApiResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState, &host::Host) -> ApiResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || match &st.host {
        Some(host) => action(&st, host),
        None => Err(ApiError(StatusCode::FORBIDDEN, HOST_OFF.into())),
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
}

/// Every drive, the ones seen before, and where the vault is.
async fn host_drives(State(st): State<Shared>) -> ApiResult<Json<serde_json::Value>> {
    host_call(st, |st, host| {
        let report = st.drives.report();
        Ok(Json(serde_json::json!({
            "vault": host.vault(),
            "separator": std::path::MAIN_SEPARATOR.to_string(),
            "drives": report.drives,
            "gone": report.gone,
            "at": report.at,
            "smartctl": report.smartctl,
        })))
    })
    .await
}

async fn host_list(State(st): State<Shared>, Query(q): Query<PathQuery>) -> ApiResult<Json<Vec<host::HostNode>>> {
    host_call(st, move |_, host| host.list(&host::parse(&q.path)?).map(Json)).await
}

async fn host_stat(State(st): State<Shared>, Query(q): Query<PathQuery>) -> ApiResult<Json<host::HostNode>> {
    host_call(st, move |_, host| host.stat(&host::parse(&q.path)?).map(Json)).await
}

async fn host_file(State(st): State<Shared>, Query(q): Query<PathQuery>, headers: HeaderMap) -> ApiResult<Response> {
    let path = host_call(st, move |_, host| host.readable_file(&host::parse(&q.path)?)).await?;
    let mime = mime_guess::from_path(&path).first_or_octet_stream().to_string();
    Ok(stream::file(&path, headers.get(header::RANGE), &mime, stream::Offer::Download).await?)
}

async fn host_mkdir(State(st): State<Shared>, Json(b): Json<HostMkdirBody>) -> ApiResult<Json<host::HostNode>> {
    host_call(st, move |_, host| host.mkdir(&host::parse(&b.parent)?, &b.name).map(Json)).await
}

async fn host_rename(State(st): State<Shared>, Json(b): Json<HostRenameBody>) -> ApiResult<Json<host::HostNode>> {
    host_call(st, move |_, host| host.rename(&host::parse(&b.path)?, &b.name).map(Json)).await
}

async fn host_move(State(st): State<Shared>, Json(b): Json<HostMoveBody>) -> ApiResult<Json<host::HostNode>> {
    host_call(st, move |_, host| host.move_to(&host::parse(&b.path)?, &host::parse(&b.target)?).map(Json)).await
}

async fn host_trash(State(st): State<Shared>, Json(b): Json<HostPathBody>) -> ApiResult<StatusCode> {
    host_call(st, move |_, host| host.recycle(&host::parse(&b.path)?)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn host_delete(State(st): State<Shared>, Json(b): Json<HostPathBody>) -> ApiResult<StatusCode> {
    host_call(st, move |_, host| host.delete(&host::parse(&b.path)?)).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// A file from another device into a folder on this computer. It lands under a hidden
/// temporary name in that folder and is renamed once whole, like a vault upload.
async fn host_upload(State(st): State<Shared>, Query(q): Query<UploadQuery>, body: Body) -> ApiResult<Json<host::HostNode>> {
    let name = q.name.clone();
    let dir = host_call(st.clone(), move |_, host| host.folder_for_new(&host::parse(&q.parent)?, &q.name)).await?;
    let tmp = dir.join(format!(".castle-upload-{}.part", random_hex(16)));
    receive(body, &tmp).await?;
    host_call(st, move |_, host| {
        let final_name = free_name(&dir, &name);
        if let Err(e) = std::fs::rename(&tmp, dir.join(&final_name)) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
        host.stat(&dir.join(final_name)).map(Json)
    })
    .await
}

/// Times a drive. The write test puts one file on it and deletes it again.
async fn host_speedtest(State(st): State<Shared>, Json(b): Json<SpeedBody>) -> ApiResult<Json<drives::SpeedResult>> {
    host_call(st, move |st, _| st.drives.speed_test(&b.path, b.write).map(Json).map_err(|msg| ApiError::conflict(&msg))).await
}

// ---------------------------------------------------------------------------------------
// Theater: films, shows, music, photos and videos from folders (see theater.rs)
// ---------------------------------------------------------------------------------------

/// How often every library is walked again, to pick up what was added.
const THEATER_RESCAN: std::time::Duration = std::time::Duration::from_secs(60 * 60);
/// The box setting that holds the OpenSubtitles API key.
const OPENSUBTITLES_SETTING: &str = "theater.opensubtitlesKey";

#[derive(Deserialize)]
struct LibraryAddBody {
    path: String,
    kind: String,
    #[serde(default)]
    name: String,
    #[serde(default = "everyone")]
    audience: String,
}

fn everyone() -> String {
    "everyone".into()
}

#[derive(Deserialize)]
struct LibraryUpdateBody {
    id: String,
    name: Option<String>,
    audience: Option<String>,
}

#[derive(Deserialize)]
struct ScanBody {
    #[serde(default)]
    id: Option<String>,
}

#[derive(Deserialize)]
struct ItemsQuery {
    library: Option<String>,
}

#[derive(Deserialize)]
struct DetailsQuery {
    id: String,
    /// Ask the internet when nothing is known yet.
    #[serde(default)]
    fetch: bool,
}

#[derive(Deserialize)]
struct TheaterArtQuery {
    id: String,
    /// thumb (from the file) or poster (from the internet)
    kind: String,
}

async fn theater_call<T, F>(st: Shared, action: F) -> ApiResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> theater::TheaterResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || action(&st).map_err(|msg| ApiError::conflict(&msg)))
        .await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
}

async fn theater_overview(State(st): State<Shared>) -> ApiResult<Json<theater::Overview>> {
    theater_call(st, |st| Ok(st.theater.overview())).await.map(Json)
}

/// A folder on this computer becomes a library, then is walked straight away. Reading any
/// folder is the same power as Storage's "../", so it is allowed exactly when that is.
async fn theater_library_add(State(st): State<Shared>, Json(b): Json<LibraryAddBody>) -> ApiResult<Json<theater::Library>> {
    let path = host_call(st.clone(), move |_, host| {
        let path = host::parse(&b.path)?;
        host.stat(&path)?;
        Ok((path, b.kind, b.name, b.audience))
    })
    .await?;
    let library = theater_call(st.clone(), move |st| st.theater.add_library(&path.0, &path.1, &path.2, &path.3)).await?;
    let id = library.id.clone();
    std::thread::spawn(move || st.theater.scan(Some(&id)));
    Ok(Json(library))
}

async fn theater_library_update(State(st): State<Shared>, Json(b): Json<LibraryUpdateBody>) -> ApiResult<Json<theater::Library>> {
    theater_call(st, move |st| st.theater.update_library(&b.id, b.name.as_deref(), b.audience.as_deref())).await.map(Json)
}

async fn theater_library_remove(State(st): State<Shared>, Json(b): Json<IdBody>) -> ApiResult<StatusCode> {
    theater_call(st, move |st| st.theater.remove_library(&b.id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Starts walking the libraries again; answers at once.
async fn theater_scan(State(st): State<Shared>, Json(b): Json<ScanBody>) -> ApiResult<Json<serde_json::Value>> {
    if st.theater.is_scanning() {
        return Ok(Json(serde_json::json!({ "started": false })));
    }
    std::thread::spawn(move || st.theater.scan(b.id.as_deref()));
    Ok(Json(serde_json::json!({ "started": true })))
}

async fn theater_items(State(st): State<Shared>, Query(q): Query<ItemsQuery>) -> ApiResult<Json<Vec<theater::Item>>> {
    theater_call(st, move |st| Ok(st.theater.items(q.library.as_deref()))).await.map(Json)
}

async fn theater_details(State(st): State<Shared>, Query(q): Query<DetailsQuery>) -> ApiResult<Json<theater::DetailsAnswer>> {
    theater_call(st, move |st| st.theater.details(&q.id, q.fetch)).await.map(Json)
}

async fn theater_art(State(st): State<Shared>, Query(q): Query<TheaterArtQuery>) -> ApiResult<Response> {
    let (path, mime) = theater_call(st, move |st| match q.kind.as_str() {
        "poster" => st.theater.poster(&q.id),
        _ => st.theater.thumb(&q.id),
    })
    .await
    .map_err(|_| ApiError::not_found())?;
    let mut res = stream::file(&path, None, &mime, stream::Offer::Inline).await?;
    // Pictures of an item change only when its file does, which changes its address.
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=86400"));
    Ok(res)
}

async fn theater_edit(State(st): State<Shared>, Json(b): Json<theater::EditBody>) -> ApiResult<Json<theater::Item>> {
    theater_call(st, move |st| st.theater.edit(b)).await.map(Json)
}

async fn theater_play(State(st): State<Shared>, Json(b): Json<IdBody>) -> ApiResult<Json<theater::PlayLink>> {
    theater_call(st, move |st| st.theater.play(&b.id)).await.map(Json)
}

async fn theater_subtitles(State(st): State<Shared>, Json(b): Json<IdBody>) -> ApiResult<Json<serde_json::Value>> {
    theater_call(st, move |st| {
        let key = read_settings(&st.root).get(OPENSUBTITLES_SETTING).and_then(|v| v.as_str()).unwrap_or("").to_string();
        st.theater.fetch_subtitles(&b.id, &key).map(|found| serde_json::json!({ "found": found }))
    })
    .await
    .map(Json)
}

/// A ticket is the 32 hex characters this box made; anything else is refused before lookup.
fn valid_ticket(ticket: &str) -> bool {
    ticket.len() == 32 && ticket.chars().all(|c| c.is_ascii_hexdigit())
}

/// What a play link opens, for a video or audio player. Needs no token: the ticket is the
/// permission, for one file, for a few hours.
async fn media(State(st): State<Shared>, UrlPath(ticket): UrlPath<String>, headers: HeaderMap) -> ApiResult<Response> {
    if !valid_ticket(&ticket) {
        return Err(ApiError::not_found());
    }
    let path = st.theater.ticket_file(&ticket).ok_or_else(ApiError::not_found)?;
    let mime = theater::media_type(&path.to_string_lossy());
    Ok(stream::file(&path, headers.get(header::RANGE), &mime, stream::Offer::Inline).await?)
}

async fn media_subtitle(State(st): State<Shared>, UrlPath((ticket, n)): UrlPath<(String, usize)>) -> ApiResult<Response> {
    if !valid_ticket(&ticket) {
        return Err(ApiError::not_found());
    }
    let text = tokio::task::spawn_blocking(move || st.theater.ticket_subtitle(&ticket, n))
        .await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
        .ok_or_else(ApiError::not_found)?;
    let mut res = Response::new(Body::from(text));
    res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("text/vtt; charset=utf-8"));
    Ok(res)
}

// ---------------------------------------------------------------------------------------
// Steam cache: games downloaded once, shared over the home network (see steam.rs, dns.rs)
// ---------------------------------------------------------------------------------------

/// Where the cache listens when turned on from the app: every address, port 80, which is
/// where Steam looks for it.
const STEAM_CACHE_DEFAULT: &str = "0.0.0.0:80";
const DNS_DEFAULT: &str = "0.0.0.0:53";
/// Box settings that remember the cache and DNS were turned on, for the next start.
const STEAM_CACHE_SETTING: &str = "steam.cacheAddress";
const DNS_SETTING: &str = "steam.dnsAddress";
/// How often game details are matched and refreshed in the background.
const STEAM_REFRESH: std::time::Duration = std::time::Duration::from_secs(10 * 60);

#[derive(Deserialize)]
struct AppQuery {
    app: u32,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

#[derive(Deserialize)]
struct KeepBody {
    app: u32,
    #[serde(default = "yes")]
    keep: bool,
}

#[derive(Deserialize)]
struct AppBody {
    app: u32,
}

/// This computer's address in the home: the one it would use to reach the internet. No
/// packet is sent to find it.
fn home_address() -> Option<std::net::Ipv4Addr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:53").ok()?;
    match socket.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) if !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

/// Starts the cache's own listener, for Steam clients in the home. No token: Steam cannot
/// send one, and the cache answers only Steam content requests.
async fn start_steam_cache(st: &Shared, addr: SocketAddr) -> Result<(), String> {
    if st.steam.listening.lock().map(|l| l.is_some()).unwrap_or(false) {
        return Ok(());
    }
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| format!("The Steam cache could not listen on {addr}: {e}. Another program may be using that port."))?;
    *st.steam.listening.lock().map_err(|_| "The cache is unavailable".to_string())? = Some(addr);
    let app = steam::router(st.steam.clone()).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    println!("steam cache: listening on http://{addr}");
    Ok(())
}

async fn start_dns(st: &Shared, addr: SocketAddr) -> Result<(), String> {
    if st.dns.lock().map(|d| d.is_some()).unwrap_or(false) {
        return Ok(());
    }
    let ip = home_address().ok_or("This computer's address in the home could not be found.")?;
    let upstream: SocketAddr = std::env::var("CASTLE_DNS_UPSTREAM").unwrap_or_else(|_| dns::DEFAULT_UPSTREAM.into()).parse().map_err(|_| "CASTLE_DNS_UPSTREAM is not an address".to_string())?;
    // Bind first, so a port already taken is reported now rather than lost in a task.
    drop(tokio::net::UdpSocket::bind(addr).await.map_err(|e| format!("The DNS server could not listen on {addr}: {e}. Port 53 is often taken, or needs administrator rights."))?);
    *st.dns.lock().map_err(|_| "DNS is unavailable".to_string())? = Some(addr);
    tokio::spawn(async move {
        if let Err(e) = dns::run(addr, ip, upstream).await {
            eprintln!("[castled] dns stopped: {e}");
        }
    });
    println!("dns: listening on {addr}, sending Steam to {ip}");
    Ok(())
}

fn remember_setting(root: &Path, key: &str, value: &str) {
    let mut saved = read_settings(root);
    saved.insert(key.into(), serde_json::Value::String(value.into()));
    let tmp = settings_file(root).with_extension("json.tmp");
    if std::fs::write(&tmp, serde_json::to_vec_pretty(&saved).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(tmp, settings_file(root));
    }
}

async fn steam_overview(State(st): State<Shared>) -> ApiResult<Json<serde_json::Value>> {
    let (resolved, points_here) = steam::steam_points_here().await;
    let state = st.clone();
    let (apps, downloading, totals, local) = tokio::task::spawn_blocking(move || {
        let local = steam::local_games();
        (state.steam.apps(&local), state.steam.downloading(), state.steam.totals(), local.len())
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?;
    Ok(Json(serde_json::json!({
        "listening": st.steam.listening.lock().ok().and_then(|l| l.map(|a| a.to_string())),
        "dns": st.dns.lock().ok().and_then(|d| d.map(|a| a.to_string())),
        "boxAddress": home_address().map(|ip| ip.to_string()),
        "cacheDir": plain_path(st.steam.dir()).to_string_lossy(),
        "steamPointsHere": points_here,
        "resolved": resolved,
        "totals": totals,
        "apps": apps,
        "downloading": downloading,
        "installedHere": local,
        "canTurnOn": st.loopback,
    })))
}

async fn steam_art(State(st): State<Shared>, Query(q): Query<AppQuery>) -> ApiResult<Response> {
    let path = st.steam.art(q.app).ok_or_else(ApiError::not_found)?;
    let mut res = stream::file(&path, None, "image/jpeg", stream::Offer::Inline).await?;
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=86400"));
    Ok(res)
}

async fn steam_search(Query(q): Query<SearchQuery>) -> ApiResult<Json<Vec<steam::Found>>> {
    let query = q.q.trim().chars().take(100).collect::<String>();
    if query.is_empty() {
        return Ok(Json(Vec::new()));
    }
    tokio::task::spawn_blocking(move || steam::search(&query))
        .await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
        .map(Json)
        .map_err(|msg| ApiError(StatusCode::BAD_GATEWAY, msg))
}

async fn steam_local() -> ApiResult<Json<Vec<steam::LocalGame>>> {
    tokio::task::spawn_blocking(steam::local_games).await.map(Json).map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))
}

/// Chooses a game to keep (or stops keeping it), then fetches its details in the background.
async fn steam_keep(State(st): State<Shared>, Json(b): Json<KeepBody>) -> ApiResult<StatusCode> {
    st.steam.keep(b.app, b.keep);
    let state = st.clone();
    std::thread::spawn(move || state.steam.refresh(&steam::local_games()));
    Ok(StatusCode::NO_CONTENT)
}

async fn steam_purge(State(st): State<Shared>, Json(b): Json<AppBody>) -> ApiResult<Json<serde_json::Value>> {
    let freed = tokio::task::spawn_blocking(move || st.steam.purge(b.app, &steam::local_games()))
        .await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
        .map_err(|msg| ApiError::conflict(&msg))?;
    Ok(Json(serde_json::json!({ "freed": freed })))
}

async fn steam_refresh(State(st): State<Shared>) -> ApiResult<StatusCode> {
    std::thread::spawn(move || st.steam.refresh(&steam::local_games()));
    Ok(StatusCode::ACCEPTED)
}

/// Turns the cache on from the app. It listens to the whole home network, so only from
/// this computer; it is remembered for the next start.
async fn steam_enable(State(st): State<Shared>) -> ApiResult<Json<serde_json::Value>> {
    local_only(&st)?;
    let addr: SocketAddr = STEAM_CACHE_DEFAULT.parse().map_err(|_| ApiError::bad("bad address"))?;
    start_steam_cache(&st, addr).await.map_err(|msg| ApiError::conflict(&msg))?;
    remember_setting(&st.root, STEAM_CACHE_SETTING, STEAM_CACHE_DEFAULT);
    Ok(Json(serde_json::json!({ "listening": addr.to_string() })))
}

async fn steam_dns_enable(State(st): State<Shared>) -> ApiResult<Json<serde_json::Value>> {
    local_only(&st)?;
    let addr: SocketAddr = DNS_DEFAULT.parse().map_err(|_| ApiError::bad("bad address"))?;
    start_dns(&st, addr).await.map_err(|msg| ApiError::conflict(&msg))?;
    remember_setting(&st.root, DNS_SETTING, DNS_DEFAULT);
    Ok(Json(serde_json::json!({ "dns": addr.to_string() })))
}

// ---------------------------------------------------------------------------------------
// AI: DOGS AI's server, run by the box (see ai.rs)
// ---------------------------------------------------------------------------------------

async fn ai_status(State(st): State<Shared>) -> ApiResult<Json<ai::Status>> {
    tokio::task::spawn_blocking(move || st.ai.status()).await.map(Json).map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))
}

/// Starting and stopping run programs on this computer, so only from this computer.
async fn ai_start(State(st): State<Shared>) -> ApiResult<Json<ai::Status>> {
    local_only(&st)?;
    tokio::task::spawn_blocking(move || {
        st.ai.start().map_err(|msg| ApiError::conflict(&msg))?;
        // Give it a moment to open its port, so the answer says whether it is up.
        for _ in 0..20 {
            if st.ai.status().running {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        Ok(Json(st.ai.status()))
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
}

async fn ai_stop(State(st): State<Shared>) -> ApiResult<Json<ai::Status>> {
    local_only(&st)?;
    tokio::task::spawn_blocking(move || {
        st.ai.stop();
        Json(st.ai.status())
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))
}

// ---------------------------------------------------------------------------------------
// Clones: machines and automations in Clonewars VMs (see clones.rs)
// ---------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct BotIdBody {
    id: String,
    #[serde(default)]
    bot: String,
}

/// Runs one clones action off the async threads: these read files, scan processes and
/// can wait on git.
async fn clones_call<T, F>(st: Shared, action: F) -> ApiResult<Json<T>>
where
    T: Send + 'static,
    F: FnOnce(&clones::Clones) -> clones::ClonesResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || match st.clones.read().ok().and_then(|slot| slot.clone()) {
        Some(clones) => action(&clones).map(Json).map_err(|msg| ApiError(StatusCode::CONFLICT, msg)),
        None => Err(ApiError(StatusCode::NOT_FOUND, "this box was not started with --clonewars".into())),
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
}

// ---------------------------------------------------------------------------------------
// Arcade save data
// ---------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct RestoreBody {
    id: String,
    hash: String,
}

/// Runs a save-data action against the games folder, off the async threads.
async fn saves_call<T, F>(st: Shared, action: F) -> ApiResult<Json<T>>
where
    T: Send + 'static,
    F: FnOnce(&AppState, &arcade::Arcade) -> saves::SavesResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || match st.arcade.read().ok().and_then(|slot| slot.clone()) {
        Some(arcade) => action(&st, &arcade).map(Json).map_err(|msg| ApiError(StatusCode::CONFLICT, msg)),
        None => Err(ApiError(StatusCode::NOT_FOUND, "this box was not started with --arcade".into())),
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
}

async fn saves_list(State(st): State<Shared>) -> ApiResult<Json<Vec<saves::SaveFile>>> {
    saves_call(st, |st, arcade| Ok(st.saves.list(arcade.root(), &arcade.rom_index()))).await
}

async fn saves_record(State(st): State<Shared>) -> ApiResult<Json<saves::Recorded>> {
    saves_call(st, |st, arcade| st.saves.record(arcade.root())).await
}

async fn saves_history(State(st): State<Shared>, Query(q): Query<PathQuery>) -> ApiResult<Json<Vec<saves::Version>>> {
    saves_call(st, move |st, _| Ok(st.saves.history(&q.path))).await
}

/// Writes to the games drive, so like launching a game it works from this computer only.
async fn saves_restore(State(st): State<Shared>, Json(b): Json<RestoreBody>) -> ApiResult<Json<serde_json::Value>> {
    local_only(&st)?;
    saves_call(st, move |st, arcade| st.saves.restore(arcade.root(), &b.id, &b.hash).map(|()| serde_json::json!({ "restored": true }))).await
}

// ---------------------------------------------------------------------------------------
// Turning Clones and Arcade on while the box runs
// ---------------------------------------------------------------------------------------
//
// Both sections run programs, so the caller never names the folder: a web page must not be
// able to point the box at a folder of programs of its choosing. The box looks in a fixed
// set of places itself, and only when it is reachable from this computer alone.

/// The folders to look in for Castle's clonewars folder: CASTLE_CLONEWARS_DIR, then a
/// `clonewars` folder beside or above the daemon's own program and its working folder
/// (which is where it sits in a checkout of Castle).
fn clonewars_candidates() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::env::var("CASTLE_CLONEWARS_DIR").ok().map(PathBuf::from).into_iter().collect();
    let starts = [std::env::current_exe().ok(), std::env::current_dir().ok()];
    for start in starts.into_iter().flatten() {
        found.extend(start.ancestors().map(|dir| dir.join("clonewars")));
    }
    found
}

/// The folders to look in for a games folder: CASTLE_ARCADE_DIR, then `Games` at the top of
/// every drive (Windows) or of every mounted disk (Linux).
fn arcade_candidates() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::env::var("CASTLE_ARCADE_DIR").ok().map(PathBuf::from).into_iter().collect();
    if cfg!(windows) {
        found.extend(('C'..='Z').map(|letter| PathBuf::from(format!(r"{letter}:\Games"))));
    } else {
        // Removable disks mount one or two folders deep under these.
        for base in ["/media", "/run/media", "/mnt"] {
            let Ok(first) = std::fs::read_dir(base) else { continue };
            for mount in first.flatten().map(|entry| entry.path()) {
                found.push(mount.join("Games"));
                if let Ok(second) = std::fs::read_dir(&mount) {
                    found.extend(second.flatten().map(|entry| entry.path().join("Games")));
                }
            }
        }
    }
    found
}

/// The real location of a folder that programs will be started from.
///
/// On Windows, resolving a path yields the long form that starts with `\\?\`, which the
/// system accepts but many programs do not: RetroArch started with its own path, core and
/// game in that form sits there with no window. So the ordinary `C:\...` form is handed on.
fn program_folder(dir: &Path) -> std::io::Result<PathBuf> {
    let resolved = dir.canonicalize()?;
    Ok(plain_path(&resolved))
}

fn plain_path(path: &Path) -> PathBuf {
    const LONG_FORM: &str = r"\\?\";
    let text = path.to_string_lossy();
    match text.strip_prefix(LONG_FORM) {
        // Only a drive path can drop the prefix; a network path in this form cannot.
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

fn local_only(st: &AppState) -> ApiResult<()> {
    if st.loopback {
        Ok(())
    } else {
        Err(ApiError(StatusCode::FORBIDDEN, "this can only be turned on from the computer the box runs on".into()))
    }
}

async fn clones_enable(State(st): State<Shared>) -> ApiResult<Json<serde_json::Value>> {
    local_only(&st)?;
    tokio::task::spawn_blocking(move || {
        let dir = clonewars_candidates()
            .into_iter()
            .filter_map(|dir| program_folder(&dir).ok())
            .find(|dir| clones::looks_like_clonewars(dir))
            .ok_or_else(|| ApiError(StatusCode::CONFLICT, "No clonewars folder was found near the box. Start the box with --clonewars and its path.".into()))?;
        let clones = clones::Clones::new(dir, st.root.join(META_DIR).join("clones"), st.loopback);
        *st.clones.write().map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))? = Some(Arc::new(clones));
        Ok(Json(serde_json::json!({ "on": true })))
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
}

async fn arcade_enable(State(st): State<Shared>) -> ApiResult<Json<serde_json::Value>> {
    local_only(&st)?;
    tokio::task::spawn_blocking(move || {
        let dir = arcade_candidates()
            .into_iter()
            .filter_map(|dir| program_folder(&dir).ok())
            .find(|dir| arcade::looks_like_arcade(dir))
            .ok_or_else(|| ApiError(StatusCode::CONFLICT, "No games folder was found. Plug in the drive that has Games (with Consoles and Roms inside) and try again.".into()))?;
        *st.arcade.write().map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))? = Some(Arc::new(arcade::Arcade::new(dir, st.loopback)));
        Ok(Json(serde_json::json!({ "on": true })))
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
}

type ClonesJson = ApiResult<Json<serde_json::Value>>;

async fn clones_list(State(st): State<Shared>) -> ApiResult<Json<Vec<serde_json::Value>>> {
    clones_call(st, |c| Ok(c.list())).await
}

async fn clones_automation_save(State(st): State<Shared>, Json(b): Json<clones::AutomationDef>) -> ClonesJson {
    clones_call(st, move |c| c.save_automation(b)).await
}

async fn clones_automation_delete(State(st): State<Shared>, Json(b): Json<IdBody>) -> ApiResult<Json<Vec<serde_json::Value>>> {
    clones_call(st, move |c| c.delete_automation(&b.id)).await
}

async fn clones_bot_save(State(st): State<Shared>, Json(b): Json<clones::BotBody>) -> ClonesJson {
    clones_call(st, move |c| c.save_bot(b)).await
}

async fn clones_bot_delete(State(st): State<Shared>, Json(b): Json<BotIdBody>) -> ClonesJson {
    clones_call(st, move |c| c.delete_bot(&b.id, &b.bot)).await
}

async fn clones_start(State(st): State<Shared>, Json(b): Json<BotIdBody>) -> ClonesJson {
    clones_call(st, move |c| c.start(&b.id, &b.bot)).await
}

async fn clones_stop(State(st): State<Shared>, Json(b): Json<BotIdBody>) -> ClonesJson {
    clones_call(st, move |c| c.stop(&b.id, &b.bot)).await
}

async fn clones_machines(State(st): State<Shared>) -> ClonesJson {
    clones_call(st, |c| Ok(c.machines())).await
}

async fn clones_machine_start(State(st): State<Shared>, Json(b): Json<clones::MachineBody>) -> ClonesJson {
    clones_call(st, move |c| c.start_machine(b)).await
}

async fn clones_machine_stop(State(st): State<Shared>, Json(b): Json<IdBody>) -> ClonesJson {
    clones_call(st, move |c| c.stop_machine(&b.id)).await
}

async fn clones_machine_delete(State(st): State<Shared>, Json(b): Json<IdBody>) -> ClonesJson {
    clones_call(st, move |c| c.delete_machine(&b.id)).await
}

/// Live control of a container's screen (VNC behind a one-time password), on or off.
#[derive(Deserialize)]
struct LiveBody {
    key: String,
    on: bool,
}

async fn clones_live(State(st): State<Shared>, Json(b): Json<LiveBody>) -> ClonesJson {
    clones_call(st, move |c| c.live(&b.key, b.on)).await
}

/// One picture of a running automation's VM screen.
async fn clones_screen(State(st): State<Shared>, Query(q): Query<PathQuery>) -> ApiResult<Response> {
    let bytes = tokio::task::spawn_blocking(move || match st.clones.read().ok().and_then(|slot| slot.clone()) {
        Some(clones) => clones.screen(&q.path).map_err(|msg| ApiError(StatusCode::CONFLICT, msg)),
        None => Err(ApiError(StatusCode::NOT_FOUND, "this box was not started with --clonewars".into())),
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))??;
    let mut res = Response::new(Body::from(bytes));
    let headers = res.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(res)
}


// ---------------------------------------------------------------------------------------
// Arcade: read-only game library from a games folder (see arcade.rs)
// ---------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct SystemQuery {
    system: String,
}

#[derive(Deserialize)]
struct ArtQuery {
    system: String,
    game: usize,
    kind: String,
}

#[derive(Deserialize)]
struct PlayBody {
    system: String,
    game: usize,
    /// Which disc of a multi-disc game; the first when left out.
    #[serde(default)]
    disc: usize,
}

#[derive(Deserialize)]
struct AboutQuery {
    system: String,
    game: usize,
}

/// Runs one arcade action off the async threads: these read the drive.
async fn arcade_call<T, F>(st: Shared, action: F) -> ApiResult<T>
where
    T: Send + 'static,
    F: FnOnce(&arcade::Arcade) -> arcade::ArcadeResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || match st.arcade.read().ok().and_then(|slot| slot.clone()) {
        Some(arcade) => action(&arcade).map_err(|msg| ApiError(StatusCode::CONFLICT, msg)),
        None => Err(ApiError(StatusCode::NOT_FOUND, "this box was not started with --arcade".into())),
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
}

async fn arcade_systems(State(st): State<Shared>) -> ApiResult<Json<Vec<arcade::SystemSummary>>> {
    arcade_call(st, |a| Ok(a.systems())).await.map(Json)
}

async fn arcade_games(State(st): State<Shared>, Query(q): Query<SystemQuery>) -> ApiResult<Json<Vec<arcade::Game>>> {
    arcade_call(st, move |a| a.games(&q.system)).await.map(Json)
}

async fn arcade_art(State(st): State<Shared>, Query(q): Query<ArtQuery>) -> ApiResult<Response> {
    let path = arcade_call(st, move |a| a.art(&q.system, q.game, &q.kind))
        .await
        .map_err(|_| ApiError::not_found())?;
    let bytes = tokio::fs::read(path).await?;
    let mut res = Response::new(Body::from(bytes));
    res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    res.headers_mut().insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    Ok(res)
}

async fn arcade_play(State(st): State<Shared>, Json(b): Json<PlayBody>) -> ApiResult<Json<serde_json::Value>> {
    let title = arcade_call(st, move |a| a.play(&b.system, b.game, b.disc)).await?;
    Ok(Json(serde_json::json!({ "started": title })))
}

/// What a game is, in a paragraph, from Wikipedia. The first time a game is asked about
/// its title is sent to Wikipedia; the answer is kept, so it is sent once. `about` is null
/// when Wikipedia has no article that is clearly about this game.
async fn arcade_about(State(st): State<Shared>, Query(q): Query<AboutQuery>) -> ApiResult<Json<serde_json::Value>> {
    let (title, short) = arcade_call(st.clone(), move |a| a.naming(&q.system, q.game)).await?;
    tokio::task::spawn_blocking(move || {
        let name = arcade::plain_title(&title);
        let key = format!("{short}|{name}");
        let about = match st.game_notes.get(&key) {
            Some(kept) => kept,
            None => {
                let http = wiki::client().map_err(|msg| ApiError(StatusCode::BAD_GATEWAY, msg))?;
                let found = wiki::find(&http, &format!("{name} video game"))
                    .map_err(|msg| ApiError(StatusCode::BAD_GATEWAY, msg))?
                    .filter(|article| wiki::is_about(&name, article));
                st.game_notes.put(&key, found.clone());
                found
            }
        };
        Ok(Json(serde_json::json!({ "about": about })))
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
}

// ---------------------------------------------------------------------------------------
// Network: the box's own connection (see network.rs)
// ---------------------------------------------------------------------------------------

/// Runs one network action off the async threads: these wait on the operating system and,
/// for the speed test and provider lookup, on the internet.
async fn network_call<T, F>(st: Shared, action: F) -> ApiResult<Json<T>>
where
    T: Send + 'static,
    F: FnOnce(&network::Network) -> network::NetworkResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || action(&st.network).map(Json).map_err(|msg| ApiError(StatusCode::BAD_GATEWAY, msg)))
        .await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
}

async fn network_reading(State(st): State<Shared>) -> ApiResult<Json<network::Reading>> {
    network_call(st, |n| Ok(n.reading())).await
}

async fn network_speedtest(State(st): State<Shared>) -> ApiResult<Json<network::SpeedResult>> {
    network_call(st, |n| n.speed_test()).await
}

async fn network_provider(State(st): State<Shared>) -> ApiResult<Json<serde_json::Value>> {
    network_call(st, |n| n.provider()).await
}

// ---------------------------------------------------------------------------------------
// Settings: what the app saves on the box, and what the box was started with
// ---------------------------------------------------------------------------------------

const MAX_SETTINGS: usize = 200;
const MAX_SETTING_KEY: usize = 64;
const MAX_SETTING_TEXT: usize = 500;

fn settings_file(root: &Path) -> PathBuf {
    root.join(META_DIR).join("settings.json")
}

fn read_settings(root: &Path) -> serde_json::Map<String, serde_json::Value> {
    std::fs::read(settings_file(root))
        .ok()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or_default()
}

/// A setting is a dotted lower-case name ("storage.trashDays") holding a switch, a number
/// or a short piece of text. The box stores them; it does not interpret them yet.
fn valid_setting(key: &str, value: &serde_json::Value) -> bool {
    let key_ok = !key.is_empty()
        && key.len() <= MAX_SETTING_KEY
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '.');
    let value_ok = match value {
        serde_json::Value::Bool(_) | serde_json::Value::Number(_) => true,
        serde_json::Value::String(text) => text.len() <= MAX_SETTING_TEXT && !text.chars().any(|c| c.is_control()),
        _ => false,
    };
    key_ok && value_ok
}

async fn settings_get(State(st): State<Shared>) -> Json<serde_json::Map<String, serde_json::Value>> {
    Json(read_settings(&st.root))
}

/// Merges the given settings into the saved ones and returns the result.
async fn settings_put(
    State(st): State<Shared>,
    Json(changes): Json<serde_json::Map<String, serde_json::Value>>,
) -> ApiResult<Json<serde_json::Map<String, serde_json::Value>>> {
    if !changes.iter().all(|(k, v)| valid_setting(k, v)) {
        return Err(ApiError::bad("that setting cannot be saved"));
    }
    let _writing = st.settings.lock().unwrap();
    let mut saved = read_settings(&st.root);
    saved.extend(changes);
    if saved.len() > MAX_SETTINGS {
        return Err(ApiError::bad("too many settings"));
    }
    let tmp = settings_file(&st.root).with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&saved).unwrap_or_default())?;
    std::fs::rename(tmp, settings_file(&st.root))?;
    Ok(Json(saved))
}

/// Processor, memory, disk and uptime of the machine the box runs on.
async fn system_vitals(State(st): State<Shared>) -> ApiResult<Json<system::Vitals>> {
    tokio::task::spawn_blocking(move || st.system.vitals(&st.root))
        .await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
        .map(Json)
        .map_err(|msg| ApiError(StatusCode::CONFLICT, msg))
}

/// The box in a few numbers, for the spec card on Home: its processor, and its disks added
/// up with the oldest one's power-on hours. No paths and no names, so unlike the drive list
/// it needs no access to the computer's folders.
async fn specs(State(st): State<Shared>) -> ApiResult<Json<serde_json::Value>> {
    tokio::task::spawn_blocking(move || serde_json::json!({ "processor": st.system.processor(), "drives": st.drives.summary() }))
        .await
        .map(Json)
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))
}

/// What this box is and how it was started: read-only facts for the Settings screen.
async fn box_info(State(st): State<Shared>) -> ApiResult<Json<serde_json::Value>> {
    let state = st.clone();
    let links = tokio::task::spawn_blocking(move || state.network.links())
        .await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?;
    Ok(Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "listen": st.listen,
        "loopback": st.loopback,
        "clones": st.clones.read().is_ok_and(|slot| slot.is_some()),
        "arcade": st.arcade.read().is_ok_and(|slot| slot.is_some()),
        "hostFiles": st.host.is_some(),
        "links": links,
    })))
}

// ---------------------------------------------------------------------------------------
// Security: the activity record, posture checks, the token, audits
// ---------------------------------------------------------------------------------------

const DEFAULT_LOG_ENTRIES: usize = 200;
const MAX_LOG_ENTRIES: usize = 2000;
/// A call's JSON body is read to record what it acted on; anything larger is refused.
const MAX_RECORDED_BODY: usize = 1024 * 1024;
/// Body fields worth recording. Never the whole body: a login's password must not land in the record.
const RECORDED_FIELDS: [&str; 8] = ["id", "name", "parent", "to", "system", "hash", "path", "target"];

async fn security_call<T, F>(st: Shared, action: F) -> ApiResult<Json<T>>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> security::SecurityResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || action(&st))
        .await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))?
        .map(Json)
        .map_err(|msg| ApiError(StatusCode::CONFLICT, msg))
}

async fn security_overview(State(st): State<Shared>) -> ApiResult<Json<serde_json::Value>> {
    security_call(st, |st| {
        let private_link = st.network.links().private_link();
        let posture = security::Posture { root: &st.root, listen: &st.listen, loopback: st.loopback, private_link };
        let token_made = std::fs::metadata(st.root.join(META_DIR).join("token")).and_then(|m| m.modified()).map(millis).unwrap_or(0);
        Ok(serde_json::json!({
            "checks": security::checks(&st.security, &posture),
            "record": st.security.verify(),
            "deniedLastDay": st.security.denied_last_day(),
            "startedAt": st.security.started(),
            "tokenMadeAt": token_made,
        }))
    })
    .await
}

#[derive(Deserialize)]
struct LogQuery {
    limit: Option<usize>,
}

async fn security_log(State(st): State<Shared>, Query(q): Query<LogQuery>) -> ApiResult<Json<Vec<security::Entry>>> {
    let limit = q.limit.unwrap_or(DEFAULT_LOG_ENTRIES).min(MAX_LOG_ENTRIES);
    security_call(st, move |st| Ok(st.security.entries(limit))).await
}

async fn security_verify(State(st): State<Shared>) -> ApiResult<Json<security::Verdict>> {
    security_call(st, |st| Ok(st.security.verify())).await
}

/// Replaces the box token. Every device holding the old one is locked out at once.
async fn security_rotate(State(st): State<Shared>) -> ApiResult<Json<serde_json::Value>> {
    security_call(st, |st| {
        let token = random_hex(32);
        write_token(&st.root, &token).map_err(|_| "could not save the new token".to_string())?;
        *st.token.write().map_err(|_| "the token is unavailable".to_string())? = token.clone();
        Ok(serde_json::json!({ "token": token }))
    })
    .await
}

async fn security_audits() -> ApiResult<Json<security::AuditList>> {
    tokio::task::spawn_blocking(security::audit_list)
        .await
        .map(Json)
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into()))
}

async fn security_audit_run(State(st): State<Shared>, Json(b): Json<IdBody>) -> ApiResult<Json<security::AuditReport>> {
    security_call(st, move |_| security::run_audit(&b.id)).await
}

// ---------------------------------------------------------------------------------------
// Auth, and the record of what each call did
// ---------------------------------------------------------------------------------------

/// What a call acted on, taken from its query string or the named fields of its JSON body.
fn recorded_detail(query: Option<&str>, body: &[u8]) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(query) = query.filter(|q| !q.is_empty()) {
        parts.push(query.to_string());
    }
    if let Ok(serde_json::Value::Object(fields)) = serde_json::from_slice(body) {
        for key in RECORDED_FIELDS {
            if let Some(serde_json::Value::String(value)) = fields.get(key) {
                parts.push(format!("{key}={value}"));
            }
        }
    }
    parts.join(" ")
}

async fn require_token(State(st): State<Shared>, req: Request, next: Next) -> Response {
    let given = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    // Constant-time, so response timing does not reveal how much of a guess was right.
    let allowed: bool = st.token.read().is_ok_and(|token| given.as_bytes().ct_eq(token.as_bytes()).into());
    let path = req.uri().path().trim_start_matches("/api/v1").to_string();
    if !allowed {
        st.security.note_denied(&path);
        return ApiError(StatusCode::UNAUTHORIZED, "this box needs its token".into()).into_response();
    }
    // Reads are not recorded; neither is re-checking the record itself.
    if req.method() == Method::GET || path == "/security/verify" {
        return next.run(req).await;
    }

    let action = path.trim_start_matches('/').replace('/', ".");
    let query = req.uri().query().map(str::to_string);
    // An upload's body is the file itself and is streamed, not read here.
    let (req, body) = if path == "/upload" || path == "/host/upload" {
        (req, Vec::new())
    } else {
        let (parts, body) = req.into_parts();
        let Ok(bytes) = axum::body::to_bytes(body, MAX_RECORDED_BODY).await else {
            return ApiError(StatusCode::PAYLOAD_TOO_LARGE, "request too large".into()).into_response();
        };
        let copy = bytes.to_vec();
        (Request::from_parts(parts, Body::from(bytes)), copy)
    };
    let response = next.run(req).await;
    let detail = format!("{} {}", response.status().as_u16(), recorded_detail(query.as_deref(), &body));
    let recorder = st.clone();
    let _ = tokio::task::spawn_blocking(move || recorder.security.record(&action, detail.trim())).await;
    response
}

/// What the browser is told about every page and answer the box serves: load code only
/// from the box itself, never show Castle inside another site's frame, and send no
/// address of this page to anyone.
const CONTENT_POLICY: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline';     img-src 'self' blob: data:; media-src 'self' blob:; font-src 'self' data:; connect-src 'self';     object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

/// The content policy, plus the one thing it lets in from elsewhere: DOGS AI's app, shown in
/// the AI section from its own server on this computer (or on the box, over the network).
fn content_policy(listen: SocketAddr) -> HeaderValue {
    let mut frames = String::from("'self' http://127.0.0.1:* http://localhost:*");
    if !listen.ip().is_loopback() && !listen.ip().is_unspecified() {
        frames.push_str(&format!(" http://{}:*", listen.ip()));
    } else if listen.ip().is_unspecified() {
        frames.push_str(" http:");
    }
    HeaderValue::from_str(&format!("{CONTENT_POLICY}; frame-src {frames}")).unwrap_or_else(|_| HeaderValue::from_static(CONTENT_POLICY))
}

/// Runs on every request, before anything else.
///
/// A box that listens only on this computer answers only to this computer's names. Without
/// that, a web page elsewhere could point a name of its own at 127.0.0.1 and have the
/// browser talk to the box as if it were that page's own server (DNS rebinding).
async fn harden(State(st): State<Shared>, req: Request, next: Next) -> Response {
    if st.loopback {
        let host = req.headers().get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("");
        let name = host.rsplit_once(':').map_or(host, |(name, _port)| name);
        if !matches!(name, "127.0.0.1" | "localhost" | "[::1]") {
            return ApiError(StatusCode::FORBIDDEN, "this box answers only to this computer".into()).into_response();
        }
    }
    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    headers.insert(header::CONTENT_SECURITY_POLICY, st.policy.clone());
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    res
}

fn write_token(root: &Path, token: &str) -> std::io::Result<()> {
    let path = root.join(META_DIR).join("token");
    let tmp = root.join(META_DIR).join("tmp").join(format!("token-{}", random_hex(8)));
    std::fs::create_dir_all(root.join(META_DIR).join("tmp"))?;
    std::fs::write(&tmp, token)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(tmp, path)
}

/// The box token: made once, kept in the vault's private folder, reused on every start.
fn load_or_create_token(root: &Path) -> std::io::Result<String> {
    let path = root.join(META_DIR).join("token");
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let existing = existing.trim().to_string();
        if existing.len() >= 32 {
            return Ok(existing);
        }
    }
    let token = random_hex(32);
    write_token(root, &token)?;
    Ok(token)
}

// ---------------------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------------------

struct Config {
    data: PathBuf,
    bind: String,
    web: Option<PathBuf>,
    clonewars: Option<PathBuf>,
    arcade: Option<PathBuf>,
    /// Let Storage open the rest of this computer even when the box can be reached from
    /// other machines. It always can when the box listens on this computer only.
    host_files: bool,
    /// Where the Steam cache listens for Steam clients in the home, and where it keeps games.
    steam_cache: Option<String>,
    steam_cache_dir: Option<PathBuf>,
    /// Where the DNS server that sends Steam to the cache listens.
    dns: Option<String>,
}

fn usage() -> ! {
    eprintln!(
        "castled -- the Castle box daemon\n\n\
         usage: castled [--data DIR] [--bind ADDR:PORT] [--web DIR] [--clonewars DIR] [--arcade DIR] [--host-files]\n                [--steam-cache ADDR:PORT] [--steam-cache-dir DIR] [--dns ADDR:PORT]\n\n\
         --data  the vault directory            (default {DEFAULT_DATA_DIR}, or CASTLE_DATA_DIR)\n\
         --bind  address to listen on           (default {DEFAULT_BIND}, or CASTLE_BIND)\n\
         --web   built Castle app to serve at / (optional, or CASTLE_WEB_DIR)
         --clonewars  Castle's clonewars folder, to run automations (optional, or CASTLE_CLONEWARS_DIR)
         --arcade     a games folder (Consoles/ + Roms/) to show in Arcade (optional, or CASTLE_ARCADE_DIR)
         --host-files let Storage open the whole computer even when other machines can reach the box (or CASTLE_HOST_FILES=1)
         --steam-cache      where the Steam cache listens, e.g. 0.0.0.0:80 (or CASTLE_STEAM_CACHE)
         --steam-cache-dir  where it keeps games (default: the vault's private folder, or CASTLE_STEAM_CACHE_DIR)
         --dns              run a DNS server that sends Steam to the cache, e.g. 0.0.0.0:53 (or CASTLE_DNS)"
    );
    std::process::exit(2)
}

fn config() -> Config {
    let mut cfg = Config {
        data: std::env::var("CASTLE_DATA_DIR").unwrap_or_else(|_| DEFAULT_DATA_DIR.into()).into(),
        bind: std::env::var("CASTLE_BIND").unwrap_or_else(|_| DEFAULT_BIND.into()),
        web: std::env::var("CASTLE_WEB_DIR").ok().map(PathBuf::from),
        clonewars: std::env::var("CASTLE_CLONEWARS_DIR").ok().map(PathBuf::from),
        arcade: std::env::var("CASTLE_ARCADE_DIR").ok().map(PathBuf::from),
        host_files: std::env::var("CASTLE_HOST_FILES").is_ok_and(|v| v == "1"),
        steam_cache: std::env::var("CASTLE_STEAM_CACHE").ok(),
        steam_cache_dir: std::env::var("CASTLE_STEAM_CACHE_DIR").ok().map(PathBuf::from),
        dns: std::env::var("CASTLE_DNS").ok(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == "--host-files" {
            cfg.host_files = true;
            continue;
        }
        let Some(value) = args.next() else { usage() };
        match flag.as_str() {
            "--data" => cfg.data = value.into(),
            "--bind" => cfg.bind = value,
            "--web" => cfg.web = Some(value.into()),
            "--clonewars" => cfg.clonewars = Some(value.into()),
            "--arcade" => cfg.arcade = Some(value.into()),
            "--steam-cache" => cfg.steam_cache = Some(value),
            "--steam-cache-dir" => cfg.steam_cache_dir = Some(value.into()),
            "--dns" => cfg.dns = Some(value),
            _ => usage(),
        }
    }
    cfg
}

fn router(state: Shared, web: Option<PathBuf>) -> Router {
    let protected = Router::new()
        .route("/list", get(list))
        .route("/tree", get(tree))
        .route("/stat", get(stat))
        .route("/stats", get(stats))
        .route("/file", get(file))
        .route("/mkdir", post(mkdir))
        .route("/rename", post(rename))
        .route("/move", post(move_item))
        .route("/star", post(star))
        .route("/trash", post(trash))
        .route("/restore", post(restore))
        .route("/delete", post(delete))
        .route("/host/drives", get(host_drives))
        .route("/host/list", get(host_list))
        .route("/host/stat", get(host_stat))
        .route("/host/file", get(host_file))
        .route("/host/mkdir", post(host_mkdir))
        .route("/host/rename", post(host_rename))
        .route("/host/move", post(host_move))
        .route("/host/trash", post(host_trash))
        .route("/host/delete", post(host_delete))
        .route("/host/speedtest", post(host_speedtest))
        .route("/theater", get(theater_overview))
        .route("/theater/library/add", post(theater_library_add))
        .route("/theater/library/update", post(theater_library_update))
        .route("/theater/library/remove", post(theater_library_remove))
        .route("/theater/scan", post(theater_scan))
        .route("/theater/items", get(theater_items))
        .route("/theater/details", get(theater_details))
        .route("/theater/art", get(theater_art))
        .route("/theater/edit", post(theater_edit))
        .route("/theater/play", post(theater_play))
        .route("/theater/subtitles", post(theater_subtitles))
        .route("/steam", get(steam_overview))
        .route("/steam/art", get(steam_art))
        .route("/steam/search", get(steam_search))
        .route("/steam/local", get(steam_local))
        .route("/steam/keep", post(steam_keep))
        .route("/steam/purge", post(steam_purge))
        .route("/steam/refresh", post(steam_refresh))
        .route("/steam/enable", post(steam_enable))
        .route("/steam/dns/enable", post(steam_dns_enable))
        .route("/ai", get(ai_status))
        .route("/ai/start", post(ai_start))
        .route("/ai/stop", post(ai_stop))
        .route("/clones", get(clones_list))
        .route("/clones/enable", post(clones_enable))
        .route("/clones/screen", get(clones_screen))
        .route("/clones/live", post(clones_live))
        .route("/arcade/enable", post(arcade_enable))
        .route("/clones/automation/save", post(clones_automation_save))
        .route("/clones/automation/delete", post(clones_automation_delete))
        .route("/clones/bot/save", post(clones_bot_save))
        .route("/clones/bot/delete", post(clones_bot_delete))
        .route("/clones/machines", get(clones_machines))
        .route("/clones/machine/start", post(clones_machine_start))
        .route("/clones/machine/stop", post(clones_machine_stop))
        .route("/clones/machine/delete", post(clones_machine_delete))
        .route("/clones/start", post(clones_start))
        .route("/clones/stop", post(clones_stop))
        .route("/arcade", get(arcade_systems))
        .route("/arcade/games", get(arcade_games))
        .route("/arcade/art", get(arcade_art))
        .route("/arcade/about", get(arcade_about))
        .route("/arcade/play", post(arcade_play))
        .route("/arcade/saves", get(saves_list))
        .route("/arcade/saves/record", post(saves_record))
        .route("/arcade/saves/history", get(saves_history))
        .route("/arcade/saves/restore", post(saves_restore))
        .route("/network", get(network_reading))
        .route("/network/speedtest", post(network_speedtest))
        .route("/network/provider", get(network_provider))
        .route("/settings", get(settings_get).put(settings_put))
        .route("/box", get(box_info))
        .route("/system", get(system_vitals))
        .route("/specs", get(specs))
        .route("/security", get(security_overview))
        .route("/security/log", get(security_log))
        .route("/security/verify", post(security_verify))
        .route("/security/token/rotate", post(security_rotate))
        .route("/security/audits", get(security_audits))
        .route("/security/audits/run", post(security_audit_run))
        // Uploads are streamed straight to disk, so the usual in-memory body cap is lifted
        // for this one route only.
        .route("/upload", put(upload).layer(DefaultBodyLimit::disable()))
        .route("/host/upload", put(host_upload).layer(DefaultBodyLimit::disable()))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_token));

    // Play links carry their own permission (see theater.rs), so a video player can use them.
    let api = Router::new()
        .route("/hello", get(hello))
        .route("/media/{ticket}", get(media))
        .route("/media/{ticket}/sub/{n}", get(media_subtitle))
        .merge(protected);

    let cors = CorsLayer::new()
        .allow_origin(APP_ORIGINS.map(HeaderValue::from_static))
        .allow_methods([Method::GET, Method::POST, Method::PUT])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::RANGE])
        .expose_headers([header::CONTENT_RANGE, header::ACCEPT_RANGES, header::CONTENT_LENGTH]);

    let app = Router::new().nest("/api/v1", api);
    let app = match web {
        Some(dir) => {
            let index = dir.join("index.html");
            app.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)))
        }
        None => app,
    };
    app.layer(middleware::from_fn_with_state(state.clone(), harden)).layer(cors).with_state(state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = config();
    std::fs::create_dir_all(cfg.data.join(META_DIR).join("trash"))?;
    let root = cfg.data.canonicalize()?;
    let token = load_or_create_token(&root)?;
    let stars: BTreeSet<String> = std::fs::read(stars_file(&root))
        .ok()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or_default();

    let addr: SocketAddr = cfg.bind.parse().map_err(|_| format!("bad --bind address: {}", cfg.bind))?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let clones = match &cfg.clonewars {
        Some(dir) => {
            let dir = program_folder(dir).map_err(|_| format!("--clonewars folder not found: {}", dir.display()))?;
            if !clones::looks_like_clonewars(&dir) {
                return Err(format!("--clonewars is not a clonewars folder: {}", dir.display()).into());
            }
            Some(clones::Clones::new(dir, root.join(META_DIR).join("clones"), addr.ip().is_loopback()))
        }
        None => None,
    };
    let arcade = match &cfg.arcade {
        Some(dir) => {
            let dir = program_folder(dir).map_err(|_| format!("--arcade folder not found: {}", dir.display()))?;
            if !arcade::looks_like_arcade(&dir) {
                return Err(format!("--arcade needs a folder with Consoles/ and Roms/ in it: {}", dir.display()).into());
            }
            Some(arcade::Arcade::new(dir, addr.ip().is_loopback()))
        }
        None => None,
    };
    let security = security::Security::new(root.join(META_DIR).join("security"))?;
    security.record("box.start", &format!("castled {} on {addr}", env!("CARGO_PKG_VERSION")));
    let state = Arc::new(AppState {
        root: root.clone(),
        token: RwLock::new(token.clone()),
        security,
        stars: Mutex::new(stars),
        clones: RwLock::new(clones.map(Arc::new)),
        arcade: RwLock::new(arcade.map(Arc::new)),
        network: network::Network::new(),
        system: system::System::new(),
        saves: saves::Saves::new(root.join(META_DIR).join("saves")),
        game_notes: wiki::Shelf::open(root.join(META_DIR).join("arcade").join("about.json")),
        host: (addr.ip().is_loopback() || cfg.host_files).then(|| host::Host::new(&root)),
        drives: drives::Drives::open(root.join(META_DIR).join("drives.json"), &root),
        theater: theater::Theater::open(root.join(META_DIR).join("theater")),
        steam: Arc::new(steam::SteamCache::open(cfg.steam_cache_dir.clone().unwrap_or_else(|| root.join(META_DIR).join("steam")))),
        dns: Mutex::new(None),
        ai: ai::Ai::new(addr, &root.join(META_DIR)),
        policy: content_policy(addr),
        listen: addr.to_string(),
        loopback: addr.ip().is_loopback(),
        settings: Mutex::new(()),
    });

    // Windows canonical paths carry a verbatim prefix that only clutters the banner.
    let shown = root.display().to_string();
    println!("castled {} -- vault: {}", env!("CARGO_PKG_VERSION"), shown.trim_start_matches(r"\\?\"));
    println!("listening on http://{addr}");
    if !addr.ip().is_loopback() {
        println!("NOTE: reachable from other machines, over plain HTTP. Use only on a network you trust.");
    }
    match &cfg.web {
        Some(_) => println!("open:  http://{addr}/#connect={token}"),
        None => println!("token: {token}   (paste into the Castle app's Connect screen)"),
    }

    // Watches for drives going in and out, so each one's plug-in time is right even when
    // no one has Storage open.
    let watcher = state.clone();
    tokio::spawn(async move {
        let mut every = tokio::time::interval(DRIVE_WATCH);
        loop {
            every.tick().await;
            let state = watcher.clone();
            let _ = tokio::task::spawn_blocking(move || state.drives.tick()).await;
        }
    });

    // The Steam cache and its DNS server, when asked for on the command line or turned on
    // from the app before (remembered in the box settings).
    let saved = read_settings(&root);
    let remembered = |key: &str| saved.get(key).and_then(|v| v.as_str()).map(str::to_string);
    if let Some(addr) = cfg.steam_cache.clone().or_else(|| remembered(STEAM_CACHE_SETTING)) {
        match addr.parse::<SocketAddr>() {
            Ok(addr) => {
                if let Err(msg) = start_steam_cache(&state, addr).await {
                    eprintln!("[castled] {msg}");
                }
            }
            Err(_) => eprintln!("[castled] bad --steam-cache address: {addr}"),
        }
    }
    if let Some(addr) = cfg.dns.clone().or_else(|| remembered(DNS_SETTING)) {
        match addr.parse::<SocketAddr>() {
            Ok(addr) => {
                if let Err(msg) = start_dns(&state, addr).await {
                    eprintln!("[castled] {msg}");
                }
            }
            Err(_) => eprintln!("[castled] bad --dns address: {addr}"),
        }
    }
    let counter = state.clone();
    std::thread::spawn(move || counter.steam.recount());
    let steam_keeper = state.clone();
    tokio::spawn(async move {
        let mut save = tokio::time::interval(std::time::Duration::from_secs(15));
        let mut last_refresh: Option<std::time::Instant> = None;
        loop {
            save.tick().await;
            let state = steam_keeper.clone();
            let refresh = last_refresh.is_none_or(|at| at.elapsed() >= STEAM_REFRESH);
            if refresh {
                last_refresh = Some(std::time::Instant::now());
            }
            let _ = tokio::task::spawn_blocking(move || {
                state.steam.save_if_due();
                if refresh {
                    state.steam.refresh(&steam::local_games());
                }
            })
            .await;
        }
    });

    // DOGS AI starts with the box when a copy of it is there and its app is built.
    let ai_state = state.clone();
    std::thread::spawn(move || {
        let status = ai_state.ai.status();
        if status.dir.is_some() && status.built && status.node && !status.running {
            if let Err(msg) = ai_state.ai.start() {
                eprintln!("[castled] {msg}");
            }
        }
    });

    // Theater walks its libraries once at start, then every hour, to pick up new files.
    let scanner = state.clone();
    tokio::spawn(async move {
        let mut every = tokio::time::interval(THEATER_RESCAN);
        loop {
            every.tick().await;
            let state = scanner.clone();
            let _ = tokio::task::spawn_blocking(move || state.theater.scan(None)).await;
        }
    });

    let stopping = state.clone();
    axum::serve(listener, router(state, cfg.web))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    // The dogs server goes down with the box.
    stopping.ai.stop();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_for_programs_lose_the_windows_long_form() {
        assert_eq!(plain_path(Path::new(r"\\?\E:\Games")), PathBuf::from(r"E:\Games"));
        assert_eq!(plain_path(Path::new(r"E:\Games")), PathBuf::from(r"E:\Games"));
        assert_eq!(plain_path(Path::new("/srv/games")), PathBuf::from("/srv/games"));
        // A network path in the long form has no drive letter and is left as it is.
        assert_eq!(plain_path(Path::new(r"\\?\UNC\box\share")), PathBuf::from(r"\\?\UNC\box\share"));
    }

    #[test]
    fn names_that_could_escape_or_hide_are_refused() {
        for bad in ["", ".", "..", ".castle", ".env", "a/b", "a\\b", "c:", "x\0y", "trail ", "dot.", "NUL", "con.txt"] {
            assert!(!valid_name(bad), "{bad:?} should be refused");
        }
        for good in ["a.txt", "New folder", "photo (2).jpg", "résumé.pdf", "console.log"] {
            assert!(valid_name(good), "{good:?} should be accepted");
        }
    }

    #[test]
    fn ids_cannot_climb_out_of_the_vault() {
        for bad in ["..", "../x", "a/../../x", "/etc/passwd", "a//b", "a/", ".castle/token", "C:/Windows"] {
            assert!(split_id(bad).is_err(), "{bad:?} should be refused");
        }
        assert_eq!(split_id("").unwrap().len(), 0);
        assert_eq!(split_id("a/b/c.txt").unwrap(), vec!["a", "b", "c.txt"]);
    }

    #[test]
    fn trash_keys_are_only_our_own_hex() {
        assert!(trash_key("trash:0123456789abcdef0123456789abcdef").is_ok());
    }

    #[test]
    fn settings_are_named_plainly_and_hold_only_simple_values() {
        use serde_json::json;
        assert!(valid_setting("storage.trashDays", &json!(30)));
        assert!(valid_setting("box.name", &json!("Home box")));
        assert!(valid_setting("remote.enabled", &json!(true)));
        for (key, value) in [("", json!(1)), ("../x", json!(1)), ("a b", json!(1)), ("ok", json!({"a": 1})), ("ok", json!([1])), ("ok", json!(null)), ("ok", json!("line
break"))] {
            assert!(!valid_setting(key, &value), "{key:?} = {value} should be refused");
        }
        for bad in ["trash:", "trash:../x", "trash:0123", "0123456789abcdef0123456789abcdef", "trash:0123456789abcdef0123456789abcdeg"] {
            assert!(trash_key(bad).is_err(), "{bad:?} should be refused");
        }
    }

    #[test]
    fn a_taken_name_gets_a_number_not_an_overwrite() {
        let dir = std::env::temp_dir().join(format!("castled-test-{}", random_hex(8)));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(free_name(&dir, "a.txt"), "a.txt");
        std::fs::write(dir.join("a.txt"), b"1").unwrap();
        assert_eq!(free_name(&dir, "a.txt"), "a (2).txt");
        std::fs::write(dir.join("a (2).txt"), b"2").unwrap();
        assert_eq!(free_name(&dir, "a.txt"), "a (3).txt");
        std::fs::create_dir(dir.join("New folder")).unwrap();
        assert_eq!(free_name(&dir, "New folder"), "New folder (2)");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
