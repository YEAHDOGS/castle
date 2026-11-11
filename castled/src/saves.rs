//! Save data for the Arcade: finds every save file the emulators have written, ties each to
//! its game, and keeps every version it has ever seen so any of them can be put back.
//!
//! Finding and recording only read the games folder. Versions are kept in the vault, not on
//! the games drive: `objects/<sha256>` holds each distinct file once, and `journal.jsonl`
//! lists every version in order. A version's name is its fingerprint, so a stored version
//! that was altered no longer matches its name.
//!
//! Putting a version back is the one thing here that writes to the games folder. It
//! replaces that single save file, and records what was there first.
//!
//! RetroArch is set to sort saves by core, so the layout read is
//! `<saves or states>/<core>/<game file name>.<ext>`: the same game has separate saves per
//! core. A save belongs to a game when its name, without the extension, is the game file's.

use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Folders, under the games folder, that hold one sub-folder of saves per core.
const SAVE_ROOTS: [&str; 5] =
    ["Consoles/RetroArch-Win64/saves", "Consoles/RetroArch-Win64/states", "Saves/RetroArch", "Saves/RetroArch/saves", "Saves/RetroArch/states"];
/// Folder names that hold cores rather than being one; they are read as roots of their own.
const NOT_A_CORE: [&str; 2] = ["saves", "states"];
/// In-game saves: what the game itself wrote to its cartridge or memory card.
const BATTERY_EXTENSIONS: [&str; 12] = ["srm", "sav", "mcr", "mcd", "ps2", "gci", "dsv", "sra", "eep", "fla", "mpk", "nv"];
/// Save states: the emulator's snapshot of a moment.
const STATE_EXTENSIONS: [&str; 2] = ["p2s", "ppst"];
const STATE_MARKER: &str = ".state"; // RetroArch: name.state, name.state1, name.state.auto
const MAX_DEPTH: usize = 6;
const MAX_SAVES: usize = 5000;
/// Larger files are listed but not recorded: no save is this big, and it bounds memory.
const MAX_RECORDED_BYTES: u64 = 64 * 1024 * 1024;
const JOURNAL: &str = "journal.jsonl";
const OBJECTS: &str = "objects";

pub type SavesResult<T> = Result<T, String>;

#[derive(Serialize, Clone)]
pub struct GameRef {
    pub system: String,
    pub id: usize,
    pub title: String,
}

/// A game file's name without its extension, and the game it is. GameCube games are also
/// listed under `gamecube_key(code)`, because their memory-card saves are named by code.
pub type RomIndex = HashMap<String, GameRef>;

/// The index key for a GameCube game code. It starts with a character no file name can
/// contain, so it can never be mistaken for a game file's name.
pub fn gamecube_key(code: &str) -> String {
    format!("/gamecube/{}", code.to_ascii_uppercase())
}

/// The game code in a GameCube memory-card file name: "7D-GZQE-GZQE.gci" -> "GZQE".
fn gci_code(name: &str) -> Option<&str> {
    const CODE_LEN: usize = 4;
    if !name.to_ascii_lowercase().ends_with(".gci") {
        return None;
    }
    let code = name.split('-').nth(1)?;
    (code.len() == CODE_LEN && code.bytes().all(|b| b.is_ascii_alphanumeric())).then_some(code)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveFile {
    /// Its path under the games folder, with forward slashes. This is how the app names it.
    id: String,
    name: String,
    core: String,
    /// `battery` (the game's own save) or `state` (an emulator save state).
    kind: &'static str,
    game: Option<GameRef>,
    size: u64,
    modified: u64,
    versions: usize,
    /// The file as it is now is already the newest recorded version.
    recorded: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Version {
    seq: u64,
    /// When Castle recorded it, milliseconds since 1970.
    at: u64,
    save: String,
    hash: String,
    size: u64,
    /// The file's own modified time when it was recorded.
    modified: u64,
}

#[derive(Serialize)]
pub struct Recorded {
    checked: usize,
    added: usize,
}

struct Found {
    id: String,
    path: PathBuf,
    name: String,
    core: String,
    kind: &'static str,
    stem: String,
    size: u64,
    modified: u64,
}

pub struct Saves {
    store: PathBuf,
    /// Held while the journal is read or written, so two recordings cannot interleave.
    lock: Mutex<()>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// What kind of save a file name is, and the game file name it belongs to. None for
/// anything that is not a save (shader caches, thumbnails, emulator settings).
fn classify(name: &str) -> Option<(&'static str, String)> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".png") || lower.ends_with(".backup") {
        return None;
    }
    if let Some(at) = lower.find(STATE_MARKER) {
        return Some(("state", name[..at].to_string()));
    }
    let (stem, extension) = name.rsplit_once('.')?;
    let extension = extension.to_ascii_lowercase();
    if BATTERY_EXTENSIONS.contains(&extension.as_str()) {
        Some(("battery", stem.to_string()))
    } else if STATE_EXTENSIONS.contains(&extension.as_str()) {
        Some(("state", stem.to_string()))
    } else {
        None
    }
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_SAVES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        // Links are never followed: a save folder must not lead out of the games folder.
        if kind.is_dir() {
            walk(&entry.path(), depth + 1, out);
        } else if kind.is_file() {
            out.push(entry.path());
        }
    }
}

fn discover(games_root: &Path) -> Vec<Found> {
    let mut found = Vec::new();
    for root in SAVE_ROOTS {
        let base = games_root.join(root);
        let Ok(cores) = std::fs::read_dir(&base) else { continue };
        for core in cores.flatten().filter(|c| c.file_type().is_ok_and(|t| t.is_dir())) {
            let core_name = core.file_name().to_string_lossy().into_owned();
            if NOT_A_CORE.contains(&core_name.to_ascii_lowercase().as_str()) {
                continue;
            }
            let mut files = Vec::new();
            walk(&core.path(), 1, &mut files);
            for path in files {
                let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else { continue };
                let Some((kind, stem)) = classify(&name) else { continue };
                let Ok(meta) = std::fs::metadata(&path) else { continue };
                let Ok(relative) = path.strip_prefix(games_root) else { continue };
                let id = relative.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
                found.push(Found { id, path, name, core: core_name.clone(), kind, stem, size: meta.len(), modified: meta.modified().map(millis).unwrap_or(0) });
            }
        }
    }
    found.sort_by(|a, b| b.modified.cmp(&a.modified));
    found
}

fn fingerprint(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

impl Saves {
    /// `store` is a folder in the vault's private area; it is created when first written to.
    pub fn new(store: PathBuf) -> Self {
        Self { store, lock: Mutex::new(()) }
    }

    fn journal(&self) -> Vec<Version> {
        let Ok(text) = std::fs::read_to_string(self.store.join(JOURNAL)) else { return Vec::new() };
        text.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
    }

    /// Every save file on the games drive, newest first, with its game and version count.
    pub fn list(&self, games_root: &Path, roms: &RomIndex) -> Vec<SaveFile> {
        let _guard = self.lock.lock();
        let mut latest: HashMap<String, (usize, Version)> = HashMap::new();
        for version in self.journal() {
            let slot = latest.entry(version.save.clone()).or_insert((0, version.clone()));
            slot.0 += 1;
            slot.1 = version;
        }
        discover(games_root)
            .into_iter()
            .map(|f| {
                let known = latest.get(&f.id);
                SaveFile {
                    recorded: known.is_some_and(|(_, v)| v.size == f.size && v.modified == f.modified),
                    versions: known.map_or(0, |(count, _)| *count),
                    game: roms.get(&f.stem).or_else(|| gci_code(&f.name).and_then(|code| roms.get(&gamecube_key(code)))).cloned(),
                    id: f.id,
                    name: f.name,
                    core: f.core,
                    kind: f.kind,
                    size: f.size,
                    modified: f.modified,
                }
            })
            .collect()
    }

    /// Records one file if it differs from its newest recorded version. Returns whether a
    /// version was added. `journal` is the in-memory copy, kept in step.
    fn record_one(&self, found: &Found, journal: &mut Vec<Version>) -> SavesResult<bool> {
        let newest = journal.iter().rev().find(|v| v.save == found.id);
        if newest.is_some_and(|v| v.size == found.size && v.modified == found.modified) || found.size > MAX_RECORDED_BYTES {
            return Ok(false);
        }
        let bytes = std::fs::read(&found.path).map_err(|_| format!("could not read {}", found.name))?;
        let hash = fingerprint(&bytes);
        if newest.is_some_and(|v| v.hash == hash) {
            return Ok(false);
        }
        let objects = self.store.join(OBJECTS);
        std::fs::create_dir_all(&objects).map_err(|_| "could not create the save store".to_string())?;
        let object = objects.join(&hash);
        if !object.is_file() {
            let tmp = objects.join(format!("{hash}.tmp"));
            std::fs::write(&tmp, &bytes).and_then(|()| std::fs::rename(&tmp, &object)).map_err(|_| "could not store the save".to_string())?;
        }
        let version = Version { seq: journal.len() as u64 + 1, at: now_ms(), save: found.id.clone(), hash, size: bytes.len() as u64, modified: found.modified };
        let line = serde_json::to_string(&version).map_err(|_| "could not write the save journal".to_string())?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.store.join(JOURNAL))
            .and_then(|mut file| writeln!(file, "{line}"))
            .map_err(|_| "could not write the save journal".to_string())?;
        journal.push(version);
        Ok(true)
    }

    /// Records every save file that changed since it was last recorded.
    pub fn record(&self, games_root: &Path) -> SavesResult<Recorded> {
        let _guard = self.lock.lock();
        let mut journal = self.journal();
        let found = discover(games_root);
        let mut added = 0;
        for file in &found {
            if self.record_one(file, &mut journal)? {
                added += 1;
            }
        }
        Ok(Recorded { checked: found.len(), added })
    }

    /// Every recorded version of one save, newest first.
    pub fn history(&self, save: &str) -> Vec<Version> {
        let _guard = self.lock.lock();
        self.journal().into_iter().rev().filter(|v| v.save == save).collect()
    }

    /// Puts a recorded version back in place of the save file. The file as it is now is
    /// recorded first, so putting a version back can itself be undone.
    pub fn restore(&self, games_root: &Path, save: &str, hash: &str) -> SavesResult<()> {
        let _guard = self.lock.lock();
        // The save must be one found on the drive right now and the version one recorded for
        // it, so this can never write anywhere but over an existing save file.
        let found = discover(games_root).into_iter().find(|f| f.id == save).ok_or("that save is not on the drive")?;
        let mut journal = self.journal();
        if !journal.iter().any(|v| v.save == save && v.hash == hash) {
            return Err("that version was never recorded for this save".into());
        }
        let bytes = std::fs::read(self.store.join(OBJECTS).join(hash)).map_err(|_| "that version is missing from the store".to_string())?;
        if fingerprint(&bytes) != hash {
            return Err("that version is damaged in the store and was not put back".into());
        }
        self.record_one(&found, &mut journal)?;
        let tmp = found.path.with_file_name(format!("{}.castle-tmp", found.name));
        std::fs::write(&tmp, &bytes).and_then(|()| std::fs::rename(&tmp, &found.path)).map_err(|_| {
            let _ = std::fs::remove_file(&tmp);
            "could not write the save file".to_string()
        })?;
        // The file put back is the newest version from now on.
        let again = discover(games_root).into_iter().find(|f| f.id == save).ok_or("the save went missing")?;
        self.record_one(&again, &mut journal).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("castled-saves-{name}-{}", now_ms()));
        let _ = std::fs::remove_dir_all(&base);
        let games = base.join("games");
        let core = games.join("Consoles/RetroArch-Win64/saves/mGBA");
        std::fs::create_dir_all(&core).unwrap();
        std::fs::create_dir_all(games.join("Consoles/RetroArch-Win64/states/mGBA")).unwrap();
        (base.clone(), games, base.join("store"))
    }

    #[test]
    fn gamecube_saves_are_tied_to_games_by_code() {
        assert_eq!(gci_code("7D-GZQE-GZQE.gci"), Some("GZQE"));
        assert_eq!(gci_code("78-GJUE-Bigjuju_2003a.sav.gci"), Some("GJUE"));
        assert_eq!(gci_code("Zelda (USA).srm"), None);
        assert_eq!(gci_code("no-code.gci"), None);

        let (base, games, store) = fixture("gci");
        let card = games.join("Consoles/RetroArch-Win64/saves/dolphin-emu/User/GC/USA/Card A");
        std::fs::create_dir_all(&card).unwrap();
        std::fs::write(card.join("7D-GZQE-GZQE.gci"), b"save").unwrap();
        let roms: RomIndex = [(gamecube_key("GZQE"), GameRef { system: "gc".into(), id: 3, title: "Robots (USA)".into() })].into();
        let listed = Saves::new(store).list(&games, &roms);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].game.as_ref().map(|g| g.title.as_str()), Some("Robots (USA)"));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn names_are_sorted_into_saves_states_and_neither() {
        assert_eq!(classify("Zelda (USA).srm"), Some(("battery", "Zelda (USA)".into())));
        assert_eq!(classify("Zelda (USA).state"), Some(("state", "Zelda (USA)".into())));
        assert_eq!(classify("Zelda (USA).state3"), Some(("state", "Zelda (USA)".into())));
        assert_eq!(classify("Zelda (USA).state.auto"), Some(("state", "Zelda (USA)".into())));
        assert_eq!(classify("Zelda (USA).state.png"), None);
        assert_eq!(classify("D3D-uber-vs.cache"), None);
        assert_eq!(classify("game.p2s.backup"), None);
    }

    #[test]
    fn saves_are_found_tied_to_games_and_recorded_once() {
        let (base, games, store) = fixture("record");
        let save = games.join("Consoles/RetroArch-Win64/saves/mGBA/Emerald (USA).srm");
        std::fs::write(&save, b"first").unwrap();
        std::fs::write(games.join("Consoles/RetroArch-Win64/saves/mGBA/shader.cache"), b"x").unwrap();
        let roms: RomIndex = [("Emerald (USA)".to_string(), GameRef { system: "gba".into(), id: 7, title: "Emerald".into() })].into();
        let saves = Saves::new(store);

        let listed = saves.list(&games, &roms);
        assert_eq!(listed.len(), 1, "only the save is listed, not the cache");
        assert_eq!(listed[0].core, "mGBA");
        assert_eq!(listed[0].game.as_ref().map(|g| g.id), Some(7));
        assert!(!listed[0].recorded);

        assert_eq!(saves.record(&games).unwrap().added, 1);
        assert_eq!(saves.record(&games).unwrap().added, 0, "an unchanged save is not recorded twice");
        assert!(saves.list(&games, &roms)[0].recorded);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn a_version_can_be_put_back_and_that_can_be_undone() {
        let (base, games, store) = fixture("restore");
        let save = games.join("Consoles/RetroArch-Win64/saves/mGBA/Emerald (USA).srm");
        let id = "Consoles/RetroArch-Win64/saves/mGBA/Emerald (USA).srm";
        let saves = Saves::new(store);
        std::fs::write(&save, b"first").unwrap();
        saves.record(&games).unwrap();
        let first = saves.history(id)[0].hash.clone();

        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&save, b"second, never recorded").unwrap();
        saves.restore(&games, id, &first).unwrap();
        assert_eq!(std::fs::read(&save).unwrap(), b"first");

        let history = saves.history(id);
        assert_eq!(history.len(), 3, "first, the unrecorded second, and first again");
        assert_eq!(history[0].hash, first);
        saves.restore(&games, id, &history[1].hash).unwrap();
        assert_eq!(std::fs::read(&save).unwrap(), b"second, never recorded");

        assert!(saves.restore(&games, id, "0000").is_err(), "an unknown version is refused");
        assert!(saves.restore(&games, "../outside.srm", &first).is_err(), "a save not on the drive is refused");
        let _ = std::fs::remove_dir_all(base);
    }
}
