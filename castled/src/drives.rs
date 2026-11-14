//! The computer's drives: which are there, which can be unplugged, when each was plugged
//! in, and how fast it reads and writes.
//!
//! Listing a drive only asks the system about it; nothing on the drive is read or written.
//! The speed test is the one thing here that touches a drive, and only when someone presses
//! its button: it writes one test file, reads it back and deletes it (or, on a drive that
//! cannot be written, reads files already there).
//!
//! Each internal and external disk is also asked about its own health with smartctl, when
//! it is installed: how many hours it has been powered on, how warm it is, and whether it
//! thinks it is failing. That is a question to the disk's firmware, not a read of its files,
//! and a disk that is asleep is left asleep.

use std::{
    collections::{BTreeMap, HashMap},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

/// A full look at every drive at least this often, even when nothing seems to have changed
/// (a disc put in a drive does not change the list of drive letters).
const RESCAN_EVERY: Duration = Duration::from_secs(30);
/// How often last-seen times are written to the vault while nothing else changes.
const SAVE_EVERY: Duration = Duration::from_secs(60);
/// Drives that have gone, shown under "seen before".
const MAX_GONE: usize = 12;

/// The speed test writes and reads in pieces this big: a whole number of sectors on every
/// drive, and big enough that the time is the drive's, not the program's.
const CHUNK: usize = 4 * 1024 * 1024;
const SECTOR_ALIGN: usize = 4096;
/// A test stops after this much, or after TEST_TIME once it has done TEST_MIN.
const TEST_MAX: u64 = 256 * 1024 * 1024;
const TEST_MIN: u64 = 32 * 1024 * 1024;
const TEST_TIME: Duration = Duration::from_secs(3);
/// Starts the name of the speed test's own file, so a left-over one is easy to recognise.
pub const TEST_FILE_PREFIX: &str = ".castle-speedtest-";

/// A disk is asked about its health this often. Power-on hours move slowly; a disk that
/// starts failing shows up within the half hour.
const SMART_EVERY: Duration = Duration::from_secs(30 * 60);
/// smartctl gets this long per disk before it is stopped (a confused USB bridge can hang).
const SMART_TIMEOUT: Duration = Duration::from_secs(20);
const SMART_POLL: Duration = Duration::from_millis(50);
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn now_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// ---------------------------------------------------------------------------------------
// What the system says about each drive
// ---------------------------------------------------------------------------------------

/// One drive as the system reports it, before Castle adds what it remembers.
#[derive(Clone, Debug, Default)]
struct Raw {
    /// Where it is opened: "E:\" or "/media/usb".
    path: String,
    /// The name the drive was given ("MOVESPEED"); empty when it has none.
    label: String,
    /// fixed, external, removable, optical, network, memory
    kind: &'static str,
    removable: bool,
    read_only: bool,
    /// False for a drive with nothing in it, such as a disc drive with no disc.
    media: bool,
    file_system: String,
    total: u64,
    free: u64,
    /// How it is connected: USB, SATA, NVMe, SD...
    bus: String,
    /// Maker and model, when the device says.
    model: String,
    /// Changes when the drive is formatted; tells two drives with the same name apart.
    serial: String,
    /// What smartctl is told to ask ("C:" on Windows, "/dev/sda" on Linux). Empty for a
    /// drive that is not asked: discs, network and memory drives, sticks and cards.
    device: String,
}

impl Raw {
    /// Who this drive is, across plugging in and out.
    fn key(&self) -> String {
        if !self.media {
            return format!("slot|{}", self.path);
        }
        let who = if self.serial.is_empty() { &self.path } else { &self.serial };
        format!("{who}|{}|{}", self.file_system, self.total)
    }
}

#[cfg(windows)]
mod win {
    //! The Windows calls behind the drive list. Each one asks about a drive; none reads or
    //! writes it.

    use std::ffi::c_void;

    use super::Raw;

    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    const DRIVE_REMOTE: u32 = 4;
    const DRIVE_CDROM: u32 = 5;
    const DRIVE_RAMDISK: u32 = 6;
    const FILE_READ_ONLY_VOLUME: u32 = 0x0008_0000;
    /// No "There is no disk in the drive" box when an empty drive is asked about.
    const SEM_FAILCRITICALERRORS: u32 = 0x0001;
    const SEM_NOOPENFILEERRORBOX: u32 = 0x8000;
    const FILE_SHARE_READ_WRITE: u32 = 0x1 | 0x2;
    const OPEN_EXISTING: u32 = 3;
    const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;
    const INVALID_HANDLE: isize = -1;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLogicalDrives() -> u32;
        fn GetDriveTypeW(root: *const u16) -> u32;
        fn GetVolumeInformationW(
            root: *const u16,
            name: *mut u16,
            name_len: u32,
            serial: *mut u32,
            max_component: *mut u32,
            flags: *mut u32,
            fs_name: *mut u16,
            fs_name_len: u32,
        ) -> i32;
        fn GetDiskFreeSpaceExW(root: *const u16, free_to_caller: *mut u64, total: *mut u64, total_free: *mut u64) -> i32;
        fn SetThreadErrorMode(new_mode: u32, old_mode: *mut u32) -> i32;
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *mut c_void,
            disposition: u32,
            flags: u32,
            template: *mut c_void,
        ) -> *mut c_void;
        fn DeviceIoControl(
            device: *mut c_void,
            code: u32,
            input: *const c_void,
            input_len: u32,
            output: *mut c_void,
            output_len: u32,
            returned: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn from_wide(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    /// Turns off Windows' "insert a disk" boxes for this thread while it lives.
    struct Quiet(u32);

    impl Quiet {
        fn new() -> Self {
            let mut old = 0;
            // SAFETY: sets a flag on this thread; `old` is a valid place for the previous one.
            unsafe { SetThreadErrorMode(SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX, &mut old) };
            Self(old)
        }
    }

    impl Drop for Quiet {
        fn drop(&mut self) {
            // SAFETY: puts back the mode this thread had.
            unsafe { SetThreadErrorMode(self.0, std::ptr::null_mut()) };
        }
    }

    /// A number that changes when a drive letter comes or goes. Costs nothing to read.
    pub fn fingerprint() -> u64 {
        // SAFETY: no arguments; returns a bit mask.
        u64::from(unsafe { GetLogicalDrives() })
    }

    /// How the device is connected and what it is called, asked of the device itself.
    /// Opening it with no access rights allows only questions like this one.
    fn device(letter: char) -> Option<(u32, String)> {
        let name = wide(&format!(r"\\.\{letter}:"));
        // SAFETY: `name` is a terminated wide string; no access is asked for.
        let handle = unsafe {
            CreateFileW(name.as_ptr(), 0, FILE_SHARE_READ_WRITE, std::ptr::null_mut(), OPEN_EXISTING, 0, std::ptr::null_mut())
        };
        if handle as isize == INVALID_HANDLE || handle.is_null() {
            return None;
        }
        // STORAGE_PROPERTY_QUERY { PropertyId: StorageDeviceProperty, QueryType: PropertyStandardQuery }
        let query = [0u32; 3];
        let mut out = vec![0u8; 1024];
        let mut returned = 0u32;
        // SAFETY: both buffers are ours and as long as the lengths given.
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_QUERY_PROPERTY,
                query.as_ptr().cast(),
                std::mem::size_of_val(&query) as u32,
                out.as_mut_ptr().cast(),
                out.len() as u32,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        // SAFETY: the handle came from CreateFileW above and is closed once.
        unsafe { CloseHandle(handle) };
        let returned = returned as usize;
        if ok == 0 || returned < 32 {
            return None;
        }
        let out = &out[..returned.min(out.len())];
        let word = |at: usize| u32::from_le_bytes([out[at], out[at + 1], out[at + 2], out[at + 3]]);
        // STORAGE_DEVICE_DESCRIPTOR: VendorIdOffset at 12, ProductIdOffset at 16, BusType at 28.
        let text = |offset: u32| -> String {
            let at = offset as usize;
            if at == 0 || at >= out.len() {
                return String::new();
            }
            let end = out[at..].iter().position(|&b| b == 0).map_or(out.len(), |n| at + n);
            String::from_utf8_lossy(&out[at..end]).trim().to_string()
        };
        let model = [text(word(12)), text(word(16))].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
        Some((word(28), model))
    }

    fn bus_name(bus: u32) -> &'static str {
        match bus {
            1 => "SCSI",
            2 => "ATAPI",
            3 => "ATA",
            4 => "FireWire",
            7 => "USB",
            8 => "RAID",
            10 => "SAS",
            11 => "SATA",
            12 => "SD",
            13 => "MMC",
            14 => "Virtual",
            15 => "Virtual",
            17 => "NVMe",
            _ => "",
        }
    }

    pub fn scan() -> Vec<Raw> {
        let _quiet = Quiet::new();
        let mask = fingerprint() as u32;
        let mut out = Vec::new();
        for (bit, letter) in ('A'..='Z').enumerate() {
            if mask & (1 << bit) == 0 {
                continue;
            }
            let path = format!("{letter}:\\");
            let root = wide(&path);
            // SAFETY: `root` is a terminated wide string.
            let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
            if !matches!(kind, DRIVE_REMOVABLE | DRIVE_FIXED | DRIVE_REMOTE | DRIVE_CDROM | DRIVE_RAMDISK) {
                continue;
            }
            let mut label = [0u16; 261];
            let mut fs_name = [0u16; 261];
            let (mut serial, mut max_component, mut flags) = (0u32, 0u32, 0u32);
            // SAFETY: every buffer is ours and its length is given.
            let media = unsafe {
                GetVolumeInformationW(
                    root.as_ptr(),
                    label.as_mut_ptr(),
                    label.len() as u32,
                    &mut serial,
                    &mut max_component,
                    &mut flags,
                    fs_name.as_mut_ptr(),
                    fs_name.len() as u32,
                )
            } != 0;
            let (mut free, mut total, mut total_free) = (0u64, 0u64, 0u64);
            if media {
                // SAFETY: three places for three numbers.
                unsafe { GetDiskFreeSpaceExW(root.as_ptr(), &mut free, &mut total, &mut total_free) };
            }
            let (bus, model) = if kind == DRIVE_REMOTE { (0, String::new()) } else { device(letter).unwrap_or_default() };
            let bus = bus_name(bus);
            // A USB or card-reader disk calls itself "fixed", but it can be unplugged all the same.
            let plugged = matches!(bus, "USB" | "SD" | "MMC" | "FireWire");
            let (kind, removable) = match kind {
                DRIVE_REMOVABLE => ("removable", true),
                DRIVE_CDROM => ("optical", true),
                DRIVE_REMOTE => ("network", false),
                DRIVE_RAMDISK => ("memory", false),
                _ if plugged => ("external", true),
                _ => ("fixed", false),
            };
            out.push(Raw {
                path,
                label: from_wide(&label),
                kind,
                removable,
                read_only: media && (flags & FILE_READ_ONLY_VOLUME != 0 || kind == "optical"),
                media,
                file_system: from_wide(&fs_name),
                total,
                free,
                bus: bus.to_string(),
                model,
                serial: if media { format!("{serial:08X}") } else { String::new() },
                device: if media && matches!(kind, "fixed" | "external") { format!("{letter}:") } else { String::new() },
            });
        }
        out
    }
}

#[cfg(not(windows))]
mod unix {
    use std::path::Path;

    use super::Raw;

    /// The whole disk a partition is on: "/dev/sda1" -> "/dev/sda", "/dev/nvme0n1p2" ->
    /// "/dev/nvme0n1". None for what has no disk behind it worth asking (loop devices,
    /// zram, device-mapper volumes, overlay and tmpfs).
    pub fn whole_disk(dev: &str) -> Option<String> {
        let real = std::fs::canonicalize(dev).ok()?;
        let base = real.file_name()?.to_string_lossy().to_string();
        if ["loop", "zram", "dm-", "ram", "sr"].iter().any(|p| base.starts_with(p)) {
            return None;
        }
        let sys = Path::new("/sys/class/block").join(&base);
        if sys.join("partition").exists() {
            let disk = std::fs::canonicalize(&sys).ok()?;
            let parent = disk.parent()?.file_name()?.to_string_lossy().to_string();
            return Some(format!("/dev/{parent}"));
        }
        sys.exists().then(|| format!("/dev/{base}"))
    }

    /// Changes when something is mounted or unmounted.
    pub fn fingerprint() -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::fs::read_to_string("/proc/mounts").unwrap_or_default().hash(&mut hasher);
        hasher.finish()
    }

    pub fn scan() -> Vec<Raw> {
        let Ok(disks) = sysinfo::Disks::new_with_refreshed_list() else { return Vec::new() };
        disks
            .list()
            .iter()
            .map(|disk| {
                let path = disk.mount_point().to_string_lossy().to_string();
                let file_system = disk.file_system().to_string_lossy().to_string();
                let optical = matches!(file_system.as_str(), "iso9660" | "udf");
                let mounted_for_user = ["/media/", "/run/media/", "/mnt/"].iter().any(|base| path.starts_with(base));
                let removable = disk.is_removable() || optical || mounted_for_user;
                let kind = if optical {
                    "optical"
                } else if disk.is_removable() {
                    "removable"
                } else if mounted_for_user {
                    "external"
                } else {
                    "fixed"
                };
                let label = disk.mount_point().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let serial = disk.name().to_string_lossy().to_string();
                let device = if matches!(kind, "fixed" | "external") { whole_disk(&serial).unwrap_or_default() } else { String::new() };
                Raw {
                    path,
                    label,
                    kind,
                    removable,
                    read_only: disk.is_read_only() || optical,
                    media: true,
                    file_system,
                    total: disk.total_space(),
                    free: disk.available_space(),
                    bus: String::new(),
                    model: String::new(),
                    serial,
                    device,
                }
            })
            .collect()
    }
}

#[cfg(windows)]
use win::{fingerprint, scan};
#[cfg(not(windows))]
use unix::{fingerprint, scan};

// ---------------------------------------------------------------------------------------
// What Castle remembers about each drive
// ---------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeedResult {
    /// When the test ran, milliseconds since 1970.
    pub at: u64,
    /// "write" when a test file was written and read back; "read" when only files already
    /// on the drive were read.
    pub mode: String,
    /// Megabytes (millions of bytes) per second. None for a read-only test.
    pub write_mbps: Option<f64>,
    pub read_mbps: f64,
    /// How much was written or read to get these numbers.
    pub bytes: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
struct Seen {
    name: String,
    path: String,
    kind: String,
    removable: bool,
    total: u64,
    first_seen: u64,
    last_seen: u64,
    /// When it was last plugged in (or, for a drive that was in when the box started, when
    /// the box last saw it go in).
    plugged_in: u64,
    /// Was it there the last time the box looked? Lets a restart tell "still in" from "just
    /// plugged in".
    present: bool,
    last_test: Option<SpeedResult>,
}

/// A drive as the app sees it.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Drive {
    pub key: String,
    pub path: String,
    pub name: String,
    pub label: String,
    pub kind: &'static str,
    pub removable: bool,
    pub read_only: bool,
    pub media: bool,
    pub file_system: String,
    pub total: u64,
    pub free: u64,
    pub bus: String,
    pub model: String,
    pub serial: String,
    /// This drive holds the Castle vault.
    pub vault: bool,
    pub first_seen: u64,
    pub plugged_in: u64,
    pub last_test: Option<SpeedResult>,
    /// What the disk says about itself, once smartctl has asked. None until it has, and
    /// for drives that are not asked.
    pub health: Option<Health>,
    /// Who smartctl asks about this drive; not sent to the app.
    #[serde(skip)]
    device: String,
}

/// A drive that has been plugged in before and is not now.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Gone {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub total: u64,
    pub first_seen: u64,
    pub last_seen: u64,
    pub last_test: Option<SpeedResult>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub drives: Vec<Drive>,
    pub gone: Vec<Gone>,
    /// When the drives were last looked at.
    pub at: u64,
    /// smartctl is installed, so disks can be asked how they are.
    pub smartctl: bool,
}

/// The box's drives in a few numbers, for the spec card. Says nothing about where anything is.
#[derive(Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Internal and external disks with something on them.
    pub disks: usize,
    /// Their sizes added up.
    pub total: u64,
    pub free: u64,
    /// The most power-on hours any of them has.
    pub oldest_hours: Option<u64>,
    /// How many have answered smartctl.
    pub reporting: usize,
    /// Disks that say they are failing, and disks with bad sectors or worn-out flash.
    pub failing: usize,
    pub worn: usize,
    pub smartctl: bool,
}

#[derive(Default)]
struct Book {
    seen: BTreeMap<String, Seen>,
    present: Vec<Drive>,
    fingerprint: Option<u64>,
    scanned: Option<Instant>,
    saved: Option<Instant>,
}

pub struct Drives {
    file: PathBuf,
    vault: PathBuf,
    book: Mutex<Book>,
    /// One speed test at a time: two at once would each measure half a drive.
    testing: Mutex<()>,
    /// What each disk last said about its health.
    smart: Arc<Smart>,
}

/// The name a drive is shown by: "MOVESPEED (E:)", "Local Disk (C:)", "DVD drive (D:)".
fn display_name(raw: &Raw) -> String {
    let place = raw.path.trim_end_matches(['\\', '/']);
    let fallback = match raw.kind {
        "optical" => "Disc drive",
        "removable" | "external" => "USB drive",
        "network" => "Network drive",
        "memory" => "Memory drive",
        _ => "Local Disk",
    };
    let label = if raw.label.is_empty() { fallback } else { raw.label.as_str() };
    if cfg!(windows) { format!("{label} ({place})") } else { label.to_string() }
}

/// Is `inner` at or under `outer`? By whole components, ignoring case on Windows.
pub fn is_within(inner: &Path, outer: &Path) -> bool {
    let fold = |p: &Path| -> Vec<String> {
        p.components()
            .map(|c| {
                let text = c.as_os_str().to_string_lossy().to_string();
                if cfg!(windows) { text.to_lowercase() } else { text }
            })
            .collect()
    };
    let (inner, outer) = (fold(inner), fold(outer));
    !outer.is_empty() && inner.len() >= outer.len() && inner[..outer.len()] == outer[..]
}

impl Drives {
    pub fn open(file: PathBuf, vault: &Path) -> Self {
        let seen = std::fs::read(&file).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default();
        Self {
            file,
            vault: crate::plain_path(vault),
            book: Mutex::new(Book { seen, ..Book::default() }),
            testing: Mutex::new(()),
            smart: Arc::new(Smart::new(find_smartctl())),
        }
    }

    fn save(&self, book: &mut Book) {
        let tmp = self.file.with_extension("json.tmp");
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::write(&tmp, serde_json::to_vec_pretty(&book.seen).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, &self.file);
        }
        book.saved = Some(Instant::now());
    }

    /// Looks at every drive now and brings what is remembered up to date.
    fn rescan(&self, book: &mut Book) {
        let now = now_millis();
        let found = scan();
        let starting = book.scanned.is_none();
        let mut changed = false;
        let mut present = Vec::new();
        let vault_drive =
            found.iter().filter(|raw| raw.media && is_within(&self.vault, Path::new(&raw.path))).map(|raw| raw.path.len()).max();

        for raw in &found {
            let key = raw.key();
            let was_present = book.present.iter().any(|d| d.key == key);
            let seen = book.seen.entry(key.clone()).or_insert_with(|| {
                changed = true;
                Seen { first_seen: now, ..Seen::default() }
            });
            // Just plugged in, unless the box has only now started and it was in last time too.
            let arrived = if starting { !seen.present } else { !was_present };
            if arrived || seen.plugged_in == 0 {
                seen.plugged_in = now;
                changed = true;
            }
            seen.name = display_name(raw);
            seen.path = raw.path.clone();
            seen.kind = raw.kind.to_string();
            seen.removable = raw.removable;
            seen.total = raw.total;
            seen.last_seen = now;
            seen.present = true;
            present.push(Drive {
                key,
                path: raw.path.clone(),
                name: seen.name.clone(),
                label: raw.label.clone(),
                kind: raw.kind,
                removable: raw.removable,
                read_only: raw.read_only,
                media: raw.media,
                file_system: raw.file_system.clone(),
                total: raw.total,
                free: raw.free,
                bus: raw.bus.clone(),
                model: raw.model.clone(),
                serial: raw.serial.clone(),
                vault: raw.media && Some(raw.path.len()) == vault_drive && is_within(&self.vault, Path::new(&raw.path)),
                first_seen: seen.first_seen,
                plugged_in: seen.plugged_in,
                last_test: seen.last_test.clone(),
                health: None,
                device: raw.device.clone(),
            });
        }
        for (key, seen) in book.seen.iter_mut() {
            if seen.present && !present.iter().any(|d| &d.key == key) {
                seen.present = false;
                changed = true;
            }
        }
        // Forget the oldest drives that are gone, beyond the few shown.
        let mut gone: Vec<(String, u64)> =
            book.seen.iter().filter(|(_, s)| !s.present).map(|(k, s)| (k.clone(), s.last_seen)).collect();
        if gone.len() > MAX_GONE * 4 {
            gone.sort_by_key(|(_, last)| *last);
            for (key, _) in gone.iter().take(gone.len() - MAX_GONE * 4) {
                book.seen.remove(key);
            }
        }

        book.present = present;
        book.fingerprint = Some(fingerprint());
        book.scanned = Some(Instant::now());
        if changed || book.saved.is_none_or(|at| at.elapsed() >= SAVE_EVERY) {
            self.save(book);
        }
    }

    /// Called every couple of seconds by the box: notices a drive going in or out without
    /// looking at every drive each time.
    pub fn tick(&self) {
        let Ok(mut book) = self.book.lock() else { return };
        let stale = book.scanned.is_none_or(|at| at.elapsed() >= RESCAN_EVERY);
        if stale || book.fingerprint != Some(fingerprint()) {
            self.rescan(&mut book);
        }
        let devices: Vec<String> = book.present.iter().map(|d| d.device.clone()).collect();
        drop(book);
        Smart::refresh(&self.smart, devices);
    }

    /// The drives there now, each with what its disk last said about its health.
    fn present_with_health(&self, book: &Book) -> Vec<Drive> {
        let readings = self.smart.readings.lock().map(|r| r.clone()).unwrap_or_default();
        book.present
            .iter()
            .cloned()
            .map(|mut d| {
                d.health = readings.get(&d.device).map(|(health, _)| health.clone());
                // Linux does not name the disk behind a mount; the disk names itself.
                if let Some(health) = d.health.as_ref().filter(|_| d.model.is_empty()) {
                    d.model = health.model.clone();
                }
                d
            })
            .collect()
    }

    /// The drives in a few numbers, for the spec card.
    pub fn summary(&self) -> Summary {
        let Ok(mut book) = self.book.lock() else { return Summary::default() };
        if book.scanned.is_none() {
            self.rescan(&mut book);
        }
        summarize(&self.present_with_health(&book), self.smart.tool.is_some())
    }

    /// Every drive there now, and the ones seen before that are not.
    pub fn report(&self) -> Report {
        let Ok(mut book) = self.book.lock() else {
            return Report { drives: Vec::new(), gone: Vec::new(), at: now_millis(), smartctl: self.smart.tool.is_some() };
        };
        self.rescan(&mut book);
        let mut gone: Vec<Gone> = book
            .seen
            .values()
            .filter(|s| !s.present && s.removable && s.total > 0)
            .map(|s| Gone {
                name: s.name.clone(),
                path: s.path.clone(),
                kind: s.kind.clone(),
                total: s.total,
                first_seen: s.first_seen,
                last_seen: s.last_seen,
                last_test: s.last_test.clone(),
            })
            .collect();
        gone.sort_by_key(|g| std::cmp::Reverse(g.last_seen));
        gone.truncate(MAX_GONE);
        Report { drives: self.present_with_health(&book), gone, at: now_millis(), smartctl: self.smart.tool.is_some() }
    }

    /// Times the drive at `path` (one of the drives there now). `write` false reads only.
    pub fn speed_test(&self, path: &str, write: bool) -> Result<SpeedResult, String> {
        let _one = self.testing.try_lock().map_err(|_| "A speed test is already running.".to_string())?;
        let drive = {
            let mut book = self.book.lock().map_err(|_| "The drive list is unavailable.".to_string())?;
            self.rescan(&mut book);
            book.present.iter().find(|d| d.path == path).cloned().ok_or("That drive is not plugged in.")?
        };
        if !drive.media {
            return Err("There is nothing in that drive.".into());
        }
        if drive.kind == "network" {
            return Err("A network drive's speed is the network's; test it from Network.".into());
        }
        let root = PathBuf::from(&drive.path);
        let result = if write {
            if drive.read_only {
                return Err("This drive cannot be written to. Run the read-only test instead.".into());
            }
            // The top of a drive first. The system's own drive does not let an ordinary
            // account write there, so then a temporary folder of the box's on that drive.
            let mut folders = vec![root.clone()];
            folders.extend([self.vault.join(crate::META_DIR).join("tmp"), std::env::temp_dir()].into_iter().filter(|dir| is_within(dir, &root)));
            write_then_read(&folders, drive.free)?
        } else {
            read_existing(&root)?
        };
        if let Ok(mut book) = self.book.lock() {
            if let Some(seen) = book.seen.get_mut(&drive.key) {
                seen.last_test = Some(result.clone());
            }
            if let Some(present) = book.present.iter_mut().find(|d| d.key == drive.key) {
                present.last_test = Some(result.clone());
            }
            self.save(&mut book);
        }
        Ok(result)
    }
}

// ---------------------------------------------------------------------------------------
// The speed test
// ---------------------------------------------------------------------------------------

/// A buffer whose start sits on a sector boundary, which reading past the system's cache needs.
struct Aligned {
    raw: Vec<u8>,
    start: usize,
}

impl Aligned {
    fn new() -> Self {
        let raw = vec![0u8; CHUNK + SECTOR_ALIGN];
        let start = raw.as_ptr().align_offset(SECTOR_ALIGN);
        Self { raw, start }
    }
    fn bytes(&mut self) -> &mut [u8] {
        &mut self.raw[self.start..self.start + CHUNK]
    }
}

/// Opens a file to be read from the drive itself, not from what the system has kept in memory.
fn open_uncached(path: &Path) -> std::io::Result<std::fs::File> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
        const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x0800_0000;
        std::fs::OpenOptions::new().read(true).custom_flags(FILE_FLAG_NO_BUFFERING | FILE_FLAG_SEQUENTIAL_SCAN).open(path)
    }
    #[cfg(not(windows))]
    {
        use std::os::fd::AsRawFd;
        unsafe extern "C" {
            fn posix_fadvise(fd: i32, offset: i64, len: i64, advice: i32) -> i32;
        }
        const POSIX_FADV_DONTNEED: i32 = 4;
        let file = std::fs::File::open(path)?;
        // SAFETY: the descriptor is open for as long as `file` lives. This only drops the
        // system's cached copy of the file, so the read that follows comes from the drive.
        unsafe { posix_fadvise(file.as_raw_fd(), 0, 0, POSIX_FADV_DONTNEED) };
        Ok(file)
    }
}

fn mbps(bytes: u64, took: Duration) -> f64 {
    let secs = took.as_secs_f64().max(1e-6);
    (bytes as f64 / secs / 1e6 * 10.0).round() / 10.0
}

/// Reads up to `limit` bytes of `path` from the drive; returns how many it read.
fn time_read(path: &Path, buf: &mut Aligned, limit: u64, started: Instant, read_so_far: u64) -> std::io::Result<u64> {
    let mut file = open_uncached(path)?;
    let mut read = 0u64;
    while read_so_far + read < limit {
        let n = file.read(buf.bytes())?;
        if n == 0 {
            break;
        }
        read += n as u64;
        if read_so_far + read >= TEST_MIN && started.elapsed() >= TEST_TIME {
            break;
        }
    }
    Ok(read)
}

/// Deletes the speed test's file however the test ends.
struct TestFile(PathBuf);

impl Drop for TestFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Writes a test file to the drive, makes sure it reached the drive, reads it back, deletes it.
fn write_then_read(folders: &[PathBuf], free: u64) -> Result<SpeedResult, String> {
    // Never use more than a quarter of what is free.
    let limit = (TEST_MAX.min(free / 4) / CHUNK as u64) * CHUNK as u64;
    if limit < (8 * CHUNK) as u64 {
        return Err("This drive is too full to test: it needs about 128 MB free.".into());
    }
    let failed = |e: std::io::Error| format!("The speed test could not use this drive: {e}");
    let name = format!("{TEST_FILE_PREFIX}{}.tmp", crate::random_hex(8));
    let mut opened = None;
    let mut last_error = None;
    for dir in folders {
        let _ = std::fs::create_dir_all(dir);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(dir.join(&name)) {
            Ok(file) => {
                opened = Some((file, TestFile(dir.join(&name))));
                break;
            }
            Err(e) => last_error = Some(e),
        }
    }
    let Some((mut file, test)) = opened else {
        return Err(failed(last_error.unwrap_or_else(|| std::io::Error::other("no folder to write in"))));
    };

    // Random bytes, so a drive that compresses what it stores is timed fairly.
    let mut block = vec![0u8; CHUNK];
    getrandom::fill(&mut block).map_err(|_| "No random numbers for the test file.".to_string())?;
    let started = Instant::now();
    let mut written = 0u64;
    {
        while written < limit {
            file.write_all(&block).map_err(failed)?;
            written += CHUNK as u64;
            if written >= TEST_MIN && started.elapsed() >= TEST_TIME {
                break;
            }
        }
        // The time counts until the drive says the bytes are on it, not in a cache.
        file.sync_all().map_err(failed)?;
    }
    let write_took = started.elapsed();
    drop(file);

    let mut buf = Aligned::new();
    let read_started = Instant::now();
    let read = time_read(&test.0, &mut buf, written, read_started, 0).map_err(failed)?;
    let read_took = read_started.elapsed();
    drop(test);
    Ok(SpeedResult {
        at: now_millis(),
        mode: "write".into(),
        write_mbps: Some(mbps(written, write_took)),
        read_mbps: mbps(read, read_took),
        bytes: written,
    })
}

/// The biggest files near the top of a drive, to read for a read-only test.
fn files_to_read(root: &Path) -> Vec<PathBuf> {
    const MAX_DEPTH: usize = 4;
    const MAX_LOOKED_AT: usize = 4000;
    let mut found: Vec<(u64, PathBuf)> = Vec::new();
    let mut folders = vec![(root.to_path_buf(), 0usize)];
    let mut looked = 0;
    while let Some((dir, depth)) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            looked += 1;
            if looked > MAX_LOOKED_AT {
                break;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() && depth < MAX_DEPTH {
                folders.push((entry.path(), depth + 1));
            } else if meta.is_file() && meta.len() >= CHUNK as u64 {
                found.push((meta.len(), entry.path()));
            }
        }
    }
    found.sort_by_key(|(size, _)| std::cmp::Reverse(*size));
    found.into_iter().map(|(_, path)| path).collect()
}

/// Reads files already on the drive. Writes nothing.
fn read_existing(root: &Path) -> Result<SpeedResult, String> {
    let mut buf = Aligned::new();
    let started = Instant::now();
    let mut read = 0u64;
    for path in files_to_read(root) {
        // A file that cannot be opened (in use, not allowed) is skipped, not fatal.
        read += time_read(&path, &mut buf, TEST_MAX, started, read).unwrap_or(0);
        if read >= TEST_MAX || (read >= TEST_MIN && started.elapsed() >= TEST_TIME) {
            break;
        }
    }
    if read < (2 * CHUNK) as u64 {
        return Err("There are not enough large files on this drive to time a read.".into());
    }
    Ok(SpeedResult { at: now_millis(), mode: "read".into(), write_mbps: None, read_mbps: mbps(read, started.elapsed()), bytes: read })
}

// ---------------------------------------------------------------------------------------
// Health: what each disk says about itself, asked with smartctl
// ---------------------------------------------------------------------------------------

/// The note for a disk that was asleep when asked. It keeps what it said last time.
const ASLEEP: &str = "The disk is asleep, and Castle did not wake it. It is asked again later.";
const NEEDS_ADMIN: &str = "Castle has to run as administrator (root on Linux) to ask this disk how it is.";
const NO_ANSWERS: &str = "This disk does not answer health questions. Common behind USB enclosures and cheap adapters.";

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    /// ok; worn (bad sectors, or an SSD past its rated life); failing (the disk itself
    /// says so); or unknown (it did not answer, and `note` says why).
    pub state: String,
    pub power_on_hours: Option<u64>,
    /// Degrees Celsius.
    pub temperature: Option<i64>,
    /// The disk's own overall verdict.
    pub passed: Option<bool>,
    /// Sectors moved aside or waiting to be (spinning disks), or media errors (NVMe).
    pub bad_sectors: Option<u64>,
    /// How much of its rated life an SSD says it has used, in percent. Can pass 100.
    pub wear_percent: Option<u64>,
    /// Why there is no reading, in plain words. Empty when there is one.
    pub note: String,
    /// When it was asked, milliseconds since 1970.
    pub at: u64,
    /// What the disk calls itself, for drives the system did not name. Not sent.
    #[serde(skip)]
    model: String,
    /// Tells partitions of one disk apart from separate disks. Not sent.
    #[serde(skip)]
    serial: String,
}

fn unread(note: &str) -> Health {
    Health { state: "unknown".into(), note: note.into(), at: now_millis(), ..Health::default() }
}

/// Reads what `smartctl -j` printed. Every field is optional: what a disk reports depends
/// on what it is (SATA, NVMe, behind USB) and on the smartctl version.
fn parse_smartctl(text: &str) -> Health {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return unread("smartctl gave an answer Castle could not read.") };
    let exit = v["smartctl"]["exit_status"].as_u64().unwrap_or(0);
    let said: Vec<&str> =
        v["smartctl"]["messages"].as_array().map(|m| m.iter().filter_map(|m| m["string"].as_str()).collect()).unwrap_or_default();
    let attribute = |id: u64| {
        v["ata_smart_attributes"]["table"]
            .as_array()
            .and_then(|table| table.iter().find(|a| a["id"].as_u64() == Some(id)))
            .and_then(|a| a["raw"]["value"].as_u64())
    };
    let nvme = &v["nvme_smart_health_information_log"];

    let power_on_hours = v["power_on_time"]["hours"].as_u64().or_else(|| nvme["power_on_hours"].as_u64()).or_else(|| attribute(9));
    let temperature = v["temperature"]["current"].as_i64().or_else(|| nvme["temperature"].as_i64());
    // Bit 3 of the exit status: the disk's own check says it is failing.
    let passed = if exit & 0x08 != 0 { Some(false) } else { v["smart_status"]["passed"].as_bool() };
    // 5: reallocated sectors; 197: sectors waiting to be reallocated.
    let bad_sectors = match (attribute(5), attribute(197)) {
        (None, None) => nvme["media_errors"].as_u64(),
        (moved, waiting) => Some(moved.unwrap_or(0) + waiting.unwrap_or(0)),
    };
    let wear_percent = nvme["percentage_used"].as_u64();
    let model = v["model_name"].as_str().unwrap_or("").trim().to_string();
    let serial = v["serial_number"].as_str().unwrap_or("").trim().to_string();

    if power_on_hours.is_none() && passed.is_none() {
        let words = said.join(" ");
        let lower = words.to_lowercase();
        let note = if lower.contains("standby") || lower.contains("sleep") {
            ASLEEP.to_string()
        } else if ["denied", "permission", "not permitted", "root", "administrator"].iter().any(|w| lower.contains(w)) {
            NEEDS_ADMIN.to_string()
        } else if ["unknown usb bridge", "unable to detect device type", "unsupported", "not support"].iter().any(|w| lower.contains(w)) {
            NO_ANSWERS.to_string()
        } else if words.is_empty() {
            "The disk did not say how it is.".to_string()
        } else {
            words
        };
        return Health { model, serial, ..unread(&note) };
    }

    let state = if passed == Some(false) {
        "failing"
    } else if bad_sectors.unwrap_or(0) > 0 || wear_percent.unwrap_or(0) >= 100 {
        "worn"
    } else {
        "ok"
    };
    Health {
        state: state.into(),
        power_on_hours,
        temperature,
        passed,
        bad_sectors,
        wear_percent,
        note: String::new(),
        at: now_millis(),
        model,
        serial,
    }
}

/// smartctl, when it is installed: CASTLE_SMARTCTL, then the system's path, then where
/// the smartmontools installer puts it.
fn find_smartctl() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "smartctl.exe" } else { "smartctl" };
    let mut places: Vec<PathBuf> = std::env::var_os("CASTLE_SMARTCTL").map(PathBuf::from).into_iter().collect();
    if let Some(path) = std::env::var_os("PATH") {
        places.extend(std::env::split_paths(&path).map(|dir| dir.join(exe)));
    }
    if cfg!(windows) {
        for base in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(dir) = std::env::var_os(base) {
                places.push(PathBuf::from(dir).join("smartmontools").join("bin").join(exe));
            }
        }
    } else {
        places.extend(["/usr/sbin/smartctl", "/usr/local/sbin/smartctl", "/sbin/smartctl"].map(PathBuf::from));
    }
    places.into_iter().find(|p| p.is_file())
}

/// Asks one disk. `-n standby` leaves a sleeping disk asleep rather than spinning it up
/// to answer. No window opens on Windows, and a disk that hangs is given up on.
fn ask_smartctl(tool: &Path, device: &str) -> Health {
    let mut cmd = Command::new(tool);
    cmd.args(["-j", "-i", "-H", "-A", "-n", "standby", device]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let Ok(mut child) = cmd.spawn() else { return unread("smartctl is installed but could not be started.") };
    let mut out = child.stdout.take();
    // Read while it runs, so a long answer cannot fill the pipe and stall it.
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(out) = out.as_mut() {
            let _ = out.read_to_string(&mut text);
        }
        text
    });
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < SMART_TIMEOUT => std::thread::sleep(SMART_POLL),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return unread("The disk took too long to answer.");
            }
        }
    }
    parse_smartctl(&reader.join().unwrap_or_default())
}

struct Smart {
    tool: Option<PathBuf>,
    /// device -> what it said, and when it was asked
    readings: Mutex<HashMap<String, (Health, Instant)>>,
    /// One round of asking at a time.
    asking: AtomicBool,
}

/// Lets the next round start when this one ends, however it ends.
struct RoundDone(Arc<Smart>);

impl Drop for RoundDone {
    fn drop(&mut self) {
        self.0.asking.store(false, Ordering::SeqCst);
    }
}

impl Smart {
    fn new(tool: Option<PathBuf>) -> Self {
        Self { tool, readings: Mutex::new(HashMap::new()), asking: AtomicBool::new(false) }
    }

    /// Asks, in the background, each of `devices` not asked in the last SMART_EVERY. Never
    /// waits for smartctl, so the drive list stays quick.
    fn refresh(smart: &Arc<Smart>, devices: Vec<String>) {
        let Some(tool) = smart.tool.clone() else { return };
        let mut due: Vec<String> = {
            let Ok(readings) = smart.readings.lock() else { return };
            devices
                .into_iter()
                .filter(|d| !d.is_empty() && readings.get(d).is_none_or(|(_, at)| at.elapsed() >= SMART_EVERY))
                .collect()
        };
        due.sort();
        due.dedup();
        if due.is_empty() || smart.asking.swap(true, Ordering::SeqCst) {
            return;
        }
        let done = RoundDone(Arc::clone(smart));
        std::thread::spawn(move || {
            let smart = &done.0;
            for device in due {
                let mut health = ask_smartctl(&tool, &device);
                if health.note == ASLEEP {
                    let before = smart.readings.lock().ok().and_then(|r| r.get(&device).map(|(h, _)| h.clone()));
                    if let Some(before) = before.filter(|h| h.note.is_empty()) {
                        health = before;
                    }
                }
                if let Ok(mut readings) = smart.readings.lock() {
                    readings.insert(device, (health, Instant::now()));
                }
            }
        });
    }
}

/// Adds up the disks that stay (not sticks, cards or discs). Two drive letters on one disk
/// count once, when the disk has said who it is.
fn summarize(drives: &[Drive], smartctl: bool) -> Summary {
    let disks: Vec<&Drive> = drives.iter().filter(|d| d.media && matches!(d.kind, "fixed" | "external")).collect();
    // One entry per physical disk: by the serial it gave, or by the drive when it gave none.
    let mut one_each: BTreeMap<String, Option<&Health>> = BTreeMap::new();
    for d in &disks {
        let health = d.health.as_ref().filter(|h| h.note.is_empty());
        let who = d.health.as_ref().filter(|h| !h.serial.is_empty()).map_or_else(|| d.key.clone(), |h| h.serial.clone());
        one_each.entry(who).or_insert(health);
    }
    let read: Vec<&Health> = one_each.values().flatten().copied().collect();
    let count = |state: &str| read.iter().filter(|h| h.state == state).count();
    Summary {
        disks: one_each.len(),
        total: disks.iter().map(|d| d.total).sum(),
        free: disks.iter().map(|d| d.free).sum(),
        oldest_hours: read.iter().filter_map(|h| h.power_on_hours).max(),
        reporting: read.len(),
        failing: count("failing"),
        worn: count("worn"),
        smartctl,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drives_are_found_and_named() {
        let found = scan();
        assert!(!found.is_empty(), "this computer has at least one drive");
        for raw in &found {
            assert!(!display_name(raw).is_empty());
            assert!(raw.free <= raw.total || !raw.media);
        }
    }

    #[test]
    fn a_drive_is_known_by_what_it_is_not_where_it_is() {
        let usb = Raw { path: "F:\\".into(), media: true, serial: "1A2B3C4D".into(), file_system: "exFAT".into(), total: 64, ..Raw::default() };
        let moved = Raw { path: "G:\\".into(), ..usb.clone() };
        assert_eq!(usb.key(), moved.key());
        let empty = Raw { path: "D:\\".into(), media: false, ..Raw::default() };
        assert_eq!(empty.key(), "slot|D:\\");
    }

    #[test]
    fn within_compares_whole_names() {
        assert!(is_within(Path::new("/srv/castle/vault"), Path::new("/srv/castle")));
        assert!(is_within(Path::new("/srv/castle"), Path::new("/srv/castle")));
        assert!(!is_within(Path::new("/srv/castles"), Path::new("/srv/castle")));
        assert!(!is_within(Path::new("/srv"), Path::new("/srv/castle")));
    }

    #[test]
    fn names_say_what_the_drive_is() {
        let disc = Raw { path: "D:\\".into(), kind: "optical", ..Raw::default() };
        let stick = Raw { path: "E:\\".into(), kind: "removable", label: "MOVESPEED".into(), ..Raw::default() };
        if cfg!(windows) {
            assert_eq!(display_name(&disc), "Disc drive (D:)");
            assert_eq!(display_name(&stick), "MOVESPEED (E:)");
        }
    }

    #[test]
    fn plugging_in_is_dated_and_remembered() {
        let dir = std::env::temp_dir().join(format!("castled-drives-{}", crate::random_hex(6)));
        let file = dir.join("drives.json");
        let drives = Drives::open(file.clone(), &dir);
        let report = drives.report();
        assert!(!report.drives.is_empty());
        let first = report.drives[0].clone();
        assert!(first.plugged_in > 0 && first.first_seen > 0);
        // A second look does not move the plug-in time.
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(drives.report().drives[0].plugged_in, first.plugged_in);
        // Nor does a restart, while the drive stays in.
        let again = Drives::open(file, &dir);
        assert_eq!(again.report().drives[0].plugged_in, first.plugged_in);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_write_test_times_a_file_and_leaves_nothing_behind() {
        let dir = std::env::temp_dir().join(format!("castled-speed-{}", crate::random_hex(6)));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a-file"), b"").unwrap();
        let result = write_then_read(&[dir.join("a-file").join("inside"), dir.clone()], 4 * TEST_MAX).unwrap();
        assert_eq!(result.mode, "write");
        assert!(result.write_mbps.unwrap() > 0.0 && result.read_mbps > 0.0);
        assert!(result.bytes >= TEST_MIN);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "the test file is deleted; only a-file is left");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_nearly_full_drive_is_not_tested() {
        assert!(write_then_read(&[std::env::temp_dir()], 100 * 1024 * 1024).is_err());
    }

    /// Trimmed from what smartctl 7.4 prints for a SATA disk off eBay.
    const OLD_HDD: &str = r#"{
      "smartctl": { "version": [7, 4], "exit_status": 64, "messages": [] },
      "device": { "name": "/dev/sda", "type": "sat" },
      "model_name": "WDC WD120EMFZ-11A6JA0", "serial_number": "9RJ1ABCD",
      "smart_status": { "passed": true },
      "ata_smart_attributes": { "table": [
        { "id": 5, "name": "Reallocated_Sector_Ct", "raw": { "value": 0 } },
        { "id": 9, "name": "Power_On_Hours", "raw": { "value": 67012 } },
        { "id": 197, "name": "Current_Pending_Sector", "raw": { "value": 0 } }
      ] },
      "power_on_time": { "hours": 67012 },
      "temperature": { "current": 38 }
    }"#;

    const NVME: &str = r#"{
      "smartctl": { "exit_status": 0 },
      "model_name": "Samsung SSD 990 PRO 1TB", "serial_number": "S6Z1NJ0W",
      "smart_status": { "passed": true },
      "nvme_smart_health_information_log": { "temperature": 41, "percentage_used": 3, "power_on_hours": 2210, "media_errors": 0 },
      "temperature": { "current": 41 },
      "power_on_time": { "hours": 2210 }
    }"#;

    #[test]
    fn an_old_disk_reports_its_hours() {
        let h = parse_smartctl(OLD_HDD);
        assert_eq!(h.state, "ok");
        assert_eq!(h.power_on_hours, Some(67012));
        assert_eq!(h.temperature, Some(38));
        assert_eq!(h.passed, Some(true));
        assert_eq!(h.bad_sectors, Some(0));
        assert_eq!(h.model, "WDC WD120EMFZ-11A6JA0");
        assert!(h.note.is_empty());
    }

    #[test]
    fn an_nvme_disk_reports_wear() {
        let h = parse_smartctl(NVME);
        assert_eq!(h.state, "ok");
        assert_eq!(h.power_on_hours, Some(2210));
        assert_eq!(h.wear_percent, Some(3));
        assert_eq!(h.bad_sectors, Some(0));
    }

    #[test]
    fn bad_sectors_and_used_up_flash_are_worn() {
        let sectors = OLD_HDD.replace(r#""id": 197, "name": "Current_Pending_Sector", "raw": { "value": 0 }"#, r#""id": 197, "raw": { "value": 8 }"#);
        let h = parse_smartctl(&sectors);
        assert_eq!((h.state.as_str(), h.bad_sectors), ("worn", Some(8)));
        let flash = NVME.replace(r#""percentage_used": 3"#, r#""percentage_used": 104"#);
        assert_eq!(parse_smartctl(&flash).state, "worn");
    }

    #[test]
    fn a_disk_that_says_it_is_failing_is_failing() {
        let verdict = OLD_HDD.replace(r#""passed": true"#, r#""passed": false"#);
        assert_eq!(parse_smartctl(&verdict).state, "failing");
        // The exit status alone is enough, even with no verdict in the answer.
        let status = r#"{ "smartctl": { "exit_status": 8 }, "power_on_time": { "hours": 9 } }"#;
        let h = parse_smartctl(status);
        assert_eq!((h.state.as_str(), h.passed), ("failing", Some(false)));
    }

    #[test]
    fn a_disk_that_did_not_answer_says_why() {
        let denied = r#"{ "smartctl": { "exit_status": 2, "messages": [ { "string": "Smartctl open device: /dev/sda failed: Permission denied", "severity": "error" } ] } }"#;
        assert_eq!(parse_smartctl(denied).note, NEEDS_ADMIN);
        let asleep = r#"{ "smartctl": { "exit_status": 2, "messages": [ { "string": "Device is in STANDBY mode, exit(2)", "severity": "information" } ] } }"#;
        assert_eq!(parse_smartctl(asleep).note, ASLEEP);
        let bridge = r#"{ "smartctl": { "exit_status": 1, "messages": [ { "string": "/dev/sdb: Unknown USB bridge [0x152d:0x0578 (0x214)]", "severity": "error" } ] } }"#;
        assert_eq!(parse_smartctl(bridge).note, NO_ANSWERS);
        let h = parse_smartctl("not json at all");
        assert_eq!(h.state, "unknown");
        assert!(!h.note.is_empty());
    }

    fn disk(key: &str, total: u64, health: Option<Health>) -> Drive {
        Drive {
            key: key.into(),
            path: String::new(),
            name: String::new(),
            label: String::new(),
            kind: "fixed",
            removable: false,
            read_only: false,
            media: true,
            file_system: String::new(),
            total,
            free: total / 2,
            bus: String::new(),
            model: String::new(),
            serial: String::new(),
            vault: false,
            first_seen: 0,
            plugged_in: 0,
            last_test: None,
            health,
            device: String::new(),
        }
    }

    #[test]
    fn the_summary_counts_disks_once_and_finds_the_oldest() {
        let old = parse_smartctl(OLD_HDD);
        let failing = parse_smartctl(&NVME.replace(r#""passed": true"#, r#""passed": false"#));
        let drives = vec![
            disk("c", 500, Some(failing.clone())),
            disk("d", 500, Some(failing)), // a second partition of the same NVMe disk
            disk("e", 12_000, Some(old)),
            disk("f", 4_000, Some(unread(NEEDS_ADMIN))),
            Drive { kind: "removable", ..disk("stick", 64, None) },
        ];
        let s = summarize(&drives, true);
        assert_eq!(s.disks, 3);
        assert_eq!(s.total, 17_000);
        assert_eq!(s.oldest_hours, Some(67012));
        assert_eq!(s.reporting, 2);
        assert_eq!((s.failing, s.worn), (1, 0));
    }

    #[test]
    fn without_smartctl_nothing_is_asked() {
        let smart = Arc::new(Smart::new(None));
        Smart::refresh(&smart, vec!["C:".into()]);
        assert!(!smart.asking.load(Ordering::SeqCst));
        assert!(smart.readings.lock().unwrap().is_empty());
    }
}
