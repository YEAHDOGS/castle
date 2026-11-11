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
//! Nothing here writes to the games folder. Starting a game runs the emulator that is
//! already on the drive against a game this module found there itself; the app only ever
//! sends back a system name and a list position, never a path or a command.

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

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    /// Position in the system's list; with the system id, this is how the app names a game.
    id: usize,
    title: String,
    size: u64,
    /// The game file is on the drive right now.
    present: bool,
    has_boxart: bool,
    has_snap: bool,
    #[serde(skip)]
    rom: PathBuf,
    #[serde(skip)]
    art_name: String,
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

impl Arcade {
    pub fn new(root: PathBuf, loopback: bool) -> Self {
        Self { root, loopback, library: Mutex::new(None) }
    }

    fn read_playlist(&self, spec: &PlaylistSystem) -> Option<System> {
        let retroarch = self.root.join(RETROARCH_DIR);
        let raw = std::fs::read(retroarch.join("playlists").join(format!("{}.lpl", spec.playlist))).ok()?;
        let json: serde_json::Value = serde_json::from_slice(&raw).ok()?;
        let thumbnails = retroarch.join("thumbnails").join(spec.playlist);
        let games = json["items"]
            .as_array()?
            .iter()
            .filter_map(|item| {
                let label = item["label"].as_str()?;
                let rom = reroot(&self.root, item["path"].as_str()?)?;
                let art_name = thumbnail_name(label);
                let size = file_size(&rom);
                Some(Game {
                    id: 0,
                    title: label.to_string(),
                    size: size.unwrap_or(0),
                    present: size.is_some(),
                    has_boxart: thumbnails.join("Named_Boxarts").join(format!("{art_name}.png")).is_file(),
                    has_snap: thumbnails.join("Named_Snaps").join(format!("{art_name}.png")).is_file(),
                    rom,
                    art_name,
                })
            })
            .enumerate()
            .map(|(id, game)| Game { id, ..game })
            .collect();
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
        let mut games: Vec<Game> = std::fs::read_dir(self.root.join(spec.roms))
            .ok()?
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                let ext = path.extension()?.to_str()?.to_ascii_lowercase();
                if !spec.extensions.contains(&ext.as_str()) {
                    return None;
                }
                let title = path.file_stem()?.to_str()?.to_string();
                Some(Game {
                    id: 0,
                    size: file_size(&path)?,
                    present: true,
                    has_boxart: false,
                    has_snap: false,
                    art_name: thumbnail_name(&title),
                    title,
                    rom: path,
                })
            })
            .collect();
        games.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        for (id, game) in games.iter_mut().enumerate() {
            game.id = id;
        }
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
                // GameCube memory-card saves are named by the game's code, not its file name.
                if let Some(code) = gamecube_code(&game.rom) {
                    index.entry(crate::saves::gamecube_key(&code)).or_insert(game_ref.clone());
                }
                if let Some(stem) = game.rom.file_stem() {
                    index.entry(stem.to_string_lossy().into_owned()).or_insert(game_ref);
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
        let path = dir.join(format!("{}.png", game.art_name));
        // The name came from a playlist on the drive; make sure it still names a file
        // directly inside the thumbnails folder and nothing else.
        if path.parent() == Some(dir.as_path()) && path.is_file() {
            Ok(path)
        } else {
            Err("no image".into())
        }
    }

    /// Opens a game in its emulator on this machine.
    pub fn play(&self, system: &str, game: usize) -> ArcadeResult<String> {
        if !self.loopback {
            return Err("games can only be started when the box listens on this machine alone".into());
        }
        let system = self.system(system)?;
        let game = system.games.get(game).ok_or_else(|| "unknown game".to_string())?;
        let (exe, args) = system.launcher.ok_or_else(|| format!("{} is not on the drive", system.emulator))?;
        if !game.present || !game.rom.starts_with(&self.root) {
            return Err("that game file is not on the drive".into());
        }
        Command::new(&exe)
            .current_dir(exe.parent().unwrap_or(&self.root))
            .args(&args)
            .arg(&game.rom)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| format!("could not start {}", system.emulator))?;
        Ok(game.title.clone())
    }
}

/// True when `dir` has the layout this module reads.
pub fn looks_like_arcade(dir: &Path) -> bool {
    dir.join("Roms").is_dir() && dir.join("Consoles").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnails_are_named_the_way_retroarch_names_them() {
        assert_eq!(thumbnail_name("Ratchet & Clank: Up Your Arsenal"), "Ratchet _ Clank_ Up Your Arsenal");
        assert_eq!(thumbnail_name("Who? (USA) <v1.1> a/b\\c|d\"e*f`g"), "Who_ (USA) _v1.1_ a_b_c_d_e_f_g");
        assert_eq!(thumbnail_name("Plain Title (USA)"), "Plain Title (USA)");
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
