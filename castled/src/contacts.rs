//! Contacts: each person's own address book, kept on the box.
//!
//! A phone, Google or iCloud hands its contacts over as a vCard file (.vcf); the box reads
//! it (versions 2.1, 3.0 and 4.0, the photos inside included) and keeps the list in the
//! vault's private folder, one book per person. Writing the book back out as a vCard is
//! how it goes onto an Android phone or an iPhone again: both open a .vcf and add what is
//! in it.
//!
//! Duplicates: two cards that share a phone number, an email address or the same name are
//! shown together, and merging keeps everything either one had.
//!
//! Nothing is lost by a mistake: before an import, a merge, a delete or a restore, the
//! whole book is kept as it was, and any of those copies can be put back.
//!
//! Who sees what: the box does not yet know which person is asking (every device holds the
//! same token), so the app names the person, exactly as Theater's limits are applied by the
//! app. A book is private by convention until the box has accounts, not by force.

use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

pub type ContactsResult<T> = Result<T, String>;

const MAX_CONTACTS: usize = 20_000;
const MAX_FIELDS: usize = 24;
const MAX_TEXT: usize = 300;
const MAX_NOTES: usize = 10_000;
const MAX_MEDIA: usize = 500;
/// A picture on a card is at most this big; phones send far smaller ones.
pub const MAX_PHOTO: usize = 5 * 1024 * 1024;
/// How many earlier copies of a book are kept.
const KEEP_SNAPSHOTS: usize = 30;
/// vCard lines are folded at 75 bytes (RFC 6350).
const FOLD_AT: usize = 75;

fn now_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// A label and a value: "Mobile" and a number, "Work" and an address.
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(default)]
pub struct Field {
    pub label: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Contact {
    pub id: String,
    /// The name as shown: "Dad", "Ana María López".
    pub name: String,
    pub given: String,
    pub family: String,
    pub nickname: String,
    pub phones: Vec<Field>,
    pub emails: Vec<Field>,
    pub addresses: Vec<Field>,
    pub urls: Vec<Field>,
    pub org: String,
    pub title: String,
    /// "1961-04-02", or "--04-02" when the year is not known.
    pub birthday: String,
    /// The bio: anything worth remembering about them.
    pub notes: String,
    pub groups: Vec<String>,
    /// Theater items this person is in (home videos with dad).
    pub media: Vec<String>,
    pub starred: bool,
    /// The kind of picture kept for the card ("image/jpeg"), empty when there is none.
    pub photo: String,
    /// When the picture last changed, so the app knows to fetch it again.
    pub photo_at: u64,
    /// Where the card came from: a file's name, or "Castle" for one made here.
    pub source: String,
    pub added: u64,
    pub modified: u64,
}

/// An earlier copy of the book, kept before something changed it.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub at: u64,
    pub reason: String,
    pub count: usize,
}

#[derive(Serialize, Deserialize, Default)]
struct SnapshotFile {
    at: u64,
    reason: String,
    contacts: Vec<Contact>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Duplicate {
    pub ids: Vec<String>,
    /// Why they look like one person: "same phone", "same email", "same name".
    pub reasons: Vec<String>,
}

#[derive(Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub added: usize,
    /// Cards already in the book, exactly.
    pub skipped: usize,
    /// Cards with no name, number or address at all.
    pub empty: usize,
}

pub struct Contacts {
    dir: PathBuf,
    /// One change to any book at a time.
    lock: Mutex<()>,
}

/// A person's name as a folder name: the app's person ids are short and plain.
pub fn valid_person(person: &str) -> bool {
    !person.is_empty() && person.len() <= 40 && person.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
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
    text[..end].to_string()
}

fn clean_fields(fields: Vec<Field>) -> Vec<Field> {
    let mut seen = BTreeSet::new();
    fields
        .into_iter()
        .map(|f| Field { label: clip(&f.label, 40), value: clip(&f.value, MAX_TEXT) })
        .filter(|f| !f.value.is_empty() && seen.insert(f.value.to_lowercase()))
        .take(MAX_FIELDS)
        .collect()
}

/// A card as the box keeps it: trimmed, capped, its name filled in from its parts.
fn tidy(mut c: Contact) -> Contact {
    c.name = clip(&c.name, MAX_TEXT);
    c.given = clip(&c.given, MAX_TEXT);
    c.family = clip(&c.family, MAX_TEXT);
    c.nickname = clip(&c.nickname, MAX_TEXT);
    c.org = clip(&c.org, MAX_TEXT);
    c.title = clip(&c.title, MAX_TEXT);
    c.notes = clip(&c.notes, MAX_NOTES);
    c.source = clip(&c.source, MAX_TEXT);
    c.birthday = birthday(&c.birthday);
    c.phones = clean_fields(c.phones);
    c.emails = clean_fields(c.emails);
    c.addresses = clean_fields(c.addresses);
    c.urls = clean_fields(c.urls);
    let mut groups: Vec<String> = c.groups.iter().map(|g| clip(g, 60)).filter(|g| !g.is_empty()).collect();
    groups.dedup();
    groups.truncate(MAX_FIELDS);
    c.groups = groups;
    let mut media: Vec<String> = Vec::new();
    for id in c.media.iter().map(|m| clip(m, 64)) {
        if !id.is_empty() && !media.contains(&id) && media.len() < MAX_MEDIA {
            media.push(id);
        }
    }
    c.media = media;
    if c.name.is_empty() {
        c.name = [c.given.as_str(), c.family.as_str()].iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join(" ");
    }
    if c.name.is_empty() {
        c.name = [&c.nickname, &c.org].iter().find(|p| !p.is_empty()).map(|p| p.to_string()).unwrap_or_default();
    }
    if c.name.is_empty() {
        c.name = c.phones.first().or(c.emails.first()).map(|f| f.value.clone()).unwrap_or_default();
    }
    c
}

fn is_empty(c: &Contact) -> bool {
    c.name.is_empty() && c.phones.is_empty() && c.emails.is_empty()
}

/// "19610402", "1961-04-02T00:00:00Z", "--0402" and "--04-02" become "1961-04-02" or "--04-02".
pub fn birthday(raw: &str) -> String {
    let raw = raw.trim();
    let (year, rest) = match raw.strip_prefix("--") {
        Some(rest) => ("", rest),
        None => {
            let date = raw.split('T').next().unwrap_or("");
            let digits: String = date.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() != 8 {
                return String::new();
            }
            return month_day(&digits[..4], &digits[4..]);
        }
    };
    let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() != 4 {
        return String::new();
    }
    month_day(year, &digits)
}

fn month_day(year: &str, md: &str) -> String {
    let (m, d): (u32, u32) = (md[..2].parse().unwrap_or(0), md[2..].parse().unwrap_or(0));
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return String::new();
    }
    if year.is_empty() || year == "1604" {
        // Apple writes 1604 for "no year".
        format!("--{m:02}-{d:02}")
    } else {
        format!("{year}-{m:02}-{d:02}")
    }
}

/// The part of a phone number that names the line: its digits, the last ten of them, so
/// "+1 (555) 010-2030" and "555-010-2030" are the same phone.
pub fn phone_key(number: &str) -> String {
    let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 5 {
        return String::new();
    }
    digits[digits.len().saturating_sub(10)..].to_string()
}

fn name_key(name: &str) -> String {
    name.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

/// What makes a card the same card: everything but its id and its times.
fn same_card(a: &Contact, b: &Contact) -> bool {
    let strip = |c: &Contact| Contact { id: String::new(), added: 0, modified: 0, photo_at: 0, source: String::new(), ..c.clone() };
    strip(a) == strip(b)
}

impl Contacts {
    pub fn open(dir: PathBuf) -> Self {
        Self { dir, lock: Mutex::new(()) }
    }

    fn person_dir(&self, person: &str) -> ContactsResult<PathBuf> {
        if !valid_person(person) {
            return Err("that person is not on this box".into());
        }
        Ok(self.dir.join(person))
    }

    fn read_book(&self, person: &str) -> ContactsResult<Vec<Contact>> {
        let file = self.person_dir(person)?.join("book.json");
        Ok(std::fs::read(file).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default())
    }

    fn write_book(&self, person: &str, book: &[Contact]) -> ContactsResult<()> {
        let dir = self.person_dir(person)?;
        write_json(&dir.join("book.json"), &book)
    }

    /// Keeps the book as it is now, before `reason` changes it.
    fn snapshot(&self, person: &str, book: &[Contact], reason: &str) -> ContactsResult<()> {
        if book.is_empty() {
            return Ok(());
        }
        let dir = self.person_dir(person)?.join("history");
        let at = now_millis();
        write_json(&dir.join(format!("{at}.json")), &SnapshotFile { at, reason: reason.into(), contacts: book.to_vec() })?;
        let mut kept = snapshot_files(&dir);
        while kept.len() > KEEP_SNAPSHOTS {
            let _ = std::fs::remove_file(dir.join(format!("{}.json", kept.remove(0))));
        }
        Ok(())
    }

    pub fn list(&self, person: &str) -> ContactsResult<Vec<Contact>> {
        let mut book = self.read_book(person)?;
        book.sort_by_key(|c| c.name.to_lowercase());
        Ok(book)
    }

    /// Adds a card (no id) or replaces one (its id).
    pub fn save(&self, person: &str, contact: Contact) -> ContactsResult<Contact> {
        let _held = self.lock.lock().map_err(|_| "contacts are unavailable".to_string())?;
        let mut book = self.read_book(person)?;
        let mut contact = tidy(contact);
        if is_empty(&contact) {
            return Err("a contact needs a name, a number or an address".into());
        }
        let now = now_millis();
        contact.modified = now;
        match book.iter_mut().find(|c| !contact.id.is_empty() && c.id == contact.id) {
            Some(existing) => {
                // The picture is changed only through its own call.
                contact.photo = existing.photo.clone();
                contact.photo_at = existing.photo_at;
                contact.added = existing.added;
                contact.source = existing.source.clone();
                *existing = contact.clone();
            }
            None => {
                if !contact.id.is_empty() {
                    return Err("that contact is gone".into());
                }
                if book.len() >= MAX_CONTACTS {
                    return Err("this book is full".into());
                }
                contact.id = crate::random_hex(8);
                contact.added = now;
                contact.photo = String::new();
                contact.photo_at = 0;
                if contact.source.is_empty() {
                    contact.source = "Castle".into();
                }
                book.push(contact.clone());
            }
        }
        self.write_book(person, &book)?;
        Ok(contact)
    }

    pub fn delete(&self, person: &str, ids: &[String]) -> ContactsResult<usize> {
        let _held = self.lock.lock().map_err(|_| "contacts are unavailable".to_string())?;
        let mut book = self.read_book(person)?;
        let before = book.len();
        let gone: Vec<Contact> = book.iter().filter(|c| ids.contains(&c.id)).cloned().collect();
        if gone.is_empty() {
            return Ok(0);
        }
        self.snapshot(person, &book, &format!("Before deleting {}", plural(gone.len(), "contact")))?;
        book.retain(|c| !ids.contains(&c.id));
        self.write_book(person, &book)?;
        let photos = self.person_dir(person)?.join("photos");
        for c in gone {
            let _ = std::fs::remove_file(photos.join(format!("{}.img", c.id)));
        }
        Ok(before - book.len())
    }

    /// Reads a vCard file into the book. Cards already in it, exactly, are skipped.
    pub fn import(&self, person: &str, text: &str, source: &str) -> ContactsResult<ImportReport> {
        let _held = self.lock.lock().map_err(|_| "contacts are unavailable".to_string())?;
        let cards = parse_vcards(text);
        if cards.is_empty() {
            return Err("there are no contacts in that file (Castle reads vCard .vcf files)".into());
        }
        let mut book = self.read_book(person)?;
        self.snapshot(person, &book, &format!("Before importing {source}"))?;
        let photos = self.person_dir(person)?.join("photos");
        let mut report = ImportReport::default();
        let now = now_millis();
        for (card, photo) in cards {
            let mut card = tidy(card);
            if is_empty(&card) {
                report.empty += 1;
                continue;
            }
            if book.iter().any(|c| same_card(&Contact { photo: String::new(), ..c.clone() }, &card)) {
                report.skipped += 1;
                continue;
            }
            if book.len() >= MAX_CONTACTS {
                return Err(format!("this book is full; {} were added before it filled", report.added));
            }
            card.id = crate::random_hex(8);
            card.source = clip(source, MAX_TEXT);
            card.added = now;
            card.modified = now;
            if let Some((mime, bytes)) = photo.filter(|(_, b)| !b.is_empty() && b.len() <= MAX_PHOTO) {
                std::fs::create_dir_all(&photos).map_err(|_| "could not keep a picture".to_string())?;
                if std::fs::write(photos.join(format!("{}.img", card.id)), &bytes).is_ok() {
                    card.photo = mime;
                    card.photo_at = now;
                }
            }
            book.push(card);
            report.added += 1;
        }
        self.write_book(person, &book)?;
        Ok(report)
    }

    /// The whole book, or the named cards, as one vCard 3.0 file with the pictures inside.
    pub fn export(&self, person: &str, ids: Option<&[String]>) -> ContactsResult<String> {
        let book = self.list(person)?;
        let photos = self.person_dir(person)?.join("photos");
        let mut out = String::new();
        for c in book.iter().filter(|c| ids.is_none_or(|ids| ids.contains(&c.id))) {
            let photo = (!c.photo.is_empty()).then(|| std::fs::read(photos.join(format!("{}.img", c.id))).ok()).flatten();
            out.push_str(&write_vcard(c, photo.as_deref()));
        }
        Ok(out)
    }

    pub fn duplicates(&self, person: &str) -> ContactsResult<Vec<Duplicate>> {
        Ok(find_duplicates(&self.read_book(person)?))
    }

    /// Folds the other cards into the first one: every number, address, group and linked
    /// item either had, and any detail the first one was missing.
    pub fn merge(&self, person: &str, ids: &[String]) -> ContactsResult<Contact> {
        let _held = self.lock.lock().map_err(|_| "contacts are unavailable".to_string())?;
        if ids.len() < 2 {
            return Err("pick at least two contacts to merge".into());
        }
        let mut book = self.read_book(person)?;
        let mut picked: Vec<Contact> = Vec::new();
        for id in ids {
            match book.iter().find(|c| &c.id == id) {
                Some(c) if !picked.iter().any(|p| p.id == c.id) => picked.push(c.clone()),
                Some(_) => {}
                None => return Err("one of those contacts is gone".into()),
            }
        }
        self.snapshot(person, &book, &format!("Before merging {}", picked.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")))?;
        let photos = self.person_dir(person)?.join("photos");
        let mut keep = picked.remove(0);
        for other in &picked {
            for (mine, theirs) in [
                (&mut keep.given, &other.given),
                (&mut keep.family, &other.family),
                (&mut keep.nickname, &other.nickname),
                (&mut keep.org, &other.org),
                (&mut keep.title, &other.title),
                (&mut keep.birthday, &other.birthday),
            ] {
                if mine.is_empty() {
                    *mine = theirs.clone();
                }
            }
            if !other.notes.is_empty() && !keep.notes.contains(&other.notes) {
                keep.notes = if keep.notes.is_empty() { other.notes.clone() } else { format!("{}\n\n{}", keep.notes, other.notes) };
            }
            keep.phones.extend(other.phones.iter().cloned());
            keep.emails.extend(other.emails.iter().cloned());
            keep.addresses.extend(other.addresses.iter().cloned());
            keep.urls.extend(other.urls.iter().cloned());
            keep.groups.extend(other.groups.iter().cloned());
            keep.media.extend(other.media.iter().cloned());
            keep.starred |= other.starred;
            if keep.photo.is_empty() && !other.photo.is_empty() {
                let from = photos.join(format!("{}.img", other.id));
                if std::fs::copy(&from, photos.join(format!("{}.img", keep.id))).is_ok() {
                    keep.photo = other.photo.clone();
                    keep.photo_at = now_millis();
                }
            }
            keep.added = keep.added.min(other.added);
        }
        // Phone numbers that differ only in how they are written are one number.
        let mut seen = BTreeSet::new();
        keep.phones.retain(|p| {
            let key = phone_key(&p.value);
            key.is_empty() || seen.insert(key)
        });
        keep.groups.sort();
        keep.groups.dedup();
        let mut keep = tidy(keep);
        keep.modified = now_millis();
        let gone: Vec<String> = picked.iter().map(|c| c.id.clone()).collect();
        book.retain(|c| !gone.contains(&c.id));
        if let Some(slot) = book.iter_mut().find(|c| c.id == keep.id) {
            *slot = keep.clone();
        }
        self.write_book(person, &book)?;
        for id in gone {
            let _ = std::fs::remove_file(photos.join(format!("{id}.img")));
        }
        Ok(keep)
    }

    pub fn snapshots(&self, person: &str) -> ContactsResult<Vec<Snapshot>> {
        let dir = self.person_dir(person)?.join("history");
        let mut list: Vec<Snapshot> = snapshot_files(&dir)
            .into_iter()
            .filter_map(|at| {
                let raw = std::fs::read(dir.join(format!("{at}.json"))).ok()?;
                let file: SnapshotFile = serde_json::from_slice(&raw).ok()?;
                Some(Snapshot { at: file.at, reason: file.reason, count: file.contacts.len() })
            })
            .collect();
        list.reverse();
        Ok(list)
    }

    /// Puts an earlier copy back. The book as it is now is kept first, so this too can be undone.
    pub fn restore(&self, person: &str, at: u64) -> ContactsResult<usize> {
        let _held = self.lock.lock().map_err(|_| "contacts are unavailable".to_string())?;
        let dir = self.person_dir(person)?.join("history");
        let raw = std::fs::read(dir.join(format!("{at}.json"))).map_err(|_| "that copy is gone".to_string())?;
        let file: SnapshotFile = serde_json::from_slice(&raw).map_err(|_| "that copy cannot be read".to_string())?;
        let book = self.read_book(person)?;
        self.snapshot(person, &book, "Before restoring an earlier copy")?;
        self.write_book(person, &file.contacts)?;
        Ok(file.contacts.len())
    }

    pub fn photo(&self, person: &str, id: &str) -> ContactsResult<(PathBuf, String)> {
        let book = self.read_book(person)?;
        let c = book.iter().find(|c| c.id == id && !c.photo.is_empty()).ok_or("no picture")?;
        Ok((self.person_dir(person)?.join("photos").join(format!("{}.img", c.id)), c.photo.clone()))
    }

    /// Sets a card's picture (or takes it away, with no bytes).
    pub fn set_photo(&self, person: &str, id: &str, mime: &str, bytes: &[u8]) -> ContactsResult<Contact> {
        let _held = self.lock.lock().map_err(|_| "contacts are unavailable".to_string())?;
        if bytes.len() > MAX_PHOTO {
            return Err("that picture is too big (5 MB at most)".into());
        }
        if !bytes.is_empty() && !matches!(mime, "image/jpeg" | "image/png" | "image/webp" | "image/gif") {
            return Err("a picture must be JPEG, PNG, WebP or GIF".into());
        }
        let mut book = self.read_book(person)?;
        let photos = self.person_dir(person)?.join("photos");
        let c = book.iter_mut().find(|c| c.id == id).ok_or("that contact is gone")?;
        let file = photos.join(format!("{}.img", c.id));
        if bytes.is_empty() {
            let _ = std::fs::remove_file(&file);
            c.photo = String::new();
        } else {
            std::fs::create_dir_all(&photos).map_err(|_| "could not keep the picture".to_string())?;
            std::fs::write(&file, bytes).map_err(|_| "could not keep the picture".to_string())?;
            c.photo = mime.into();
        }
        c.photo_at = now_millis();
        c.modified = c.photo_at;
        let changed = c.clone();
        self.write_book(person, &book)?;
        Ok(changed)
    }
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// The times of the kept copies in a history folder, oldest first.
fn snapshot_files(dir: &Path) -> Vec<u64> {
    let mut list: Vec<u64> = std::fs::read_dir(dir)
        .map(|entries| entries.flatten().filter_map(|e| e.file_name().to_str()?.strip_suffix(".json")?.parse().ok()).collect())
        .unwrap_or_default();
    list.sort();
    list
}

pub(crate) fn write_json<T: Serialize + ?Sized>(file: &Path, value: &T) -> Result<(), String> {
    let dir = file.parent().ok_or("no folder")?;
    std::fs::create_dir_all(dir).map_err(|_| "could not save".to_string())?;
    let tmp = file.with_extension(format!("tmp-{}", crate::random_hex(4)));
    let bytes = serde_json::to_vec(value).map_err(|_| "could not save".to_string())?;
    std::fs::write(&tmp, bytes).map_err(|_| "could not save".to_string())?;
    std::fs::rename(&tmp, file).map_err(|_| {
        let _ = std::fs::remove_file(&tmp);
        "could not save".to_string()
    })
}

fn find_duplicates(book: &[Contact]) -> Vec<Duplicate> {
    // Union-find over the cards, joined by anything two of them share.
    let mut parent: Vec<usize> = (0..book.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut reasons: HashMap<(usize, usize), BTreeSet<&str>> = HashMap::new();
    let mut by_key: HashMap<(&str, String), usize> = HashMap::new();
    for (i, c) in book.iter().enumerate() {
        let mut keys: Vec<(&str, String)> = Vec::new();
        keys.extend(c.phones.iter().map(|p| ("same phone", phone_key(&p.value))));
        keys.extend(c.emails.iter().map(|e| ("same email", e.value.trim().to_lowercase())));
        keys.push(("same name", name_key(&c.name)));
        for (why, key) in keys.into_iter().filter(|(_, k)| !k.is_empty()) {
            match by_key.get(&(why, key.clone())) {
                Some(&j) if j != i => {
                    let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                    parent[a] = b;
                    reasons.entry((i.min(j), i.max(j))).or_default().insert(why);
                }
                Some(_) => {}
                None => {
                    by_key.insert((why, key), i);
                }
            }
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..book.len() {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    let mut out: Vec<Duplicate> = groups
        .into_values()
        .filter(|members| members.len() > 1)
        .map(|members| {
            let mut why: BTreeSet<&str> = BTreeSet::new();
            for (&(a, b), set) in &reasons {
                if members.contains(&a) && members.contains(&b) {
                    why.extend(set.iter().copied());
                }
            }
            // The fullest card first: it is the one a merge keeps.
            let mut members = members;
            members.sort_by_key(|&i| std::cmp::Reverse(fullness(&book[i])));
            Duplicate { ids: members.iter().map(|&i| book[i].id.clone()).collect(), reasons: why.into_iter().map(String::from).collect() }
        })
        .collect();
    out.sort_by_key(|d| book.iter().find(|c| c.id == d.ids[0]).map(|c| c.name.to_lowercase()).unwrap_or_default());
    out
}

fn fullness(c: &Contact) -> usize {
    c.phones.len() + c.emails.len() + c.addresses.len() + usize::from(!c.photo.is_empty()) * 2 + usize::from(!c.notes.is_empty()) + usize::from(!c.birthday.is_empty())
}

// ---------------------------------------------------------------------------------------
// vCard
// ---------------------------------------------------------------------------------------

/// One property of a card: `item1.TEL;TYPE=CELL:+1 555` is group item1, name TEL.
struct Prop {
    group: String,
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

impl Prop {
    fn param(&self, key: &str) -> Option<&str> {
        self.params.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// Every TYPE given, lower case: "TYPE=cell,voice", "TYPE=HOME" and 2.1's bare "CELL".
    fn types(&self) -> Vec<String> {
        self.params
            .iter()
            .filter(|(k, _)| k == "TYPE" || k.is_empty())
            .flat_map(|(_, v)| v.split(',').map(|t| t.trim().trim_matches('"').to_lowercase()))
            .filter(|t| !t.is_empty())
            .collect()
    }
}

/// Lines with their folds undone: a line starting with a space or tab continues the one
/// before it, and in 2.1's quoted-printable a line ending in "=" goes on to the next.
fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut qp_open = false;
    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if qp_open {
            if let Some(last) = lines.last_mut() {
                last.pop(); // the soft break's "="
                last.push_str(line);
                qp_open = line.ends_with('=');
                continue;
            }
        }
        if (line.starts_with(' ') || line.starts_with('\t')) && !lines.is_empty() {
            lines.last_mut().unwrap().push_str(&line[1..]);
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        let upper = line.to_ascii_uppercase();
        qp_open = upper.contains("QUOTED-PRINTABLE") && line.ends_with('=') && line.contains(':');
        lines.push(line.to_string());
    }
    lines
}

fn parse_prop(line: &str) -> Option<Prop> {
    // The value starts at the first colon that is not inside a quoted parameter.
    let mut quoted = false;
    let colon = line.char_indices().find(|&(_, c)| {
        if c == '"' {
            quoted = !quoted;
        }
        c == ':' && !quoted
    })?;
    let (head, value) = (&line[..colon.0], &line[colon.0 + 1..]);
    let mut parts = head.split(';');
    let full = parts.next()?.trim();
    let (group, name) = match full.rsplit_once('.') {
        Some((g, n)) => (g.to_string(), n.to_ascii_uppercase()),
        None => (String::new(), full.to_ascii_uppercase()),
    };
    let params = parts
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (k.trim().to_ascii_uppercase(), v.trim().trim_matches('"').to_string()),
            None => (String::new(), p.trim().to_string()),
        })
        .collect();
    Some(Prop { group, name, params, value: value.to_string() })
}

/// The text of a value: quoted-printable decoded, then vCard's backslash escapes undone.
fn text_of(prop: &Prop) -> String {
    let raw = if prop.param("ENCODING").is_some_and(|e| e.eq_ignore_ascii_case("QUOTED-PRINTABLE")) || prop.params.iter().any(|(k, v)| k.is_empty() && v.eq_ignore_ascii_case("QUOTED-PRINTABLE")) {
        quoted_printable(&prop.value)
    } else {
        prop.value.clone()
    };
    unescape(&raw)
}

fn quoted_printable(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(b) = hex {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') | Some('N') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// A structured value (N, ADR) split at its unescaped semicolons.
fn components(raw: &str) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut escaped = false;
    for c in raw.chars() {
        if escaped {
            parts.last_mut().unwrap().push('\\');
            parts.last_mut().unwrap().push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == ';' {
            parts.push(String::new());
        } else {
            parts.last_mut().unwrap().push(c);
        }
    }
    parts.iter().map(|p| unescape(p).trim().to_string()).collect()
}

fn label_for(types: &[String], custom: Option<&String>) -> String {
    if let Some(label) = custom {
        return label.trim_start_matches("_$!<").trim_end_matches(">!$_").to_string();
    }
    for (t, label) in [("cell", "Mobile"), ("iphone", "iPhone"), ("main", "Main"), ("home", "Home"), ("work", "Work"), ("fax", "Fax"), ("pager", "Pager"), ("other", "Other")] {
        if types.iter().any(|x| x == t) {
            return label.into();
        }
    }
    String::new()
}

fn image_mime(kind: &str, bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xD8]) {
        return "image/jpeg".into();
    }
    if bytes.starts_with(b"\x89PNG") {
        return "image/png".into();
    }
    if bytes.starts_with(b"GIF8") {
        return "image/gif".into();
    }
    if bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        return "image/webp".into();
    }
    match kind.to_ascii_lowercase().as_str() {
        "png" | "image/png" => "image/png".into(),
        _ => "image/jpeg".into(),
    }
}

/// A picture inside a card: base64 after ENCODING=b or BASE64, or a data: address in 4.0.
/// A picture somewhere on the internet is not fetched.
fn photo_of(prop: &Prop) -> Option<(String, Vec<u8>)> {
    let value = prop.value.trim();
    let (kind, data) = if let Some(rest) = value.strip_prefix("data:") {
        let (meta, data) = rest.split_once(',')?;
        if !meta.contains("base64") {
            return None;
        }
        (meta.split(';').next().unwrap_or("").to_string(), data.to_string())
    } else {
        let encoded = prop.param("ENCODING").is_some_and(|e| e.eq_ignore_ascii_case("b") || e.eq_ignore_ascii_case("BASE64"))
            || prop.params.iter().any(|(k, v)| k.is_empty() && v.eq_ignore_ascii_case("BASE64"));
        if !encoded {
            return None;
        }
        (prop.types().into_iter().next().unwrap_or_default(), value.to_string())
    };
    let bytes = base64_decode(&data)?;
    Some((image_mime(&kind, &bytes), bytes))
}

/// Every card in a vCard file, with its picture when it carries one.
pub fn parse_vcards(text: &str) -> Vec<(Contact, Option<(String, Vec<u8>)>)> {
    let text = text.trim_start_matches('\u{feff}');
    let mut cards = Vec::new();
    let mut props: Vec<Prop> = Vec::new();
    let mut inside = false;
    for line in unfold(text) {
        let Some(prop) = parse_prop(&line) else { continue };
        match (prop.name.as_str(), prop.value.trim().to_ascii_uppercase().as_str()) {
            ("BEGIN", "VCARD") => {
                inside = true;
                props.clear();
            }
            ("END", "VCARD") if inside => {
                inside = false;
                cards.push(card_from(&props));
                props.clear();
            }
            _ if inside => props.push(prop),
            _ => {}
        }
    }
    cards
}

fn card_from(props: &[Prop]) -> (Contact, Option<(String, Vec<u8>)>) {
    // iOS names a field in a separate property of the same group: item1.X-ABLabel:Gym.
    let labels: HashMap<&str, String> = props.iter().filter(|p| p.name == "X-ABLABEL" && !p.group.is_empty()).map(|p| (p.group.as_str(), text_of(p))).collect();
    let mut c = Contact::default();
    let mut photo = None;
    for p in props {
        let custom = labels.get(p.group.as_str());
        let field = || Field { label: label_for(&p.types(), custom), value: text_of(p).trim().to_string() };
        match p.name.as_str() {
            "FN" => c.name = text_of(p),
            "N" => {
                let raw = if p.param("ENCODING").is_some_and(|e| e.eq_ignore_ascii_case("QUOTED-PRINTABLE")) { quoted_printable(&p.value) } else { p.value.clone() };
                let parts = components(&raw);
                c.family = parts.first().cloned().unwrap_or_default();
                c.given = [parts.get(1), parts.get(2)].iter().flatten().filter(|s| !s.is_empty()).map(|s| s.as_str()).collect::<Vec<_>>().join(" ");
            }
            "NICKNAME" => c.nickname = text_of(p),
            "TEL" => c.phones.push(field()),
            "EMAIL" => c.emails.push(field()),
            "URL" => c.urls.push(field()),
            "ADR" => {
                let raw = if p.param("ENCODING").is_some_and(|e| e.eq_ignore_ascii_case("QUOTED-PRINTABLE")) { quoted_printable(&p.value) } else { p.value.clone() };
                // post box; extended; street; city; region; postcode; country
                let parts = components(&raw);
                let line = parts.iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(", ");
                c.addresses.push(Field { label: label_for(&p.types(), custom), value: line });
            }
            "ORG" => c.org = components(&p.value).into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", "),
            "TITLE" => c.title = text_of(p),
            "BDAY" => c.birthday = text_of(p),
            "NOTE" => c.notes = text_of(p),
            "CATEGORIES" => c.groups.extend(text_of(p).split(',').map(|g| g.trim().to_string())),
            "PHOTO" if photo.is_none() => photo = photo_of(p),
            _ => {}
        }
    }
    (c, photo)
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\n', "\\n").replace(',', "\\,").replace(';', "\\;")
}

/// Folds a line at 75 bytes, never inside a character.
fn fold(line: &str, out: &mut String) {
    let mut start = 0;
    let mut first = true;
    while start < line.len() {
        let room = if first { FOLD_AT } else { FOLD_AT - 1 };
        let mut end = (start + room).min(line.len());
        while !line.is_char_boundary(end) {
            end -= 1;
        }
        if !first {
            out.push(' ');
        }
        out.push_str(&line[start..end]);
        out.push_str("\r\n");
        start = end;
        first = false;
    }
}

pub fn write_vcard(c: &Contact, photo: Option<&[u8]>) -> String {
    let mut lines: Vec<String> = vec!["BEGIN:VCARD".into(), "VERSION:3.0".into(), format!("FN:{}", escape(&c.name))];
    lines.push(format!("N:{};{};;;", escape(&c.family), escape(&c.given)));
    if !c.nickname.is_empty() {
        lines.push(format!("NICKNAME:{}", escape(&c.nickname)));
    }
    let typed = |label: &str| match label.to_lowercase().as_str() {
        "mobile" | "cell" | "iphone" => Some("CELL"),
        "home" => Some("HOME"),
        "work" => Some("WORK"),
        "fax" => Some("FAX"),
        "main" => Some("MAIN"),
        "pager" => Some("PAGER"),
        "other" => Some("OTHER"),
        _ => None,
    };
    let mut item = 0;
    let mut push_field = |lines: &mut Vec<String>, name: &str, f: &Field, value: String| match typed(&f.label) {
        Some(t) => lines.push(format!("{name};TYPE={t}:{value}")),
        None if f.label.is_empty() => lines.push(format!("{name}:{value}")),
        None => {
            item += 1;
            lines.push(format!("item{item}.{name}:{value}"));
            lines.push(format!("item{item}.X-ABLabel:{}", escape(&f.label)));
        }
    };
    for f in &c.phones {
        push_field(&mut lines, "TEL", f, escape(&f.value));
    }
    for f in &c.emails {
        push_field(&mut lines, "EMAIL", f, escape(&f.value));
    }
    for f in &c.addresses {
        push_field(&mut lines, "ADR", f, format!(";;{};;;;", escape(&f.value)));
    }
    for f in &c.urls {
        push_field(&mut lines, "URL", f, escape(&f.value));
    }
    if !c.org.is_empty() {
        lines.push(format!("ORG:{}", escape(&c.org)));
    }
    if !c.title.is_empty() {
        lines.push(format!("TITLE:{}", escape(&c.title)));
    }
    if !c.birthday.is_empty() {
        lines.push(format!("BDAY:{}", c.birthday));
    }
    if !c.notes.is_empty() {
        lines.push(format!("NOTE:{}", escape(&c.notes)));
    }
    if !c.groups.is_empty() {
        lines.push(format!("CATEGORIES:{}", c.groups.iter().map(|g| escape(g)).collect::<Vec<_>>().join(",")));
    }
    if let Some(bytes) = photo {
        let kind = if c.photo == "image/png" { "PNG" } else { "JPEG" };
        lines.push(format!("PHOTO;ENCODING=b;TYPE={kind}:{}", base64_encode(bytes)));
    }
    lines.push("END:VCARD".into());
    let mut out = String::new();
    for line in lines {
        fold(&line, &mut out);
    }
    out
}

// ---------------------------------------------------------------------------------------
// base64, for pictures inside cards and messages
// ---------------------------------------------------------------------------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Decodes base64, standard or URL-safe, ignoring whitespace. None when it is not base64.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0;
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b' ' | b'\t' | b'\r' | b'\n' => continue,
            _ => return None,
        };
        buf = (buf << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANDROID: &str = "BEGIN:VCARD\r\nVERSION:2.1\r\nN;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:L=C3=B3pez;Ana;Mar=C3=ADa;;\r\nFN;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:Ana Mar=C3=ADa L=C3=B3pez\r\nTEL;CELL;PREF:+1 (555) 010-2030\r\nTEL;HOME:555-010-9999\r\nEMAIL;HOME:ana@example.com\r\nBDAY:19610402\r\nNOTE;ENCODING=QUOTED-PRINTABLE:Likes the=0D=0Agarden=\r\n and tea\r\nEND:VCARD\r\n";

    const IPHONE: &str = "BEGIN:VCARD\nVERSION:3.0\nPRODID:-//Apple Inc.//iPhone OS 18.0//EN\nN:Cruz;Dad;;;\nFN:Dad\nitem1.TEL;type=pref:5550102030\nitem1.X-ABLabel:_$!<Mobile>!$_\nitem2.EMAIL;type=INTERNET:dad@example.com\nitem2.X-ABLabel:Fishing club\nADR;type=HOME:;;12 Oak St;Springfield;IL;62701;USA\nBDAY;X-APPLE-OMIT-YEAR=1604:1604-07-19\nPHOTO;ENCODING=b;TYPE=JPEG:/9j/4AAQ\n SkZJRg==\nEND:VCARD\n";

    #[test]
    fn an_android_export_is_read_quoted_printable_and_all() {
        let cards = parse_vcards(ANDROID);
        assert_eq!(cards.len(), 1);
        let c = tidy(cards[0].0.clone());
        assert_eq!(c.name, "Ana María López");
        assert_eq!(c.family, "López");
        assert_eq!(c.given, "Ana María");
        assert_eq!(c.phones, vec![Field { label: "Mobile".into(), value: "+1 (555) 010-2030".into() }, Field { label: "Home".into(), value: "555-010-9999".into() }]);
        assert_eq!(c.birthday, "1961-04-02");
        assert_eq!(c.notes, "Likes the\r\ngarden and tea");
    }

    #[test]
    fn an_iphone_export_keeps_custom_labels_addresses_and_the_picture() {
        let cards = parse_vcards(IPHONE);
        let (c, photo) = &cards[0];
        let c = tidy(c.clone());
        assert_eq!(c.name, "Dad");
        assert_eq!(c.phones[0].label, "Mobile");
        assert_eq!(c.emails[0], Field { label: "Fishing club".into(), value: "dad@example.com".into() });
        assert_eq!(c.addresses[0].value, "12 Oak St, Springfield, IL, 62701, USA");
        assert_eq!(c.birthday, "--07-19");
        let (mime, bytes) = photo.as_ref().unwrap();
        assert_eq!(mime, "image/jpeg");
        assert!(bytes.starts_with(&[0xFF, 0xD8]));
    }

    #[test]
    fn a_card_written_out_reads_back_the_same() {
        let c = tidy(Contact {
            name: "Jo; the \"builder\", Smith".into(),
            given: "Jo".into(),
            family: "Smith".into(),
            phones: vec![Field { label: "Mobile".into(), value: "+44 20 7946 0000".into() }, Field { label: "Boat".into(), value: "555 1234".into() }],
            notes: "Line one\nLine two, with a comma; and a semicolon".into(),
            birthday: "--12-25".into(),
            groups: vec!["Family".into()],
            ..Default::default()
        });
        let text = write_vcard(&c, Some(&[0xFF, 0xD8, 1, 2, 3]));
        assert!(text.lines().all(|l| l.len() <= FOLD_AT));
        let (back, photo) = parse_vcards(&text).remove(0);
        let back = tidy(back);
        assert_eq!(back.name, c.name);
        assert_eq!(back.phones, c.phones);
        assert_eq!(back.notes, c.notes);
        assert_eq!(back.birthday, c.birthday);
        assert_eq!(back.groups, c.groups);
        assert_eq!(photo.unwrap().1, vec![0xFF, 0xD8, 1, 2, 3]);
    }

    #[test]
    fn base64_goes_both_ways() {
        for bytes in [&b""[..], b"a", b"ab", b"abc", b"hello world!"] {
            assert_eq!(base64_decode(&base64_encode(bytes)).unwrap(), bytes);
        }
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert!(base64_decode("not*base64").is_none());
    }

    #[test]
    fn duplicates_are_found_by_phone_email_and_name() {
        let card = |id: &str, name: &str, phone: &str, email: &str| Contact {
            id: id.into(),
            name: name.into(),
            phones: if phone.is_empty() { vec![] } else { vec![Field { label: String::new(), value: phone.into() }] },
            emails: if email.is_empty() { vec![] } else { vec![Field { label: String::new(), value: email.into() }] },
            ..Default::default()
        };
        let book = vec![
            card("a", "Dad", "+1 555 010 2030", ""),
            card("b", "Robert Cruz", "(555) 010-2030", "bob@example.com"),
            card("c", "Bob", "", "BOB@example.com"),
            card("d", "Ana", "555 777 8888", ""),
            card("e", "ana", "", ""),
            card("f", "Someone else", "555 999 0000", ""),
        ];
        let found = find_duplicates(&book);
        assert_eq!(found.len(), 2);
        let mut first = found.iter().find(|d| d.ids.contains(&"a".to_string())).unwrap().ids.clone();
        first.sort();
        assert_eq!(first, vec!["a", "b", "c"]);
        let ana = found.iter().find(|d| d.ids.contains(&"d".to_string())).unwrap();
        assert_eq!(ana.reasons, vec!["same name"]);
    }

    #[test]
    fn import_merge_delete_and_restore_keep_the_book_safe() {
        let dir = std::env::temp_dir().join(format!("castle-contacts-{}", crate::random_hex(6)));
        let book = Contacts::open(dir.clone());
        let report = book.import("alex", &format!("{ANDROID}{IPHONE}"), "phone.vcf").unwrap();
        assert_eq!(report, ImportReport { added: 2, skipped: 0, empty: 0 });
        // The same file again adds nothing.
        assert_eq!(book.import("alex", IPHONE, "phone.vcf").unwrap().skipped, 1);
        let list = book.list("alex").unwrap();
        let dad = list.iter().find(|c| c.name == "Dad").unwrap();
        assert_eq!(dad.photo, "image/jpeg");
        let ana = list.iter().find(|c| c.name.starts_with("Ana")).unwrap();

        // Dad and Ana share 555-010-2030 in this test, so they show as one person.
        let dups = book.duplicates("alex").unwrap();
        assert_eq!(dups.len(), 1);
        let merged = book.merge("alex", &[ana.id.clone(), dad.id.clone()]).unwrap();
        assert_eq!(merged.phones.len(), 2, "the same number written two ways is kept once");
        assert_eq!(merged.emails.len(), 2);
        assert_eq!(merged.photo, "image/jpeg", "the picture comes over from the merged card");
        assert_eq!(book.list("alex").unwrap().len(), 1);

        // Another person's book is their own.
        assert!(book.list("jordan").unwrap().is_empty());
        assert!(book.list("../alex").is_err());

        let copies = book.snapshots("alex").unwrap();
        let before_merge = copies.iter().find(|s| s.reason.starts_with("Before merging")).unwrap();
        assert_eq!(before_merge.count, 2);
        assert_eq!(book.restore("alex", before_merge.at).unwrap(), 2);
        assert_eq!(book.list("alex").unwrap().len(), 2);

        let exported = book.export("alex", None).unwrap();
        assert_eq!(parse_vcards(&exported).len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_card_needs_something_to_it() {
        let dir = std::env::temp_dir().join(format!("castle-contacts-{}", crate::random_hex(6)));
        let book = Contacts::open(dir.clone());
        assert!(book.save("alex", Contact::default()).is_err());
        let made = book.save("alex", Contact { given: "Sam".into(), family: "Lee".into(), birthday: "2015-3-4".into(), ..Default::default() }).unwrap();
        assert_eq!(made.name, "Sam Lee");
        assert_eq!(made.birthday, "", "a birthday that is not a date is dropped");
        let _ = std::fs::remove_dir_all(dir);
    }
}
