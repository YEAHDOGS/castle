//! Clones: virtual machines and the automations that run in them, driven through Clonewars.
//!
//! Three things live here:
//!   * the catalogue of operating systems Clonewars can boot (read from its target list),
//!   * machines: a target's own disk, a named disk of its own, or a throwaway run whose
//!     writes are dropped when the VM closes,
//!   * automations: a recipe (target, project, packages, tasks, sign-in fields) plus any
//!     number of bots, each with its own sign-in and its own disk, so several run at once.
//!
//! Every launch is `start.ps1` in the clonewars folder, given a profile this module writes.
//! What the scripts download and create (disks, ISOs, the Android SDK, payloads) is in its
//! data folder: `data/clonewars` beside the clonewars folder (`app/data/clonewars` in a
//! checkout), or CASTLE_CLONEWARS_DATA. The scripts look in the same place.
//! A unit runs in one of three engines: a QEMU VM (its disk in the data folder), a container
//! (`container.ps1`: Docker, one image shared by all, the container is the disk), or
//! Google's Android emulator (`android.ps1`, one AVD per unit). Android automations run
//! their tasks on the phone (`castle-tasks.sh`), never on the box.
//! Tasks and scripts run inside the guest, never on the box itself. All of it is off unless
//! the daemon has a clonewars folder, and it only acts when the daemon listens on this
//! machine alone: an automation's tasks are commands, so this API is for the owner at the
//! keyboard and nobody else.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU16, AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    results::{Results, RunRef},
    script,
};

const LOG_TAIL_LINES: usize = 60;
const MAX_VALUE_LEN: usize = 200;
const MAX_COMMAND_LEN: usize = 2000;
const MAX_LIST: usize = 64;
const MAX_BOTS: usize = 32;
const SLUG_LEN: usize = 32;
const BENCH_KEEP: usize = 200;
/// A disk that was created but never installed to holds only its own bookkeeping.
const EMPTY_DISK_BYTES: u64 = 1024 * 1024;
const SCAN_CACHE: Duration = Duration::from_secs(3);
const QMP_TIMEOUT: Duration = Duration::from_secs(5);
/// When the box stops, how long its VMs get to shut down when asked before they are switched off.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(20);
const WATCH_PAUSE: Duration = Duration::from_millis(300);
const QMP_FLAG: &str = "-qmp tcp:127.0.0.1:";
/// The launcher prints this just before it hands the machine to QEMU.
const LOG_VM_HANDOFF: &str = "QMP:";
/// container.ps1 prints this when it hands the container to Docker.
const LOG_CONTAINER_HANDOFF: &str = "CONTAINER:";
/// Docker label every Castle container carries (container.ps1).
const CONTAINER_LABEL: &str = "net.wearedogs.castle.unit";
/// The script an Android automation runs on the phone.
const TASKS_FILE: &str = "castle-tasks.sh";
const MAIN_BOT: &str = "main";
const BOT_ENV_FILE: &str = "castle-bot.env";
const PROJECT_ZIP: &str = "project.zip";
/// Git hosts a project may be fetched from. Anything else has to be put on the box by hand.
const TRUSTED_HOSTS: [&str; 4] = ["github.com", "gitlab.com", "codeberg.org", "bitbucket.org"];
const WALMART_ID: &str = "walmart-motor-oil";
const WALMART_SCRIPT: &str = include_str!("../../app/clonewars/automation/examples/walmart-motor-oil.castle");
/// The castle library and its runner, inside the clonewars folder.
const LIBRARY_DIR: &str = "automation/castle";
/// What a script run brings with it into the machine.
const RUN_FILE: &str = "castle-run.json";
const SCRIPT_TASK: &str = "bash castle/run.sh";
/// How often the scheduler looks at the clock and at runs that asked to be stopped.
const TICK: Duration = Duration::from_secs(5);
/// A finished script's machine is stopped this long after it said so, to let it exit cleanly.
const STOP_AFTER_FINISH: Duration = Duration::from_secs(4);
/// The scheduler looks at the clock every this many ticks (half a minute).
const SCHEDULE_EVERY_TICKS: u64 = 6;
/// What `report` answers for a secret the box never gave out.
pub const REFUSED: &str = "this run needs its own secret";
/// Hides the console of helper processes on Windows; the VM's own window still appears.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Asks Clonewars for its target list as JSON. Plain data: nothing is downloaded or booted.
const TARGETS_SCRIPT: &str = ". ./modules/targets.ps1; . ./modules/launch.ps1; ConvertTo-Json -Compress @($IsoMatrix | ForEach-Object { @{ id=$_.Id; name=$_.Name; description=[string]$_.Description; family=[string]$_.OsFamily; runtime=[string]$_.Runtime; cloud=($_.ImageKind -eq 'cloud'); inject=[bool]$_.SupportsCopy; disk=(Get-TargetDiskName $_); iso=[string]$_.IsoName; size=[string]$_.DiskSize } })";

pub type ClonesResult<T> = Result<T, String>;

/// One operating system Clonewars can boot.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct Target {
    id: String,
    name: String,
    description: String,
    /// linux | windows | android | macos
    family: String,
    /// Empty for a QEMU machine; "docker" or "avd" for targets with a runtime of their own.
    runtime: String,
    cloud: bool,
    /// Packages, commands and files can be put into this guest from the host.
    inject: bool,
    /// File name of its disk in the data folder; `*` stands for a version.
    disk: String,
    /// File name of the image it downloads.
    iso: String,
    size: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Task {
    name: String,
    command: String,
    /// Runs as root before the desktop session, instead of as the user inside it.
    root: bool,
}

/// One value a bot needs, kept as a line of the project's `.env`.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Field {
    env: String,
    label: String,
    secret: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
struct Bot {
    id: String,
    name: String,
    /// Field values saved for this bot. A field left out falls back to the project's `.env`.
    values: HashMap<String, String>,
}

/// An automation as it is saved, and as the app sends it when one is created or changed.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AutomationDef {
    id: String,
    name: String,
    description: String,
    target: String,
    /// The project: an https git address on a trusted host, or a folder on the box.
    source: String,
    /// A Clonewars setup script of its own (`scripts/setup-<name>.sh`). Only built-in
    /// automations carry one; the app cannot set it.
    setup: String,
    memory_gb: u32,
    cores: u32,
    screen: String,
    apt: Vec<String>,
    pacman: Vec<String>,
    tasks: Vec<Task>,
    fields: Vec<Field>,
    /// Start the tasks again whenever they finish.
    restart: bool,
    /// Shut the machine down this many minutes after it starts. 0 means never.
    stop_after_minutes: u32,
    /// Every run is a throwaway: nothing a bot writes to its disk is kept.
    temporary: bool,
    builtin: bool,
    bots: Vec<Bot>,
    /// A Castle script (see script.rs). When there is one it is the automation: Castle
    /// compiles it, brings the castle library and runs it, and `source` and `tasks` are unused.
    script: String,
    /// "HH:MM": start the first bot every day at that time on the box's clock. Empty: never.
    schedule: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct BotBody {
    pub id: String,
    pub bot: String,
    pub name: String,
    pub values: HashMap<String, String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MachineBody {
    target: String,
    /// existing | new | temporary
    mode: String,
    /// existing: which machine to start.
    machine: String,
    /// new: what to call it.
    name: String,
    memory_gb: u32,
    cores: u32,
    /// new: install from scratch instead of starting from the target's built disk.
    fresh: bool,
    apt: Vec<String>,
    pacman: Vec<String>,
    root_commands: Vec<String>,
    user_commands: Vec<String>,
    /// Files and folders on the box to put into the guest (they arrive in ~/castle-copy).
    paths: Vec<String>,
}

/// A machine with a disk of its own, made from the app.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
struct Machine {
    id: String,
    name: String,
    target: String,
    temporary: bool,
    memory_gb: u32,
    cores: u32,
}

/// One thing that can be started and stopped: a bot or a machine.
struct Unit {
    key: String,
    target: String,
    /// Its own disk in the data folder's instances/; None means the target's main disk.
    instance: Option<String>,
    engine: Engine,
}

/// Where a unit runs.
#[derive(Clone, PartialEq)]
enum Engine {
    Vm,
    /// A Docker container of this name (`castle-<instance or target>`).
    Container(String),
    /// Google's emulator with an AVD of this name.
    Emulator(String),
}

impl Engine {
    /// vm | container | emulator, as the app sees it.
    fn label(&self) -> &'static str {
        match self {
            Engine::Vm => "vm",
            Engine::Container(_) => "container",
            Engine::Emulator(_) => "emulator",
        }
    }
}

struct Launch {
    serial: u64,
    child: Child,
    /// Exit code once the launcher finished; None while it is still setting the VM up.
    exit: Option<i32>,
}

struct Scan {
    at: Instant,
    /// Command lines of every running QEMU process.
    qemu: Vec<String>,
}

struct ContainerScan {
    at: Instant,
    /// Every Castle container there is, and whether it is running.
    all: Vec<(String, bool)>,
}

/// The Clonewars profile written for one launch.
#[derive(Default)]
struct Profile {
    description: String,
    target: String,
    instance: Option<String>,
    memory_gb: u32,
    cores: u32,
    screen: String,
    /// (folder on the box, zip name in the guest, files to add that git ignores)
    payload: Vec<(PathBuf, String, Vec<String>)>,
    apt: Vec<String>,
    pacman: Vec<String>,
    root: Vec<String>,
    user: Vec<String>,
    setup: String,
    /// CASTLE_* variables for this launch only.
    env: Vec<(String, String)>,
}

struct Plan {
    unit: Unit,
    label: String,
    mode: &'static str,
    /// None starts the target plainly; a profile is needed to put anything into the guest.
    profile: Option<Profile>,
    snapshot: bool,
    base: Option<PathBuf>,
    memory_gb: u32,
    cores: u32,
    copy_dir: Option<PathBuf>,
    /// The payload folder is the whole injection: no profile, the launcher just takes its
    /// files (the Android emulator installs the APKs and pushes the rest).
    payload_only: bool,
    /// CASTLE_* variables for the launcher.
    env: Vec<(String, String)>,
}

pub struct Clones {
    dir: PathBuf,
    log_dir: PathBuf,
    loopback: bool,
    launches: Arc<Mutex<HashMap<String, Launch>>>,
    scan: Mutex<Option<Scan>>,
    containers: Mutex<Option<ContainerScan>>,
    /// Whether a docker command is on this box, looked up once.
    docker: std::sync::OnceLock<bool>,
    /// Held while a screen picture is taken: a VM's control port serves one caller at a time.
    screen: Mutex<()>,
    targets: Mutex<Option<Arc<Vec<Target>>>>,
    /// Held while one of the saved lists (automations, machines, benchmarks) is rewritten.
    files: Arc<Mutex<()>>,
    serial: AtomicU64,
    /// What script runs found (results.rs), opened on first use.
    results: OnceLock<Option<Results>>,
    /// The port the box listens on: a script reports to it from inside its machine.
    port: AtomicU16,
    /// Units whose script finished, to be stopped by the scheduler's next tick.
    to_stop: Mutex<Vec<(String, Instant)>>,
}

fn hidden(mut cmd: Command) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.stdin(Stdio::null());
    cmd
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// A value that is safe to put on one line of a file.
fn one_line(value: &str, max: usize) -> bool {
    value.len() <= max && !value.chars().any(|c| c.is_control())
}

/// A lowercase name made of letters, digits and dashes, safe as a file name and an id.
fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= SLUG_LEN {
            break;
        }
    }
    out.trim_end_matches('-').to_string()
}

fn env_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase() || c == '_')
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && name.len() <= 64
}

fn is_git_address(source: &str) -> bool {
    source.contains("://") || source.starts_with("git@")
}

/// True for an https address on one of the trusted git hosts.
fn trusted_source(source: &str) -> bool {
    source
        .strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .is_some_and(|host| TRUSTED_HOSTS.contains(&host.to_ascii_lowercase().as_str()))
}

/// Reads `NAME=value` from `.env` text the way the project does: last `=` side trimmed,
/// one pair of matching quotes removed.
fn env_value(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (key, value) = line.trim().split_once('=')?;
        if key.trim() != name {
            return None;
        }
        let value = value.trim();
        let quoted = value.len() >= 2
            && (value.starts_with('"') && value.ends_with('"')
                || value.starts_with('\'') && value.ends_with('\''));
        Some(if quoted { value[1..value.len() - 1].to_string() } else { value.to_string() })
    })
}

/// Returns `.env` text with `name` set to `value`, keeping every other line as it was.
fn with_env_value(text: &str, name: &str, value: &str) -> String {
    let line = format!("{name}=\"{value}\"");
    let mut replaced = false;
    let mut out: Vec<String> = text
        .lines()
        .map(|existing| {
            let is_key = existing.trim().split_once('=').is_some_and(|(k, _)| k.trim() == name);
            if is_key && !replaced {
                replaced = true;
                line.clone()
            } else {
                existing.to_string()
            }
        })
        .collect();
    if !replaced {
        out.push(line);
    }
    out.join("\n") + "\n"
}

/// The QMP control port from a QEMU command line, if it has one on loopback.
fn qmp_port(command_line: &str) -> Option<u16> {
    let rest = &command_line[command_line.find(QMP_FLAG)? + QMP_FLAG.len()..];
    rest.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
}

fn strip_ansi(line: &str) -> String {
    let mut out = String::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        // Skip "ESC [ ... letter": the colour codes PowerShell writes.
        for skipped in chars.by_ref() {
            if skipped.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}

fn tail(path: &Path) -> Vec<String> {
    std::fs::read(path)
        .map(|raw| {
            let text = String::from_utf8_lossy(&raw);
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
            let from = lines.len().saturating_sub(LOG_TAIL_LINES);
            lines[from..].iter().map(|l| strip_ansi(l)).collect()
        })
        .unwrap_or_default()
}

fn load<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn save<T: Serialize + ?Sized>(path: &Path, value: &T) -> ClonesResult<()> {
    let fail = |_| "could not save that on the box".to_string();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(fail)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value).map_err(|_| "could not save that on the box".to_string())?).map_err(fail)?;
    std::fs::rename(&tmp, path).map_err(fail)
}

/// A PowerShell single-quoted string. Curly quotes end one too, so they are doubled as well.
fn ps(text: &str) -> String {
    let mut out = String::from("'");
    for c in text.chars() {
        out.push(c);
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}') {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

fn ps_list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().filter(|s| !s.trim().is_empty()).map(|s| ps(s.trim())).collect();
    format!("@({})", quoted.join(", "))
}

impl Profile {
    /// The profile as a PowerShell data file. It is data only: Clonewars loads it without
    /// running anything in it.
    fn render(&self) -> String {
        let mut lines = vec![
            "@{".to_string(),
            format!("    Description  = {}", ps(&self.description)),
            format!("    Target       = {}", ps(&self.target)),
            "    Background   = $true".to_string(),
        ];
        if let Some(instance) = &self.instance {
            lines.push(format!("    Instance     = {}", ps(instance)));
        }
        if self.memory_gb > 0 {
            lines.push(format!("    MemoryGB     = {}", self.memory_gb));
        }
        if self.cores > 0 {
            lines.push(format!("    Cores        = {}", self.cores));
        }
        if !self.screen.is_empty() {
            lines.push(format!("    Screen       = {}", ps(&self.screen)));
        }
        if !self.payload.is_empty() {
            lines.push("    Payload      = @(".to_string());
            for (source, zip, include) in &self.payload {
                lines.push(format!("        @{{ Source = {}; As = {}; Include = {} }}", ps(&source.to_string_lossy()), ps(zip), ps_list(include)));
            }
            lines.push("    )".to_string());
        }
        lines.push(format!("    Packages     = @{{ apt = {}; pacman = {} }}", ps_list(&self.apt), ps_list(&self.pacman)));
        lines.push(format!("    RootCommands = {}", ps_list(&self.root)));
        lines.push(format!("    UserCommands = {}", ps_list(&self.user)));
        if !self.setup.is_empty() {
            lines.push(format!("    Setup        = {}", ps(&self.setup)));
        }
        if !self.env.is_empty() {
            let pairs: Vec<String> = self.env.iter().map(|(k, v)| format!("{k} = {}", ps(v))).collect();
            lines.push(format!("    Env          = @{{ {} }}", pairs.join("; ")));
        }
        lines.push("}".to_string());
        lines.join("\r\n") + "\r\n"
    }
}

/// The example every box starts with, and the model for automations made in the app.
fn penguin() -> AutomationDef {
    AutomationDef {
        id: "penguin".into(),
        name: "Penguin AutoSurf".into(),
        description: "Signs in to Club Penguin Journey and plays Cart Surfer on a loop for coins.".into(),
        target: "debian-x11".into(),
        source: "../../../penguin-autosurf".into(),
        setup: "penguin".into(),
        memory_gb: 4,
        cores: 8,
        screen: "1440x900".into(),
        fields: vec![
            Field { env: "PENGUIN_USERNAME".into(), label: "Username".into(), secret: false },
            Field { env: "PENGUIN_PASSWORD".into(), label: "Password".into(), secret: true },
        ],
        builtin: true,
        bots: vec![Bot { id: MAIN_BOT.into(), name: "Penguin 1".into(), values: HashMap::new() }],
        ..Default::default()
    }
}

/// The second example: a Castle script that checks Walmart's motor oil prices and says, day by
/// day, whether today is a good day to buy. It runs in a container by default; a VM works too.
fn walmart() -> AutomationDef {
    AutomationDef {
        id: WALMART_ID.into(),
        name: "Walmart Motor Oil Prices".into(),
        description: "Searches Walmart for 0W-20 motor oil, works out what each listing costs a quart, and says whether today is a good day to buy.".into(),
        target: "container-debian".into(),
        memory_gb: 4,
        cores: 2,
        screen: "1440x900".into(),
        stop_after_minutes: 30,
        builtin: true,
        script: WALMART_SCRIPT.replace("\r\n", "\n"),
        bots: vec![Bot { id: MAIN_BOT.into(), name: "Price checker".into(), values: HashMap::new() }],
        ..Default::default()
    }
}

/// The examples every box has. They can be changed, not removed.
fn builtins() -> Vec<AutomationDef> {
    vec![penguin(), walmart()]
}

/// "HH:MM" on a 24-hour clock, as (hour, minute).
fn schedule_time(text: &str) -> Option<(u32, u32)> {
    let (h, m) = text.trim().split_once(':')?;
    let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
    (h < 24 && m < 60 && text.trim().len() == 5).then_some((h, m))
}

/// The first problem of a script that did not compile, with its line.
fn first_problem(problems: &[script::Diagnostic]) -> String {
    match problems.first() {
        Some(d) if problems.len() > 1 => format!("line {}: {} (and {} more problems)", d.line, d.message, problems.len() - 1),
        Some(d) => format!("line {}: {}", d.line, d.message),
        None => "the script did not compile".into(),
    }
}

/// Standard base64 (with or without padding) to bytes. None when it is not base64.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a') as u32 + 26),
            b'0'..=b'9' => Some((c - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let clean: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    let body = clean.strip_suffix(b"==").or_else(|| clean.strip_suffix(b"=")).unwrap_or(&clean);
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    for chunk in body.chunks(4) {
        if chunk.len() == 1 {
            return None;
        }
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= value(*c)? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Some(out)
}

fn sha256_hex(text: &str) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("the operating system provides randomness");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Copies the castle library into a project. Shell and Python files are read by Linux, so
/// they lose any Windows line endings a checkout gave them.
fn copy_library(from: &Path, to: &Path) -> ClonesResult<()> {
    let unreadable = |_| format!("could not read the castle library in {}", from.display());
    std::fs::create_dir_all(to).map_err(unreadable)?;
    for entry in std::fs::read_dir(from).map_err(unreadable)?.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if name != "__pycache__" {
                copy_library(&path, &to.join(&name))?;
            }
            continue;
        }
        let text_file = path.extension().is_some_and(|e| e == "sh" || e == "py");
        let bytes = std::fs::read(&path).map_err(unreadable)?;
        let bytes = if text_file { String::from_utf8_lossy(&bytes).replace("\r\n", "\n").into_bytes() } else { bytes };
        std::fs::write(to.join(&name), bytes).map_err(|_| "could not write the project for the machine".to_string())?;
    }
    Ok(())
}

/// The one command a made-in-the-app automation runs in the guest's desktop session:
/// unpack the project, give it this bot's sign-in, then run the tasks in order.
fn session_command(a: &AutomationDef) -> Option<String> {
    let tasks: Vec<String> = a.tasks.iter().filter(|t| !t.root && !t.command.trim().is_empty()).map(|t| format!("( {} )", t.command.trim())).collect();
    if tasks.is_empty() {
        return None;
    }
    let body = tasks.join("; ");
    let run = if a.restart { format!("while true; do {body}; sleep 5; done") } else { body };
    Some(format!(
        "cd ~/castle-copy && mkdir -p project && {{ [ ! -f {PROJECT_ZIP} ] || bsdtar -xf {PROJECT_ZIP} -C project; }} && {{ [ ! -f {BOT_ENV_FILE} ] || cp {BOT_ENV_FILE} project/.env; }} && cd project && set -a && {{ [ ! -f .env ] || . ./.env; }} && set +a && {{ {run}; }} >> ~/castle-automation.log 2>&1"
    ))
}

/// A shell single-quoted string.
fn sh(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// The script an Android automation runs on the phone (Android's own `sh`): the fields as
/// variables, then the tasks in order, again and again when the automation restarts. Root
/// tasks go through `su 0`, which google_apis images have and Play Store images do not.
fn android_tasks_script(a: &AutomationDef, bot: &str, values: &[(String, String)], shutdown_after_minutes: u32) -> String {
    let mut lines = vec![
        "#!/system/bin/sh".to_string(),
        format!("# Castle automation: {} / {} (written by the Castle app on every start)", a.name.replace('\n', " "), bot.replace('\n', " ")),
    ];
    for (name, value) in values {
        lines.push(format!("export {name}={}", sh(value)));
    }
    // Shared storage is mounted a little after Android says it has booted.
    lines.push("for i in $(seq 1 60); do [ -d /sdcard/Download ] && break; sleep 1; done".into());
    // Fresh from a boot, the launcher is still coming up and would cover an app started now.
    lines.push("[ \"$(cut -d. -f1 /proc/uptime)\" -lt 180 ] && sleep 15".into());
    lines.push("mkdir -p /sdcard/Download/castle-copy 2>/dev/null".into());
    lines.push("cd /sdcard/Download/castle-copy 2>/dev/null || cd /data/local/tmp".into());
    if shutdown_after_minutes > 0 {
        lines.push(format!("( sleep {}; echo \"[castle] the stop time has come\"; su 0 reboot -p 2>/dev/null || reboot -p ) &", shutdown_after_minutes * 60));
    }
    let tasks: Vec<String> = a
        .tasks
        .iter()
        .filter(|t| !t.command.trim().is_empty())
        .map(|t| {
            let name = if t.name.trim().is_empty() { t.command.trim() } else { t.name.trim() };
            let run = if t.root {
                format!("su 0 sh -c {} || echo \"[castle] root task failed (it needs a google_apis image)\"", sh(t.command.trim()))
            } else {
                format!("( {} )", t.command.trim())
            };
            format!("  echo \"[castle] $(date +%T) {}\"; {run}", name.replace(['"', '\\', '$', '`'], ""))
        })
        .collect();
    if tasks.is_empty() {
        lines.push("echo \"[castle] no tasks: the apps are installed and opened\"".into());
    } else if a.restart {
        lines.push("while true; do".into());
        lines.extend(tasks);
        lines.push("  sleep 5".into());
        lines.push("done".into());
    } else {
        lines.extend(tasks);
        lines.push("echo \"[castle] tasks done\"".into());
    }
    lines.join("\n") + "\n"
}

fn check_commands(commands: &[String]) -> ClonesResult<()> {
    if commands.len() > MAX_LIST || commands.iter().any(|c| !one_line(c, MAX_COMMAND_LEN)) {
        return Err("a command is too long, or there are too many of them".into());
    }
    Ok(())
}

fn check_packages(packages: &[String]) -> ClonesResult<()> {
    let fine = |p: &String| !p.is_empty() && p.len() <= 80 && p.chars().all(|c| c.is_ascii_alphanumeric() || "+-._:".contains(c)) && !p.starts_with('-');
    if packages.len() > MAX_LIST || !packages.iter().all(fine) {
        return Err("a package name has characters a package name cannot have".into());
    }
    Ok(())
}

/// Copies a file, or a folder and everything in it, into `dir` under its own name.
fn copy_into(from: &Path, dir: &Path) -> ClonesResult<()> {
    let name = from.file_name().ok_or_else(|| format!("not a file or folder: {}", from.display()))?;
    let to = dir.join(name);
    let unreadable = |_| format!("could not read {}", from.display());
    if from.is_dir() {
        std::fs::create_dir_all(&to).map_err(unreadable)?;
        for entry in std::fs::read_dir(from).map_err(unreadable)?.flatten() {
            copy_into(&entry.path(), &to)?;
        }
        Ok(())
    } else if from.is_file() {
        std::fs::copy(from, &to).map(|_| ()).map_err(unreadable)
    } else {
        Err(format!("not on the box: {}", from.display()))
    }
}

fn trimmed(list: &[String]) -> Vec<String> {
    list.iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// Follows one launch to its end and writes down how long it took.
fn watch(launches: Arc<Mutex<HashMap<String, Launch>>>, files: Arc<Mutex<()>>, key: String, serial: u64, log: PathBuf, bench_file: PathBuf, mut record: Value) {
    std::thread::spawn(move || {
        let started = Instant::now();
        let seconds = |from: Instant| (from.elapsed().as_secs_f64() * 10.0).round() / 10.0;
        let mut prepared: Option<f64> = None;
        let code = loop {
            std::thread::sleep(WATCH_PAUSE);
            if prepared.is_none() && std::fs::read(&log).is_ok_and(|raw| { let text = String::from_utf8_lossy(&raw); text.contains(LOG_VM_HANDOFF) || text.contains(LOG_CONTAINER_HANDOFF) }) {
                prepared = Some(seconds(started));
            }
            let mut map = launches.lock().unwrap();
            // Stopped, or started again since: that launch has a watcher of its own.
            let Some(launch) = map.get_mut(&key).filter(|l| l.serial == serial) else { return };
            if launch.exit.is_none() {
                if let Ok(Some(status)) = launch.child.try_wait() {
                    launch.exit = Some(status.code().unwrap_or(-1));
                }
            }
            if let Some(code) = launch.exit {
                break code;
            }
        };
        record["prepSeconds"] = json!(prepared);
        record["readySeconds"] = json!(seconds(started));
        record["ok"] = json!(code == 0);
        let _one_writer = files.lock().unwrap();
        let mut all: Vec<Value> = load(&bench_file).unwrap_or_default();
        all.push(record);
        let from = all.len().saturating_sub(BENCH_KEEP);
        let _ = save(&bench_file, &all[from..]);
    });
}

impl Clones {
    pub fn new(dir: PathBuf, log_dir: PathBuf, loopback: bool) -> Self {
        Self {
            dir,
            log_dir,
            loopback,
            launches: Arc::new(Mutex::new(HashMap::new())),
            scan: Mutex::new(None),
            containers: Mutex::new(None),
            docker: std::sync::OnceLock::new(),
            screen: Mutex::new(()),
            targets: Mutex::new(None),
            files: Arc::new(Mutex::new(())),
            serial: AtomicU64::new(1),
            results: OnceLock::new(),
            port: AtomicU16::new(8787),
            to_stop: Mutex::new(Vec::new()),
        }
    }

    fn require_loopback(&self) -> ClonesResult<()> {
        if self.loopback {
            Ok(())
        } else {
            Err("machines and automations can only be controlled when the box listens on this machine alone".into())
        }
    }

    fn data(&self) -> PathBuf {
        match std::env::var_os("CASTLE_CLONEWARS_DATA") {
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => self.dir.parent().unwrap_or(&self.dir).join("data").join("clonewars"),
        }
    }

    fn file_key(key: &str) -> String {
        key.replace(':', "-")
    }

    fn log_file(&self, key: &str) -> PathBuf {
        self.log_dir.join(format!("{}.log", Self::file_key(key)))
    }

    fn automations_file(&self) -> PathBuf {
        self.log_dir.join("automations.json")
    }

    fn machines_file(&self) -> PathBuf {
        self.log_dir.join("machines.json")
    }

    fn bench_file(&self) -> PathBuf {
        self.log_dir.join("benchmarks.json")
    }

    // ---- the catalogue ----

    /// Every target Clonewars knows, read from its own list once and kept.
    fn targets(&self) -> Arc<Vec<Target>> {
        let mut slot = self.targets.lock().unwrap();
        if let Some(known) = slot.as_ref() {
            return known.clone();
        }
        let mut cmd = hidden(Command::new("pwsh"));
        cmd.current_dir(&self.dir).args(["-NoProfile", "-NonInteractive", "-Command", TARGETS_SCRIPT]);
        let list: Vec<Target> = cmd
            .output()
            .ok()
            .and_then(|out| {
                let text = String::from_utf8_lossy(&out.stdout).into_owned();
                serde_json::from_str(&text[text.find('[')?..]).ok()
            })
            .unwrap_or_default();
        let list = Arc::new(list);
        if !list.is_empty() {
            *slot = Some(list.clone());
        }
        list
    }

    fn target(&self, id: &str) -> ClonesResult<Target> {
        self.targets().iter().find(|t| t.id == id).cloned().ok_or_else(|| "unknown operating system".to_string())
    }

    /// A target's main disk. A `*` in its name stands for a version: the file that is there
    /// wins, unless it is a download or belongs to a target with a more exact name
    /// (`debian-*` must not claim `debian-x11.qcow2`).
    fn main_disk(&self, t: &Target) -> PathBuf {
        let data = self.data();
        let Some((before, after)) = t.disk.split_once('*') else { return data.join(&t.disk) };
        let fits = |name: &str, before: &str, after: &str| name.len() >= before.len() + after.len() && name.starts_with(before) && name.ends_with(after);
        let all = self.targets();
        let someone_elses = |name: &str| {
            all.iter().any(|other| {
                other.iso == name
                    || (other.id != t.id
                        && match other.disk.split_once('*') {
                            Some((b, a)) => b.len() > before.len() && fits(name, b, a),
                            None => other.disk == name,
                        })
            })
        };
        std::fs::read_dir(&data)
            .ok()
            .and_then(|entries| entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).find(|name| fits(name, before, after) && !someone_elses(name)))
            .map(|name| data.join(name))
            .unwrap_or_else(|| data.join(format!("{}.qcow2", t.id)))
    }

    fn is_built(disk: &Path) -> bool {
        std::fs::metadata(disk).is_ok_and(|m| m.len() > EMPTY_DISK_BYTES)
    }

    fn instance_disk(&self, instance: &str) -> PathBuf {
        self.data().join("instances").join(format!("{instance}.qcow2"))
    }

    /// The disk a unit runs from, or None for a target that is not a QEMU machine.
    fn disk(&self, unit: &Unit) -> Option<PathBuf> {
        // A container is its own disk, and an emulator keeps its phone in the AVD folder.
        if unit.engine != Engine::Vm {
            return None;
        }
        if let Some(instance) = &unit.instance {
            return Some(self.instance_disk(instance));
        }
        match self.targets().iter().find(|t| t.id == unit.target) {
            Some(t) if !t.runtime.is_empty() => None,
            Some(t) => Some(self.main_disk(t)),
            None => Some(self.data().join(format!("{}.qcow2", unit.target))),
        }
    }

    // ---- what is running ----

    /// Command lines of running QEMU processes, refreshed at most every few seconds.
    fn qemu_command_lines(&self) -> Vec<String> {
        let mut scan = self.scan.lock().unwrap();
        if let Some(s) = scan.as_ref() {
            if s.at.elapsed() < SCAN_CACHE {
                return s.qemu.clone();
            }
        }
        let mut cmd = hidden(Command::new("powershell"));
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-CimInstance Win32_Process -Filter \"Name like 'qemu-system%'\" | ForEach-Object { $_.CommandLine }",
        ]);
        let qemu: Vec<String> = cmd
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(str::to_string).collect())
            .unwrap_or_default();
        *scan = Some(Scan { at: Instant::now(), qemu: qemu.clone() });
        qemu
    }

    fn forget_scan(&self) {
        *self.scan.lock().unwrap() = None;
        *self.containers.lock().unwrap() = None;
    }

    /// The QEMU command line running this disk out of this clonewars folder, if there is
    /// one. Matching on the disk path means a VM from some other checkout, or on another
    /// disk, is never mistaken for it.
    fn vm_command_line(&self, disk: &Path) -> Option<String> {
        let disk = disk.to_string_lossy().trim_start_matches(r"\\?\").to_lowercase();
        self.qemu_command_lines().into_iter().find(|line| line.to_lowercase().contains(&disk))
    }

    fn runtime_of(&self, target: &str) -> Option<String> {
        self.targets().iter().find(|t| t.id == target).map(|t| t.runtime.clone())
    }

    /// A unit, with the engine its target runs in.
    fn unit(&self, key: String, target: &str, instance: Option<String>) -> Unit {
        let name = instance.clone().unwrap_or_else(|| target.to_string());
        let engine = match self.runtime_of(target).as_deref() {
            Some("container") => Engine::Container(format!("castle-{name}")),
            // The target's own emulator keeps the AVD android.ps1 has always called "castle".
            Some("avd") => Engine::Emulator(instance.as_ref().map(|i| format!("castle-{i}")).unwrap_or_else(|| "castle".into())),
            _ => Engine::Vm,
        };
        Unit { key, target: target.to_string(), instance, engine }
    }

    fn has_docker(&self) -> bool {
        *self.docker.get_or_init(|| {
            let mut cmd = hidden(Command::new("docker"));
            cmd.args(["--version"]);
            cmd.output().is_ok_and(|o| o.status.success())
        })
    }

    /// Every Castle container and whether it runs, refreshed at most every few seconds.
    fn container_list(&self) -> Vec<(String, bool)> {
        if !self.has_docker() {
            return Vec::new();
        }
        let mut scan = self.containers.lock().unwrap();
        if let Some(s) = scan.as_ref() {
            if s.at.elapsed() < SCAN_CACHE {
                return s.all.clone();
            }
        }
        let mut cmd = hidden(Command::new("docker"));
        cmd.args(["ps", "-a", "--filter", &format!("label={CONTAINER_LABEL}"), "--format", "{{.Names}}\t{{.State}}"]);
        let all: Vec<(String, bool)> = cmd
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter_map(|line| line.split_once('\t').map(|(name, state)| (name.trim().to_string(), state.trim() == "running")))
                    .collect()
            })
            .unwrap_or_default();
        *scan = Some(ContainerScan { at: Instant::now(), all: all.clone() });
        all
    }

    fn container_exists(&self, name: &str) -> bool {
        self.container_list().iter().any(|(n, _)| n == name)
    }

    fn docker(&self, args: &[&str]) -> ClonesResult<std::process::Output> {
        let mut cmd = hidden(Command::new("docker"));
        cmd.args(args);
        cmd.output().map_err(|_| "Docker is not installed on this box".to_string())
    }

    fn android_state_dir(&self) -> PathBuf {
        self.data().join("android-sdk").join("castle")
    }

    /// True while the emulator android.ps1 started for this AVD is alive (its PID file).
    fn emulator_running(&self, avd: &str) -> bool {
        let Some(pid) = std::fs::read_to_string(self.android_state_dir().join(format!("{avd}.pid"))).ok().and_then(|t| t.trim().parse::<u32>().ok()) else {
            return false;
        };
        let pid = sysinfo::Pid::from_u32(pid);
        let Ok(mut sys) = sysinfo::System::new() else { return false };
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
        sys.process(pid).is_some_and(|p| {
            let name = p.name().to_string_lossy().to_lowercase();
            name.contains("emulator") || name.contains("qemu")
        })
    }

    /// Whether the machine behind a unit is up, whatever engine it runs in.
    fn is_up(&self, unit: &Unit) -> bool {
        match &unit.engine {
            Engine::Container(name) => self.container_list().iter().any(|(n, running)| n == name && *running),
            Engine::Emulator(avd) => self.emulator_running(avd),
            Engine::Vm => self.disk(unit).is_some_and(|disk| self.vm_command_line(&disk).is_some()),
        }
    }

    fn launcher(&self, script: &str) -> Command {
        let mut cmd = hidden(Command::new("pwsh"));
        cmd.current_dir(&self.dir).args(["-NoProfile", "-NonInteractive", "-File", script]);
        cmd
    }

    /// stopped | launching | running | failed
    fn state_of(&self, unit: &Unit) -> &'static str {
        let launch_state = {
            let mut launches = self.launches.lock().unwrap();
            launches.get_mut(&unit.key).map(|launch| {
                if launch.exit.is_none() {
                    if let Ok(Some(status)) = launch.child.try_wait() {
                        launch.exit = Some(status.code().unwrap_or(-1));
                    }
                }
                launch.exit
            })
        };
        let running = self.is_up(unit);
        match launch_state {
            Some(None) => "launching",
            _ if running => "running",
            Some(Some(code)) if code != 0 => "failed",
            _ => "stopped",
        }
    }

    // ---- starting and stopping ----

    fn launch(&self, plan: Plan) -> ClonesResult<()> {
        let unit = &plan.unit;
        let state = self.state_of(unit);
        if state == "launching" || state == "running" {
            return Err("it is already running".into());
        }
        let no_log = |_| "could not open the log".to_string();
        std::fs::create_dir_all(&self.log_dir).map_err(no_log)?;
        let log_file = self.log_file(&unit.key);
        let log = std::fs::File::create(&log_file).map_err(no_log)?;
        let log_err = log.try_clone().map_err(no_log)?;
        let first_boot = self.disk(unit).is_some_and(|disk| !disk.exists());

        let mut cmd = hidden(Command::new("pwsh"));
        cmd.current_dir(&self.dir).args(["-NoProfile", "-NonInteractive", "-File", "start.ps1"]);
        match &plan.profile {
            Some(profile) => {
                let path = self.log_dir.join("profiles").join(format!("{}.psd1", Self::file_key(&unit.key)));
                let fail = |_| "could not write the launch profile".to_string();
                std::fs::create_dir_all(path.parent().unwrap_or(&self.log_dir)).map_err(fail)?;
                std::fs::write(&path, profile.render()).map_err(fail)?;
                cmd.arg("-Profile").arg(&path);
            }
            None => {
                cmd.arg(&unit.target).arg("-Background");
                if let Some(instance) = &unit.instance {
                    cmd.arg("-Instance").arg(instance);
                }
                if plan.memory_gb > 0 {
                    cmd.env("CASTLE_VM_RAM_GB", plan.memory_gb.to_string());
                }
                if plan.cores > 0 {
                    cmd.env("CASTLE_VM_CORES", plan.cores.to_string());
                }
                if plan.payload_only {
                    cmd.arg("-Copy").env("CASTLE_AVD_PUSH", "1");
                }
            }
        }
        if let Some(base) = plan.base.as_ref().filter(|_| first_boot) {
            cmd.arg("-BaseDisk").arg(base);
        }
        if plan.snapshot {
            cmd.env("CASTLE_VM_SNAPSHOT", "1");
        }
        if let Some(dir) = &plan.copy_dir {
            cmd.env("CASTLE_COPY_DIR", dir);
        }
        if let Engine::Emulator(avd) = &unit.engine {
            cmd.env("CASTLE_AVD_NAME", avd);
        }
        for (name, value) in &plan.env {
            cmd.env(name, value);
        }
        let child = cmd.stdout(log).stderr(log_err).spawn().map_err(|_| "could not start PowerShell 7 (pwsh)".to_string())?;
        let serial = self.serial.fetch_add(1, Ordering::Relaxed);
        self.launches.lock().unwrap().insert(unit.key.clone(), Launch { serial, child, exit: None });
        self.forget_scan();
        let record = json!({ "at": now_ms(), "key": unit.key, "label": plan.label, "target": unit.target, "mode": plan.mode, "firstBoot": first_boot && plan.base.is_none() });
        watch(self.launches.clone(), self.files.clone(), unit.key.clone(), serial, log_file, self.bench_file(), record);
        Ok(())
    }

    /// Asks the VM to shut down the way its power button would. Only the VM running this
    /// unit's disk is ever addressed.
    fn halt(&self, unit: &Unit) -> ClonesResult<()> {
        if let Some(mut launch) = self.launches.lock().unwrap().remove(&unit.key) {
            if launch.exit.is_none() {
                let _ = launch.child.kill();
                let _ = launch.child.wait();
            }
        }
        match &unit.engine {
            Engine::Container(name) => {
                if self.container_list().iter().any(|(n, running)| n == name && *running) {
                    let out = self.docker(&["stop", "-t", "20", name])?;
                    if !out.status.success() {
                        return Err(format!("Docker could not stop it: {}", String::from_utf8_lossy(&out.stderr).trim()));
                    }
                }
            }
            Engine::Emulator(avd) => {
                // The emulator is not ours to talk to over QMP; its own script shuts it down.
                self.launcher("android.ps1").args(["-Name", avd, "-Stop"]).output().map_err(|_| "could not start PowerShell 7 (pwsh)".to_string())?;
            }
            Engine::Vm => {
                if let Some(line) = self.disk(unit).and_then(|disk| self.vm_command_line(&disk)) {
                    let port = qmp_port(&line).ok_or_else(|| "this VM has no control port".to_string())?;
                    power_off(port)?;
                }
            }
        }
        self.forget_scan();
        Ok(())
    }

    /// A fresh, empty payload folder for one launch.
    fn copy_dir(&self, key: &str) -> ClonesResult<PathBuf> {
        let dir = self.data().join("copy-app").join(Self::file_key(key));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|_| "could not prepare the files for the machine".to_string())?;
        Ok(dir)
    }

    fn unit_for(&self, key: &str) -> ClonesResult<Unit> {
        let unknown = || "unknown machine".to_string();
        let mut parts = key.splitn(3, ':');
        match (parts.next(), parts.next(), parts.next()) {
            (Some("a"), Some(id), Some(bot)) => {
                let a = self.automation(id)?;
                let bot = a.bots.iter().find(|b| b.id == bot).ok_or_else(unknown)?;
                Ok(self.bot_unit(&a, bot))
            }
            (Some("m"), Some(id), None) => self.machine_unit(id),
            _ => Err(unknown()),
        }
    }

    /// A picture of a running machine's screen, as PNG. It only looks: nothing is typed,
    /// clicked or changed in the machine, and its own window is left alone.
    pub fn screen(&self, key: &str) -> ClonesResult<Vec<u8>> {
        let unit = self.unit_for(key)?;
        let not_running = || "this machine is not running".to_string();
        let png = |out: std::process::Output| {
            if out.status.success() && out.stdout.starts_with(b"\x89PNG") {
                Ok(out.stdout)
            } else {
                Err("the machine did not produce a picture".to_string())
            }
        };
        match &unit.engine {
            // Inside the container: Xvfb's screen for Linux, `screencap` for the phone.
            Engine::Container(name) => {
                if !self.is_up(&unit) {
                    return Err(not_running());
                }
                return png(self.docker(&["exec", name, "castle-screen"])?);
            }
            Engine::Emulator(avd) => {
                if !self.is_up(&unit) {
                    return Err(not_running());
                }
                let port = std::fs::read_to_string(self.android_state_dir().join(format!("{avd}.port"))).map_err(|_| not_running())?;
                let adb = self.data().join("android-sdk").join("platform-tools").join(if cfg!(windows) { "adb.exe" } else { "adb" });
                let mut cmd = hidden(Command::new(adb));
                cmd.args(["-s", &format!("emulator-{}", port.trim()), "exec-out", "screencap", "-p"]);
                return png(cmd.output().map_err(|_| "adb is not installed in the Android SDK folder".to_string())?);
            }
            Engine::Vm => {}
        }
        let line = self.disk(&unit).and_then(|disk| self.vm_command_line(&disk)).ok_or_else(|| "this machine is not running".to_string())?;
        let port = qmp_port(&line).ok_or_else(|| "this VM has no control port".to_string())?;
        let _one_at_a_time = self.screen.lock();
        std::fs::create_dir_all(&self.log_dir).map_err(|_| "could not prepare a place for the picture".to_string())?;
        let file = self.log_dir.join(format!("{}-screen.png", Self::file_key(key)));
        let taken = screen_dump(port, &file);
        let bytes = std::fs::read(&file);
        let _ = std::fs::remove_file(&file);
        taken?;
        bytes.map_err(|_| "the machine did not produce a picture".to_string())
    }

    /// Live control of a container's screen: `on` starts a VNC server and a noVNC viewer
    /// inside it behind a new one-time password and returns the viewer's address (on this
    /// machine's loopback only); `!on` stops them. The picture view needs none of this.
    pub fn live(&self, key: &str, on: bool) -> ClonesResult<Value> {
        self.require_loopback()?;
        let unit = self.unit_for(key)?;
        let Engine::Container(name) = &unit.engine else {
            return Err("taking control works for machines in containers. A VM has its own window".into());
        };
        if self.runtime_kind(&unit.target) != "linux" {
            return Err("taking control of an Android container is not there yet; its screen shows here".into());
        }
        if !self.is_up(&unit) {
            return Err("this machine is not running".into());
        }
        if !on {
            self.docker(&["exec", name, "castle-live", "stop"])?;
            return Ok(json!({ "live": false }));
        }
        let out = self.docker(&["exec", name, "castle-live", "start"])?;
        let password = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !out.status.success() || password.len() < 8 || !password.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err("the live viewer did not start in the container".into());
        }
        // "127.0.0.1:49153", one line per address family.
        let port = self.docker(&["port", name, "6080/tcp"])?;
        let port = String::from_utf8_lossy(&port.stdout)
            .lines()
            .find_map(|line| line.trim().strip_prefix("127.0.0.1:").and_then(|p| p.parse::<u16>().ok()))
            .ok_or_else(|| "this container was made before live control existed. Delete it and start it again".to_string())?;
        Ok(json!({
            "live": true,
            "url": format!("http://127.0.0.1:{port}/vnc.html?autoconnect=1&resize=scale&reconnect=1&show_dot=1&password={password}"),
        }))
    }

    /// "linux" or "android": what a target's guest is.
    fn runtime_kind(&self, target: &str) -> &'static str {
        if self.targets().iter().any(|t| t.id == target && t.family == "android") { "android" } else { "linux" }
    }

    /// Deletes everything a unit keeps of its own: its disk, container or AVD.
    fn remove_unit(&self, unit: &Unit) {
        match &unit.engine {
            Engine::Container(name) => {
                let _ = self.docker(&["rm", "-f", name]);
                let _ = std::fs::remove_dir_all(self.data().join("containers").join(name));
            }
            Engine::Emulator(avd) if avd != "castle" => {
                let _ = self.launcher("android.ps1").args(["-Name", avd, "-Purge"]).output();
            }
            _ => {}
        }
        if let Some(instance) = &unit.instance {
            self.remove_instance(instance);
        }
        self.forget_scan();
    }

    // ---- automations ----

    /// The saved automations, with every built-in example the list does not have yet.
    fn all_automations(&self) -> Vec<AutomationDef> {
        let mut all: Vec<AutomationDef> = load(&self.automations_file()).unwrap_or_default();
        for (at, example) in builtins().into_iter().enumerate() {
            if !all.iter().any(|a| a.id == example.id) {
                all.insert(at.min(all.len()), example);
            }
        }
        all
    }

    fn automation(&self, id: &str) -> ClonesResult<AutomationDef> {
        self.all_automations().into_iter().find(|a| a.id == id).ok_or_else(|| "unknown automation".to_string())
    }

    /// Rewrites one automation in the saved list (or adds it), under the file lock.
    fn change_automations(&self, change: impl FnOnce(&mut Vec<AutomationDef>) -> ClonesResult<()>) -> ClonesResult<()> {
        let _one_writer = self.files.lock().unwrap();
        let mut all = self.all_automations();
        change(&mut all)?;
        save(&self.automations_file(), &all)
    }

    fn bot_unit(&self, a: &AutomationDef, bot: &Bot) -> Unit {
        // The first bot keeps the target's main disk, so a box that already built it starts
        // at once. Every other bot, and every throwaway run, has a disk of its own. In a
        // container or an emulator every bot is its own machine: nothing is shared but the image.
        let own = bot.id != MAIN_BOT || a.temporary || self.runtime_of(&a.target).is_some_and(|r| r == "container" || r == "avd");
        let instance = own.then(|| format!("{}-{}", a.id, bot.id));
        self.unit(format!("a:{}:{}", a.id, bot.id), &a.target, instance)
    }

    /// Where the automation's project is on the box.
    fn source_dir(&self, a: &AutomationDef) -> Option<PathBuf> {
        let source = a.source.trim();
        if source.is_empty() {
            return None;
        }
        if is_git_address(source) {
            let name = slug(source.trim_end_matches('/').trim_end_matches(".git").rsplit('/').next().unwrap_or("project"));
            return Some(self.data().join("projects").join(if name.is_empty() { "project".to_string() } else { name }));
        }
        let path = PathBuf::from(source);
        Some(if path.is_absolute() { path } else { self.dir.join(path) })
    }

    /// Fetches the project if it is not on the box yet. An existing folder is left exactly
    /// as it is, so local work is never overwritten.
    fn ensure_source(&self, a: &AutomationDef) -> ClonesResult<()> {
        let Some(dir) = self.source_dir(a) else { return Ok(()) };
        if dir.is_dir() {
            return Ok(());
        }
        let url = if is_git_address(a.source.trim()) {
            a.source.trim().to_string()
        } else {
            let name = format!("CASTLE_{}_REPO", a.id.to_uppercase().replace('-', "_"));
            std::env::var(&name).ok().filter(|u| trusted_source(u.trim())).ok_or_else(|| format!("the project folder is not on the box: {}", dir.display()))?
        };
        if !trusted_source(url.trim()) {
            return Err(format!("projects are only fetched over https from {}", TRUSTED_HOSTS.join(", ")));
        }
        let mut cmd = hidden(Command::new("git"));
        cmd.args(["clone", "--depth", "1", url.trim()]).arg(&dir);
        match cmd.output() {
            Ok(out) if out.status.success() => Ok(()),
            Ok(out) => Err(format!("could not fetch the project: {}", String::from_utf8_lossy(&out.stderr).trim())),
            Err(_) => Err("could not fetch the project: git is not installed".into()),
        }
    }

    fn source_env(&self, a: &AutomationDef) -> String {
        self.source_dir(a).and_then(|dir| std::fs::read_to_string(dir.join(".env")).ok()).unwrap_or_default()
    }

    /// Where each of a bot's fields gets its value: "saved" with the bot, "env" from the
    /// project's own `.env`, or "missing".
    fn field_sources(a: &AutomationDef, bot: &Bot, env: &str) -> Vec<(&'static str, String)> {
        a.fields
            .iter()
            .map(|f| match bot.values.get(&f.env).filter(|v| !v.is_empty()) {
                Some(saved) => ("saved", saved.clone()),
                None if env_value(env, &f.env).is_some_and(|v| !v.is_empty()) => ("env", String::new()),
                None => ("missing", String::new()),
            })
            .collect()
    }

    fn automation_json(&self, a: &AutomationDef) -> Value {
        let env = self.source_env(a);
        let mut any_live = "stopped";
        let bots: Vec<Value> = a
            .bots
            .iter()
            .map(|bot| {
                let unit = self.bot_unit(a, bot);
                let state = self.state_of(&unit);
                self.settle_run(a, bot, state);
                if state == "running" || (state == "launching" && any_live != "running") || (state == "failed" && any_live == "stopped") {
                    any_live = state;
                }
                let sources = Self::field_sources(a, bot, &env);
                let values: Vec<Value> = a
                    .fields
                    .iter()
                    .zip(&sources)
                    // A secret is never sent back, and neither is anything read from the project's .env.
                    .map(|(f, (from, value))| json!({ "env": f.env, "from": from, "value": if f.secret { "" } else { value.as_str() } }))
                    .collect();
                json!({
                    "id": bot.id, "name": bot.name, "key": unit.key, "state": state,
                    "ready": sources.iter().all(|(from, _)| *from != "missing"),
                    "ownDisk": unit.instance.is_some(), "values": values, "log": tail(&self.log_file(&unit.key)),
                    "engine": unit.engine.label(), "controllable": matches!(unit.engine, Engine::Container(_)) && self.runtime_kind(&a.target) == "linux",
                })
            })
            .collect();
        let dir = self.source_dir(a);
        json!({
            "id": a.id, "name": a.name, "description": a.description, "target": a.target, "source": a.source,
            "sourceTrusted": !is_git_address(a.source.trim()) || trusted_source(a.source.trim()),
            "repoPresent": dir.as_ref().is_none_or(|d| d.is_dir()),
            "memoryGb": a.memory_gb, "cores": a.cores, "screen": a.screen, "apt": a.apt, "pacman": a.pacman,
            "tasks": a.tasks, "fields": a.fields, "restart": a.restart, "stopAfterMinutes": a.stop_after_minutes,
            "temporary": a.temporary, "builtin": a.builtin, "state": any_live, "bots": bots,
            "kind": if a.script.is_empty() { "project" } else { "script" }, "script": a.script, "schedule": a.schedule,
            "latest": if a.script.is_empty() { Value::Null } else { self.results().ok().and_then(|r| r.latest(&a.id)).unwrap_or(Value::Null) },
            "family": self.runtime_kind(&a.target), "engine": self.unit(String::new(), &a.target, None).engine.label(),
        })
    }

    pub fn list(&self) -> Vec<Value> {
        self.all_automations().iter().map(|a| self.automation_json(a)).collect()
    }

    fn automation_status(&self, id: &str) -> ClonesResult<Value> {
        Ok(self.automation_json(&self.automation(id)?))
    }

    /// Creates an automation, or changes one. Its bots and their sign-ins are kept.
    pub fn save_automation(&self, mut def: AutomationDef) -> ClonesResult<Value> {
        self.require_loopback()?;
        def.name = def.name.trim().to_string();
        def.source = def.source.trim().to_string();
        def.script = def.script.replace("\r\n", "\n");
        def.schedule = def.schedule.trim().to_string();
        if !def.schedule.is_empty() && schedule_time(&def.schedule).is_none() {
            return Err("a daily time is written like 08:00 or 18:30".into());
        }
        if def.script.len() > 64 * 1024 {
            return Err("a script can be 64 KB at most".into());
        }
        if def.script.trim().is_empty() {
            def.script = String::new();
        } else {
            // The script is the automation: it brings its own project and its one task.
            script::compile(&def.script).map_err(|e| first_problem(&e))?;
            def.source = String::new();
            def.tasks.clear();
        }
        def.apt = trimmed(&def.apt);
        def.pacman = trimmed(&def.pacman);
        def.tasks.retain(|t| !t.command.trim().is_empty());
        def.fields.retain(|f| !f.env.trim().is_empty());
        for f in &mut def.fields {
            f.env = f.env.trim().to_string();
            if f.label.trim().is_empty() {
                f.label = f.env.clone();
            }
        }
        if def.name.is_empty() || !one_line(&def.name, 80) || !one_line(&def.description, 400) {
            return Err("give the automation a name".into());
        }
        let target = self.target(&def.target)?;
        if !(target.inject || (target.family == "android" && !target.runtime.is_empty())) {
            return Err("this operating system cannot be set up from the host yet. Pick one marked as scriptable".into());
        }
        if !def.script.is_empty() && target.family == "android" {
            return Err("Castle scripts run on Linux machines. Pick a Linux system or container".into());
        }
        if is_git_address(&def.source) && !trusted_source(&def.source) {
            return Err(format!("a project address must be https on one of: {}", TRUSTED_HOSTS.join(", ")));
        }
        if !one_line(&def.source, 400) || !one_line(&def.screen, 16) || def.fields.len() > MAX_LIST || !def.fields.iter().all(|f| env_name(&f.env) && one_line(&f.label, 60)) {
            return Err("a field name must be capital letters, digits and underscores, like API_KEY".into());
        }
        check_packages(&def.apt)?;
        check_packages(&def.pacman)?;
        check_commands(&def.tasks.iter().map(|t| t.command.clone()).collect::<Vec<_>>())?;
        let wanted = if def.id.is_empty() { slug(&def.name) } else { def.id.clone() };
        if wanted.is_empty() {
            return Err("give the automation a name with letters or digits in it".into());
        }
        let creating = def.id.is_empty();
        let mut id = wanted.clone();
        let schedule = def.schedule.clone();
        let mut schedule_before = String::new();
        self.change_automations(|all| {
            if creating {
                let mut n = 2;
                while all.iter().any(|a| a.id == id) {
                    id = format!("{wanted}-{n}");
                    n += 1;
                }
                def.id = id.clone();
                def.setup = String::new();
                def.builtin = false;
                def.bots = vec![Bot { id: MAIN_BOT.into(), name: format!("{} 1", def.name), values: HashMap::new() }];
                all.push(def);
            } else {
                let existing = all.iter_mut().find(|a| a.id == id).ok_or_else(|| "unknown automation".to_string())?;
                schedule_before = existing.schedule.clone();
                def.setup = existing.setup.clone();
                def.builtin = existing.builtin;
                def.bots = std::mem::take(&mut existing.bots);
                *existing = def;
            }
            Ok(())
        })?;
        if schedule != schedule_before {
            self.schedule_changed(&id, &schedule);
        }
        self.automation_status(&id)
    }

    pub fn delete_automation(&self, id: &str) -> ClonesResult<Vec<Value>> {
        self.require_loopback()?;
        let a = self.automation(id)?;
        if a.builtin {
            return Err("the built-in example cannot be removed".into());
        }
        if a.bots.iter().any(|b| matches!(self.state_of(&self.bot_unit(&a, b)), "running" | "launching")) {
            return Err("stop its bots first".into());
        }
        self.change_automations(|all| {
            all.retain(|x| x.id != id);
            Ok(())
        })?;
        if let Ok(results) = self.results() {
            let _ = results.forget(id);
        }
        Ok(self.list())
    }

    /// Adds a bot (empty `bot`) or changes one. An empty value keeps what was saved; a
    /// value of a single space clears it, so the field falls back to the project's `.env`.
    pub fn save_bot(&self, b: BotBody) -> ClonesResult<Value> {
        self.require_loopback()?;
        if !one_line(&b.name, 60) || b.values.values().any(|v| !one_line(v, MAX_VALUE_LEN)) {
            return Err("that name or value cannot be saved".into());
        }
        self.change_automations(|all| {
            let a = all.iter_mut().find(|a| a.id == b.id).ok_or_else(|| "unknown automation".to_string())?;
            let known: Vec<String> = a.fields.iter().map(|f| f.env.clone()).collect();
            let at = match a.bots.iter().position(|x| x.id == b.bot) {
                Some(at) => at,
                None if !b.bot.is_empty() => return Err("unknown bot".into()),
                None => {
                    if a.bots.len() >= MAX_BOTS {
                        return Err("that is as many bots as one automation can have".into());
                    }
                    let mut n = a.bots.len() + 1;
                    while a.bots.iter().any(|x| x.id == format!("b{n}")) {
                        n += 1;
                    }
                    let id = if a.bots.is_empty() { MAIN_BOT.to_string() } else { format!("b{n}") };
                    a.bots.push(Bot { id, name: format!("{} {}", a.name, a.bots.len() + 1), values: HashMap::new() });
                    a.bots.len() - 1
                }
            };
            let bot = &mut a.bots[at];
            if !b.name.trim().is_empty() {
                bot.name = b.name.trim().to_string();
            }
            for (env, value) in &b.values {
                if !known.contains(env) || value.is_empty() {
                    continue;
                }
                if value.trim().is_empty() {
                    bot.values.remove(env);
                } else {
                    bot.values.insert(env.clone(), value.clone());
                }
            }
            Ok(())
        })?;
        self.automation_status(&b.id)
    }

    /// Removes a bot and, when it had a disk of its own, that disk.
    pub fn delete_bot(&self, id: &str, bot: &str) -> ClonesResult<Value> {
        self.require_loopback()?;
        let a = self.automation(id)?;
        let found = a.bots.iter().find(|b| b.id == bot).ok_or_else(|| "unknown bot".to_string())?;
        let unit = self.bot_unit(&a, found);
        if matches!(self.state_of(&unit), "running" | "launching") {
            return Err("stop it first".into());
        }
        self.remove_unit(&unit);
        self.launches.lock().unwrap().remove(&unit.key);
        self.change_automations(|all| {
            if let Some(a) = all.iter_mut().find(|a| a.id == id) {
                a.bots.retain(|b| b.id != bot);
            }
            Ok(())
        })?;
        self.automation_status(id)
    }

    pub fn start(&self, id: &str, bot: &str) -> ClonesResult<Value> {
        self.start_as(id, bot, "start")
    }

    /// Starts a bot. `trigger` says who asked: "start" from the app, "schedule" from the clock.
    fn start_as(&self, id: &str, bot: &str, trigger: &str) -> ClonesResult<Value> {
        self.require_loopback()?;
        let a = self.automation(id)?;
        let bot = a.bots.iter().find(|b| b.id == bot || (bot.is_empty() && b.id == MAIN_BOT)).cloned().ok_or_else(|| "unknown bot".to_string())?;
        let unit = self.bot_unit(&a, &bot);
        if matches!(self.state_of(&unit), "running" | "launching") {
            return Err("it is already running".into());
        }
        // A Castle script runs as a project Castle writes for this run (see script_run).
        let (a, run) = if a.script.trim().is_empty() {
            (a, None)
        } else {
            let env = self.source_env(&a);
            if let Some((field, _)) = a.fields.iter().zip(Self::field_sources(&a, &bot, &env)).find(|(_, (from, _))| *from == "missing") {
                return Err(format!("this bot has no {} yet", field.label.to_lowercase()));
            }
            let (def, run) = self.script_run(&a, &bot, &unit, trigger)?;
            (def, Some(run))
        };
        let started = self.start_unit(&a, &bot, unit);
        if let (Err(e), Some(run), Ok(results)) = (&started, &run, self.results()) {
            let _ = results.end(run, "failed", Some(0), "starting the machine", e);
        }
        started?;
        self.automation_status(id)
    }

    /// Writes the project a script run takes into its machine: the compiled script, the
    /// castle library, and this run's ticket (castle-run.json: where to report, and the
    /// secret that lets it). Returns the automation as the launch sees it, and the run.
    fn script_run(&self, a: &AutomationDef, bot: &Bot, unit: &Unit, trigger: &str) -> ClonesResult<(AutomationDef, RunRef)> {
        if self.runtime_kind(&a.target) == "android" {
            return Err("Castle scripts run on Linux machines. Pick a Linux system or container".into());
        }
        let compiled = script::compile(&a.script).map_err(|e| first_problem(&e))?;
        if let Some(f) = compiled.fields.iter().find(|f| !a.fields.iter().any(|x| &x.env == *f)) {
            return Err(format!("the script uses {{{f}}}: add a field called {f} under What each bot fills in"));
        }
        let library = self.dir.join(LIBRARY_DIR);
        if !library.join("__init__.py").is_file() {
            return Err(format!("the castle library is missing from {}", library.display()));
        }
        let results = self.results()?;
        let project = self.data().join("automations").join(Self::file_key(&format!("{}-{}", a.id, bot.id)));
        let _ = std::fs::remove_dir_all(&project);
        let write_fail = |_| "could not write the project for the machine".to_string();
        std::fs::create_dir_all(&project).map_err(write_fail)?;
        copy_library(&library, &project.join("castle"))?;
        std::fs::write(project.join("main.py"), &compiled.python).map_err(write_fail)?;
        std::fs::write(project.join("automation.castle"), a.script.replace("

", "
")).map_err(write_fail)?;

        let token = random_hex(32);
        let id = format!("{:x}-{}", now_ms(), random_hex(3));
        let port = self.port.load(Ordering::Relaxed);
        // The box as the machine sees it: Docker's name for the host from a container, QEMU's
        // user network (10.0.2.2) from a VM. Both reach the box's own 127.0.0.1.
        let hosts: &[&str] = match unit.engine {
            Engine::Container(_) => &["host.docker.internal", "172.17.0.1", "10.0.2.2"],
            _ => &["10.0.2.2", "host.docker.internal"],
        };
        let urls: Vec<String> = hosts.iter().map(|h| format!("http://{h}:{port}")).collect();
        let ticket = json!({ "run": id, "token": token, "urls": urls, "automation": a.id, "bot": bot.id, "web": compiled.web });
        std::fs::write(project.join(RUN_FILE), serde_json::to_vec_pretty(&ticket).unwrap_or_default()).map_err(write_fail)?;
        results.begin(&id, &a.id, &bot.id, &sha256_hex(&token), trigger)?;

        let mut def = a.clone();
        def.source = project.to_string_lossy().into_owned();
        def.setup = String::new();
        def.tasks = vec![Task { name: "Run the script".into(), command: SCRIPT_TASK.into(), root: false }];
        if compiled.web {
            def.apt.extend(["python3-venv", "chromium", "fonts-liberation"].map(String::from));
            def.pacman.extend(["python", "chromium", "ttf-liberation"].map(String::from));
        } else {
            def.apt.push("python3-venv".into());
            def.pacman.push("python".into());
        }
        Ok((def, RunRef { id, automation: a.id.clone(), bot: bot.id.clone(), finished: false }))
    }

    /// Launches a bot's machine with the automation as given.
    fn start_unit(&self, a: &AutomationDef, bot: &Bot, unit: Unit) -> ClonesResult<()> {
        self.ensure_source(a)?;
        let env = self.source_env(a);
        let sources = Self::field_sources(a, bot, &env);
        if let Some((field, _)) = a.fields.iter().zip(&sources).find(|(_, (from, _))| *from == "missing") {
            return Err(format!("this bot has no {} yet", field.label.to_lowercase()));
        }
        let copy_dir = self.copy_dir(&unit.key)?;
        let label = format!("{} / {}", a.name, bot.name);
        let mut env_vars: Vec<(String, String)> = Vec::new();
        let in_container = matches!(unit.engine, Engine::Container(_));
        if a.stop_after_minutes > 0 && in_container {
            env_vars.push(("CASTLE_STOP_AFTER_MIN".into(), a.stop_after_minutes.to_string()));
        }

        // Android: the project's APKs are installed and its other files pushed to the phone;
        // the tasks run on the phone as castle-tasks.sh, with the fields as variables.
        if self.runtime_kind(&a.target) == "android" {
            if let Some(dir) = self.source_dir(a) {
                copy_into(&dir, &copy_dir)?;
                // The project's git history and its .env have no business on the phone; the
                // fields the tasks need are in the script.
                if let Some(name) = dir.file_name() {
                    let _ = std::fs::remove_dir_all(copy_dir.join(name).join(".git"));
                    let _ = std::fs::remove_file(copy_dir.join(name).join(".env"));
                }
            }
            let values: Vec<(String, String)> = a
                .fields
                .iter()
                .zip(&sources)
                .map(|(f, (from, saved))| (f.env.clone(), if *from == "saved" { saved.clone() } else { env_value(&env, &f.env).unwrap_or_default() }))
                .collect();
            let shutdown = if in_container { 0 } else { a.stop_after_minutes };
            std::fs::write(copy_dir.join(TASKS_FILE), android_tasks_script(a, &bot.name, &values, shutdown)).map_err(|_| "could not prepare the tasks for the phone".to_string())?;
            if a.memory_gb > 0 && !in_container {
                env_vars.push(("CASTLE_AVD_RAM_MB".into(), (a.memory_gb * 1024).to_string()));
            }
            return self.launch(Plan { label, mode: "automation", profile: None, snapshot: a.temporary, base: None, memory_gb: a.memory_gb, cores: a.cores, copy_dir: Some(copy_dir), payload_only: true, env: env_vars, unit });
        }

        // The bot's own sign-in: the project's .env with this bot's values written over it.
        if !bot.values.is_empty() {
            let text = a.fields.iter().zip(&sources).filter(|(_, (from, _))| *from == "saved").fold(env.clone(), |text, (f, (_, value))| with_env_value(&text, &f.env, value));
            std::fs::write(copy_dir.join(BOT_ENV_FILE), text).map_err(|_| "could not prepare the sign-in for the machine".to_string())?;
        }
        let mut profile = Profile {
            description: a.name.clone(),
            target: a.target.clone(),
            instance: unit.instance.clone(),
            memory_gb: a.memory_gb,
            cores: a.cores,
            screen: a.screen.clone(),
            apt: a.apt.clone(),
            pacman: a.pacman.clone(),
            root: a.tasks.iter().filter(|t| t.root).map(|t| t.command.clone()).collect(),
            setup: a.setup.clone(),
            ..Default::default()
        };
        if let Some(dir) = self.source_dir(a) {
            let zip = if a.setup.is_empty() { PROJECT_ZIP.to_string() } else { format!("{}.zip", dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "project".into())) };
            let include = if dir.join(".env").is_file() { vec![".env".to_string()] } else { Vec::new() };
            profile.payload.push((dir, zip, include));
        }
        if a.setup.is_empty() {
            profile.apt.push("libarchive-tools".into());
            profile.pacman.push("libarchive".into());
            profile.user.extend(session_command(a));
        } else {
            profile.user = a.tasks.iter().filter(|t| !t.root).map(|t| t.command.clone()).collect();
        }
        if a.stop_after_minutes > 0 && !in_container {
            profile.root.push(format!("shutdown -h +{}", a.stop_after_minutes));
        }
        // A container's own clock (the container stops itself); the profile carries it in.
        profile.env = env_vars;
        // A bot with a disk of its own starts from the target's built disk when there is one.
        let base = self.target(&a.target).ok().filter(|t| t.runtime.is_empty()).map(|t| self.main_disk(&t)).filter(|disk| unit.instance.is_some() && Self::is_built(disk));
        self.launch(Plan { label, mode: "automation", profile: Some(profile), snapshot: a.temporary, base, memory_gb: a.memory_gb, cores: a.cores, copy_dir: Some(copy_dir), payload_only: false, env: Vec::new(), unit })
    }

    pub fn stop(&self, id: &str, bot: &str) -> ClonesResult<Value> {
        self.require_loopback()?;
        let a = self.automation(id)?;
        let bot = a.bots.iter().find(|b| b.id == bot || (bot.is_empty() && b.id == MAIN_BOT)).ok_or_else(|| "unknown bot".to_string())?;
        self.halt(&self.bot_unit(&a, bot))?;
        if let (false, Ok(results)) = (a.script.trim().is_empty(), self.results()) {
            if let Some(run) = results.open_run(&a.id, &bot.id) {
                let _ = results.end(&run, "stopped", None, "", "stopped from Castle");
            }
        }
        self.automation_status(id)
    }

    /// Stops every machine and bot of Castle's that is running, for when the box itself
    /// stops. Only Castle's own: a VM started some other way, from a disk that is not one of
    /// these, is not looked at. VMs are asked to shut down, and switched off if they have not
    /// within SHUTDOWN_GRACE. Returns what could not be stopped.
    pub fn stop_all(&self) -> Vec<String> {
        let mut units: Vec<Unit> = self.targets().iter().map(|t| self.unit(format!("m:{}", t.id), &t.id, None)).collect();
        units.extend(self.registry().into_iter().map(|m| self.unit(format!("m:{}", m.id), &m.target, Some(m.id))));
        for a in self.all_automations() {
            units.extend(a.bots.iter().map(|bot| self.bot_unit(&a, bot)));
        }
        let mut failed = Vec::new();
        let mut vms = Vec::new();
        for unit in units {
            if !matches!(self.state_of(&unit), "launching" | "running") {
                continue;
            }
            let port = matches!(unit.engine, Engine::Vm).then(|| self.disk(&unit).and_then(|d| self.vm_command_line(&d)).and_then(|l| qmp_port(&l))).flatten();
            match self.halt(&unit) {
                Ok(()) => vms.extend(port.map(|p| (unit.key.clone(), unit, p))),
                Err(e) => failed.push(format!("{}: {e}", unit.key)),
            }
        }
        let deadline = std::time::Instant::now() + SHUTDOWN_GRACE;
        while !vms.is_empty() {
            self.forget_scan();
            vms.retain(|(_, unit, _)| self.is_up(unit));
            if vms.is_empty() || std::time::Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        for (key, _, port) in vms {
            if let Err(e) = switch_off(port) {
                failed.push(format!("{key}: {e}"));
            }
        }
        failed
    }

    // ---- script runs: what they report, the results they leave, the clock ----

    /// The results database, opened on first use beside the automations list.
    fn results(&self) -> ClonesResult<&Results> {
        self.results
            .get_or_init(|| Results::open(&self.log_dir).map_err(|e| eprintln!("castled: results: {e}")).ok())
            .as_ref()
            .ok_or_else(|| "the results database could not be opened".to_string())
    }

    /// The port the box listens on, which script runs report to.
    pub fn set_port(&self, port: u16) {
        self.port.store(port, Ordering::Relaxed);
    }

    /// True when `token` is the secret of a run this box started.
    pub fn run_known(&self, token: &str) -> bool {
        token.len() >= 32 && self.results().is_ok_and(|r| r.by_token(&sha256_hex(token)).is_some())
    }

    /// A report from a script inside its machine (see the castle library). The run's own
    /// secret is the only key: it opens this run's results and nothing else, and only until
    /// the run ends. `Err(REFUSED)` means the secret is not one the box gave out.
    pub fn report(&self, token: &str, body: &Value) -> ClonesResult<Value> {
        let results = self.results()?;
        if token.len() < 32 {
            return Err(REFUSED.into());
        }
        let run = results.by_token(&sha256_hex(token)).ok_or_else(|| REFUSED.to_string())?;
        if run.finished {
            return Ok(json!({ "ok": false, "ended": true }));
        }
        let text = |key: &str, max: usize| -> String { body[key].as_str().unwrap_or_default().chars().take(max).collect() };
        let line = body["line"].as_i64().unwrap_or(0).clamp(0, 1_000_000);
        match body["kind"].as_str().unwrap_or_default() {
            "hello" => results.running(&run.id)?,
            "step" => results.step(&run.id, line, &text("said", 400))?,
            "log" => results.log(&run.id, line, &text("text", 2000))?,
            "rows" => {
                let dataset = text("dataset", 60);
                let rows = body["rows"].as_array().ok_or("rows must be a list")?;
                if dataset.trim().is_empty() || !rows.iter().all(Value::is_object) {
                    return Err("rows are a dataset name and a list of objects".into());
                }
                let kept = results.rows(&run, dataset.trim(), rows)?;
                return Ok(json!({ "ok": true, "kept": kept }));
            }
            "metric" => {
                let value = body["value"].as_f64().ok_or("a tracked value is a number")?;
                let better = match text("better", 10).as_str() {
                    b @ ("lower" | "higher") => b.to_string(),
                    _ => String::new(),
                };
                results.metric(&run, &text("name", 60), value, &better)?;
            }
            "shot" => {
                let png = base64_decode(body["png"].as_str().unwrap_or_default()).ok_or("a picture is a base64 PNG")?;
                results.shot(&run.id, &text("label", 120), &png)?;
            }
            kind @ ("finish" | "fail") => {
                let failed = kind == "fail";
                let ended = results.end(&run, if failed { "failed" } else { "done" }, failed.then_some(line), &text("said", 400), &text("message", 1000))?;
                if ended {
                    self.stop_when_done(&run);
                }
            }
            _ => return Err("unknown report".into()),
        }
        Ok(json!({ "ok": true }))
    }

    /// A script that ended has done its work: its machine is stopped shortly, unless the
    /// automation runs its tasks again and again.
    fn stop_when_done(&self, run: &RunRef) {
        let Ok(a) = self.automation(&run.automation) else { return };
        if a.restart {
            return;
        }
        if let Some(bot) = a.bots.iter().find(|b| b.id == run.bot) {
            let key = self.bot_unit(&a, bot).key;
            self.to_stop.lock().unwrap().push((key, Instant::now() + STOP_AFTER_FINISH));
        }
    }

    /// Checks a script without running it: its problems, or the Python it becomes.
    pub fn check_script(&self, text: &str) -> Value {
        match script::compile(text) {
            Ok(c) => json!({ "ok": true, "problems": [], "python": c.python, "web": c.web, "datasets": c.datasets, "tracked": c.tracked, "fields": c.fields }),
            Err(problems) => json!({ "ok": false, "problems": problems }),
        }
    }

    /// An automation's runs, the rows its datasets hold now, and its tracked values.
    pub fn results_of(&self, id: &str) -> ClonesResult<Value> {
        self.automation(id)?;
        self.results()?.overview(id)
    }

    pub fn result_shot(&self, path: &str) -> ClonesResult<Vec<u8>> {
        self.results()?.shot_bytes(path)
    }

    /// A run whose machine is no longer up never said how it ended: it is ended here.
    fn settle_run(&self, a: &AutomationDef, bot: &Bot, state: &str) {
        if a.script.trim().is_empty() || state == "launching" || state == "running" {
            return;
        }
        let Ok(results) = self.results() else { return };
        if let Some(run) = results.open_run(&a.id, &bot.id) {
            let (status, message) = if state == "failed" {
                ("failed", "the machine did not start; its launch log says why")
            } else {
                ("stopped", "the machine stopped before the script finished")
            };
            let _ = results.end(&run, status, None, "", message);
        }
    }

    /// The scheduler: once a day at each automation's time it starts the first bot, and it
    /// stops machines whose script has finished. Runs for as long as the box does.
    pub fn background(self: &Arc<Self>) {
        let clones = Arc::downgrade(self);
        std::thread::spawn(move || {
            let mut ticks: u64 = 0;
            loop {
                std::thread::sleep(TICK);
                let Some(clones) = clones.upgrade() else { return };
                clones.stop_finished();
                if ticks % SCHEDULE_EVERY_TICKS == 0 {
                    clones.run_schedules(chrono::Local::now().naive_local());
                }
                ticks += 1;
            }
        });
    }

    fn stop_finished(&self) {
        let due: Vec<String> = {
            let mut list = self.to_stop.lock().unwrap();
            let now = Instant::now();
            let (due, later): (Vec<_>, Vec<_>) = list.drain(..).partition(|(_, at)| *at <= now);
            *list = later;
            due.into_iter().map(|(key, _)| key).collect()
        };
        for key in due {
            if let Ok(unit) = self.unit_for(&key) {
                if let Err(e) = self.halt(&unit) {
                    eprintln!("castled: could not stop {key} after its script finished: {e}");
                }
            }
        }
    }

    /// Starts every automation whose time has come today and has not run for it yet. A box
    /// that was off at that time runs it when it is next up, the same day.
    fn run_schedules(&self, now: chrono::NaiveDateTime) {
        if !self.loopback {
            return;
        }
        let Ok(results) = self.results() else { return };
        let today = now.date().to_string();
        for a in self.all_automations() {
            let Some((h, m)) = schedule_time(&a.schedule) else { continue };
            let due = now.time() >= chrono::NaiveTime::from_hms_opt(h, m, 0).unwrap_or_default();
            if !due || results.last_scheduled(&a.id).as_deref() == Some(today.as_str()) {
                continue;
            }
            let _ = results.set_last_scheduled(&a.id, &today);
            let Some(bot) = a.bots.first() else { continue };
            match self.start_as(&a.id, &bot.id, "schedule") {
                Ok(_) => eprintln!("castled: started {} on its schedule ({})", a.name, a.schedule),
                Err(e) => eprintln!("castled: {} did not start on its schedule: {e}", a.name),
            }
        }
    }

    /// When a schedule is set or changed after its time today, today counts as done: the
    /// first run is tomorrow, not the moment it is saved.
    fn schedule_changed(&self, id: &str, schedule: &str) {
        let (Some((h, m)), Ok(results)) = (schedule_time(schedule), self.results()) else { return };
        let now = chrono::Local::now().naive_local();
        if now.time() >= chrono::NaiveTime::from_hms_opt(h, m, 0).unwrap_or_default() {
            let _ = results.set_last_scheduled(id, &now.date().to_string());
        }
    }

    // ---- machines ----

    fn registry(&self) -> Vec<Machine> {
        load(&self.machines_file()).unwrap_or_default()
    }

    fn change_registry(&self, change: impl FnOnce(&mut Vec<Machine>)) -> ClonesResult<()> {
        let _one_writer = self.files.lock().unwrap();
        let mut all = self.registry();
        change(&mut all);
        save(&self.machines_file(), &all)
    }

    fn machine_unit(&self, id: &str) -> ClonesResult<Unit> {
        let key = format!("m:{id}");
        if let Some(m) = self.registry().into_iter().find(|m| m.id == id) {
            return Ok(self.unit(key, &m.target, Some(m.id)));
        }
        self.target(id).map(|t| self.unit(key, &t.id, None)).map_err(|_| "unknown machine".to_string())
    }

    /// Deletes an instance's disk and everything the launcher staged beside it.
    fn remove_instance(&self, instance: &str) {
        let data = self.data();
        let seeds = data.join("seeds");
        let _ = std::fs::remove_file(self.instance_disk(instance));
        for file in [data.join("instances").join(format!("{instance}.vars.fd")), data.join("scratch").join(format!("{instance}-scratch.qcow2")), seeds.join(format!("copy-share-{instance}.iso"))] {
            let _ = std::fs::remove_file(file);
        }
        for dir in [data.join("instances").join("seeds").join(instance), seeds.join(format!("copy-share-{instance}")), seeds.join(format!("copy-share-{instance}.iso.tree")), seeds.join(format!("profile-{instance}")), data.join("copy-app").join(format!("m-{instance}"))] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// Throwaway machines that have closed are removed, disk and all.
    fn sweep_temporaries(&self) {
        let closed: Vec<Machine> = self
            .registry()
            .into_iter()
            .filter(|m| m.temporary)
            .filter(|m| {
                let unit = self.unit(format!("m:{}", m.id), &m.target, Some(m.id.clone()));
                if self.state_of(&unit) != "stopped" {
                    return false;
                }
                // Look again with a fresh process list before anything is deleted.
                self.forget_scan();
                self.state_of(&unit) == "stopped"
            })
            .collect();
        if closed.is_empty() {
            return;
        }
        for m in &closed {
            self.remove_unit(&self.unit(format!("m:{}", m.id), &m.target, Some(m.id.clone())));
            self.launches.lock().unwrap().remove(&format!("m:{}", m.id));
            let _ = std::fs::remove_file(self.log_file(&format!("m:{}", m.id)));
        }
        let _ = self.change_registry(|all| all.retain(|m| !closed.iter().any(|c| c.id == m.id)));
    }

    /// The catalogue, the machines that exist, and how long recent launches took.
    pub fn machines(&self) -> Value {
        self.sweep_temporaries();
        let targets = self.targets();
        let registry = self.registry();
        let size = |disk: &Path| std::fs::metadata(disk).map(|m| m.len()).unwrap_or(0);
        let mut machines = Vec::new();
        let mut catalogue = Vec::new();
        for t in targets.iter() {
            let unit = self.unit(format!("m:{}", t.id), &t.id, None);
            let disk = self.disk(&unit);
            let built = match &unit.engine {
                Engine::Container(name) => self.container_exists(name),
                _ => disk.as_ref().is_some_and(|d| size(d) > EMPTY_DISK_BYTES),
            };
            let state = self.state_of(&unit);
            if built || state != "stopped" {
                machines.push(json!({
                    "id": t.id, "key": unit.key, "name": t.name, "target": t.id, "kind": "main", "state": state,
                    "engine": unit.engine.label(), "controllable": matches!(unit.engine, Engine::Container(_)) && t.family != "android",
                    "diskBytes": disk.as_ref().map(|d| size(d)).unwrap_or(0), "log": tail(&self.log_file(&unit.key)),
                }));
            }
            let mut entry = serde_json::to_value(t).unwrap_or_default();
            entry["built"] = json!(built);
            entry["apk"] = json!(t.family == "android" && !t.runtime.is_empty());
            // Android targets with an engine Castle drives can carry automations (tasks on the phone).
            entry["automatable"] = json!(t.inject || (t.family == "android" && !t.runtime.is_empty()));
            entry["disks"] = json!(registry.iter().filter(|m| m.target == t.id && !m.temporary).count() + usize::from(built));
            catalogue.push(entry);
        }
        for m in &registry {
            let unit = self.unit(format!("m:{}", m.id), &m.target, Some(m.id.clone()));
            machines.push(json!({
                "id": m.id, "key": unit.key, "name": m.name, "target": m.target, "kind": if m.temporary { "temporary" } else { "instance" },
                "engine": unit.engine.label(), "controllable": matches!(unit.engine, Engine::Container(_)) && self.runtime_kind(&m.target) == "linux",
                "state": self.state_of(&unit), "diskBytes": size(&self.instance_disk(&m.id)), "memoryGb": m.memory_gb, "cores": m.cores,
                "log": tail(&self.log_file(&unit.key)),
            }));
        }
        let mut bench: Vec<Value> = load(&self.bench_file()).unwrap_or_default();
        bench.reverse();
        bench.truncate(50);
        json!({ "targets": catalogue, "machines": machines, "bench": bench, "docker": self.has_docker() })
    }

    /// Starts a machine: one that exists, a new one with a disk it keeps, or a throwaway.
    pub fn start_machine(&self, b: MachineBody) -> ClonesResult<Value> {
        self.require_loopback()?;
        let apt = trimmed(&b.apt);
        let pacman = trimmed(&b.pacman);
        let root = trimmed(&b.root_commands);
        let user = trimmed(&b.user_commands);
        let paths = trimmed(&b.paths);
        check_packages(&apt)?;
        check_packages(&pacman)?;
        check_commands(&root)?;
        check_commands(&user)?;
        check_commands(&paths)?;
        if b.memory_gb > 1024 || b.cores > 512 {
            return Err("that is more memory or more cores than a machine can have".into());
        }

        let mut registered: Option<Machine> = None;
        let (unit, label, mode, target) = match b.mode.as_str() {
            "existing" => {
                let unit = self.machine_unit(&b.machine)?;
                let target = self.target(&unit.target)?;
                let name = self.registry().into_iter().find(|m| m.id == b.machine).map(|m| m.name).unwrap_or_else(|| target.name.clone());
                (unit, name, "existing", target)
            }
            mode @ ("new" | "temporary") => {
                let target = self.target(&b.target)?;
                if !target.runtime.is_empty() && target.runtime != "container" {
                    return Err("this one runs outside QEMU and has a single machine. Start the existing one".into());
                }
                let temporary = mode == "temporary";
                let name = if temporary { format!("{} (temporary)", target.name) } else { b.name.trim().to_string() };
                let id = if temporary { format!("tmp-{}-{:x}", target.id, now_ms() & 0xff_ffff) } else { slug(&b.name) };
                if id.is_empty() || !one_line(&name, 60) {
                    return Err("give the new machine a name with letters or digits in it".into());
                }
                let taken = self.registry().iter().any(|m| m.id == id) || self.targets().iter().any(|t| t.id == id) || self.instance_disk(&id).exists();
                if taken {
                    return Err("there is already a machine with that name".into());
                }
                registered = Some(Machine { id: id.clone(), name: name.clone(), target: target.id.clone(), temporary, memory_gb: b.memory_gb, cores: b.cores });
                (self.unit(format!("m:{id}"), &target.id, Some(id)), name, if temporary { "temporary" } else { "new" }, target)
            }
            _ => return Err("choose an existing machine, a new one or a temporary one".into()),
        };

        let injecting = !(apt.is_empty() && pacman.is_empty() && root.is_empty() && user.is_empty() && paths.is_empty());
        let mut copy_dir = None;
        let mut profile = None;
        let payload_only = injecting && target.family == "android" && !target.runtime.is_empty();
        if payload_only {
            if !(apt.is_empty() && pacman.is_empty() && root.is_empty() && user.is_empty()) {
                return Err("Android takes files only: APKs are installed, anything else lands in Download/castle-copy".into());
            }
            let dir = self.copy_dir(&unit.key)?;
            for path in &paths {
                copy_into(Path::new(path), &dir)?;
            }
            copy_dir = Some(dir);
        } else if injecting {
            if !target.inject {
                return Err("scripts and files cannot be put into this operating system from the host yet".into());
            }
            let dir = self.copy_dir(&unit.key)?;
            let mut p = Profile { description: label.clone(), target: target.id.clone(), instance: unit.instance.clone(), memory_gb: b.memory_gb, cores: b.cores, apt, pacman, root, user, ..Default::default() };
            for path in &paths {
                let path = PathBuf::from(path);
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| format!("not a file or folder: {}", path.display()))?;
                if path.is_dir() {
                    p.payload.push((path, format!("{name}.zip"), Vec::new()));
                } else if path.is_file() {
                    std::fs::copy(&path, dir.join(&name)).map_err(|_| format!("could not read {}", path.display()))?;
                } else {
                    return Err(format!("not on the box: {}", path.display()));
                }
            }
            copy_dir = Some(dir);
            profile = Some(p);
        }
        // A new disk starts from the target's built one unless a fresh install was asked for.
        let base = Some(self.main_disk(&target)).filter(|disk| target.runtime.is_empty() && unit.instance.is_some() && Self::is_built(disk) && (mode == "temporary" || !b.fresh));
        let (memory_gb, cores) = match self.registry().into_iter().find(|m| Some(&m.id) == unit.instance.as_ref()) {
            Some(m) => (if b.memory_gb > 0 { b.memory_gb } else { m.memory_gb }, if b.cores > 0 { b.cores } else { m.cores }),
            None => (b.memory_gb, b.cores),
        };
        if let Some(p) = profile.as_mut() {
            p.memory_gb = memory_gb;
            p.cores = cores;
        }
        self.launch(Plan { unit, label, mode, profile, snapshot: mode == "temporary", base, memory_gb, cores, copy_dir, payload_only, env: Vec::new() })?;
        if let Some(machine) = registered {
            self.change_registry(|all| all.push(machine))?;
        }
        Ok(self.machines())
    }

    pub fn stop_machine(&self, id: &str) -> ClonesResult<Value> {
        self.require_loopback()?;
        self.halt(&self.machine_unit(id)?)?;
        Ok(self.machines())
    }

    /// Deletes a machine made from the app, and its disk. A target's main disk is not
    /// something this can remove.
    pub fn delete_machine(&self, id: &str) -> ClonesResult<Value> {
        self.require_loopback()?;
        if !self.registry().iter().any(|m| m.id == id) {
            return Err("only machines made here can be deleted here".into());
        }
        let unit = self.machine_unit(id)?;
        self.forget_scan();
        if matches!(self.state_of(&unit), "running" | "launching") {
            return Err("stop it first".into());
        }
        self.remove_instance(id);
        self.launches.lock().unwrap().remove(&unit.key);
        let _ = std::fs::remove_file(self.log_file(&unit.key));
        self.change_registry(|all| all.retain(|m| m.id != id))?;
        Ok(self.machines())
    }
}

/// QMP handshake, then the ACPI power-button request.
fn power_off(port: u16) -> ClonesResult<()> {
    let fail = |_| "the VM did not answer its control port".to_string();
    let stream = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), QMP_TIMEOUT).map_err(fail)?;
    stream.set_read_timeout(Some(QMP_TIMEOUT)).map_err(fail)?;
    let mut reader = BufReader::new(stream.try_clone().map_err(fail)?);
    let mut writer = stream;
    let mut line = String::new();
    reader.read_line(&mut line).map_err(fail)?; // greeting
    writer.write_all(b"{\"execute\":\"qmp_capabilities\"}\n").map_err(fail)?;
    reader.read_line(&mut line).map_err(fail)?;
    writer.write_all(b"{\"execute\":\"system_powerdown\"}\n").map_err(fail)?;
    Ok(())
}

/// Ends a VM at once, like pulling its plug: for one that did not shut down when asked.
fn switch_off(port: u16) -> ClonesResult<()> {
    let fail = |_| "the VM did not answer its control port".to_string();
    let stream = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), QMP_TIMEOUT).map_err(fail)?;
    stream.set_read_timeout(Some(QMP_TIMEOUT)).map_err(fail)?;
    let mut reader = BufReader::new(stream.try_clone().map_err(fail)?);
    let mut writer = stream;
    let mut line = String::new();
    reader.read_line(&mut line).map_err(fail)?; // greeting
    writer.write_all(b"{\"execute\":\"qmp_capabilities\"}\n").map_err(fail)?;
    reader.read_line(&mut line).map_err(fail)?;
    writer.write_all(b"{\"execute\":\"quit\"}\n").map_err(fail)?;
    Ok(())
}

/// QMP handshake, then a request for a PNG of the display, written by QEMU to `file`.
fn screen_dump(port: u16, file: &Path) -> ClonesResult<()> {
    const MAX_REPLY_LINES: usize = 20; // events can arrive before the answer; skip a few, not forever
    let fail = |_| "the VM did not answer its control port".to_string();
    let stream = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), QMP_TIMEOUT).map_err(fail)?;
    stream.set_read_timeout(Some(QMP_TIMEOUT)).map_err(fail)?;
    let mut reader = BufReader::new(stream.try_clone().map_err(fail)?);
    let mut writer = stream;
    let mut line = String::new();
    reader.read_line(&mut line).map_err(fail)?; // greeting
    writer.write_all(b"{\"execute\":\"qmp_capabilities\"}\n").map_err(fail)?;
    reader.read_line(&mut line).map_err(fail)?;
    let request = serde_json::json!({ "execute": "screendump", "arguments": { "filename": file.to_string_lossy(), "format": "png" } });
    writer.write_all(format!("{request}\n").as_bytes()).map_err(fail)?;
    for _ in 0..MAX_REPLY_LINES {
        line.clear();
        reader.read_line(&mut line).map_err(fail)?;
        let Ok(reply) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        if reply.get("return").is_some() {
            return Ok(());
        }
        if reply.get("error").is_some() {
            return Err("the VM could not take a picture of its screen".into());
        }
    }
    Err("the VM did not answer its control port".into())
}

/// True when `dir` looks like a clonewars folder this module can drive.
pub fn looks_like_clonewars(dir: &Path) -> bool {
    dir.join("start.ps1").is_file() && dir.join("profiles").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_lines_round_trip_and_keep_their_neighbours() {
        let start = "# login\nOTHER=1\nPENGUIN_USERNAME=old\n";
        let text = with_env_value(start, "PENGUIN_USERNAME", "Dog Name");
        let text = with_env_value(&text, "PENGUIN_PASSWORD", "p=ss");
        assert_eq!(env_value(&text, "PENGUIN_USERNAME").as_deref(), Some("Dog Name"));
        assert_eq!(env_value(&text, "PENGUIN_PASSWORD").as_deref(), Some("p=ss"));
        assert!(text.contains("# login\nOTHER=1\n"));
    }

    #[test]
    fn a_value_cannot_smuggle_in_a_second_line() {
        assert!(one_line("Dog Name", MAX_VALUE_LEN));
        assert!(!one_line("name\nEVIL=1", MAX_VALUE_LEN));
        assert!(!one_line(&"x".repeat(MAX_VALUE_LEN + 1), MAX_VALUE_LEN));
    }

    #[test]
    fn the_control_port_is_read_from_the_vm_command_line() {
        let line = r#"qemu-system-x86_64.exe -m 4G -qmp tcp:127.0.0.1:4445,server=on,wait=off -drive file=C:\x\data\clonewars\debian-x11.qcow2"#;
        assert_eq!(qmp_port(line), Some(4445));
        assert_eq!(qmp_port("qemu-system-x86_64.exe -m 4G"), None);
    }

    #[test]
    fn colour_codes_are_removed_from_log_lines() {
        assert_eq!(strip_ansi("\u{1b}[32;1m  [OK]\u{1b}[0m done"), "  [OK] done");
    }

    #[test]
    fn a_quote_cannot_end_a_profile_string_early() {
        assert_eq!(ps("it's"), "'it''s'");
        assert_eq!(ps("a\u{2019}; calc"), "'a\u{2019}\u{2019}; calc'");
        let profile = Profile { description: "x'; Remove-Item".into(), target: "debian-x11".into(), user: vec!["echo 'hi'".into()], ..Default::default() };
        let text = profile.render();
        assert!(text.contains("Description  = 'x''; Remove-Item'"));
        assert!(text.contains("UserCommands = @('echo ''hi''')"));
    }

    #[test]
    fn only_trusted_https_hosts_are_fetched_from() {
        assert!(trusted_source("https://github.com/someone/project.git"));
        assert!(!trusted_source("http://github.com/someone/project"));
        assert!(!trusted_source("https://github.com.evil.example/someone/project"));
        assert!(!trusted_source("git@github.com:someone/project.git"));
        assert!(is_git_address("git@github.com:someone/project.git"));
        assert!(!is_git_address(r"C:\projects\mine"));
    }

    #[test]
    fn names_become_safe_ids() {
        assert_eq!(slug("  My Bot #2! "), "my-bot-2");
        assert_eq!(slug("../../etc"), "etc");
        assert!(env_name("API_KEY") && !env_name("api key") && !env_name("1X"));
    }

    /// A Clones with a fixed catalogue, so no test asks PowerShell for the target list.
    fn test_clones() -> Clones {
        let clones = Clones::new(std::env::temp_dir().join("castle-clones-test"), std::env::temp_dir().join("castle-clones-test-log"), true);
        let target = |id: &str, family: &str, runtime: &str| Target { id: id.into(), name: id.into(), family: family.into(), runtime: runtime.into(), ..Default::default() };
        *clones.targets.lock().unwrap() = Some(Arc::new(vec![
            target("debian-x11", "linux", ""),
            target("container-debian", "linux", "container"),
            target("container-android", "android", "container"),
            target("android-avd", "android", "avd"),
        ]));
        clones
    }

    #[test]
    fn each_bot_gets_its_own_disk_except_the_first() {
        let clones = test_clones();
        let mut a = penguin();
        a.bots.push(Bot { id: "b2".into(), name: "Penguin 2".into(), values: HashMap::new() });
        assert!(clones.bot_unit(&a, &a.bots[0]).instance.is_none());
        assert_eq!(clones.bot_unit(&a, &a.bots[1]).instance.as_deref(), Some("penguin-b2"));
        a.temporary = true;
        assert_eq!(clones.bot_unit(&a, &a.bots[0]).instance.as_deref(), Some("penguin-main"));
    }

    #[test]
    fn in_a_container_or_an_emulator_every_bot_is_its_own_machine() {
        let clones = test_clones();
        let mut a = penguin();
        a.target = "container-debian".into();
        let unit = clones.bot_unit(&a, &a.bots[0]);
        assert_eq!(unit.instance.as_deref(), Some("penguin-main"));
        assert!(unit.engine == Engine::Container("castle-penguin-main".into()));
        a.target = "android-avd".into();
        assert!(clones.bot_unit(&a, &a.bots[0]).engine == Engine::Emulator("castle-penguin-main".into()));
        // The target's own emulator keeps the AVD android.ps1 always called "castle".
        assert!(clones.unit("m:android-avd".into(), "android-avd", None).engine == Engine::Emulator("castle".into()));
        assert!(clones.unit("m:debian-x11".into(), "debian-x11", None).engine == Engine::Vm);
        assert_eq!(clones.runtime_kind("container-android"), "android");
    }

    #[test]
    fn android_tasks_quote_the_fields_and_root_goes_through_su() {
        let mut a = penguin();
        a.restart = true;
        a.tasks = vec![
            Task { name: "Open".into(), command: "am start -a android.settings.SETTINGS".into(), root: false },
            Task { name: "As root".into(), command: "id > /data/local/tmp/x".into(), root: true },
        ];
        let script = android_tasks_script(&a, "Bot 1", &[("GREETING".into(), "it's $HOME `x`".into())], 0);
        assert!(script.starts_with("#!/system/bin/sh\n"));
        assert!(script.contains("export GREETING='it'\\''s $HOME `x`'\n"));
        assert!(script.contains("( am start -a android.settings.SETTINGS )"));
        assert!(script.contains("su 0 sh -c 'id > /data/local/tmp/x'"));
        assert!(script.contains("while true; do") && script.contains("done\n"));
        assert!(!script.contains("reboot"));
        assert!(android_tasks_script(&a, "Bot 1", &[], 30).contains("sleep 1800"));
    }

    #[test]
    fn a_bot_falls_back_to_the_project_env_and_never_echoes_it() {
        let a = penguin();
        let mut bot = a.bots[0].clone();
        let env = "PENGUIN_USERNAME=\"Dog\"\nPENGUIN_PASSWORD=\"secret\"\n";
        let from: Vec<&str> = Clones::field_sources(&a, &bot, env).iter().map(|(from, _)| *from).collect();
        assert_eq!(from, ["env", "env"]);
        assert!(Clones::field_sources(&a, &bot, env).iter().all(|(_, value)| value.is_empty()));
        bot.values.insert("PENGUIN_USERNAME".into(), "Other".into());
        assert_eq!(Clones::field_sources(&a, &bot, "")[0], ("saved", "Other".to_string()));
        assert_eq!(Clones::field_sources(&a, &bot, "")[1].0, "missing");
    }

    #[test]
    fn every_box_has_both_examples_and_keeps_its_own_changes() {
        let dir = std::env::temp_dir().join(format!("castle-clones-builtins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let clones = Clones::new(dir.clone(), dir.join("log"), true);
        let ids: Vec<String> = clones.all_automations().into_iter().map(|a| a.id).collect();
        assert_eq!(ids, ["penguin", WALMART_ID]);
        // A list saved before the Walmart example existed gets it, after what it had.
        let mut mine = penguin();
        mine.description = "changed".into();
        save(&clones.automations_file(), &vec![mine, AutomationDef { id: "mine".into(), ..Default::default() }]).unwrap();
        let all = clones.all_automations();
        assert_eq!(all.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["penguin", WALMART_ID, "mine"]);
        assert_eq!(all[0].description, "changed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_walmart_example_is_a_script_that_compiles() {
        let w = walmart();
        assert!(w.builtin && w.setup.is_empty() && w.source.is_empty());
        let compiled = script::compile(&w.script).unwrap();
        assert!(compiled.web && compiled.fields.is_empty());
    }

    #[test]
    fn daily_times_are_hours_and_minutes() {
        assert_eq!(schedule_time("08:00"), Some((8, 0)));
        assert_eq!(schedule_time("23:59"), Some((23, 59)));
        assert_eq!(schedule_time("24:00"), None);
        assert_eq!(schedule_time("8:00"), None);
        assert_eq!(schedule_time("08-00"), None);
    }

    #[test]
    fn pictures_arrive_as_base64() {
        assert_eq!(base64_decode("iVBORw==").unwrap(), b"\x89PNG");
        assert_eq!(base64_decode("aGk").unwrap(), b"hi");
        assert!(base64_decode("a").is_none() && base64_decode("!!!!").is_none());
    }

    #[test]
    fn the_library_loses_windows_line_endings_on_the_way_in() {
        let dir = std::env::temp_dir().join(format!("castle-clones-lib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("from").join("__pycache__")).unwrap();
        std::fs::write(dir.join("from").join("run.sh"), "echo hi\r\n").unwrap();
        std::fs::write(dir.join("from").join("__pycache__").join("x.pyc"), "junk").unwrap();
        copy_library(&dir.join("from"), &dir.join("to")).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("to").join("run.sh")).unwrap(), "echo hi\n");
        assert!(!dir.join("to").join("__pycache__").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tasks_run_in_order_and_loop_when_asked() {
        let mut a = AutomationDef { tasks: vec![Task { name: "a".into(), command: "python main.py".into(), root: false }, Task { name: "r".into(), command: "apt update".into(), root: true }], ..Default::default() };
        let once = session_command(&a).unwrap();
        assert!(once.contains("{ ( python main.py ); }") && !once.contains("apt update"));
        a.restart = true;
        assert!(session_command(&a).unwrap().contains("while true; do ( python main.py ); sleep 5; done"));
    }
}
