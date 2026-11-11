//! Clones: one-click automations that run inside Clonewars VMs.
//!
//! An automation is a named, fixed recipe: a Clonewars launch profile, the project it runs,
//! and the login it needs. The app can save that login, start the VM, stop it, and read
//! what happened. It cannot choose the command: the daemon only ever runs
//! `start.ps1 -Profile <name>` for a name in the table below, so this API is not a way to
//! run arbitrary programs on the box.
//!
//! This is off unless the daemon is started with `--clonewars <dir>`, and it only acts
//! when the daemon listens on this machine alone.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};

use serde::Serialize;

const LOG_TAIL_LINES: usize = 60;
const MAX_LOGIN_LEN: usize = 200;
const SCAN_CACHE: Duration = Duration::from_secs(3);
const QMP_TIMEOUT: Duration = Duration::from_secs(5);
const QMP_FLAG: &str = "-qmp tcp:127.0.0.1:";
/// Hides the console of helper processes on Windows; the VM's own window still appears.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// One automation the box knows how to run.
struct Automation {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    /// Clonewars launch profile: `clonewars/profiles/<profile>.psd1`.
    profile: &'static str,
    /// Clonewars target the profile boots; its disk is `clonewars/data/<target>.qcow2`.
    target: &'static str,
    /// The project checkout, relative to the clonewars directory.
    repo_dir: &'static str,
    /// Environment variable holding the project's git address, used only when the
    /// checkout is missing. The address is configuration, not something to compile in.
    repo_url_env: &'static str,
    /// Names of the two login lines in the project's `.env`.
    env_user: &'static str,
    env_pass: &'static str,
}

const AUTOMATIONS: [Automation; 1] = [Automation {
    id: "penguin",
    name: "Penguin AutoSurf",
    description: "Signs in to Club Penguin Journey and plays Cart Surfer on a loop for coins.",
    profile: "penguin",
    target: "debian-x11",
    repo_dir: "../../penguin-autosurf",
    repo_url_env: "CASTLE_PENGUIN_REPO",
    env_user: "PENGUIN_USERNAME",
    env_pass: "PENGUIN_PASSWORD",
}];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationStatus {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    target: &'static str,
    /// stopped | launching | running | failed
    state: &'static str,
    /// The project is checked out next to Castle (it is fetched on first start if not).
    repo_present: bool,
    /// The saved account name. The password is never sent back, only whether one is saved.
    username: String,
    has_password: bool,
    log: Vec<String>,
}

struct Launch {
    child: Child,
    /// Exit code once the launcher finished; None while it is still setting the VM up.
    exit: Option<i32>,
}

struct Scan {
    at: Instant,
    /// Command lines of every running QEMU process.
    qemu: Vec<String>,
}

pub struct Clones {
    dir: PathBuf,
    log_dir: PathBuf,
    loopback: bool,
    launches: Mutex<HashMap<&'static str, Launch>>,
    scan: Mutex<Option<Scan>>,
    /// Held while a screen picture is taken: a VM's control port serves one caller at a time.
    screen: Mutex<()>,
}

pub type ClonesResult<T> = Result<T, String>;

fn find(id: &str) -> ClonesResult<&'static Automation> {
    AUTOMATIONS.iter().find(|a| a.id == id).ok_or_else(|| "unknown automation".to_string())
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

/// A login value that is safe to put on one `.env` line.
fn valid_login(value: &str) -> bool {
    value.len() <= MAX_LOGIN_LEN && !value.chars().any(|c| c.is_control())
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

impl Clones {
    pub fn new(dir: PathBuf, log_dir: PathBuf, loopback: bool) -> Self {
        Self { dir, log_dir, loopback, launches: Mutex::new(HashMap::new()), scan: Mutex::new(None), screen: Mutex::new(()) }
    }

    fn repo(&self, a: &Automation) -> PathBuf {
        self.dir.join(a.repo_dir)
    }

    fn log_file(&self, a: &Automation) -> PathBuf {
        self.log_dir.join(format!("{}.log", a.id))
    }

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

    /// The QEMU command line running this automation's VM disk out of this clonewars
    /// folder, if there is one. Matching on the disk path means a VM from some other
    /// checkout, or a different target, is never mistaken for it.
    fn vm_command_line(&self, a: &Automation) -> Option<String> {
        let disk = self.dir.join("data").join(format!("{}.qcow2", a.target));
        let disk = disk.to_string_lossy().trim_start_matches(r"\\?\").to_lowercase();
        self.qemu_command_lines().into_iter().find(|line| line.to_lowercase().contains(&disk))
    }

    fn status_of(&self, a: &'static Automation) -> AutomationStatus {
        let env = std::fs::read_to_string(self.repo(a).join(".env")).unwrap_or_default();
        let launch_state = {
            let mut launches = self.launches.lock().unwrap();
            launches.get_mut(a.id).map(|launch| {
                if launch.exit.is_none() {
                    if let Ok(Some(status)) = launch.child.try_wait() {
                        launch.exit = Some(status.code().unwrap_or(-1));
                    }
                }
                launch.exit
            })
        };
        let state = match launch_state {
            Some(None) => "launching",
            _ if self.vm_command_line(a).is_some() => "running",
            Some(Some(code)) if code != 0 => "failed",
            _ => "stopped",
        };
        let log = std::fs::read(self.log_file(a))
            .map(|raw| {
                let text = String::from_utf8_lossy(&raw);
                let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
                let from = lines.len().saturating_sub(LOG_TAIL_LINES);
                lines[from..].iter().map(|l| strip_ansi(l)).collect()
            })
            .unwrap_or_default();
        AutomationStatus {
            id: a.id,
            name: a.name,
            description: a.description,
            target: a.target,
            state,
            repo_present: self.repo(a).join(".git").exists(),
            username: env_value(&env, a.env_user).unwrap_or_default(),
            has_password: env_value(&env, a.env_pass).is_some_and(|p| !p.is_empty()),
            log,
        }
    }

    pub fn list(&self) -> Vec<AutomationStatus> {
        AUTOMATIONS.iter().map(|a| self.status_of(a)).collect()
    }

    fn require_loopback(&self) -> ClonesResult<()> {
        if self.loopback {
            Ok(())
        } else {
            Err("automations can only be controlled when the box listens on this machine alone".into())
        }
    }

    /// Fetches the project next to Castle if it is not there yet and its address is
    /// configured. An existing checkout is left exactly as it is, so local work is never
    /// overwritten.
    fn ensure_repo(&self, a: &Automation) -> ClonesResult<()> {
        let repo = self.repo(a);
        if repo.join(".git").exists() {
            return Ok(());
        }
        let url = std::env::var(a.repo_url_env).ok().filter(|u| !u.trim().is_empty()).ok_or_else(|| {
            format!("the project is not next to Castle. Clone it there, or set {} to its git address", a.repo_url_env)
        })?;
        let mut cmd = hidden(Command::new("git"));
        cmd.args(["clone", "--depth", "1", url.trim()]).arg(&repo);
        match cmd.output() {
            Ok(out) if out.status.success() => Ok(()),
            Ok(out) => Err(format!("could not fetch the project: {}", String::from_utf8_lossy(&out.stderr).trim())),
            Err(_) => Err("could not fetch the project: git is not installed".into()),
        }
    }

    /// Saves the account the automation signs in with. An empty password keeps the saved one.
    pub fn save_login(&self, id: &str, username: &str, password: &str) -> ClonesResult<AutomationStatus> {
        self.require_loopback()?;
        let a = find(id)?;
        let username = username.trim();
        if username.is_empty() || !valid_login(username) || !valid_login(password) {
            return Err("that name or password cannot be saved".into());
        }
        self.ensure_repo(a)?;
        let path = self.repo(a).join(".env");
        let mut text = std::fs::read_to_string(&path).unwrap_or_default();
        text = with_env_value(&text, a.env_user, username);
        if !password.is_empty() {
            text = with_env_value(&text, a.env_pass, password);
        }
        let tmp = path.with_extension("env.tmp");
        std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &path)).map_err(|_| "could not save the login".to_string())?;
        Ok(self.status_of(a))
    }

    pub fn start(&self, id: &str) -> ClonesResult<AutomationStatus> {
        self.require_loopback()?;
        let a = find(id)?;
        let before = self.status_of(a);
        if before.state == "launching" || before.state == "running" {
            return Err("it is already running".into());
        }
        self.ensure_repo(a)?;
        let after_fetch = self.status_of(a);
        if after_fetch.username.is_empty() || !after_fetch.has_password {
            return Err("save the account name and password first".into());
        }
        std::fs::create_dir_all(&self.log_dir).map_err(|_| "could not open the log".to_string())?;
        let log = std::fs::File::create(self.log_file(a)).map_err(|_| "could not open the log".to_string())?;
        let log_err = log.try_clone().map_err(|_| "could not open the log".to_string())?;
        let mut cmd = hidden(Command::new("pwsh"));
        cmd.current_dir(&self.dir)
            .args(["-NoProfile", "-NonInteractive", "-File", "start.ps1", "-Profile", a.profile])
            .stdout(log)
            .stderr(log_err);
        let child = cmd.spawn().map_err(|_| "could not start PowerShell 7 (pwsh)".to_string())?;
        self.launches.lock().unwrap().insert(a.id, Launch { child, exit: None });
        *self.scan.lock().unwrap() = None;
        Ok(self.status_of(a))
    }

    /// Asks the VM to shut down the way its power button would. Only the VM running this
    /// automation's disk is ever addressed.
    /// A picture of a running automation's VM screen, as PNG. It only looks: nothing is
    /// typed, clicked or changed in the machine, and its own window is left alone.
    pub fn screen(&self, id: &str) -> ClonesResult<Vec<u8>> {
        let a = find(id)?;
        let line = self.vm_command_line(a).ok_or_else(|| "this automation's machine is not running".to_string())?;
        let port = qmp_port(&line).ok_or_else(|| "this VM has no control port".to_string())?;
        let _one_at_a_time = self.screen.lock();
        std::fs::create_dir_all(&self.log_dir).map_err(|_| "could not prepare a place for the picture".to_string())?;
        let file = self.log_dir.join(format!("{}-screen.png", a.id));
        let taken = screen_dump(port, &file);
        let bytes = std::fs::read(&file);
        let _ = std::fs::remove_file(&file);
        taken?;
        bytes.map_err(|_| "the machine did not produce a picture".to_string())
    }

    pub fn stop(&self, id: &str) -> ClonesResult<AutomationStatus> {
        self.require_loopback()?;
        let a = find(id)?;
        if let Some(mut launch) = self.launches.lock().unwrap().remove(a.id) {
            if launch.exit.is_none() {
                let _ = launch.child.kill();
                let _ = launch.child.wait();
            }
        }
        if let Some(line) = self.vm_command_line(a) {
            let port = qmp_port(&line).ok_or_else(|| "this VM has no control port".to_string())?;
            power_off(port)?;
        }
        *self.scan.lock().unwrap() = None;
        Ok(self.status_of(a))
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
        let text = with_env_value(&text, "PENGUIN_PASSWORD", "p\"a#s s=w");
        assert_eq!(text, "# login\nOTHER=1\nPENGUIN_USERNAME=\"Dog Name\"\nPENGUIN_PASSWORD=\"p\"a#s s=w\"\n");
        assert_eq!(env_value(&text, "PENGUIN_USERNAME").as_deref(), Some("Dog Name"));
        assert_eq!(env_value(&text, "PENGUIN_PASSWORD").as_deref(), Some("p\"a#s s=w"));
        assert_eq!(env_value(&text, "OTHER").as_deref(), Some("1"));
        assert_eq!(env_value(&text, "MISSING"), None);
    }

    #[test]
    fn a_login_cannot_smuggle_in_a_second_line() {
        assert!(valid_login("plain pass 123!"));
        assert!(!valid_login("a\nEVIL=1"));
        assert!(!valid_login("a\rb"));
        assert!(!valid_login(&"x".repeat(MAX_LOGIN_LEN + 1)));
    }

    #[test]
    fn the_control_port_is_read_from_the_vm_command_line() {
        let line = r#""qemu.exe" -m 4G -qmp tcp:127.0.0.1:4445,server=on,wait=off -display gtk"#;
        assert_eq!(qmp_port(line), Some(4445));
        assert_eq!(qmp_port("qemu.exe -m 4G"), None);
        assert_eq!(qmp_port("qemu.exe -qmp tcp:127.0.0.1:,server=on"), None);
    }

    #[test]
    fn only_listed_automations_exist() {
        assert!(find("penguin").is_ok());
        for bad in ["", "Penguin", "../penguin", "penguin -Target x", "list"] {
            assert!(find(bad).is_err(), "{bad:?} should be refused");
        }
    }

    #[test]
    fn colour_codes_are_removed_from_log_lines() {
        assert_eq!(strip_ansi("\u{1b}[32m  [OK] ready\u{1b}[0m"), "  [OK] ready");
    }
}
