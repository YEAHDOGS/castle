//! Arcade: a read-only view of a games folder laid out the way the reference drive is.
//!
//! ```text
//! <arcade>/
//!   Consoles/RetroArch-Win64/   retroarch.exe, cores/, playlists/*.lpl, thumbnails/
//!   Consoles/PCSX2/             pcsx2-qt.exe
//!   Consoles/PPSSPP/            PPSSPPWindows64.exe
//!   Roms/<maker>/<system>/      the games
//!   Saves/                      per-emulator saves
//! ```
//!
//! RetroArch's playlists are the library: each lists a system's games, and its thumbnails
//! folder holds the box art under the same names. Systems RetroArch has no playlist for
//! (PlayStation 2, PSP) are read straight from their Roms folder and open in their own
//! emulator.
//!
//! A game that comes on several discs is one game here: "Title (USA) (Disc 1)" and
//! "Title (USA) (Disc 2)" are shown once, as "Title (USA)" with two discs to choose from.
//!
//! Nothing here writes to the games folder. Starting a game runs the emulator that is
//! already on the drive against a game this module found there itself; the app only ever
//! sends back a system name, a list position and a disc number, never a path or a command.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};

use serde::Serialize;

const RETROARCH_DIR: &str = "Consoles/RetroArch-Win64";
const RETROARCH_EXE: &str = "retroarch.exe";
const LIBRARY_CACHE: Duration = Duration::from_secs(60);
/// Characters RetroArch replaces with `_` when it names a thumbnail after a game.
const THUMBNAIL_UNSAFE: &str = "&*/:`<>?\\|\"";
/// Where playlist paths are cut to re-root them: the drive letter in a playlist is the one
/// the drive had when RetroArch scanned it, not necessarily the one it has now.
const ROMS_MARKER: &str = "\\Roms\\";

/// How one RetroArch playlist is shown and which core plays it.
struct PlaylistSystem {
    playlist: &'static str,
    short: &'static str,
    core: &'static str,
}

const PLAYLIST_SYSTEMS: [PlaylistSystem; 9] = [
    PlaylistSystem { playlist: "Nintendo - Nintendo Entertainment System", short: "NES", core: "mesen_libretro.dll" },
    PlaylistSystem { playlist: "Nintendo - Super Nintendo Entertainment System", short: "SNES", core: "snes9x_libretro.dll" },
    PlaylistSystem { playlist: "Nintendo - Nintendo 64", short: "N64", core: "mupen64plus_next_libretro.dll" },
    PlaylistSystem { playlist: "Nintendo - GameCube", short: "GameCube", core: "dolphin_libretro.dll" },
    PlaylistSystem { playlist: "Nintendo - Wii", short: "Wii", core: "dolphin_libretro.dll" },
    PlaylistSystem { playlist: "Nintendo - Game Boy Color", short: "GBC", core: "mgba_libretro.dll" },
    PlaylistSystem { playlist: "Nintendo - Game Boy Advance", short: "GBA", core: "mgba_libretro.dll" },
    PlaylistSystem { playlist: "Nintendo - Nintendo DS", short: "DS", core: "desmume_libretro.dll" },
    PlaylistSystem { playlist: "Sony - PlayStation", short: "PS1", core: "swanstation_libretro.dll" },
];

/// A system with no playlist: its games are the files in one folder, opened by one program.
struct FolderSystem {
    name: &'static str,
    short: &'static str,
    roms: &'static str,
    extensions: &'static [&'static str],
    emulator: &'static str,
    exe: &'static str,
}

const FOLDER_SYSTEMS: [FolderSystem; 2] = [
    FolderSystem {
        name: "Sony - PlayStation 2",
        short: "PS2",
        roms: "Roms/PlayStation 2",
        extensions: &["iso", "chd"],
        emulator: "PCSX2",
        exe: "Consoles/PCSX2/pcsx2-qt.exe",
    },
    FolderSystem {
        name: "Sony - PlayStation Portable",
        short: "PSP",
        roms: "Roms/PlayStation Portable",
        extensions: &["iso", "chd", "cso"],
        emulator: "PPSSPP",
        exe: "Consoles/PPSSPP/PPSSPPWindows64.exe",
    },
];

/// One game file as the drive lists it, before discs of the same game are put together.
struct Entry {
    title: String,
    size: u64,
    present: bool,
    has_boxart: bool,
    has_snap: bool,
    rom: PathBuf,
    art_name: String,
}

/// One disc of a game. A game that is a single file has exactly one, with an empty label.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Disc {
    /// "Disc 2", or "" for a game that is one file.
    label: String,
    size: u64,
    /// The file is on the drive right now.
    present: bool,
    #[serde(skip)]
    rom: PathBuf,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    /// Position in the system's list; with the system id, this is how the app names a game.
    id: usize,
    /// Without any "(Disc N)": every disc of the game shares it.
    title: String,
    /// All discs together.
    size: u64,
    /// At least one disc is on the drive right now.
    present: bool,
    has_boxart: bool,
    has_snap: bool,
    /// In playing order. The app sends back a position in this list to start one.
    discs: Vec<Disc>,
    #[serde(skip)]
    art_name: String,
    #[serde(skip)]
    snap_name: String,
}

/// Words that mark one disc of several inside a title's brackets: "(Disc 2)", "(CD 1 of 3)".
const DISC_WORDS: [&str; 4] = ["disc", "disk", "cd", "dvd"];

/// Splits "Title (USA) (Disc 2 of 4) (Rev 1)" into the title every disc shares
/// ("Title (USA) (Rev 1)"), the disc's label ("Disc 2") and its place in the order.
/// None for a title that names no disc.
fn split_disc(title: &str) -> Option<(String, String, u32)> {
    let mut search_from = 0;
    while let Some(open) = title[search_from..].find('(').map(|at| at + search_from) {
        let close = title[open..].find(')').map(|at| at + open)?;
        let inside = title[open + 1..close].trim();
        if let Some(order) = disc_order(inside) {
            let before = title[..open].trim_end();
            let after = title[close + 1..].trim_start();
            let shared = if after.is_empty() { before.to_string() } else { format!("{before} {after}") };
            return Some((shared, format!("Disc {}", disc_name(order)), order));
        }
        search_from = close + 1;
    }
    None
}

/// "Disc 2", "disk 2 of 4", "CD B" -> 2. Letters count from A = 1. None for anything else.
fn disc_order(inside: &str) -> Option<u32> {
    let lower = inside.to_ascii_lowercase();
    let rest = DISC_WORDS.iter().find_map(|word| lower.strip_prefix(word))?;
    // The word must stand alone: "Discovery Edition" is not a disc.
    let rest = rest.strip_prefix(' ')?.trim();
    let number = rest.split(" of ").next()?.trim();
    if let Ok(n) = number.parse::<u32>() {
        return (n > 0).then_some(n);
    }
    let mut letters = number.chars();
    match (letters.next(), letters.next()) {
        (Some(letter), None) if letter.is_ascii_lowercase() => Some(letter as u32 - 'a' as u32 + 1),
        _ => None,
    }
}

/// How a disc's place is written back: numbers as they are.
fn disc_name(order: u32) -> String {
    order.to_string()
}

/// Puts the discs of one game together, keeping the list in the order the first disc of
/// each game appeared. The cover is the first disc's that has one.
fn group_discs(entries: Vec<Entry>) -> Vec<Game> {
    struct Group {
        title: String,
        discs: Vec<(u32, Entry, String)>,
    }
    let mut groups: Vec<Group> = Vec::new();
    let mut by_title: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for entry in entries {
        match split_disc(&entry.title) {
            Some((shared, label, order)) => {
                let at = *by_title.entry(shared.to_lowercase()).or_insert_with(|| {
                    groups.push(Group { title: shared.clone(), discs: Vec::new() });
                    groups.len() - 1
                });
                groups[at].discs.push((order, entry, label));
            }
            // A single-file game never joins a group, even one with the same title.
            None => groups.push(Group { title: entry.title.clone(), discs: vec![(0, entry, String::new())] }),
        }
    }
    groups
        .into_iter()
        .enumerate()
        .map(|(id, mut group)| {
            group.discs.sort_by_key(|(order, _, _)| *order);
            let cover = group.discs.iter().find(|(_, e, _)| e.has_boxart).map(|(_, e, _)| e.art_name.clone());
            let snap = group.discs.iter().find(|(_, e, _)| e.has_snap).map(|(_, e, _)| e.art_name.clone());
            // A lone disc of a set ("Disc 2" with no "Disc 1" on the drive) keeps its label.
            let lone = group.discs.len() == 1;
            Game {
                id,
                size: group.discs.iter().map(|(_, e, _)| e.size).sum(),
                present: group.discs.iter().any(|(_, e, _)| e.present),
                has_boxart: cover.is_some(),
                has_snap: snap.is_some(),
                art_name: cover.unwrap_or_default(),
                snap_name: snap.unwrap_or_default(),
                title: if lone { group.discs[0].1.title.clone() } else { group.title },
                discs: group
                    .discs
                    .into_iter()
                    .map(|(_, e, label)| Disc { label: if lone { String::new() } else { label }, size: e.size, present: e.present, rom: e.rom })
                    .collect(),
            }
        })
        .collect()
}

#[derive(Clone)]
struct System {
    id: String,
    name: String,
    short: &'static str,
    emulator: String,
    /// Program and leading arguments that open a game; the game path is appended.
    launcher: Option<(PathBuf, Vec<String>)>,
    thumbnails: Option<PathBuf>,
    games: Vec<Game>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSummary {
    id: String,
    name: String,
    short: &'static str,
    emulator: String,
    games: usize,
    with_art: usize,
    bytes: u64,
    can_play: bool,
}

pub struct Arcade {
    root: PathBuf,
    loopback: bool,
    library: Mutex<Option<(Instant, Vec<System>)>>,
}

pub type ArcadeResult<T> = Result<T, String>;

/// The file name RetroArch gives a game's thumbnail.
fn thumbnail_name(label: &str) -> String {
    label.chars().map(|c| if THUMBNAIL_UNSAFE.contains(c) { '_' } else { c }).collect()
}

/// Re-roots a playlist path under the arcade folder: everything from `\Roms\` on is kept.
/// Returns None for a path that does not point into a Roms folder or tries to climb out.
fn reroot(root: &Path, playlist_path: &str) -> Option<PathBuf> {
    let tail = &playlist_path[playlist_path.find(ROMS_MARKER)? + 1..];
    let mut path = root.to_path_buf();
    for part in tail.split('\\') {
        if part.is_empty() || part == "." || part == ".." || part.contains(['/', ':']) {
            return None;
        }
        path.push(part);
    }
    Some(path)
}

/// The four-character game code of a GameCube or Wii disc image (the "GZQE" of "GZQE7D"),
/// read from the start of the file. None for anything else.
fn gamecube_code(rom: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    const COMPRESSED_MAGICS: [&[u8; 4]; 2] = [b"RVZ", b"WIA"];
    const COMPRESSED_HEADER_AT: u64 = 0x58; // where RVZ and WIA keep a copy of the disc's first bytes
    const GAMECUBE_MAGIC_AT: usize = 0x1c;
    const GAMECUBE_MAGIC: [u8; 4] = [0xc2, 0x33, 0x9f, 0x3d];
    const CODE_LEN: usize = 4;

    let extension = rom.extension()?.to_string_lossy().to_ascii_lowercase();
    if !["rvz", "wia", "iso", "gcm"].contains(&extension.as_str()) {
        return None;
    }
    let mut file = std::fs::File::open(rom).ok()?;
    let mut head = [0u8; 0x20];
    file.read_exact(&mut head).ok()?;
    let code = if COMPRESSED_MAGICS.iter().any(|magic| head.starts_with(*magic)) {
        let mut inner = [0u8; CODE_LEN];
        file.seek(SeekFrom::Start(COMPRESSED_HEADER_AT)).ok()?;
        file.read_exact(&mut inner).ok()?;
        inner
    } else if head[GAMECUBE_MAGIC_AT..GAMECUBE_MAGIC_AT + 4] == GAMECUBE_MAGIC {
        [head[0], head[1], head[2], head[3]]
    } else {
        return None;
    };
    code.iter().all(u8::is_ascii_alphanumeric).then(|| String::from_utf8_lossy(&code).into_owned())
}

fn file_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().filter(|m| m.is_file()).map(|m| m.len())
}

/// A cue sheet is a few lines of text; anything larger is not read as one.
const MAX_SHEET_BYTES: u64 = 64 * 1024;

/// How much room a game takes on the drive. A disc image listed by its .cue sheet is the
/// sheet plus the track files it names, which sit beside it and hold the actual game.
fn game_size(path: &Path) -> Option<u64> {
    let own = file_size(path)?;
    let is_sheet = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("cue"));
    if !is_sheet || own > MAX_SHEET_BYTES {
        return Some(own);
    }
    let (Ok(text), Some(dir)) = (std::fs::read_to_string(path), path.parent()) else { return Some(own) };
    let tracks: u64 = text.lines().filter_map(cue_track).filter_map(|name| file_size(&dir.join(name))).sum();
    Some(own + tracks)
}

/// The file a cue sheet line names: `FILE "Game (Track 1).bin" BINARY` -> `Game (Track 1).bin`.
/// None for other lines, and for a name that points anywhere but beside the sheet.
fn cue_track(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix("FILE")?.trim_start();
    let name = match rest.strip_prefix('"') {
        Some(quoted) => quoted.split('"').next()?,
        None => rest.split_whitespace().next()?,
    };
    (!name.is_empty() && !name.contains(['/', '\\', ':']) && name != "..").then_some(name)
}

impl Arcade {
    pub fn new(root: PathBuf, loopback: bool) -> Self {
        Self { root, loopback, library: Mutex::new(None) }
    }

    fn read_playlist(&self, spec: &PlaylistSystem) -> Option<System> {
        let retroarch = self.root.join(RETROARCH_DIR);
        let raw = std::fs::read(retroarch.join("playlists").join(format!("{}.lpl", spec.playlist))).ok()?;
        let json: serde_json::Value = serde_json::from_slice(&raw).ok()?;
        let thumbnails = retroarch.join("thumbnails").join(spec.playlist);
        let entries = json["items"]
            .as_array()?
            .iter()
            .filter_map(|item| {
                let label = item["label"].as_str()?;
                let rom = reroot(&self.root, item["path"].as_str()?)?;
                let art_name = thumbnail_name(label);
                let size = game_size(&rom);
                Some(Entry {
                    title: label.to_string(),
                    size: size.unwrap_or(0),
                    present: size.is_some(),
                    has_boxart: thumbnails.join("Named_Boxarts").join(format!("{art_name}.png")).is_file(),
                    has_snap: thumbnails.join("Named_Snaps").join(format!("{art_name}.png")).is_file(),
                    rom,
                    art_name,
                })
            })
            .collect();
        let mut games = group_discs(entries);
        // A multi-disc game's cover may be filed under the title all its discs share.
        for game in games.iter_mut().filter(|g| g.discs.len() > 1) {
            let shared = thumbnail_name(&game.title);
            if !game.has_boxart && thumbnails.join("Named_Boxarts").join(format!("{shared}.png")).is_file() {
                game.has_boxart = true;
                game.art_name = shared.clone();
            }
            if !game.has_snap && thumbnails.join("Named_Snaps").join(format!("{shared}.png")).is_file() {
                game.has_snap = true;
                game.snap_name = shared;
            }
        }
        let exe = retroarch.join(RETROARCH_EXE);
        let core = retroarch.join("cores").join(spec.core);
        let launcher = (exe.is_file() && core.is_file())
            .then(|| (exe, vec!["-L".to_string(), core.to_string_lossy().into_owned()]));
        Some(System {
            id: spec.playlist.to_string(),
            name: spec.playlist.to_string(),
            short: spec.short,
            emulator: "RetroArch".to_string(),
            launcher,
            thumbnails: Some(thumbnails),
            games,
        })
    }

    fn read_folder(&self, spec: &FolderSystem) -> Option<System> {
        let mut entries: Vec<Entry> = std::fs::read_dir(self.root.join(spec.roms))
            .ok()?
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                let ext = path.extension()?.to_str()?.to_ascii_lowercase();
                if !spec.extensions.contains(&ext.as_str()) {
                    return None;
                }
                let title = path.file_stem()?.to_str()?.to_string();
                Some(Entry {
                    size: game_size(&path)?,
                    present: true,
                    has_boxart: false,
                    has_snap: false,
                    art_name: thumbnail_name(&title),
                    title,
                    rom: path,
                })
            })
            .collect();
        entries.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        let games = group_discs(entries);
        let exe = self.root.join(spec.exe);
        Some(System {
            id: spec.name.to_string(),
            name: spec.name.to_string(),
            short: spec.short,
            emulator: spec.emulator.to_string(),
            launcher: exe.is_file().then(|| (exe, Vec::new())),
            thumbnails: None,
            games,
        })
    }

    /// Every system found on the drive, re-read at most once a minute.
    fn library(&self) -> Vec<System> {
        let mut cached = self.library.lock().unwrap();
        if let Some((at, systems)) = cached.as_ref() {
            if at.elapsed() < LIBRARY_CACHE {
                return systems.clone();
            }
        }
        let systems: Vec<System> = PLAYLIST_SYSTEMS
            .iter()
            .filter_map(|spec| self.read_playlist(spec))
            .chain(FOLDER_SYSTEMS.iter().filter_map(|spec| self.read_folder(spec)))
            .filter(|system| !system.games.is_empty())
            .collect();
        *cached = Some((Instant::now(), systems.clone()));
        systems
    }

    fn system(&self, id: &str) -> ArcadeResult<System> {
        self.library().into_iter().find(|s| s.id == id).ok_or_else(|| "unknown system".to_string())
    }

    pub fn systems(&self) -> Vec<SystemSummary> {
        self.library()
            .into_iter()
            .map(|s| SystemSummary {
                games: s.games.len(),
                with_art: s.games.iter().filter(|g| g.has_boxart).count(),
                bytes: s.games.iter().map(|g| g.size).sum(),
                can_play: self.loopback && s.launcher.is_some(),
                id: s.id,
                name: s.name,
                short: s.short,
                emulator: s.emulator,
            })
            .collect()
    }

    /// The games folder itself.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Every game by its file name without the extension, which is how emulators name saves.
    pub fn rom_index(&self) -> crate::saves::RomIndex {
        let mut index = crate::saves::RomIndex::new();
        for system in self.library() {
            for game in &system.games {
                let game_ref = crate::saves::GameRef { system: system.id.clone(), id: game.id, title: game.title.clone() };
                // A save made while any disc was in belongs to the one game.
                for disc in &game.discs {
                    // GameCube memory-card saves are named by the game's code, not its file name.
                    if let Some(code) = gamecube_code(&disc.rom) {
                        index.entry(crate::saves::gamecube_key(&code)).or_insert(game_ref.clone());
                    }
                    if let Some(stem) = disc.rom.file_stem() {
                        index.entry(stem.to_string_lossy().into_owned()).or_insert(game_ref.clone());
                    }
                }
                // Started from a disc list (.m3u), a multi-disc game saves under its shared title.
                if game.discs.len() > 1 {
                    index.entry(game.title.clone()).or_insert(game_ref);
                }
            }
        }
        index
    }

    pub fn games(&self, system: &str) -> ArcadeResult<Vec<Game>> {
        Ok(self.system(system)?.games)
    }

    /// The image file for one game: `boxart` or `snap`, straight from RetroArch's folder.
    pub fn art(&self, system: &str, game: usize, kind: &str) -> ArcadeResult<PathBuf> {
        let folder = match kind {
            "boxart" => "Named_Boxarts",
            "snap" => "Named_Snaps",
            _ => return Err("unknown image kind".into()),
        };
        let system = self.system(system)?;
        let game = system.games.get(game).ok_or_else(|| "unknown game".to_string())?;
        let dir = system.thumbnails.ok_or_else(|| "no image".to_string())?.join(folder);
        let name = if kind == "snap" { &game.snap_name } else { &game.art_name };
        if name.is_empty() {
            return Err("no image".into());
        }
        let path = dir.join(format!("{name}.png"));
        // The name came from a playlist on the drive; make sure it still names a file
        // directly inside the thumbnails folder and nothing else.
        if path.parent() == Some(dir.as_path()) && path.is_file() {
            Ok(path)
        } else {
            Err("no image".into())
        }
    }

    /// One game's title and its system's short name ("PS1"), for looking the game up.
    pub fn naming(&self, system: &str, game: usize) -> ArcadeResult<(String, &'static str)> {
        let system = self.system(system)?;
        let game = system.games.get(game).ok_or_else(|| "unknown game".to_string())?;
        Ok((game.title.clone(), system.short))
    }

    /// Opens one disc of a game in its emulator on this machine.
    pub fn play(&self, system: &str, game: usize, disc: usize) -> ArcadeResult<String> {
        if !self.loopback {
            return Err("games can only be started when the box listens on this machine alone".into());
        }
        let system = self.system(system)?;
        let game = system.games.get(game).ok_or_else(|| "unknown game".to_string())?;
        let disc = game.discs.get(disc).ok_or_else(|| "that game has no such disc".to_string())?;
        let (exe, args) = system.launcher.ok_or_else(|| format!("{} is not on the drive", system.emulator))?;
        if !disc.present || !disc.rom.starts_with(&self.root) {
            return Err("that game file is not on the drive".into());
        }
        Command::new(&exe)
            .current_dir(exe.parent().unwrap_or(&self.root))
            .args(&args)
            .arg(&disc.rom)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| format!("could not start {}", system.emulator))?;
        Ok(if disc.label.is_empty() { game.title.clone() } else { format!("{} ({})", game.title, disc.label) })
    }
}

/// True when `dir` has the layout this module reads.
pub fn looks_like_arcade(dir: &Path) -> bool {
    dir.join("Roms").is_dir() && dir.join("Consoles").is_dir()
}

/// What to search for to find out about a game: its title as people write it, without
/// the region and version in brackets, and with a trailing article put back in front
/// ("Legend of Zelda, The - Ocarina of Time (USA)" -> "The Legend of Zelda: Ocarina of Time").
pub fn plain_title(title: &str) -> String {
    let name = title.split(" (").next().unwrap_or(title).split(" [").next().unwrap_or(title).trim();
    let (first, rest) = match name.split_once(" - ") {
        Some((first, rest)) => (first, Some(rest)),
        None => (name, None),
    };
    let first = ["The", "A", "An"]
        .iter()
        .find_map(|article| first.strip_suffix(&format!(", {article}")).map(|base| format!("{article} {base}")))
        .unwrap_or_else(|| first.to_string());
    match rest {
        Some(rest) => format!("{first}: {}", rest.replace(" - ", ": ")),
        None => first,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_put_the_way_people_write_them() {
        assert_eq!(plain_title("Legend of Zelda, The - Ocarina of Time (USA) (Rev 2)"), "The Legend of Zelda: Ocarina of Time");
        assert_eq!(plain_title("Final Fantasy IX (USA)"), "Final Fantasy IX");
        assert_eq!(plain_title("Sims, The (USA)"), "The Sims");
        assert_eq!(plain_title("Ratchet & Clank - Up Your Arsenal [SCUS-97353]"), "Ratchet & Clank: Up Your Arsenal");
        assert_eq!(plain_title("Tetris"), "Tetris");
    }

    #[test]
    fn thumbnails_are_named_the_way_retroarch_names_them() {
        assert_eq!(thumbnail_name("Ratchet & Clank: Up Your Arsenal"), "Ratchet _ Clank_ Up Your Arsenal");
        assert_eq!(thumbnail_name("Who? (USA) <v1.1> a/b\\c|d\"e*f`g"), "Who_ (USA) _v1.1_ a_b_c_d_e_f_g");
        assert_eq!(thumbnail_name("Plain Title (USA)"), "Plain Title (USA)");
    }

    fn entry(title: &str, has_boxart: bool) -> Entry {
        Entry {
            title: title.to_string(),
            size: 100,
            present: true,
            has_boxart,
            has_snap: false,
            rom: PathBuf::from(format!("{title}.chd")),
            art_name: thumbnail_name(title),
        }
    }

    #[test]
    fn a_disc_image_is_as_big_as_its_tracks() {
        assert_eq!(cue_track(r#"FILE "Game (USA) (Track 1).bin" BINARY"#), Some("Game (USA) (Track 1).bin"));
        assert_eq!(cue_track("  FILE game.bin BINARY"), Some("game.bin"));
        for other in ["  TRACK 01 MODE2/2352", "    INDEX 01 00:00:00", r#"FILE "..\..\secret.bin" BINARY"#, r#"FILE "C:\x.bin" BINARY"#, r#"FILE "" BINARY"#] {
            assert_eq!(cue_track(other), None, "{other:?}");
        }

        let dir = std::env::temp_dir().join(format!("castled-cue-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.bin"), vec![0u8; 5000]).unwrap();
        std::fs::write(dir.join("b.bin"), vec![0u8; 700]).unwrap();
        let sheet = "FILE \"a.bin\" BINARY\n  TRACK 01 MODE2/2352\nFILE \"b.bin\" BINARY\n  TRACK 02 AUDIO\nFILE \"gone.bin\" BINARY\n";
        std::fs::write(dir.join("game.cue"), sheet).unwrap();
        assert_eq!(game_size(&dir.join("game.cue")), Some(sheet.len() as u64 + 5700));
        assert_eq!(game_size(&dir.join("a.bin")), Some(5000));
        assert_eq!(game_size(&dir.join("missing.cue")), None);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_title_names_its_disc_in_brackets() {
        assert_eq!(split_disc("Final Fantasy IX (USA) (Disc 1)"), Some(("Final Fantasy IX (USA)".into(), "Disc 1".into(), 1)));
        assert_eq!(split_disc("Final Fantasy IX (USA) (Disc 3) (Rev 1)"), Some(("Final Fantasy IX (USA) (Rev 1)".into(), "Disc 3".into(), 3)));
        assert_eq!(split_disc("Riven (Europe) (CD 2 of 5)"), Some(("Riven (Europe)".into(), "Disc 2".into(), 2)));
        assert_eq!(split_disc("Some Game (Disk B)"), Some(("Some Game".into(), "Disc 2".into(), 2)));
        for single in ["Crash Bandicoot (USA)", "Discworld (USA)", "Game (Discovery Edition)", "Game (Disc)", "Game (Disc 0)", "Game (Disc One)", "Broken (Disc 1"] {
            assert_eq!(split_disc(single), None, "{single:?} is one file");
        }
    }

    #[test]
    fn discs_of_one_game_become_one_game() {
        let games = group_discs(vec![
            entry("Alpha (USA)", true),
            entry("Final Fantasy IX (USA) (Disc 2)", false),
            entry("Final Fantasy IX (USA) (Disc 1)", true),
            entry("Zeta (USA)", false),
            entry("Final Fantasy IX (USA) (Disc 4)", false),
            entry("Final Fantasy IX (USA) (Disc 3)", false),
            entry("Final Fantasy IX (Europe) (Disc 1)", false),
        ]);
        let titles: Vec<&str> = games.iter().map(|g| g.title.as_str()).collect();
        // One card per game, in the order each was first met; another region is another game.
        assert_eq!(titles, ["Alpha (USA)", "Final Fantasy IX (USA)", "Zeta (USA)", "Final Fantasy IX (Europe) (Disc 1)"]);
        assert_eq!(games.iter().map(|g| g.id).collect::<Vec<_>>(), [0, 1, 2, 3]);

        let ff9 = &games[1];
        assert_eq!(ff9.discs.iter().map(|d| d.label.as_str()).collect::<Vec<_>>(), ["Disc 1", "Disc 2", "Disc 3", "Disc 4"]);
        assert_eq!(ff9.size, 400);
        // The cover comes from the disc that has one.
        assert!(ff9.has_boxart);
        assert_eq!(ff9.art_name, "Final Fantasy IX (USA) (Disc 1)");
        assert_eq!(ff9.discs[0].rom, PathBuf::from("Final Fantasy IX (USA) (Disc 1).chd"));

        // A game that is one file has one unnamed disc; so does a lone disc of a set.
        assert_eq!(games[0].discs.len(), 1);
        assert_eq!(games[0].discs[0].label, "");
        assert_eq!(games[3].discs[0].label, "");
    }

    #[test]
    fn playlist_paths_follow_the_drive_wherever_it_is_mounted() {
        let root = Path::new("X:\\Games");
        let found = reroot(root, "E:\\Games\\Roms\\Nintendo\\Game Boy Advanced\\Game (USA).gba").unwrap();
        assert_eq!(found, Path::new("X:\\Games").join("Roms").join("Nintendo").join("Game Boy Advanced").join("Game (USA).gba"));
    }

    #[test]
    fn playlist_paths_cannot_leave_the_games_folder() {
        let root = Path::new("X:\\Games");
        for bad in [
            "C:\\Windows\\System32\\cmd.exe",
            "E:\\Games\\Roms\\..\\..\\Windows\\cmd.exe",
            "E:\\Games\\Roms\\a\\C:\\x.gba",
            "E:\\Games\\Roms\\a/../../x.gba",
            "E:\\Games\\Roms\\\\x.gba",
        ] {
            assert!(reroot(root, bad).is_none(), "{bad:?} should be refused");
        }
    }
}
