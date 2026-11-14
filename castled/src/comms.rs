//! Comms: each person's texts, picture messages, calls, emails and chats, kept on the box.
//!
//! Nothing here talks to a phone company or a mail server. A person exports what they have
//! and hands the file to the box, which reads it and keeps every message:
//!
//!   * Android texts, picture messages (MMS) and the call log, from SMS Backup & Restore's
//!     .xml files (the free app most Android phones use for exactly this).
//!   * Email, from an .mbox (Gmail through Google Takeout, Thunderbird, Apple Mail) or .eml.
//!   * Discord, from DiscordChatExporter's JSON or Discord's own data package.
//!   * Facebook Messenger, from Facebook's "Download your information" in JSON.
//!   * Call recordings, from a phone's recorder: audio files named with the number and time,
//!     put with the call they belong to when the call log has it.
//!   * A .zip of any of those (a whole Takeout or Facebook download), read as it is.
//!
//! The same file twice adds nothing: every message is known by what it is, so a newer
//! backup only adds what is new.
//!
//! A conversation is one person or group on one channel: texts with Dad, calls with Dad,
//! email with Dad. The app puts them together with the person's card from Contacts by
//! their numbers and addresses.
//!
//! Who sees what: as with Contacts, the app names the person, and the box keeps each
//! person's archive apart. Until the box has accounts that is a convention, not a lock.

use std::{
    collections::{HashMap, HashSet},
    io::{BufRead, BufReader, Read, Seek},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::contacts::{base64_decode, phone_key, valid_person, write_json};

pub type CommsResult<T> = Result<T, String>;

/// The most messages one thread answer carries; older ones come with `before`.
pub const PAGE: usize = 400;
const MAX_BODY: usize = 100_000;
const MAX_ATTACHMENT: usize = 50 * 1024 * 1024;
/// A file inside a zip is read into memory only up to this; bigger ones are streamed.
const MAX_IN_MEMORY: u64 = 64 * 1024 * 1024;
const MAX_SEARCH_HITS: usize = 200;
const MAX_IMPORTS_KEPT: usize = 200;
/// A recording belongs to a call that started this close to it.
const RECORDING_SLACK_MS: i64 = 10 * 60 * 1000;
const AUDIO: [&str; 9] = ["m4a", "mp3", "amr", "3gp", "wav", "ogg", "opus", "aac", "awb"];

fn now_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn short_hash(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest.iter().take(10).map(|b| format!("{b:02x}")).collect()
}

fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Attachment {
    pub name: String,
    pub mime: String,
    /// Where the box keeps it: "<hash>.<ext>", shared by every message that sent the same file.
    pub file: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Call {
    pub seconds: u64,
    /// incoming, outgoing, missed, voicemail, rejected, blocked
    pub direction: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Message {
    pub id: String,
    pub at: u64,
    /// Who sent it, as shown: a name when the export had one, else the number or address.
    pub from: String,
    /// The sender's number, address or handle.
    pub address: String,
    /// Sent by the person whose archive this is.
    pub me: bool,
    /// sms, mms, email, call, recording, chat
    pub kind: String,
    pub subject: String,
    pub body: String,
    pub call: Option<Call>,
    pub attachments: Vec<Attachment>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Conversation {
    pub id: String,
    pub channel: String,
    pub title: String,
    /// The other people's numbers, addresses or names, for finding them in Contacts.
    pub participants: Vec<String>,
    pub count: usize,
    pub first: u64,
    pub last: u64,
    pub snippet: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Report {
    pub texts: usize,
    pub calls: usize,
    pub emails: usize,
    pub chats: usize,
    pub recordings: usize,
    pub attachments: usize,
    /// Messages the box already had.
    pub known: usize,
    pub conversations: usize,
    /// Files inside the upload that were not something Comms reads.
    pub ignored: usize,
    /// Contacts files found inside the upload, handed to Contacts.
    pub contacts_files: usize,
}

impl Report {
    pub fn added(&self) -> usize {
        self.texts + self.calls + self.emails + self.chats + self.recordings
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ImportRecord {
    pub at: u64,
    pub source: String,
    pub report: Report,
}

/// An import the box is working through.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub person: String,
    pub source: String,
    pub started: u64,
    /// What it is doing now, in words.
    pub step: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub conversations: Vec<Conversation>,
    pub imports: Vec<ImportRecord>,
    pub importing: Option<Job>,
    /// The last import's outcome when it failed, for the app to say.
    pub problem: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub conversation: Conversation,
    pub messages: Vec<Message>,
    /// There are older messages than these.
    pub more: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub conversation: String,
    pub message: Message,
}

/// What an import found that is not a message: contacts files, for Contacts.
pub struct Found {
    pub report: Report,
    pub vcards: Vec<(String, String)>,
}

pub struct Comms {
    dir: PathBuf,
    lock: Mutex<()>,
    job: Mutex<Option<Job>>,
    problem: Mutex<HashMap<String, String>>,
}

impl Comms {
    pub fn open(dir: PathBuf) -> Self {
        Self { dir, lock: Mutex::new(()), job: Mutex::new(None), problem: Mutex::new(HashMap::new()) }
    }

    fn person_dir(&self, person: &str) -> CommsResult<PathBuf> {
        if !valid_person(person) {
            return Err("that person is not on this box".into());
        }
        Ok(self.dir.join(person))
    }

    fn read_index(&self, person: &str) -> CommsResult<Vec<Conversation>> {
        Ok(read_json(&self.person_dir(person)?.join("index.json")))
    }

    fn read_thread(&self, person: &str, id: &str) -> CommsResult<Vec<Message>> {
        if !valid_id(id) {
            return Err("no such conversation".into());
        }
        Ok(read_json(&self.person_dir(person)?.join("threads").join(format!("{id}.json"))))
    }

    pub fn overview(&self, person: &str) -> CommsResult<Overview> {
        let mut conversations = self.read_index(person)?;
        conversations.sort_by_key(|c| std::cmp::Reverse(c.last));
        let mut imports: Vec<ImportRecord> = read_json(&self.person_dir(person)?.join("imports.json"));
        imports.reverse();
        let importing = self.job.lock().ok().and_then(|j| j.clone()).filter(|j| j.person == person);
        let problem = self.problem.lock().ok().and_then(|p| p.get(person).cloned());
        Ok(Overview { conversations, imports, importing, problem })
    }

    /// The newest messages of a conversation, or the ones before `before`.
    pub fn thread(&self, person: &str, id: &str, before: Option<u64>) -> CommsResult<Thread> {
        let conversation = self.read_index(person)?.into_iter().find(|c| c.id == id).ok_or("no such conversation")?;
        let all = self.read_thread(person, id)?;
        let upto = before.map_or(all.len(), |b| all.partition_point(|m| m.at < b));
        let from = upto.saturating_sub(PAGE);
        Ok(Thread { conversation, messages: all[from..upto].to_vec(), more: from > 0 })
    }

    /// Every message whose words, sender or subject hold all of the words asked for.
    pub fn search(&self, person: &str, query: &str) -> CommsResult<Vec<Hit>> {
        let words: Vec<String> = query.to_lowercase().split_whitespace().map(String::from).collect();
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let mut hits = Vec::new();
        let mut index = self.read_index(person)?;
        index.sort_by_key(|c| std::cmp::Reverse(c.last));
        for conv in index {
            for m in self.read_thread(person, &conv.id)?.into_iter().rev() {
                let hay = format!("{} {} {} {}", m.body, m.subject, m.from, m.attachments.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(" ")).to_lowercase();
                if words.iter().all(|w| hay.contains(w)) {
                    hits.push(Hit { conversation: conv.id.clone(), message: m });
                    if hits.len() >= MAX_SEARCH_HITS {
                        return Ok(hits);
                    }
                }
            }
        }
        Ok(hits)
    }

    /// Forgets a conversation, and every attachment no other message still uses.
    pub fn delete(&self, person: &str, id: &str) -> CommsResult<()> {
        let _held = self.lock.lock().map_err(|_| "comms are unavailable".to_string())?;
        let dir = self.person_dir(person)?;
        let mut index = self.read_index(person)?;
        let before = index.len();
        index.retain(|c| c.id != id);
        if index.len() == before {
            return Err("no such conversation".into());
        }
        write_json(&dir.join("index.json"), &index)?;
        let _ = std::fs::remove_file(dir.join("threads").join(format!("{id}.json")));
        let mut used = HashSet::new();
        for conv in &index {
            for m in self.read_thread(person, &conv.id)? {
                used.extend(m.attachments.into_iter().map(|a| a.file));
            }
        }
        if let Ok(entries) = std::fs::read_dir(dir.join("files")) {
            for e in entries.flatten() {
                if !used.contains(&e.file_name().to_string_lossy().to_string()) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        Ok(())
    }

    /// Where an attachment is, and its kind.
    pub fn file(&self, person: &str, name: &str) -> CommsResult<(PathBuf, String)> {
        let ok = name.split_once('.').is_some_and(|(h, e)| h.len() == 32 && h.chars().all(|c| c.is_ascii_hexdigit()) && !e.is_empty() && e.len() <= 8 && e.chars().all(|c| c.is_ascii_alphanumeric()));
        if !ok {
            return Err("no such file".into());
        }
        let path = self.person_dir(person)?.join("files").join(name);
        if !path.is_file() {
            return Err("no such file".into());
        }
        Ok((path, mime_guess::from_path(name).first_or_octet_stream().to_string()))
    }

    /// Marks an import as started; refused while another is going.
    pub fn begin(&self, person: &str, source: &str) -> CommsResult<()> {
        valid_person(person).then_some(()).ok_or("that person is not on this box")?;
        let mut job = self.job.lock().map_err(|_| "comms are unavailable".to_string())?;
        if job.is_some() {
            return Err("an import is already going; wait for it to finish".into());
        }
        *job = Some(Job { person: person.into(), source: source.into(), started: now_millis(), step: "Reading".into() });
        if let Ok(mut p) = self.problem.lock() {
            p.remove(person);
        }
        Ok(())
    }

    /// Ends an import that never got its file (the upload broke off).
    pub fn cancel(&self) {
        if let Ok(mut job) = self.job.lock() {
            *job = None;
        }
    }

    fn step(&self, text: &str) {
        if let Ok(mut job) = self.job.lock() {
            if let Some(job) = job.as_mut() {
                job.step = text.into();
            }
        }
    }

    /// Reads an uploaded file into the person's archive and marks the import finished.
    /// `offset` is the phone's minutes behind UTC (as a browser says it), for recordings
    /// named in local time.
    pub fn import(&self, person: &str, path: &Path, source: &str, offset: i64) -> CommsResult<Found> {
        let result = self.import_inner(person, path, source, offset);
        if let Ok(mut job) = self.job.lock() {
            *job = None;
        }
        if let Err(e) = &result {
            if let Ok(mut p) = self.problem.lock() {
                p.insert(person.into(), e.clone());
            }
        }
        result
    }

    fn import_inner(&self, person: &str, path: &Path, source: &str, offset: i64) -> CommsResult<Found> {
        let dir = self.person_dir(person)?;
        let mut batch = Batch::new(dir.join("files"), offset);
        let lower = source.to_lowercase();
        let file = std::fs::File::open(path).map_err(|_| "the upload is gone".to_string())?;
        if lower.ends_with(".zip") {
            self.step("Opening the zip");
            read_zip(file, &mut batch, &|s| self.step(s))?;
        } else {
            read_one(source, BufReader::new(file), &mut batch)?;
        }
        self.step("Putting conversations together");
        batch.finish();
        if batch.convs.is_empty() && batch.vcards.is_empty() {
            return Err(format!("there were no messages in {source} that Castle can read"));
        }
        self.step("Saving");
        let _held = self.lock.lock().map_err(|_| "comms are unavailable".to_string())?;
        let mut report = batch.report.clone();
        self.merge(person, &mut batch, &mut report)?;
        let mut imports: Vec<ImportRecord> = read_json(&dir.join("imports.json"));
        imports.push(ImportRecord { at: now_millis(), source: clip(source, 200), report: report.clone() });
        if imports.len() > MAX_IMPORTS_KEPT {
            imports.drain(..imports.len() - MAX_IMPORTS_KEPT);
        }
        write_json(&dir.join("imports.json"), &imports)?;
        Ok(Found { report, vcards: std::mem::take(&mut batch.vcards) })
    }

    /// Adds a batch's messages to what is kept, skipping the ones already there.
    fn merge(&self, person: &str, batch: &mut Batch, report: &mut Report) -> CommsResult<()> {
        let dir = self.person_dir(person)?;
        let mut index = self.read_index(person)?;
        // The counts in the batch are what was read; these become what was new.
        let (mut texts, mut calls, mut emails, mut chats, mut recordings) = (0, 0, 0, 0, 0);
        for (id, (meta, messages)) in batch.convs.drain() {
            let mut kept = self.read_thread(person, &id)?;
            let known: HashSet<String> = kept.iter().map(|m| m.id.clone()).collect();
            let mut added = 0;
            for m in messages {
                if known.contains(&m.id) {
                    report.known += 1;
                    continue;
                }
                match m.kind.as_str() {
                    "sms" | "mms" => texts += 1,
                    "call" => calls += 1,
                    "email" => emails += 1,
                    "recording" => recordings += 1,
                    _ => chats += 1,
                }
                kept.push(m);
                added += 1;
            }
            // A recording found later goes onto its call, which is already kept.
            if let Some(recs) = batch.late_recordings.remove(&id) {
                for (call_id, att) in recs {
                    if let Some(call) = kept.iter_mut().find(|m| m.id == call_id) {
                        if !call.attachments.iter().any(|a| a.file == att.file) {
                            call.attachments.push(att);
                            recordings += 1;
                            added += 1;
                        } else {
                            report.known += 1;
                        }
                    }
                }
            }
            if added == 0 {
                continue;
            }
            kept.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
            let last = kept.last().cloned().unwrap_or_default();
            let entry = match index.iter_mut().find(|c| c.id == id) {
                Some(entry) => entry,
                None => {
                    report.conversations += 1;
                    index.push(Conversation { id: id.clone(), channel: meta.channel.clone(), ..Default::default() });
                    index.last_mut().unwrap()
                }
            };
            if entry.title.is_empty() || entry.title == entry.participants.first().cloned().unwrap_or_default() {
                entry.title = meta.title.clone();
            }
            for p in meta.participants {
                if !entry.participants.contains(&p) {
                    entry.participants.push(p);
                }
            }
            entry.count = kept.len();
            entry.first = kept.first().map_or(0, |m| m.at);
            entry.last = last.at;
            entry.snippet = snippet(&last);
            write_json(&dir.join("threads").join(format!("{id}.json")), &kept)?;
        }
        write_json(&dir.join("index.json"), &index)?;
        (report.texts, report.calls, report.emails, report.chats, report.recordings) = (texts, calls, emails, chats, recordings);
        Ok(())
    }
}

fn snippet(m: &Message) -> String {
    let text = if let Some(call) = &m.call {
        format!("{} call{}", capitalise(&call.direction), if call.seconds > 0 { format!(", {}", duration(call.seconds)) } else { String::new() })
    } else if !m.subject.is_empty() {
        m.subject.clone()
    } else if !m.body.is_empty() {
        m.body.clone()
    } else if let Some(a) = m.attachments.first() {
        format!("Sent {}", if a.mime.starts_with("image/") { "a picture".to_string() } else { a.name.clone() })
    } else {
        String::new()
    };
    clip(&text.replace(['\r', '\n'], " "), 140)
}

fn capitalise(word: &str) -> String {
    let mut c = word.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn duration(seconds: u64) -> String {
    if seconds >= 3600 {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    } else if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

fn valid_id(id: &str) -> bool {
    id.len() == 20 && id.chars().all(|c| c.is_ascii_hexdigit())
}

fn read_json<T: serde::de::DeserializeOwned + Default>(file: &Path) -> T {
    std::fs::read(file).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default()
}

// ---------------------------------------------------------------------------------------
// A batch: what one import read, before it is put with what is kept
// ---------------------------------------------------------------------------------------

#[derive(Default, Clone)]
struct Meta {
    channel: String,
    title: String,
    participants: Vec<String>,
}

/// A picture message, held until the whole file is read: only then is it known which
/// numbers are the phone's own.
struct PendingMms {
    at: u64,
    sent: bool,
    from: String,
    addresses: Vec<String>,
    name: String,
    body: String,
    subject: String,
    attachments: Vec<Attachment>,
}

struct PendingEmail {
    at: u64,
    from: (String, String),
    to: Vec<(String, String)>,
    subject: String,
    body: String,
    message_id: String,
    attachments: Vec<Attachment>,
}

struct FbThread {
    title: String,
    path: String,
    participants: Vec<String>,
    messages: Vec<(u64, String, String, Vec<Attachment>, Option<u64>)>,
}

struct Recording {
    number: String,
    at: Option<u64>,
    name: String,
    attachment: Attachment,
}

struct Batch {
    files: PathBuf,
    offset_ms: i64,
    convs: HashMap<String, (Meta, Vec<Message>)>,
    report: Report,
    mms: Vec<PendingMms>,
    my_numbers: HashSet<String>,
    emails: Vec<PendingEmail>,
    fb: Vec<FbThread>,
    recordings: Vec<Recording>,
    /// Recordings for calls kept before this import: conversation -> (call, recording).
    late_recordings: HashMap<String, Vec<(String, Attachment)>>,
    vcards: Vec<(String, String)>,
}

fn conv_id(channel: &str, key: &str) -> String {
    short_hash(&format!("{channel}|{key}"))
}

fn message_id(conv: &str, at: u64, address: &str, kind: &str, text: &str) -> String {
    let head: String = text.chars().take(300).collect();
    short_hash(&format!("{conv}|{at}|{}|{kind}|{head}", address.to_lowercase()))
}

/// The part of a number or address that names it in a conversation's key.
fn address_key(address: &str) -> String {
    let key = phone_key(address);
    if key.is_empty() { address.trim().to_lowercase() } else { key }
}

impl Batch {
    fn new(files: PathBuf, offset_minutes: i64) -> Self {
        Self {
            files,
            offset_ms: offset_minutes.clamp(-24 * 60, 24 * 60) * 60_000,
            convs: HashMap::new(),
            report: Report::default(),
            mms: Vec::new(),
            my_numbers: HashSet::new(),
            emails: Vec::new(),
            fb: Vec::new(),
            recordings: Vec::new(),
            late_recordings: HashMap::new(),
            vcards: Vec::new(),
        }
    }

    fn push(&mut self, channel: &str, key: &str, title: &str, participants: Vec<String>, mut m: Message) {
        let id = conv_id(channel, key);
        m.body = clip(&m.body, MAX_BODY);
        m.subject = clip(&m.subject, 300);
        m.from = clip(&m.from, 200);
        if m.id.is_empty() {
            let text = format!("{}{}{}", m.subject, m.body, m.attachments.iter().map(|a| a.file.as_str()).collect::<String>());
            m.id = message_id(&id, m.at, &m.address, &m.kind, &text);
        }
        let entry = self.convs.entry(id).or_insert_with(|| (Meta { channel: channel.into(), title: title.into(), participants: Vec::new() }, Vec::new()));
        if entry.0.title.is_empty() || entry.0.title == key {
            entry.0.title = title.into();
        }
        for p in participants {
            if !p.is_empty() && !entry.0.participants.contains(&p) {
                entry.0.participants.push(p);
            }
        }
        if !entry.1.iter().any(|x| x.id == m.id) {
            entry.1.push(m);
        }
    }

    /// Keeps a file once, however many messages carry it.
    fn keep(&mut self, name: &str, mime: &str, bytes: &[u8]) -> Option<Attachment> {
        if bytes.is_empty() || bytes.len() > MAX_ATTACHMENT {
            return None;
        }
        let digest = Sha256::digest(bytes);
        let hash: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
        let mime = if mime.is_empty() || mime == "application/octet-stream" { mime_guess::from_path(name).first_or_octet_stream().to_string() } else { mime.to_lowercase() };
        let ext = extension_for(name, &mime);
        let file = format!("{hash}.{ext}");
        let path = self.files.join(&file);
        if !path.exists() {
            std::fs::create_dir_all(&self.files).ok()?;
            std::fs::write(&path, bytes).ok()?;
        }
        self.report.attachments += 1;
        Some(Attachment { name: clip(name, 200), mime, file, size: bytes.len() as u64 })
    }

    /// Everything that had to wait for the whole upload: picture messages, email and
    /// Messenger need to know whose archive it is; recordings need the calls.
    fn finish(&mut self) {
        for m in std::mem::take(&mut self.mms) {
            let mut others: Vec<String> = m.addresses.iter().filter(|a| !self.my_numbers.contains(&address_key(a))).cloned().collect();
            if others.is_empty() {
                others = m.addresses.clone();
            }
            let mut keys: Vec<String> = others.iter().map(|a| address_key(a)).collect();
            keys.sort();
            keys.dedup();
            let title = if m.name.is_empty() || m.name == "(Unknown)" { others.join(", ") } else { m.name.replace('~', ", ") };
            let from = if m.sent { "Me".to_string() } else if others.len() == 1 && !m.name.is_empty() && m.name != "(Unknown)" { m.name.clone() } else { m.from.clone() };
            let msg = Message { at: m.at, from, address: m.from.clone(), me: m.sent, kind: "mms".into(), subject: m.subject, body: m.body, attachments: m.attachments, ..Default::default() };
            self.push("texts", &keys.join(","), &title, others, msg);
        }

        // Whose mail is it: the address in the most messages, sent or received.
        let mut seen: HashMap<String, usize> = HashMap::new();
        for e in &self.emails {
            let mut addresses: HashSet<String> = e.to.iter().map(|(_, a)| a.to_lowercase()).collect();
            addresses.insert(e.from.1.to_lowercase());
            for a in addresses {
                *seen.entry(a).or_default() += 1;
            }
        }
        let me = seen.iter().max_by_key(|(a, n)| (**n, std::cmp::Reverse((*a).clone()))).map(|(a, _)| a.clone()).filter(|_| self.emails.len() > 1).unwrap_or_default();
        for e in std::mem::take(&mut self.emails) {
            let mine = !me.is_empty() && e.from.1.eq_ignore_ascii_case(&me);
            let other = if mine { e.to.iter().find(|(_, a)| !a.eq_ignore_ascii_case(&me)).cloned().unwrap_or_else(|| e.from.clone()) } else { e.from.clone() };
            let key = other.1.to_lowercase();
            let title = if other.0.is_empty() { other.1.clone() } else { other.0.clone() };
            let id = conv_id("email", &key);
            let msg_id = if e.message_id.is_empty() { String::new() } else { short_hash(&format!("{id}|{}", e.message_id)) };
            let from = if mine { "Me".to_string() } else if e.from.0.is_empty() { e.from.1.clone() } else { e.from.0.clone() };
            let msg = Message { id: msg_id, at: e.at, from, address: e.from.1, me: mine, kind: "email".into(), subject: e.subject, body: e.body, attachments: e.attachments, ..Default::default() };
            self.push("email", &key, &title, vec![other.1], msg);
        }

        // Messenger: the person in every thread is the one who downloaded them.
        let mut names: HashMap<String, usize> = HashMap::new();
        for t in &self.fb {
            for p in t.participants.iter().collect::<HashSet<_>>() {
                *names.entry(p.clone()).or_default() += 1;
            }
        }
        let me = if self.fb.len() > 1 { names.iter().filter(|(_, n)| **n == self.fb.len()).map(|(p, _)| p.clone()).next().unwrap_or_default() } else { String::new() };
        for t in std::mem::take(&mut self.fb) {
            let others: Vec<String> = t.participants.iter().filter(|p| **p != me).cloned().collect();
            let title = if t.title.is_empty() { others.join(", ") } else { t.title.clone() };
            for (at, sender, text, attachments, call) in t.messages {
                let mine = !me.is_empty() && sender == me;
                let (kind, call) = match call {
                    Some(seconds) => ("call", Some(Call { seconds, direction: if seconds == 0 { "missed".into() } else if mine { "outgoing".into() } else { "incoming".into() } })),
                    None => ("chat", None),
                };
                let msg = Message { at, from: if mine { "Me".into() } else { sender.clone() }, address: sender, me: mine, kind: kind.into(), body: text, call, attachments, ..Default::default() };
                self.push("facebook", &t.path, &title, others.clone(), msg);
            }
        }

        for r in std::mem::take(&mut self.recordings) {
            let key = address_key(&r.number);
            let calls_id = conv_id("calls", &key);
            // On a call from this same upload?
            if let (false, Some(at)) = (key.is_empty(), r.at) {
                if let Some((_, list)) = self.convs.get_mut(&calls_id) {
                    if let Some(call) = list.iter_mut().filter(|m| m.kind == "call").min_by_key(|m| (m.at as i64 - at as i64).abs()).filter(|m| (m.at as i64 - at as i64).abs() <= RECORDING_SLACK_MS) {
                        call.attachments.push(r.attachment);
                        self.report.recordings += 1;
                        continue;
                    }
                }
            }
            let at = r.at.unwrap_or_else(now_millis);
            if key.is_empty() {
                let msg = Message { at, from: r.name.clone(), kind: "recording".into(), body: r.name.clone(), attachments: vec![r.attachment], ..Default::default() };
                self.push("calls", "recordings", "Call recordings", Vec::new(), msg);
            } else {
                // Held as its own entry; merge() moves it onto a kept call when one is close.
                let msg = Message { at, from: r.number.clone(), address: r.number.clone(), kind: "recording".into(), body: r.name.clone(), attachments: vec![r.attachment], ..Default::default() };
                self.push("calls", &key, &r.number, vec![r.number.clone()], msg);
            }
            self.report.recordings += 1;
        }
    }
}

fn extension_for(name: &str, mime: &str) -> String {
    let from_name = Path::new(name).extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).filter(|e| e.len() <= 8 && e.chars().all(|c| c.is_ascii_alphanumeric()));
    if let Some(ext) = from_name {
        return ext;
    }
    mime_guess::get_mime_extensions_str(mime).and_then(|list| list.first()).map(|e| e.to_string()).unwrap_or_else(|| "bin".into())
}

// ---------------------------------------------------------------------------------------
// Reading files
// ---------------------------------------------------------------------------------------

/// What kind of export a file is, from its name and its first bytes.
fn read_one<R: BufRead>(name: &str, mut reader: R, batch: &mut Batch) -> CommsResult<()> {
    let lower = name.to_lowercase();
    let ext = Path::new(&lower).extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
    if AUDIO.contains(&ext.as_str()) {
        let mut bytes = Vec::new();
        reader.take(MAX_ATTACHMENT as u64 + 1).read_to_end(&mut bytes).map_err(|_| "could not read the recording".to_string())?;
        return read_recording(name, &bytes, batch);
    }
    let head = reader.fill_buf().map(|b| String::from_utf8_lossy(&b[..b.len().min(4096)]).to_string()).unwrap_or_default();
    let trimmed = head.trim_start_matches('\u{feff}').trim_start();
    if ext == "vcf" || trimmed.starts_with("BEGIN:VCARD") {
        let mut text = String::new();
        reader.read_to_string(&mut text).map_err(|_| "could not read the contacts file".to_string())?;
        batch.vcards.push((name.to_string(), text));
        batch.report.contacts_files += 1;
        return Ok(());
    }
    if ext == "xml" || trimmed.starts_with("<?xml") {
        return read_sms_backup(reader, batch);
    }
    if ext == "mbox" || head.starts_with("From ") {
        return read_mbox(reader, batch);
    }
    if ext == "eml" {
        let mut raw = Vec::new();
        reader.take(MAX_IN_MEMORY).read_to_end(&mut raw).map_err(|_| "could not read the email".to_string())?;
        read_email(&raw, None, batch);
        return Ok(());
    }
    if ext == "json" || trimmed.starts_with('{') || trimmed.starts_with('[') {
        let mut raw = Vec::new();
        reader.take(MAX_IN_MEMORY).read_to_end(&mut raw).map_err(|_| "could not read the file".to_string())?;
        let value: serde_json::Value = serde_json::from_slice(&raw).map_err(|_| format!("{name} is not JSON Castle can read"))?;
        return read_json_export(name, &value, None, batch);
    }
    batch.report.ignored += 1;
    Err(format!("Castle does not know how to read {name}"))
}

/// A whole export as a zip: every file in it that Comms knows, and pictures that
/// Messenger's messages point at.
fn read_zip<R: Read + Seek>(file: R, batch: &mut Batch, step: &dyn Fn(&str)) -> CommsResult<()> {
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "that zip cannot be opened".to_string())?;
    let names: Vec<String> = zip.file_names().filter_map(Result::ok).map(|n| n.to_string()).collect();
    // Discord's package keeps each channel's facts beside its messages.
    let mut channel_info: HashMap<String, serde_json::Value> = HashMap::new();
    for name in names.iter().filter(|n| n.ends_with("/channel.json")) {
        if let Some(v) = zip_json(&mut zip, name) {
            channel_info.insert(name.trim_end_matches("channel.json").to_string(), v);
        }
    }
    let total = names.len();
    for (i, name) in names.iter().enumerate() {
        if name.ends_with('/') || name.ends_with("/channel.json") {
            continue;
        }
        if i % 50 == 0 {
            step(&format!("Reading file {} of {total}", i + 1));
        }
        let lower = name.to_lowercase();
        let ext = Path::new(&lower).extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
        let wanted = matches!(ext.as_str(), "xml" | "mbox" | "eml" | "vcf" | "json") || AUDIO.contains(&ext.as_str());
        // Messenger's own pictures are read when a message points at them, not on their own.
        let in_messenger = lower.contains("messages/") && (lower.contains("/photos/") || lower.contains("/videos/") || lower.contains("/audio/") || lower.contains("/files/") || lower.contains("/gifs/"));
        if !wanted || in_messenger {
            if !in_messenger && !name.contains("/.") {
                batch.report.ignored += 1;
            }
            continue;
        }
        if ext == "json" {
            let Some(value) = zip_json(&mut zip, name) else { continue };
            let folder = name.rsplit_once('/').map(|(d, _)| format!("{d}/")).unwrap_or_default();
            let info = channel_info.get(&folder).cloned();
            let _ = read_json_export(name, &value, info.as_ref(), batch);
            continue;
        }
        let Ok(entry) = zip.by_name(name) else { continue };
        let short = name.rsplit('/').next().unwrap_or(name).to_string();
        // A file that is not a message export (a sitemap, say) is skipped, not fatal.
        let _ = read_one(&short, BufReader::new(entry), batch);
    }
    // Messenger: now that every thread is read, fetch the pictures they sent.
    let wanted: Vec<(usize, usize, usize, String)> = batch
        .fb
        .iter()
        .enumerate()
        .flat_map(|(t, thread)| thread.messages.iter().enumerate().flat_map(move |(m, msg)| msg.3.iter().enumerate().map(move |(a, att)| (t, m, a, att.name.clone()))))
        .collect();
    if !wanted.is_empty() {
        step("Reading Messenger's pictures");
    }
    let mut kept: HashMap<(usize, usize, usize), Option<Attachment>> = HashMap::new();
    for (t, m, a, uri) in wanted {
        let found = names.iter().find(|n| !uri.is_empty() && (n.ends_with(&uri) || uri.ends_with(n.as_str()))).cloned();
        let att = found.and_then(|n| {
            let mut entry = zip.by_name(&n).ok()?;
            if entry.size() > MAX_ATTACHMENT as u64 {
                return None;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).ok()?;
            let short = n.rsplit('/').next().unwrap_or(&n).to_string();
            batch.keep(&short, "", &bytes)
        });
        kept.insert((t, m, a), att);
    }
    for (t, thread) in batch.fb.iter_mut().enumerate() {
        for (m, msg) in thread.messages.iter_mut().enumerate() {
            let list = std::mem::take(&mut msg.3);
            msg.3 = list.into_iter().enumerate().filter_map(|(a, _)| kept.remove(&(t, m, a)).flatten()).collect();
        }
    }
    Ok(())
}

fn zip_json<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Option<serde_json::Value> {
    let mut entry = zip.by_name(name).ok()?;
    if entry.size() > MAX_IN_MEMORY {
        return None;
    }
    let mut raw = Vec::new();
    entry.read_to_end(&mut raw).ok()?;
    serde_json::from_slice(&raw).ok()
}

// ---- SMS Backup & Restore ----

/// XML's escapes, written out by hand because SMS Backup & Restore writes an emoji as two
/// halves (&#55357;&#56832;), which XML itself does not allow.
fn unescape_xml(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    let mut high: Option<u32> = None;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let Some(semi) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        rest = &rest[semi + 1..];
        let code = if let Some(hex) = entity.strip_prefix("#x").or_else(|| entity.strip_prefix("#X")) {
            u32::from_str_radix(hex, 16).ok()
        } else if let Some(dec) = entity.strip_prefix('#') {
            dec.parse().ok()
        } else {
            match entity {
                "amp" => Some('&' as u32),
                "lt" => Some('<' as u32),
                "gt" => Some('>' as u32),
                "quot" => Some('"' as u32),
                "apos" => Some('\'' as u32),
                _ => None,
            }
        };
        match code {
            Some(c @ 0xD800..=0xDBFF) => {
                if high.is_some() {
                    out.push('\u{FFFD}');
                }
                high = Some(c);
                continue;
            }
            Some(c @ 0xDC00..=0xDFFF) => match high.take() {
                Some(h) => out.push(char::from_u32(0x10000 + ((h - 0xD800) << 10) + (c - 0xDC00)).unwrap_or('\u{FFFD}')),
                None => out.push('\u{FFFD}'),
            },
            Some(c) => {
                if high.take().is_some() {
                    out.push('\u{FFFD}');
                }
                out.push(char::from_u32(c).unwrap_or('\u{FFFD}'));
            }
            None => {
                out.push('&');
                out.push_str(entity);
                out.push(';');
            }
        }
    }
    if high.is_some() {
        out.push('\u{FFFD}');
    }
    out.push_str(rest);
    out
}

fn attrs(e: &quick_xml::events::BytesStart) -> HashMap<String, String> {
    let mut list = e.attributes();
    list.with_checks(false);
    list.flatten().map(|a| (a.key.as_ref().to_string(), unescape_xml(&a.value))).collect()
}

fn present(v: Option<&String>) -> String {
    v.map(|s| s.trim()).filter(|s| !s.is_empty() && *s != "null").unwrap_or("").to_string()
}

fn read_sms_backup<R: BufRead>(reader: R, batch: &mut Batch) -> CommsResult<()> {
    use quick_xml::events::Event;
    let mut xml = quick_xml::Reader::from_reader(reader);
    xml.config_mut().check_end_names = false;
    let mut buf = Vec::new();
    let mut found = false;
    // The picture message being read: its own attributes, its parts, its addresses.
    let mut mms: Option<(HashMap<String, String>, Vec<HashMap<String, String>>, Vec<HashMap<String, String>>)> = None;
    loop {
        let event = xml.read_event_into(&mut buf).map_err(|e| format!("the backup file is damaged ({e})"))?;
        let (is_start, e) = match &event {
            Event::Start(e) => (true, e.clone()),
            Event::Empty(e) => (false, e.clone()),
            Event::End(e) => {
                if e.name().as_ref() == "mms" {
                    if let Some((a, parts, addrs)) = mms.take() {
                        mms_message(a, parts, addrs, batch);
                    }
                }
                buf.clear();
                continue;
            }
            Event::Eof => break,
            _ => {
                buf.clear();
                continue;
            }
        };
        match e.name().as_ref() {
            "smses" | "calls" => found = true,
            "sms" => {
                let a = attrs(&e);
                let address = present(a.get("address"));
                let at: u64 = present(a.get("date")).parse().unwrap_or(0);
                let sent = matches!(present(a.get("type")).as_str(), "2" | "4" | "5" | "6");
                let name = present(a.get("contact_name"));
                let name = if name == "(Unknown)" { String::new() } else { name };
                let title = if name.is_empty() { address.clone() } else { name.clone() };
                let from = if sent { "Me".to_string() } else { title.clone() };
                let msg = Message { at, from, address: if sent { String::new() } else { address.clone() }, me: sent, kind: "sms".into(), body: present(a.get("body")), subject: present(a.get("subject")), ..Default::default() };
                batch.push("texts", &address_key(&address), &title, vec![address], msg);
                batch.report.texts += 1;
            }
            "call" => {
                let a = attrs(&e);
                let number = present(a.get("number"));
                let at: u64 = present(a.get("date")).parse().unwrap_or(0);
                let direction = match present(a.get("type")).as_str() {
                    "1" => "incoming",
                    "2" => "outgoing",
                    "3" => "missed",
                    "4" => "voicemail",
                    "5" => "rejected",
                    "6" => "blocked",
                    _ => "incoming",
                };
                let name = present(a.get("contact_name"));
                let name = if name == "(Unknown)" { String::new() } else { name };
                let title = if name.is_empty() { number.clone() } else { name.clone() };
                let seconds = present(a.get("duration")).parse().unwrap_or(0);
                let mine = direction == "outgoing";
                let msg = Message { at, from: if mine { "Me".into() } else { title.clone() }, address: number.clone(), me: mine, kind: "call".into(), call: Some(Call { seconds, direction: direction.into() }), ..Default::default() };
                batch.push("calls", &address_key(&number), &title, vec![number], msg);
                batch.report.calls += 1;
            }
            "mms" if is_start => mms = Some((attrs(&e), Vec::new(), Vec::new())),
            "part" => {
                if let Some((_, parts, _)) = mms.as_mut() {
                    parts.push(attrs(&e));
                }
            }
            "addr" => {
                if let Some((_, _, addrs)) = mms.as_mut() {
                    addrs.push(attrs(&e));
                }
            }
            _ => {}
        }
        buf.clear();
    }
    if !found {
        return Err("that XML file is not an SMS Backup & Restore backup".into());
    }
    Ok(())
}

fn mms_message(a: HashMap<String, String>, parts: Vec<HashMap<String, String>>, addrs: Vec<HashMap<String, String>>, batch: &mut Batch) {
    // Dates are milliseconds; some phones write seconds.
    let mut at: u64 = present(a.get("date")).parse().unwrap_or(0);
    if at > 0 && at < 100_000_000_000 {
        at *= 1000;
    }
    let sent = present(a.get("msg_box")) == "2";
    // 137 is who sent it; 151 and 130 who it went to.
    let from = addrs.iter().find(|x| present(x.get("type")) == "137").map(|x| present(x.get("address"))).unwrap_or_default();
    if sent && !from.is_empty() && from != "insert-address-token" {
        batch.my_numbers.insert(address_key(&from));
    }
    let mut addresses: Vec<String> = present(a.get("address")).split('~').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    for x in &addrs {
        let addr = present(x.get("address"));
        if !addr.is_empty() && addr != "insert-address-token" && !addresses.iter().any(|y| address_key(y) == address_key(&addr)) {
            addresses.push(addr);
        }
    }
    let mut body = Vec::new();
    let mut attachments = Vec::new();
    for p in parts {
        let ct = present(p.get("ct")).to_lowercase();
        if ct == "application/smil" {
            continue;
        }
        if ct.starts_with("text/plain") {
            let text = present(p.get("text"));
            if !text.is_empty() {
                body.push(text);
            }
            continue;
        }
        let Some(bytes) = p.get("data").and_then(|d| base64_decode(d)) else { continue };
        let name = [present(p.get("cl")), present(p.get("name")), present(p.get("fn"))].into_iter().find(|n| !n.is_empty()).unwrap_or_else(|| "attachment".into());
        if let Some(att) = batch.keep(&name, &ct, &bytes) {
            attachments.push(att);
        }
    }
    batch.mms.push(PendingMms {
        at,
        sent,
        from: if from == "insert-address-token" { String::new() } else { from },
        addresses,
        name: present(a.get("contact_name")),
        body: body.join("\n"),
        subject: present(a.get("sub")),
        attachments,
    });
    batch.report.texts += 1;
}

// ---- Email ----

fn read_mbox<R: BufRead>(reader: R, batch: &mut Batch) -> CommsResult<()> {
    let mut any = false;
    for message in mail_parser::mailbox::mbox::MessageIterator::new(reader) {
        let Ok(message) = message else { break };
        let fallback = message.internal_date() * 1000;
        read_email(message.contents(), Some(fallback), batch);
        any = true;
    }
    if !any {
        return Err("there is no mail in that mbox".into());
    }
    Ok(())
}

fn read_email(raw: &[u8], fallback_at: Option<u64>, batch: &mut Batch) {
    let Some(message) = mail_parser::MessageParser::new().parse(raw) else { return };
    let first = |list: Option<mail_parser::AddressList>| -> Vec<(String, String)> {
        list.map(|l| l.mailboxes().filter_map(|m| Some((m.name().unwrap_or("").trim().to_string(), m.address()?.trim().to_string()))).collect()).unwrap_or_default()
    };
    let Some(from) = first(message.from()).into_iter().next() else { return };
    let mut to = first(message.to());
    to.extend(first(message.cc()));
    let at = message.date().map(|d| d.to_timestamp()).filter(|t| *t > 0).map(|t| t as u64 * 1000).or(fallback_at).unwrap_or(0);
    let body = message.body_text(0).map(|b| b.to_string()).or_else(|| message.body_html(0).map(|h| mail_parser::html_to_text(&h))).unwrap_or_default();
    let mut attachments = Vec::new();
    for part in message.attachments() {
        let name = part.attachment_name().unwrap_or("attachment").to_string();
        let mime = part.content_type().map(|c| format!("{}/{}", c.ctype(), c.subtype().unwrap_or("octet-stream"))).unwrap_or_default();
        if let Some(att) = batch.keep(&name, &mime, &part.decoded()) {
            attachments.push(att);
        }
    }
    batch.emails.push(PendingEmail {
        at,
        from,
        to,
        subject: message.subject().unwrap_or("").to_string(),
        body: body.trim().to_string(),
        message_id: message.message_id().unwrap_or("").to_string(),
        attachments,
    });
    batch.report.emails += 1;
}

// ---- Discord and Messenger ----

/// Facebook writes UTF-8 as if each byte were a character ("cafÃ©"); this undoes that.
fn unmangle(text: &str) -> String {
    if text.chars().all(|c| (c as u32) < 0x100) && text.chars().any(|c| (c as u32) >= 0x80) {
        let bytes: Vec<u8> = text.chars().map(|c| c as u32 as u8).collect();
        if let Ok(fixed) = String::from_utf8(bytes) {
            return fixed;
        }
    }
    text.to_string()
}

fn s(v: &serde_json::Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn read_json_export(name: &str, v: &serde_json::Value, channel_info: Option<&serde_json::Value>, batch: &mut Batch) -> CommsResult<()> {
    // DiscordChatExporter: { guild, channel, messages: [{ timestamp, content, author }] }
    if v.get("channel").is_some() && v.get("messages").is_some_and(|m| m.is_array()) && v.get("guild").is_some() {
        let channel = &v["channel"];
        let guild = s(&v["guild"], "name");
        let chan = s(channel, "name");
        let title = if guild.is_empty() || guild == "Direct Messages" { chan.clone() } else { format!("{guild} · #{chan}") };
        let key = format!("{}|{}", s(channel, "id"), chan);
        let mut people = HashSet::new();
        for m in v["messages"].as_array().into_iter().flatten() {
            let author = &m["author"];
            let who = [s(author, "nickname"), s(author, "name")].into_iter().find(|n| !n.is_empty()).unwrap_or_default();
            people.insert(who.clone());
            let at = iso_millis(&s(m, "timestamp")).unwrap_or(0);
            let mut body = s(m, "content");
            for a in m["attachments"].as_array().into_iter().flatten() {
                let file = s(a, "fileName");
                if !file.is_empty() {
                    body.push_str(&format!("{}[{file}]", if body.is_empty() { "" } else { "\n" }));
                }
            }
            let msg = Message { id: short_hash(&format!("discord|{}", s(m, "id"))), at, from: who.clone(), address: s(author, "name"), kind: "chat".into(), body, ..Default::default() };
            batch.push("discord", &key, &title, Vec::new(), msg);
            batch.report.chats += 1;
        }
        if let Some((_, list)) = batch.convs.get_mut(&conv_id("discord", &key)) {
            let _ = list;
        }
        let id = conv_id("discord", &key);
        if let Some((meta, _)) = batch.convs.get_mut(&id) {
            meta.participants.extend(people.into_iter().filter(|p| !p.is_empty()));
        }
        return Ok(());
    }
    // Discord's own package: messages/c<id>/messages.json, only what you wrote.
    if let Some(list) = v.as_array().filter(|l| l.first().is_some_and(|m| m.get("Timestamp").is_some() && (m.get("Contents").is_some() || m.get("ID").is_some()))) {
        let info = channel_info.cloned().unwrap_or_default();
        let id = s(&info, "id");
        let title = {
            let named = s(&info, "name");
            let guild = info.get("guild").map(|g| s(g, "name")).unwrap_or_default();
            if !named.is_empty() && !guild.is_empty() {
                format!("{guild} · #{named}")
            } else if !named.is_empty() {
                named
            } else {
                format!("Discord channel {}", if id.is_empty() { name.to_string() } else { id.clone() })
            }
        };
        let key = if id.is_empty() { name.to_string() } else { id };
        for m in list {
            let at = iso_millis(&s(m, "Timestamp")).unwrap_or(0);
            let mut body = s(m, "Contents");
            let att = s(m, "Attachments");
            if !att.is_empty() {
                body.push_str(&format!("{}[{att}]", if body.is_empty() { "" } else { "\n" }));
            }
            let msg = Message { id: short_hash(&format!("discord|{}", m.get("ID").map(|x| x.to_string()).unwrap_or_default())), at, from: "Me".into(), me: true, kind: "chat".into(), body, ..Default::default() };
            batch.push("discord", &key, &title, Vec::new(), msg);
            batch.report.chats += 1;
        }
        return Ok(());
    }
    // Messenger: { participants, messages: [{ sender_name, timestamp_ms, content, photos }] },
    // or the newer { participants: ["name"], messages: [{ senderName, timestamp, text, media }] }.
    if v.get("participants").is_some() && v.get("messages").is_some_and(|m| m.is_array()) {
        let participants: Vec<String> = v["participants"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| unmangle(p.as_str().map(String::from).unwrap_or_else(|| s(p, "name")).as_str()))
            .filter(|p| !p.is_empty())
            .collect();
        let path = [s(v, "thread_path"), s(v, "threadName")].into_iter().find(|p| !p.is_empty()).unwrap_or_else(|| name.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_else(|| name.to_string()));
        let mut thread = FbThread { title: unmangle(&s(v, "title")), path, participants, messages: Vec::new() };
        for m in v["messages"].as_array().into_iter().flatten() {
            if m.get("is_unsent").and_then(|x| x.as_bool()).unwrap_or(false) {
                continue;
            }
            let sender = unmangle(&[s(m, "sender_name"), s(m, "senderName")].into_iter().find(|n| !n.is_empty()).unwrap_or_default());
            let at = m.get("timestamp_ms").or_else(|| m.get("timestamp")).and_then(|t| t.as_u64()).unwrap_or(0);
            let text = unmangle(&[s(m, "content"), s(m, "text")].into_iter().find(|n| !n.is_empty()).unwrap_or_default());
            let mut attachments = Vec::new();
            for key in ["photos", "videos", "audio_files", "files", "gifs", "media"] {
                for item in m[key].as_array().into_iter().flatten() {
                    let uri = s(item, "uri");
                    if !uri.is_empty() {
                        // The name is the address inside the download until read_zip finds it.
                        attachments.push(Attachment { name: uri, ..Default::default() });
                    }
                }
            }
            let call = m.get("call_duration").and_then(|d| d.as_u64());
            thread.messages.push((at, sender, text, attachments, call));
            batch.report.chats += 1;
        }
        batch.fb.push(thread);
        return Ok(());
    }
    batch.report.ignored += 1;
    Ok(())
}

// ---- Call recordings ----

/// A recorder names its files with the number and the time: "Call recording Dad_231113_142233",
/// "+15550102030_20231113142233", "Call@+1 555 010 2030(00:12)_20231113-142233".
fn read_recording(name: &str, bytes: &[u8], batch: &mut Batch) -> CommsResult<()> {
    let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let (number, at) = recording_name(stem);
    let at = at.map(|local| (local + batch.offset_ms).max(0) as u64);
    let mime = mime_guess::from_path(name).first_or_octet_stream().to_string();
    let Some(attachment) = batch.keep(name, &mime, bytes) else { return Err("that recording is empty or too big".into()) };
    batch.report.attachments -= 1; // counted as a recording instead
    batch.recordings.push(Recording { number, at, name: stem.to_string(), attachment });
    Ok(())
}

/// The number and the local time (milliseconds, as if it were UTC) a recording's name gives.
fn recording_name(stem: &str) -> (String, Option<i64>) {
    let runs: Vec<(usize, String)> = {
        let mut runs = Vec::new();
        let mut current = String::new();
        let mut start = 0;
        for (i, c) in stem.char_indices() {
            if c.is_ascii_digit() {
                if current.is_empty() {
                    start = i;
                }
                current.push(c);
            } else if !current.is_empty() {
                runs.push((start, std::mem::take(&mut current)));
            }
        }
        if !current.is_empty() {
            runs.push((start, current));
        }
        runs
    };
    // The time: yyyyMMddHHmmss in one run, or yyyyMMdd / yyMMdd then HHmmss.
    let mut at = None;
    let mut used = HashSet::new();
    for (i, (_, run)) in runs.iter().enumerate() {
        let joined = match (run.len(), runs.get(i + 1)) {
            (14, _) => Some(run.clone()),
            (8, Some((_, next))) if next.len() == 6 => Some(format!("{run}{next}")),
            (6, Some((_, next))) if next.len() == 6 => Some(format!("20{run}{next}")),
            _ => None,
        };
        if let Some(t) = joined.and_then(|t| civil_millis(&t)) {
            at = Some(t);
            used.insert(i);
            if run.len() != 14 {
                used.insert(i + 1);
            }
            break;
        }
    }
    // The number: the longest run of digits (with its spaces, dashes and brackets) left.
    let mut number = String::new();
    let cleaned: String = stem.chars().map(|c| if c.is_ascii_digit() || "+ -()".contains(c) { c } else { '|' }).collect();
    for piece in cleaned.split('|') {
        // "2030(00" is a number and the start of a call length in brackets.
        let piece = match piece.rfind('(') {
            Some(i) if !piece[i..].contains(')') => &piece[..i],
            _ => piece,
        };
        let digits: String = piece.chars().filter(|c| c.is_ascii_digit()).collect();
        let is_time = runs.iter().enumerate().any(|(i, (_, r))| used.contains(&i) && piece.contains(r.as_str()));
        if digits.len() >= 7 && digits.len() <= 15 && !is_time && digits.len() > number.chars().filter(|c| c.is_ascii_digit()).count() {
            number = piece.trim().trim_matches(|c| c == '-' || c == '(' || c == ')').trim().to_string();
        }
    }
    (number, at)
}

/// "20231113142233" as milliseconds, the time read as if it were UTC.
fn civil_millis(t: &str) -> Option<i64> {
    let n = |a: usize, b: usize| t.get(a..b)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, s) = (n(0, 4)?, n(4, 6)?, n(6, 8)?, n(8, 10)?, n(10, 12)?, n(12, 14)?);
    if !(1990..=2100).contains(&y) || !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    Some((days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + s) * 1000)
}

/// Days since 1970-01-01 (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// "2023-05-01T12:34:56.789+02:00", "…Z" or "2023-05-01 12:34:56" (UTC) as milliseconds.
fn iso_millis(text: &str) -> Option<u64> {
    let t = text.trim();
    let n = |a: usize, b: usize| t.get(a..b)?.parse::<i64>().ok();
    let (y, mo, d) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (h, mi, s) = (n(11, 13).unwrap_or(0), n(14, 16).unwrap_or(0), n(17, 19).unwrap_or(0));
    let mut rest = t.get(19..).unwrap_or("");
    let mut ms = 0;
    if let Some(frac) = rest.strip_prefix('.') {
        let digits: String = frac.chars().take_while(|c| c.is_ascii_digit()).collect();
        ms = format!("{:0<3}", &digits[..digits.len().min(3)]).parse().unwrap_or(0);
        rest = &frac[digits.len()..];
    }
    let offset = match rest.chars().next() {
        Some(sign @ ('+' | '-')) => {
            let hh: i64 = rest.get(1..3)?.parse().ok()?;
            let mm: i64 = rest.get(rest.len() - 2..)?.parse().unwrap_or(0);
            let total = (hh * 60 + mm) * 60_000;
            if sign == '+' { total } else { -total }
        }
        _ => 0,
    };
    let millis = (days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + s) * 1000 + ms - offset;
    (millis >= 0).then_some(millis as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const SMS: &str = r#"<?xml version='1.0' encoding='UTF-8' standalone='yes' ?>
<smses count="4">
  <sms protocol="0" address="+15550102030" date="1700000000000" type="1" body="Dinner at 6? &#55357;&#56832;" contact_name="Dad" />
  <sms protocol="0" address="(555) 010-2030" date="1700000060000" type="2" body="On my way&#10;see you" contact_name="Dad" />
  <mms date="1700000120000" msg_box="1" address="+15550102030~+15557770000" contact_name="Dad, Ana" sub="null">
    <parts>
      <part seq="-1" ct="application/smil" text="&lt;smil&gt;" />
      <part seq="0" ct="text/plain" text="Look at this" />
      <part seq="1" ct="image/jpeg" cl="IMG_1.jpg" data="/9j/4AAQSkZJRg==" />
    </parts>
    <addrs>
      <addr address="+15550102030" type="137" />
      <addr address="+15559990000" type="151" />
      <addr address="+15557770000" type="151" />
    </addrs>
  </mms>
  <mms date="1700000180000" msg_box="2" address="+15550102030~+15557770000" contact_name="Dad, Ana">
    <parts><part seq="0" ct="text/plain" text="Nice" /></parts>
    <addrs>
      <addr address="+15559990000" type="137" />
      <addr address="+15550102030" type="151" />
      <addr address="+15557770000" type="151" />
    </addrs>
  </mms>
</smses>"#;

    const CALLS: &str = r#"<?xml version='1.0' encoding='UTF-8' standalone='yes' ?>
<calls count="2">
  <call number="+15550102030" duration="125" date="1700003600000" type="2" contact_name="Dad" />
  <call number="5557770000" duration="0" date="1700007200000" type="3" contact_name="(Unknown)" />
</calls>"#;

    const MBOX: &str = "From x@y Sat Jan  1 00:00:00 2022\nFrom: Me <me@home.test>\nTo: Dad <dad@example.com>\nSubject: Photos\nDate: Mon, 13 Nov 2023 10:00:00 +0000\nMessage-ID: <1@home.test>\n\nHere they are.\n\nFrom x@y Sat Jan  1 00:00:00 2022\nFrom: Dad <dad@example.com>\nTo: me@home.test\nSubject: Re: Photos\nDate: Mon, 13 Nov 2023 11:00:00 +0000\nMessage-ID: <2@example.com>\n\nLovely, thanks!\n\nFrom x@y Sat Jan  1 00:00:00 2022\nFrom: Shop <news@shop.test>\nTo: me@home.test\nSubject: Sale\nDate: Mon, 13 Nov 2023 12:00:00 +0000\nMessage-ID: <3@shop.test>\n\nEverything must go.\n";

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("castle-comms-{}", crate::random_hex(6)));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn import(comms: &Comms, dir: &Path, name: &str, bytes: &[u8]) -> Report {
        let file = dir.join(format!("upload-{}", crate::random_hex(4)));
        std::fs::write(&file, bytes).unwrap();
        comms.begin("alex", name).unwrap();
        comms.import("alex", &file, name, 0).unwrap().report
    }

    #[test]
    fn xml_escapes_and_split_emoji_are_read() {
        assert_eq!(unescape_xml("a &amp; b &lt;3 &#55357;&#56832; &#x41;&#10;"), "a & b <3 😀 A\n");
        assert_eq!(unescape_xml("lonely &#55357; half"), "lonely \u{FFFD} half");
        assert_eq!(unescape_xml("R&D & co"), "R&D & co");
    }

    #[test]
    fn times_are_read_in_every_spelling() {
        assert_eq!(iso_millis("1970-01-01T00:00:01Z"), Some(1000));
        assert_eq!(iso_millis("2023-11-14T22:13:20.5+00:00"), Some(1_700_000_000_500));
        assert_eq!(iso_millis("2023-11-15T00:13:20+02:00"), Some(1_700_000_000_000));
        assert_eq!(iso_millis("2023-11-14 22:13:20"), Some(1_700_000_000_000));
        assert_eq!(civil_millis("20231114221320"), Some(1_700_000_000_000));
    }

    #[test]
    fn recording_names_give_their_number_and_time() {
        assert_eq!(recording_name("+15550102030_20231114221320"), ("+15550102030".into(), Some(1_700_000_000_000)));
        assert_eq!(recording_name("Call recording Dad_231114_221320"), (String::new(), Some(1_700_000_000_000)));
        assert_eq!(recording_name("Call@+1 555 010 2030(00:12)_20231114-221320").0, "+1 555 010 2030");
    }

    #[test]
    fn facebook_text_is_put_right() {
        assert_eq!(unmangle("caf\u{00c3}\u{00a9}"), "café");
        assert_eq!(unmangle("plain"), "plain");
        assert_eq!(unmangle("already café"), "already café");
    }

    #[test]
    fn texts_picture_messages_and_calls_land_in_their_conversations() {
        let dir = temp();
        let comms = Comms::open(dir.join("comms"));
        let report = import(&comms, &dir, "sms-20231114.xml", SMS.as_bytes());
        assert_eq!(report.texts, 4);
        assert_eq!(report.attachments, 1);
        let calls = import(&comms, &dir, "calls-20231114.xml", CALLS.as_bytes());
        assert_eq!(calls.calls, 2);

        let overview = comms.overview("alex").unwrap();
        let dad = overview.conversations.iter().find(|c| c.channel == "texts" && c.title == "Dad").unwrap();
        let thread = comms.thread("alex", &dad.id, None).unwrap();
        assert_eq!(thread.messages.len(), 2, "both spellings of Dad's number are one conversation");
        assert_eq!(thread.messages[0].body, "Dinner at 6? 😀");
        assert!(thread.messages[1].me);

        // The group: the phone's own number (+1 555 999 0000) is not one of the people in it.
        let group = overview.conversations.iter().find(|c| c.title == "Dad, Ana").unwrap();
        assert_eq!(group.participants.len(), 2);
        let messages = comms.thread("alex", &group.id, None).unwrap().messages;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].attachments[0].mime, "image/jpeg");
        assert!(comms.file("alex", &messages[0].attachments[0].file).is_ok());
        assert!(messages[1].me);

        let call = overview.conversations.iter().find(|c| c.channel == "calls" && c.title == "Dad").unwrap();
        assert_eq!(call.snippet, "Outgoing call, 2m 5s");

        // The same backup again adds nothing.
        let again = import(&comms, &dir, "sms-20231114.xml", SMS.as_bytes());
        assert_eq!(again.texts, 0);
        assert_eq!(again.known, 4);
        assert_eq!(comms.search("alex", "dinner").unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mail_is_sorted_by_the_other_person() {
        let dir = temp();
        let comms = Comms::open(dir.join("comms"));
        let report = import(&comms, &dir, "All mail.mbox", MBOX.as_bytes());
        assert_eq!(report.emails, 3);
        let overview = comms.overview("alex").unwrap();
        assert_eq!(overview.conversations.len(), 2);
        let dad = overview.conversations.iter().find(|c| c.title == "Dad").unwrap();
        let thread = comms.thread("alex", &dad.id, None).unwrap();
        assert_eq!(thread.messages.len(), 2);
        assert!(thread.messages[0].me, "the address in every message is the owner's");
        assert_eq!(thread.messages[1].body, "Lovely, thanks!");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_zip_of_messenger_discord_and_a_recording_is_read_whole() {
        let dir = temp();
        let comms = Comms::open(dir.join("comms"));
        let zip_path = dir.join("export.zip");
        {
            let file = std::fs::File::create(&zip_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default();
            let fb = |people: &str, msgs: &str| format!(r#"{{"participants":[{people}],"messages":[{msgs}],"title":"T","thread_path":"inbox/t"}}"#);
            zip.start_file("your_facebook_activity/messages/inbox/ana_1/message_1.json", opts).unwrap();
            zip.write_all(fb(r#"{"name":"Ana"},{"name":"Alex Cruz"}"#, r#"{"sender_name":"Ana","timestamp_ms":1700000000000,"content":"cafÃ©?","photos":[{"uri":"your_facebook_activity/messages/inbox/ana_1/photos/1.jpg"}]},{"sender_name":"Alex Cruz","timestamp_ms":1700000001000,"content":"yes"}"#).replace("\"T\"", "\"Ana\"").replace("inbox/t", "inbox/ana_1").as_bytes()).unwrap();
            zip.start_file("your_facebook_activity/messages/inbox/ana_1/photos/1.jpg", opts).unwrap();
            zip.write_all(&[0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
            zip.start_file("your_facebook_activity/messages/inbox/sam_2/message_1.json", opts).unwrap();
            zip.write_all(fb(r#"{"name":"Sam"},{"name":"Alex Cruz"}"#, r#"{"sender_name":"Sam","timestamp_ms":1700000002000,"call_duration":30}"#).replace("\"T\"", "\"Sam\"").replace("inbox/t", "inbox/sam_2").as_bytes()).unwrap();
            zip.start_file("discord/general.json", opts).unwrap();
            zip.write_all(br#"{"guild":{"name":"Home"},"channel":{"id":"9","name":"general"},"messages":[{"id":"1","timestamp":"2023-11-14T22:13:20+00:00","content":"gg","author":{"name":"jordan","nickname":"Jordan"}}]}"#).unwrap();
            zip.start_file("Recordings/+15550102030_20231114221320.m4a", opts).unwrap();
            zip.write_all(b"fake audio").unwrap();
            zip.start_file("contacts.vcf", opts).unwrap();
            zip.write_all(b"BEGIN:VCARD\nVERSION:3.0\nFN:Ana\nEND:VCARD\n").unwrap();
            zip.start_file("readme.txt", opts).unwrap();
            zip.write_all(b"hi").unwrap();
            zip.finish().unwrap();
        }
        comms.begin("alex", "export.zip").unwrap();
        let found = comms.import("alex", &zip_path, "export.zip", 0).unwrap();
        assert_eq!(found.vcards.len(), 1);
        assert_eq!(found.report.chats, 3);
        assert_eq!(found.report.recordings, 1);
        let overview = comms.overview("alex").unwrap();
        let ana = overview.conversations.iter().find(|c| c.channel == "facebook" && c.title == "Ana").unwrap();
        let messages = comms.thread("alex", &ana.id, None).unwrap().messages;
        assert_eq!(messages[0].body, "café?");
        assert_eq!(messages[0].attachments.len(), 1, "the photo comes from inside the zip");
        assert!(messages[1].me, "the person in every thread is the owner");
        let sam = overview.conversations.iter().find(|c| c.title == "Sam").unwrap();
        assert_eq!(comms.thread("alex", &sam.id, None).unwrap().messages[0].call.as_ref().unwrap().direction, "incoming");
        assert!(overview.conversations.iter().any(|c| c.channel == "discord" && c.title == "Home · #general"));

        // The recording goes onto the call from the log, imported afterwards it stays its own.
        let calls = import(&comms, &dir, "calls.xml", br#"<?xml version='1.0'?><calls><call number="+15550102030" duration="60" date="1700000030000" type="1" contact_name="Dad" /></calls>"#);
        assert_eq!(calls.calls, 1);
        assert!(overview.importing.is_none());

        let rec_dir = dir.join("rec");
        std::fs::create_dir_all(&rec_dir).unwrap();
        let rec = import(&comms, &rec_dir, "+15550102030_20231114221400.m4a", b"another recording");
        assert_eq!(rec.recordings, 1);

        // Deleting a conversation removes its pictures too.
        let file = dir.join("comms").join("alex").join("files");
        let before = std::fs::read_dir(&file).unwrap().count();
        comms.delete("alex", &ana.id).unwrap();
        assert_eq!(std::fs::read_dir(&file).unwrap().count(), before - 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn people_and_files_are_checked() {
        let comms = Comms::open(temp());
        assert!(comms.overview("../x").is_err());
        assert!(comms.file("alex", "../../token").is_err());
        assert!(comms.thread("alex", "nope", None).is_err());
    }
}
