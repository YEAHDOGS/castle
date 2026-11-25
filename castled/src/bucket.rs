//! A cloud bucket in Storage: files kept with a storage provider, beside the vault.
//!
//! Any bucket that speaks the S3 API works (Cloudflare R2, Backblaze B2, MinIO, Amazon S3).
//! The box is given the bucket's address and a pair of keys once (Storage > Bucket), keeps
//! them in the vault's private folder, and from then on lists, uploads, renames, moves and
//! deletes there for the app. The keys never leave the box again: no call returns them, and
//! the browser never talks to the provider itself.
//!
//! A bucket has no folders, only names with slashes in them ("music/2026/song.mp3"). This
//! shows them as folders all the same. An item's id is its whole name; a folder's id ends
//! with a slash ("music/2026/"), and the top of the bucket is "".
//!
//! Rules kept here, the same ones the vault keeps:
//!   * No overwrites. An upload whose name is taken becomes "name (2)"; a rename or move
//!     onto a taken name is refused. Only `write` replaces a file, and it is for small text
//!     files the app edits in place (a catalog).
//!   * Whole files only. An upload is one request to the provider, which keeps nothing of
//!     a dropped one.
//!   * There is no trash in a bucket. A delete is for good, and the app asks first.

use std::{
    path::{Path, PathBuf},
    sync::{Mutex, RwLock},
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use chrono::{DateTime, Utc};
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CONFIG_FILE: &str = "bucket.json";
const SERVICE: &str = "s3";
const ALGORITHM: &str = "AWS4-HMAC-SHA256";
const UNSIGNED: &str = "UNSIGNED-PAYLOAD";
const DEFAULT_REGION: &str = "auto";
const AMZ_DATE: &str = "%Y%m%dT%H%M%SZ";
const SHA256_BLOCK: usize = 64;
/// How long the provider gets to start answering a small call.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// The longest name a provider accepts for one object.
const MAX_KEY_LEN: usize = 1024;
const MAX_NAME_LEN: usize = 255;
const MAX_FIELD_LEN: usize = 300;
/// One listing page from the provider.
const PAGE_SIZE: usize = 1000;
/// A folder shows at most this many pages of items.
const MAX_LIST_PAGES: usize = 20;
/// Adding up the whole bucket stops after this many pages.
const MAX_USAGE_PAGES: usize = 500;
/// A folder with more files than this is not renamed, moved or deleted in one go.
const MAX_FOLDER_OBJECTS: usize = 1000;
/// How many "name (n)" are tried before an upload gives up.
const MAX_NAME_TRIES: usize = 50;
/// The largest file `write` replaces: it is for text the app edits, not for uploads.
pub const MAX_WRITE_BYTES: usize = 1024 * 1024;
/// How long the bucket's total size is remembered before it is added up again.
const USAGE_FRESH: Duration = Duration::from_secs(10 * 60);
const KEY_ID_SHOWN: usize = 4;
const BYTES_PER_GB: u64 = 1024 * 1024 * 1024;
const NOT_CONNECTED: &str = "No bucket is connected. Connect one in Storage > Bucket.";
const UNREACHABLE: &str = "The bucket could not be reached.";
const KEYS_REFUSED: &str = "The bucket refused those keys.";
const FOLDER_TOO_BIG: &str = "That folder holds more than 1,000 files. Do it in smaller pieces.";
const NAME_TAKEN: &str = "that name is already taken";
/// Characters a signed address keeps as they are; everything else is percent-encoded.
const UNRESERVED: &[u8] = b"-._~";

/// Why a bucket call failed, in words fit to show.
#[derive(Debug, PartialEq)]
pub enum Fail {
    NotFound,
    Bad(String),
    Conflict(String),
    /// The provider said no to the keys, or to this action with them.
    Refused(String),
    /// The provider could not be reached or answered with something unexpected.
    Upstream(String),
}

pub type BucketResult<T> = Result<T, Fail>;

fn bad(msg: &str) -> Fail {
    Fail::Bad(msg.to_string())
}

/// What the box keeps about the bucket. The secret is written to disk and nowhere else.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Config {
    /// "https://<account>.r2.cloudflarestorage.com": the provider's S3 address, no path.
    endpoint: String,
    bucket: String,
    region: String,
    access_key_id: String,
    secret_access_key: String,
    /// Where the bucket's files are public, if they are ("https://data.example.com").
    #[serde(default)]
    public_url: String,
    /// How much the bucket may hold, in bytes; 0 when no limit was given.
    #[serde(default)]
    quota: u64,
}

/// What the app sends to connect a bucket.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectBody {
    endpoint: String,
    bucket: String,
    access_key_id: String,
    secret_access_key: String,
    #[serde(default)]
    region: String,
    #[serde(default)]
    public_url: String,
    #[serde(default)]
    quota_gb: f64,
}

/// What the app is told about the connection. Never the secret.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub connected: bool,
    pub endpoint: String,
    pub bucket: String,
    pub public_url: String,
    /// The end of the key id, to tell two keys apart.
    pub key_id: String,
    pub quota: u64,
}

#[derive(Serialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BucketNode {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub parent_id: String,
    pub mime: String,
    pub size: u64,
    pub modified: i64,
    /// The file's public address, when the bucket has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
pub struct Usage {
    pub used: u64,
    pub objects: u64,
    pub quota: u64,
    /// False when the bucket holds more than was counted.
    pub complete: bool,
}

pub struct Bucket {
    file: PathBuf,
    http: reqwest::Client,
    config: RwLock<Option<Config>>,
    usage: Mutex<Option<(Instant, Usage)>>,
}

// ---------------------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------------------

/// One part of an object's name: what the app calls a file or folder name.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && name != "."
        && name != ".."
        && name.trim() == name
        && !name.chars().any(|c| c == '/' || c == '\\' || c.is_control())
}

/// A whole name: parts joined by single slashes, with no way to mean anything but itself.
/// "" is the top of the bucket; a folder ends with a slash.
fn valid_key(key: &str) -> bool {
    if key.is_empty() {
        return true;
    }
    key.len() <= MAX_KEY_LEN && key.strip_suffix('/').unwrap_or(key).split('/').all(valid_name)
}

fn checked(key: &str) -> BucketResult<&str> {
    if valid_key(key) { Ok(key) } else { Err(bad("invalid name")) }
}

fn is_folder(key: &str) -> bool {
    key.is_empty() || key.ends_with('/')
}

/// The folder an item is in: "a/b/c.txt" and "a/b/c/" are both in "a/b/".
fn parent_of(key: &str) -> &str {
    let inner = key.strip_suffix('/').unwrap_or(key);
    inner.rfind('/').map_or("", |at| &key[..=at])
}

fn name_of(key: &str) -> &str {
    let inner = key.strip_suffix('/').unwrap_or(key);
    inner.rsplit('/').next().unwrap_or(inner)
}

/// "song.mp3" -> "song (2).mp3", for the nth try at a free name.
fn numbered(name: &str, n: usize) -> String {
    if n < 2 {
        return name.to_string();
    }
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem} ({n}).{ext}"),
        _ => format!("{name} ({n})"),
    }
}

// ---------------------------------------------------------------------------------------
// Signing (AWS Signature Version 4, which every S3-compatible provider checks)
// ---------------------------------------------------------------------------------------

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hmac(key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut block = [0u8; SHA256_BLOCK];
    if key.len() > SHA256_BLOCK {
        let digest = Sha256::digest(key);
        block[..digest.len()].copy_from_slice(&digest);
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(block.map(|b| b ^ 0x36));
    inner.update(message);
    let mut outer = Sha256::new();
    outer.update(block.map(|b| b ^ 0x5c));
    outer.update(inner.finalize());
    outer.finalize().to_vec()
}

/// Percent-encodes for a signed address. A path keeps its slashes; a query value does not.
fn encode(text: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || UNRESERVED.contains(&byte) || (keep_slash && byte == b'/') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn canonical_query(query: &[(&str, String)]) -> String {
    let mut pairs: Vec<String> = query.iter().map(|(k, v)| format!("{}={}", encode(k, false), encode(v, false))).collect();
    pairs.sort();
    pairs.join("&")
}

/// One request, as the signature sees it.
struct Signing<'a> {
    secret: &'a str,
    region: &'a str,
    method: &'a str,
    /// Already percent-encoded, starting with a slash.
    path: &'a str,
    query: &'a str,
    /// Every header that is signed, lower-case names. Sorted here.
    headers: Vec<(String, String)>,
    payload_hash: &'a str,
    /// "20130524T000000Z"
    amz_date: &'a str,
}

/// The signature and the list of signed header names.
fn signature(mut s: Signing) -> (String, String) {
    s.headers.sort();
    let date = &s.amz_date[..8];
    let names: Vec<&str> = s.headers.iter().map(|(name, _)| name.as_str()).collect();
    let signed = names.join(";");
    let lines: String = s.headers.iter().map(|(name, value)| format!("{name}:{}\n", value.trim())).collect();
    let request = format!("{}\n{}\n{}\n{lines}\n{signed}\n{}", s.method, s.path, s.query, s.payload_hash);
    let scope = format!("{date}/{}/{SERVICE}/aws4_request", s.region);
    let to_sign = format!("{ALGORITHM}\n{}\n{scope}\n{}", s.amz_date, hex(&Sha256::digest(request.as_bytes())));
    let mut key = hmac(format!("AWS4{}", s.secret).as_bytes(), date.as_bytes());
    for part in [s.region, SERVICE, "aws4_request"] {
        key = hmac(&key, part.as_bytes());
    }
    (hex(&hmac(&key, to_sign.as_bytes())), signed)
}

// ---------------------------------------------------------------------------------------
// What the provider answers
// ---------------------------------------------------------------------------------------

#[derive(Debug, PartialEq, Default)]
struct Object {
    key: String,
    size: u64,
    modified: i64,
}

#[derive(Debug, PartialEq, Default)]
struct Page {
    objects: Vec<Object>,
    folders: Vec<String>,
    next: Option<String>,
}

/// "&amp;" and its kin, which the reader hands over as a name ("amp", "#38", "#x26").
fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let code = name.strip_prefix('#')?;
            let number = match code.strip_prefix('x') {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => code.parse().ok()?,
            };
            char::from_u32(number)
        }
    }
}

fn millis(stamp: &str) -> i64 {
    DateTime::parse_from_rfc3339(stamp.trim()).map(|t| t.timestamp_millis()).unwrap_or(0)
}

/// Reads one page of a listing (ListObjectsV2).
fn parse_listing(xml: &[u8]) -> BucketResult<Page> {
    let damaged = || Fail::Upstream("The bucket sent a listing that could not be read.".into());
    let mut reader = quick_xml::Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut page = Page::default();
    let mut object = Object::default();
    let mut open: Vec<String> = Vec::new();
    let mut text = String::new();
    loop {
        match reader.read_event_into(&mut buf).map_err(|_| damaged())? {
            Event::Start(e) => {
                open.push(e.name().as_ref().to_string());
                text.clear();
            }
            Event::Text(t) => text.push_str(t.as_ref()),
            Event::GeneralRef(r) => text.extend(entity(r.as_ref())),
            Event::End(_) => {
                let name = open.pop().ok_or_else(damaged)?;
                let inside = open.last().map(String::as_str).unwrap_or("");
                match (name.as_str(), inside) {
                    ("Key", "Contents") => object.key = std::mem::take(&mut text),
                    ("Size", "Contents") => object.size = text.trim().parse().unwrap_or(0),
                    ("LastModified", "Contents") => object.modified = millis(&text),
                    ("Contents", _) => page.objects.push(std::mem::take(&mut object)),
                    ("Prefix", "CommonPrefixes") => page.folders.push(std::mem::take(&mut text)),
                    ("NextContinuationToken", _) => page.next = Some(std::mem::take(&mut text)).filter(|t| !t.is_empty()),
                    _ => {}
                }
                text.clear();
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(page)
}

/// A failed call's status, as a reason fit to show.
fn failure(status: reqwest::StatusCode) -> Fail {
    match status.as_u16() {
        404 => Fail::NotFound,
        401 | 403 => Fail::Refused(KEYS_REFUSED.into()),
        412 => Fail::Conflict(NAME_TAKEN.into()),
        code => Fail::Upstream(format!("The bucket answered with an error ({code}).")),
    }
}

// ---------------------------------------------------------------------------------------
// The bucket
// ---------------------------------------------------------------------------------------

/// "https://host[:port]" with nothing after it. Plain http only for a provider on this computer.
fn clean_endpoint(raw: &str) -> BucketResult<String> {
    let text = raw.trim().trim_end_matches('/');
    let url = reqwest::Url::parse(text).map_err(|_| bad("That is not a web address."))?;
    let host = url.host_str().ok_or_else(|| bad("That is not a web address."))?;
    let local = matches!(host, "127.0.0.1" | "localhost" | "[::1]");
    let secure = url.scheme() == "https" || (url.scheme() == "http" && local);
    if !secure || url.path() != "/" || url.query().is_some() || !url.username().is_empty() {
        return Err(bad("The address must be https://, with nothing after the name."));
    }
    Ok(text.to_string())
}

/// A public address for the bucket's files, or "" when it has none.
fn clean_public_url(raw: &str) -> BucketResult<String> {
    let text = raw.trim().trim_end_matches('/');
    if text.is_empty() {
        return Ok(String::new());
    }
    let ok = reqwest::Url::parse(text).is_ok_and(|url| url.scheme() == "https" && url.query().is_none());
    if ok { Ok(text.to_string()) } else { Err(bad("The public address must start with https://")) }
}

fn valid_bucket_name(name: &str) -> bool {
    (3..=63).contains(&name.len()) && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
}

fn plain_field(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_FIELD_LEN && !text.chars().any(|c| c.is_control() || c.is_whitespace())
}

impl Config {
    fn from_body(b: ConnectBody) -> BucketResult<Self> {
        let bucket = b.bucket.trim().to_string();
        if !valid_bucket_name(&bucket) {
            return Err(bad("That is not a bucket name."));
        }
        let (id, secret) = (b.access_key_id.trim().to_string(), b.secret_access_key.trim().to_string());
        if !plain_field(&id) || !plain_field(&secret) {
            return Err(bad("Both keys are needed."));
        }
        let region = b.region.trim();
        if !region.is_empty() && !plain_field(region) {
            return Err(bad("That is not a region."));
        }
        let quota = if b.quota_gb.is_finite() && b.quota_gb > 0.0 { (b.quota_gb * BYTES_PER_GB as f64) as u64 } else { 0 };
        Ok(Self {
            endpoint: clean_endpoint(&b.endpoint)?,
            bucket,
            region: if region.is_empty() { DEFAULT_REGION.into() } else { region.into() },
            access_key_id: id,
            secret_access_key: secret,
            public_url: clean_public_url(&b.public_url)?,
            quota,
        })
    }

    fn status(&self) -> Status {
        let shown = self.access_key_id.len().saturating_sub(KEY_ID_SHOWN);
        Status {
            connected: true,
            endpoint: self.endpoint.clone(),
            bucket: self.bucket.clone(),
            public_url: self.public_url.clone(),
            key_id: self.access_key_id.chars().skip(shown).collect(),
            quota: self.quota,
        }
    }

    fn public(&self, key: &str) -> Option<String> {
        (!self.public_url.is_empty()).then(|| format!("{}/{}", self.public_url, encode(key, true)))
    }

    fn file_node(&self, key: &str, size: u64, modified: i64) -> BucketNode {
        BucketNode {
            id: key.to_string(),
            name: name_of(key).to_string(),
            kind: "file",
            parent_id: parent_of(key).to_string(),
            mime: mime_guess::from_path(name_of(key)).first_or_octet_stream().to_string(),
            size,
            modified,
            url: self.public(key),
        }
    }

    fn folder_node(&self, key: &str) -> BucketNode {
        BucketNode {
            id: key.to_string(),
            name: name_of(key).to_string(),
            kind: "folder",
            parent_id: parent_of(key).to_string(),
            mime: String::new(),
            size: 0,
            modified: 0,
            url: None,
        }
    }
}

/// One call to the provider, before it is signed.
struct Call<'a> {
    method: reqwest::Method,
    key: &'a str,
    query: Vec<(&'a str, String)>,
    /// Signed "x-amz-..." headers, lower-case.
    amz: Vec<(&'a str, String)>,
    /// Headers sent as they are, unsigned (Range, Content-Type, Content-Length).
    plain: Vec<(header::HeaderName, String)>,
}

impl<'a> Call<'a> {
    fn new(method: reqwest::Method, key: &'a str) -> Self {
        Self { method, key, query: Vec::new(), amz: Vec::new(), plain: Vec::new() }
    }
}

impl Bucket {
    /// The bucket whose keys, if any, are in `meta` (the vault's private folder).
    pub fn open(meta: &Path) -> Self {
        let file = meta.join(CONFIG_FILE);
        let config = std::fs::read(&file).ok().and_then(|raw| serde_json::from_slice(&raw).ok());
        let http = reqwest::Client::builder().connect_timeout(CONNECT_TIMEOUT).build().unwrap_or_default();
        Self { file, http, config: RwLock::new(config), usage: Mutex::new(None) }
    }

    fn config(&self) -> BucketResult<Config> {
        self.config.read().ok().and_then(|slot| slot.clone()).ok_or_else(|| Fail::Conflict(NOT_CONNECTED.into()))
    }

    fn forget_usage(&self) {
        if let Ok(mut usage) = self.usage.lock() {
            *usage = None;
        }
    }

    pub fn status(&self) -> Status {
        match self.config() {
            Ok(config) => config.status(),
            Err(_) => Status { connected: false, endpoint: String::new(), bucket: String::new(), public_url: String::new(), key_id: String::new(), quota: 0 },
        }
    }

    /// Tries the keys with one listing, and keeps them only if the provider accepts them.
    pub async fn connect(&self, body: ConnectBody) -> BucketResult<Status> {
        let config = Config::from_body(body)?;
        self.page(&config, "", true, None, 1).await.map_err(|fail| match fail {
            Fail::NotFound => bad("There is no bucket with that name at that address."),
            other => other,
        })?;
        let saved = serde_json::to_vec_pretty(&config).unwrap_or_default();
        write_private(&self.file, &saved).map_err(|_| Fail::Upstream("The keys could not be saved on the box.".into()))?;
        let status = config.status();
        if let Ok(mut slot) = self.config.write() {
            *slot = Some(config);
        }
        self.forget_usage();
        Ok(status)
    }

    /// Forgets the keys. Nothing in the bucket changes.
    pub fn disconnect(&self) -> BucketResult<()> {
        if let Ok(mut slot) = self.config.write() {
            *slot = None;
        }
        self.forget_usage();
        match std::fs::remove_file(&self.file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(Fail::Upstream("The keys could not be removed from the box.".into())),
            _ => Ok(()),
        }
    }

    fn request(&self, config: &Config, call: Call) -> reqwest::RequestBuilder {
        let path = format!("/{}/{}", config.bucket, encode(call.key, true));
        let query = canonical_query(&call.query);
        let host = config.endpoint.split_once("://").map_or(config.endpoint.as_str(), |(_, host)| host);
        let amz_date = Utc::now().format(AMZ_DATE).to_string();
        let mut headers: Vec<(String, String)> = call.amz.iter().map(|(name, value)| (name.to_string(), value.clone())).collect();
        headers.push(("host".into(), host.to_string()));
        headers.push(("x-amz-content-sha256".into(), UNSIGNED.into()));
        headers.push(("x-amz-date".into(), amz_date.clone()));
        let (signature, signed) = signature(Signing {
            secret: &config.secret_access_key,
            region: &config.region,
            method: call.method.as_str(),
            path: &path,
            query: &query,
            headers: headers.clone(),
            payload_hash: UNSIGNED,
            amz_date: &amz_date,
        });
        let scope = format!("{}/{}/{SERVICE}/aws4_request", &amz_date[..8], config.region);
        let authorization = format!("{ALGORITHM} Credential={}/{scope}, SignedHeaders={signed}, Signature={signature}", config.access_key_id);
        let address = if query.is_empty() { format!("{}{path}", config.endpoint) } else { format!("{}{path}?{query}", config.endpoint) };
        let mut request = self.http.request(call.method, address).header(header::AUTHORIZATION.as_str(), authorization);
        for (name, value) in headers.iter().filter(|(name, _)| name != "host") {
            request = request.header(name.as_str(), value.as_str());
        }
        for (name, value) in call.plain {
            request = request.header(name.as_str(), value);
        }
        request
    }

    /// Sends a call and hands back the answer if the provider said yes.
    async fn send(&self, request: reqwest::RequestBuilder) -> BucketResult<reqwest::Response> {
        let answer = request.send().await.map_err(|_| Fail::Upstream(UNREACHABLE.into()))?;
        if answer.status().is_success() { Ok(answer) } else { Err(failure(answer.status())) }
    }

    async fn small(&self, config: &Config, call: Call<'_>) -> BucketResult<reqwest::Response> {
        self.send(self.request(config, call).timeout(CALL_TIMEOUT)).await
    }

    /// One page of names starting with `prefix`: with `folders`, only what is directly in it.
    async fn page(&self, config: &Config, prefix: &str, folders: bool, after: Option<&str>, max: usize) -> BucketResult<Page> {
        let mut call = Call::new(reqwest::Method::GET, "");
        call.query = vec![("list-type", "2".into()), ("max-keys", max.to_string()), ("prefix", prefix.into())];
        if folders {
            call.query.push(("delimiter", "/".into()));
        }
        if let Some(token) = after {
            call.query.push(("continuation-token", token.into()));
        }
        let answer = self.small(config, call).await?;
        let xml = answer.bytes().await.map_err(|_| Fail::Upstream(UNREACHABLE.into()))?;
        parse_listing(&xml)
    }

    /// What is directly inside a folder.
    pub async fn list(&self, folder: &str) -> BucketResult<Vec<BucketNode>> {
        let config = self.config()?;
        if !is_folder(checked(folder)?) {
            return Err(bad("that is not a folder"));
        }
        let mut nodes = Vec::new();
        let mut after: Option<String> = None;
        for _ in 0..MAX_LIST_PAGES {
            let page = self.page(&config, folder, true, after.as_deref(), PAGE_SIZE).await?;
            nodes.extend(page.folders.iter().filter(|key| valid_key(key)).map(|key| config.folder_node(key)));
            // The folder's own marker (see mkdir) is not something in it.
            nodes.extend(page.objects.iter().filter(|o| o.key != folder && !is_folder(&o.key) && valid_key(&o.key)).map(|o| config.file_node(&o.key, o.size, o.modified)));
            after = page.next;
            if after.is_none() {
                break;
            }
        }
        Ok(nodes)
    }

    /// Every object under a folder, or an error when there are too many to act on at once.
    async fn everything_in(&self, config: &Config, folder: &str) -> BucketResult<Vec<Object>> {
        let mut all = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let page = self.page(config, folder, false, after.as_deref(), PAGE_SIZE).await?;
            all.extend(page.objects);
            if all.len() > MAX_FOLDER_OBJECTS {
                return Err(Fail::Conflict(FOLDER_TOO_BIG.into()));
            }
            after = page.next;
            if after.is_none() {
                return Ok(all);
            }
        }
    }

    async fn exists(&self, config: &Config, key: &str) -> BucketResult<bool> {
        if is_folder(key) {
            return Ok(!self.page(config, key, false, None, 1).await?.objects.is_empty());
        }
        match self.small(config, Call::new(reqwest::Method::HEAD, key)).await {
            Ok(_) => Ok(true),
            Err(Fail::NotFound) => Ok(false),
            Err(other) => Err(other),
        }
    }

    pub async fn stat(&self, key: &str) -> BucketResult<BucketNode> {
        let config = self.config()?;
        self.stat_with(&config, checked(key)?).await
    }

    async fn stat_with(&self, config: &Config, key: &str) -> BucketResult<BucketNode> {
        if key.is_empty() {
            return Err(Fail::NotFound);
        }
        if is_folder(key) {
            return if self.exists(config, key).await? { Ok(config.folder_node(key)) } else { Err(Fail::NotFound) };
        }
        let answer = self.small(config, Call::new(reqwest::Method::HEAD, key)).await?;
        let size = answer.headers().get(header::CONTENT_LENGTH.as_str()).and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(0);
        let modified = answer
            .headers()
            .get(header::LAST_MODIFIED.as_str())
            .and_then(|v| v.to_str().ok())
            .and_then(|v| DateTime::parse_from_rfc2822(v).ok())
            .map_or(0, |t| t.timestamp_millis());
        Ok(config.file_node(key, size, modified))
    }

    /// A file's bytes, passed straight through from the provider. `range` asks for a part.
    pub async fn file(&self, key: &str, range: Option<&HeaderValue>) -> BucketResult<Response> {
        let config = self.config()?;
        if is_folder(checked(key)?) {
            return Err(Fail::NotFound);
        }
        let mut call = Call::new(reqwest::Method::GET, key);
        if let Some(range) = range.and_then(|v| v.to_str().ok()) {
            call.plain.push((header::RANGE, range.to_string()));
        }
        let answer = self.send(self.request(&config, call)).await?;
        let status = StatusCode::from_u16(answer.status().as_u16()).unwrap_or(StatusCode::OK);
        let passed = [header::CONTENT_LENGTH, header::CONTENT_RANGE, header::ACCEPT_RANGES];
        let kept: Vec<(header::HeaderName, HeaderValue)> = passed
            .into_iter()
            .filter_map(|name| {
                let value = answer.headers().get(name.as_str()).and_then(|v| HeaderValue::from_bytes(v.as_bytes()).ok())?;
                Some((name, value))
            })
            .collect();
        let mut res = Response::new(Body::from_stream(answer.bytes_stream()));
        *res.status_mut() = status;
        let headers = res.headers_mut();
        headers.extend(kept);
        // The type comes from the name, as for every file the box serves, and it is a
        // download: nothing kept in a bucket is shown or run as a page of the box.
        if let Ok(mime) = HeaderValue::from_str(mime_guess::from_path(name_of(key)).first_or_octet_stream().as_ref()) {
            headers.insert(header::CONTENT_TYPE, mime);
        }
        headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
        headers.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment"));
        Ok(res)
    }

    /// A folder is a name ending in a slash with nothing in it, so that it shows while empty.
    pub async fn mkdir(&self, parent: &str, name: &str) -> BucketResult<BucketNode> {
        let config = self.config()?;
        if !is_folder(checked(parent)?) || !valid_name(name) {
            return Err(bad("invalid name"));
        }
        let mut made = None;
        for n in 1..=MAX_NAME_TRIES {
            let key = format!("{parent}{}/", numbered(name, n));
            let file_twin = key.trim_end_matches('/');
            if !self.exists(&config, &key).await? && !self.exists(&config, file_twin).await? {
                made = Some(key);
                break;
            }
        }
        let key = made.ok_or_else(|| Fail::Conflict(NAME_TAKEN.into()))?;
        let mut call = Call::new(reqwest::Method::PUT, checked(&key)?);
        call.plain.push((header::CONTENT_LENGTH, "0".into()));
        self.small(&config, call).await?;
        self.forget_usage();
        Ok(config.folder_node(&key))
    }

    async fn copy(&self, config: &Config, from: &str, to: &str) -> BucketResult<()> {
        let mut call = Call::new(reqwest::Method::PUT, to);
        call.amz.push(("x-amz-copy-source", format!("/{}/{}", config.bucket, encode(from, true))));
        call.plain.push((header::CONTENT_LENGTH, "0".into()));
        self.send(self.request(config, call)).await.map(|_| ())
    }

    async fn remove(&self, config: &Config, key: &str) -> BucketResult<()> {
        self.small(config, Call::new(reqwest::Method::DELETE, key)).await.map(|_| ())
    }

    /// Gives an item a new whole name. A bucket cannot rename: each file is copied to the
    /// new name, and the old ones are removed only once every copy is there.
    async fn relocate(&self, config: &Config, from: &str, to: &str) -> BucketResult<BucketNode> {
        if from == to {
            return self.stat_with(config, from).await;
        }
        checked(to)?;
        if is_folder(from) && to.starts_with(from) {
            return Err(bad("a folder cannot be moved into itself"));
        }
        // A file and a folder may not share a name: the app could not tell them apart.
        let twin = if is_folder(to) { to.trim_end_matches('/').to_string() } else { format!("{to}/") };
        if self.exists(config, to).await? || self.exists(config, &twin).await? {
            return Err(Fail::Conflict(NAME_TAKEN.into()));
        }
        if !is_folder(from) {
            self.copy(config, from, to).await?;
            self.remove(config, from).await?;
        } else {
            let inside = self.everything_in(config, from).await?;
            if inside.is_empty() {
                return Err(Fail::NotFound);
            }
            for object in &inside {
                let moved = format!("{to}{}", &object.key[from.len()..]);
                self.copy(config, &object.key, checked(&moved)?).await?;
            }
            for object in &inside {
                self.remove(config, &object.key).await?;
            }
        }
        self.forget_usage();
        self.stat_with(config, to).await
    }

    pub async fn rename(&self, key: &str, name: &str) -> BucketResult<BucketNode> {
        let config = self.config()?;
        if checked(key)?.is_empty() || !valid_name(name) {
            return Err(bad("invalid name"));
        }
        let slash = if is_folder(key) { "/" } else { "" };
        self.relocate(&config, key, &format!("{}{name}{slash}", parent_of(key))).await
    }

    pub async fn move_to(&self, key: &str, folder: &str) -> BucketResult<BucketNode> {
        let config = self.config()?;
        if checked(key)?.is_empty() || !is_folder(checked(folder)?) {
            return Err(bad("invalid name"));
        }
        let slash = if is_folder(key) { "/" } else { "" };
        self.relocate(&config, key, &format!("{folder}{}{slash}", name_of(key))).await
    }

    /// Removes a file, or a folder and everything in it. There is no way back.
    pub async fn delete(&self, key: &str) -> BucketResult<()> {
        let config = self.config()?;
        if checked(key)?.is_empty() {
            return Err(bad("invalid name"));
        }
        if is_folder(key) {
            for object in self.everything_in(&config, key).await? {
                self.remove(&config, &object.key).await?;
            }
        } else {
            self.remove(&config, key).await?;
        }
        self.forget_usage();
        Ok(())
    }

    /// Sends a file of `length` bytes into a folder as it arrives, under a name not in use.
    pub async fn upload(&self, parent: &str, name: &str, length: u64, body: Body) -> BucketResult<BucketNode> {
        let config = self.config()?;
        if !is_folder(checked(parent)?) || !valid_name(name) {
            return Err(bad("invalid name"));
        }
        let mut free = None;
        for n in 1..=MAX_NAME_TRIES {
            let key = format!("{parent}{}", numbered(name, n));
            if !self.exists(&config, &key).await? && !self.exists(&config, &format!("{key}/")).await? {
                free = Some(key);
                break;
            }
        }
        let key = free.ok_or_else(|| Fail::Conflict(NAME_TAKEN.into()))?;
        let mut call = Call::new(reqwest::Method::PUT, checked(&key)?);
        call.plain.push((header::CONTENT_LENGTH, length.to_string()));
        call.plain.push((header::CONTENT_TYPE, mime_guess::from_path(name).first_or_octet_stream().to_string()));
        let sending = self.request(&config, call).body(reqwest::Body::wrap_stream(body.into_data_stream()));
        self.send(sending).await?;
        self.forget_usage();
        self.stat_with(&config, &key).await
    }

    /// Replaces a small text file, or makes it. The one call that overwrites.
    pub async fn write(&self, key: &str, bytes: Vec<u8>) -> BucketResult<BucketNode> {
        let config = self.config()?;
        if is_folder(checked(key)?) || bytes.len() > MAX_WRITE_BYTES {
            return Err(bad("invalid name"));
        }
        let mut call = Call::new(reqwest::Method::PUT, key);
        call.plain.push((header::CONTENT_TYPE, mime_guess::from_path(name_of(key)).first_or_octet_stream().to_string()));
        self.send(self.request(&config, call).timeout(CALL_TIMEOUT).body(bytes)).await?;
        self.forget_usage();
        self.stat_with(&config, key).await
    }

    /// How much the whole bucket holds. Added up from a full listing, then remembered a while.
    pub async fn usage(&self) -> BucketResult<Usage> {
        let config = self.config()?;
        if let Some((at, usage)) = self.usage.lock().ok().and_then(|slot| *slot)
            && at.elapsed() < USAGE_FRESH
        {
            return Ok(usage);
        }
        let mut usage = Usage { used: 0, objects: 0, quota: config.quota, complete: false };
        let mut after: Option<String> = None;
        for _ in 0..MAX_USAGE_PAGES {
            let page = self.page(&config, "", false, after.as_deref(), PAGE_SIZE).await?;
            usage.used += page.objects.iter().map(|o| o.size).sum::<u64>();
            usage.objects += page.objects.iter().filter(|o| !is_folder(&o.key)).count() as u64;
            after = page.next;
            if after.is_none() {
                usage.complete = true;
                break;
            }
        }
        if let Ok(mut slot) = self.usage.lock() {
            *slot = Some((Instant::now(), usage));
        }
        Ok(usage)
    }
}

/// Writes a file only this box's own account can read, whole or not at all.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_SECRET: &str = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
    const EXAMPLE_HOST: &str = "examplebucket.s3.amazonaws.com";
    const EXAMPLE_DATE: &str = "20130524T000000Z";
    const EMPTY_HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn example(method: &str, path: &str, query: &str, extra: &[(&str, &str)]) -> String {
        let mut headers: Vec<(String, String)> = extra.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        headers.push(("host".into(), EXAMPLE_HOST.into()));
        headers.push(("x-amz-content-sha256".into(), EMPTY_HASH.into()));
        headers.push(("x-amz-date".into(), EXAMPLE_DATE.into()));
        signature(Signing { secret: EXAMPLE_SECRET, region: "us-east-1", method, path, query, headers, payload_hash: EMPTY_HASH, amz_date: EXAMPLE_DATE }).0
    }

    /// The worked examples in Amazon's own description of Signature Version 4 for S3.
    #[test]
    fn requests_are_signed_the_way_the_published_examples_are() {
        assert_eq!(example("GET", "/test.txt", "", &[("range", "bytes=0-9")]), "f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41");
        let listing = canonical_query(&[("prefix", "J".into()), ("max-keys", "2".into())]);
        assert_eq!(listing, "max-keys=2&prefix=J");
        assert_eq!(example("GET", "/", &listing, &[]), "34b48302e7b5fa45bde8084f4b7868a86f0a534bc59db6670ed5711ef69dc6f7");
    }

    #[test]
    fn names_are_encoded_for_an_address() {
        assert_eq!(encode("music/2026/a b+c.mp3", true), "music/2026/a%20b%2Bc.mp3");
        assert_eq!(encode("a/b", false), "a%2Fb");
        assert_eq!(encode("résumé~1.pdf", true), "r%C3%A9sum%C3%A9~1.pdf");
    }

    #[test]
    fn names_that_could_mean_something_else_are_refused() {
        for bad in ["/a", "a//b", "../a", "a/../b", "a/./b", "a\\b", "a/ b", "a\nb", "//"] {
            assert!(!valid_key(bad), "{bad:?} should be refused");
        }
        for good in ["", "a.txt", "music/", "music/2026/song (2).mp3", ".well-known/x"] {
            assert!(valid_key(good), "{good:?} should be accepted");
        }
    }

    #[test]
    fn folders_and_files_know_where_they_are() {
        assert_eq!(parent_of("a/b/c.txt"), "a/b/");
        assert_eq!(parent_of("a/b/c/"), "a/b/");
        assert_eq!(parent_of("c.txt"), "");
        assert_eq!(parent_of("music/"), "");
        assert_eq!(name_of("a/b/c.txt"), "c.txt");
        assert_eq!(name_of("a/b/c/"), "c");
        assert_eq!(numbered("song.mp3", 1), "song.mp3");
        assert_eq!(numbered("song.mp3", 2), "song (2).mp3");
        assert_eq!(numbered("New folder", 3), "New folder (3)");
        assert_eq!(numbered(".env", 2), ".env (2)");
    }

    #[test]
    fn a_listing_is_read_into_folders_and_files() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Name>dogs</Name><Prefix>music/</Prefix>
<Contents><Key>music/</Key><Size>0</Size><LastModified>2026-06-25T01:43:12.933Z</LastModified></Contents>
<Contents><Key>music/tom &amp; jerry &#60;1&#x3e;.mp3</Key><LastModified>2026-06-25T01:43:12.000Z</LastModified><ETag>&quot;abc&quot;</ETag><Size>3643200</Size></Contents>
<CommonPrefixes><Prefix>music/2026/</Prefix></CommonPrefixes>
<IsTruncated>true</IsTruncated><NextContinuationToken>abc==</NextContinuationToken></ListBucketResult>"#;
        let page = parse_listing(xml).unwrap();
        assert_eq!(page.folders, vec!["music/2026/"]);
        assert_eq!(page.next.as_deref(), Some("abc=="));
        assert_eq!(page.objects.len(), 2);
        assert_eq!(page.objects[1], Object { key: "music/tom & jerry <1>.mp3".into(), size: 3643200, modified: 1782351792000 });
        assert_eq!(parse_listing(b"<ListBucketResult><KeyCount>0</KeyCount></ListBucketResult>").unwrap(), Page::default());
    }

    fn body(endpoint: &str, bucket: &str) -> ConnectBody {
        ConnectBody {
            endpoint: endpoint.into(),
            bucket: bucket.into(),
            access_key_id: "0123456789abcdef".into(),
            secret_access_key: "secret".into(),
            region: String::new(),
            public_url: "https://data.example.com/".into(),
            quota_gb: 10.0,
        }
    }

    #[test]
    fn only_a_plain_secure_address_is_kept() {
        let config = Config::from_body(body("https://abc.r2.cloudflarestorage.com/", "dogs-data")).unwrap();
        assert_eq!(config.endpoint, "https://abc.r2.cloudflarestorage.com");
        assert_eq!(config.region, "auto");
        assert_eq!(config.quota, 10 * BYTES_PER_GB);
        assert_eq!(config.public("music/a b.mp3").as_deref(), Some("https://data.example.com/music/a%20b.mp3"));
        // The app is shown the end of the key id and never the secret.
        let status = config.status();
        assert_eq!(status.key_id, "cdef");
        assert!(!serde_json::to_string(&status).unwrap().contains("secret"));
        assert!(Config::from_body(body("http://127.0.0.1:9000", "local")).is_ok());
        for refused in ["http://abc.r2.cloudflarestorage.com", "https://abc.example.com/path", "https://user@abc.example.com", "ftp://abc.example.com", "abc"] {
            assert!(Config::from_body(body(refused, "dogs-data")).is_err(), "{refused:?} should be refused");
        }
        for refused in ["", "ab", "Dogs", "dogs/data", "dogs data"] {
            assert!(Config::from_body(body("https://abc.example.com", refused)).is_err(), "{refused:?} should be refused");
        }
    }

    #[test]
    fn nothing_is_asked_of_a_bucket_that_is_not_connected() {
        let dir = std::env::temp_dir().join(format!("castled-bucket-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bucket = Bucket::open(&dir);
        assert!(!bucket.status().connected);
        assert_eq!(bucket.config().err(), Some(Fail::Conflict(NOT_CONNECTED.into())));
        assert_eq!(bucket.disconnect(), Ok(()));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
