//! Castle Security: the box's activity record, its posture checks, and its audits.
//!
//! The activity record is an append-only file where every entry carries the hash of the
//! entry before it. Changing or removing an entry in the middle breaks every hash after
//! it, which `verify` reports. What it cannot catch on its own: someone with access to the
//! disk rewriting the whole file, or cutting entries off the end. That needs the newest
//! hash to be held somewhere else (another Castle device), which is not built yet.
//!
//! Audits are bash scripts compiled into this binary and fed to `bash` on its standard
//! input, so there is no script on disk for anyone to swap. They only read; none changes
//! the system. They are for a Linux box and are reported as unavailable elsewhere.

use std::{
    collections::VecDeque,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const LOG_FILE: &str = "activity.log";
const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_DETAIL_CHARS: usize = 300;
const MAX_DENIED_KEPT: usize = 1000;
/// Wrong-token calls are counted every time but written to the record at most this often,
/// so a flood of them cannot fill the disk.
const DENIED_LOG_EVERY: Duration = Duration::from_secs(60);
const DAY_MS: u64 = 24 * 60 * 60 * 1000;
const AUDIT_TIMEOUT: Duration = Duration::from_secs(30);
const AUDIT_POLL: Duration = Duration::from_millis(50);
const MAX_AUDIT_ROWS: usize = 5000;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub type SecurityResult<T> = Result<T, String>;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn quiet(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.stdin(Stdio::null()).stderr(Stdio::null());
    cmd
}

// ---------------------------------------------------------------------------------------
// The activity record
// ---------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub seq: u64,
    /// Milliseconds since 1970, by the box's clock.
    pub at: u64,
    pub action: String,
    pub detail: String,
    /// The hash of the entry before this one.
    pub prev: String,
    pub hash: String,
}

fn entry_hash(seq: u64, at: u64, action: &str, detail: &str, prev: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{seq}\n{at}\n{action}\n{detail}\n{prev}"));
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

/// One line, no control characters, bounded length: what is safe to store and to show.
fn clean(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(MAX_DETAIL_CHARS).collect()
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    pub intact: bool,
    pub entries: u64,
    /// The newest hash. Holding a copy of it elsewhere is what would expose a rewrite.
    pub head: String,
    /// The first entry that does not follow from the one before it.
    pub broken_at: Option<u64>,
}

struct Chain {
    seq: u64,
    head: String,
}

#[derive(Default)]
struct Denied {
    times: VecDeque<u64>,
    last_logged: Option<SystemTime>,
}

pub struct Security {
    file: PathBuf,
    chain: Mutex<Chain>,
    denied: Mutex<Denied>,
    started: u64,
}

fn read_entries(file: &Path) -> Vec<Result<Entry, ()>> {
    let Ok(text) = std::fs::read_to_string(file) else { return Vec::new() };
    text.lines().filter(|line| !line.trim().is_empty()).map(|line| serde_json::from_str(line).map_err(|_| ())).collect()
}

fn verify_file(file: &Path) -> Verdict {
    let mut prev = GENESIS.to_string();
    let mut count = 0u64;
    let mut broken_at = None;
    for (index, line) in read_entries(file).into_iter().enumerate() {
        let expected_seq = index as u64 + 1;
        count = expected_seq;
        let follows = match &line {
            Ok(e) => e.seq == expected_seq && e.prev == prev && e.hash == entry_hash(e.seq, e.at, &e.action, &e.detail, &e.prev),
            Err(()) => false,
        };
        if !follows && broken_at.is_none() {
            broken_at = Some(expected_seq);
        }
        if let Ok(e) = line {
            prev = e.hash;
        }
    }
    Verdict { intact: broken_at.is_none(), entries: count, head: prev, broken_at }
}

impl Security {
    /// `dir` is the box's private security folder; it is created if missing.
    pub fn new(dir: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let file = dir.join(LOG_FILE);
        // New entries continue from the last readable one, so a damaged record keeps
        // recording; `verify` still reports where it broke.
        let last = read_entries(&file).into_iter().rev().find_map(Result::ok);
        let count = read_entries(&file).len() as u64;
        let chain = Chain { seq: count, head: last.map(|e| e.hash).unwrap_or_else(|| GENESIS.into()) };
        Ok(Self { file, chain: Mutex::new(chain), denied: Mutex::default(), started: now_ms() })
    }

    /// Adds one entry. A record that cannot be written is reported on the box's own log
    /// and does not fail the action being recorded.
    pub fn record(&self, action: &str, detail: &str) {
        let Ok(mut chain) = self.chain.lock() else { return };
        let seq = chain.seq + 1;
        let at = now_ms();
        let (action, detail) = (clean(action), clean(detail));
        let hash = entry_hash(seq, at, &action, &detail, &chain.head);
        let entry = Entry { seq, at, action, detail, prev: chain.head.clone(), hash: hash.clone() };
        let written = serde_json::to_string(&entry).map_err(std::io::Error::other).and_then(|line| {
            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&self.file)?;
            writeln!(file, "{line}")
        });
        match written {
            Ok(()) => {
                chain.seq = seq;
                chain.head = hash;
            }
            Err(e) => eprintln!("[castled] could not write the activity record: {e}"),
        }
    }

    /// A call arrived with a missing or wrong token.
    pub fn note_denied(&self, path: &str) {
        let should_log = {
            let Ok(mut denied) = self.denied.lock() else { return };
            denied.times.push_back(now_ms());
            while denied.times.len() > MAX_DENIED_KEPT {
                denied.times.pop_front();
            }
            let due = denied.last_logged.and_then(|t| t.elapsed().ok()).is_none_or(|gap| gap >= DENIED_LOG_EVERY);
            if due {
                denied.last_logged = Some(SystemTime::now());
            }
            due
        };
        if should_log {
            self.record("access.denied", path);
        }
    }

    /// Wrong-token calls in the last 24 hours, counted since the box started.
    pub fn denied_last_day(&self) -> usize {
        let cutoff = now_ms().saturating_sub(DAY_MS);
        self.denied.lock().map(|d| d.times.iter().filter(|t| **t >= cutoff).count()).unwrap_or(0)
    }

    pub fn started(&self) -> u64 {
        self.started
    }

    /// The newest `limit` entries, newest first.
    pub fn entries(&self, limit: usize) -> Vec<Entry> {
        let _guard = self.chain.lock();
        read_entries(&self.file).into_iter().rev().filter_map(Result::ok).take(limit).collect()
    }

    pub fn verify(&self) -> Verdict {
        let _guard = self.chain.lock();
        verify_file(&self.file)
    }
}

// ---------------------------------------------------------------------------------------
// Posture checks
// ---------------------------------------------------------------------------------------

/// `ok` passes, `warn` wants attention, `idle` is available but not set up, `off` is a
/// protection Castle does not have yet,
/// `unknown` could not be determined on this machine.
#[derive(Serialize, Clone, Debug)]
pub struct Check {
    pub id: &'static str,
    pub label: &'static str,
    pub state: &'static str,
    pub detail: String,
}

fn check(id: &'static str, label: &'static str, state: &'static str, detail: impl Into<String>) -> Check {
    Check { id, label, state, detail: detail.into() }
}

/// Whether the disk holding the vault is encrypted by the operating system.
#[cfg(windows)]
fn disk_encryption(root: &Path) -> Check {
    const LABEL: &str = "Disk encryption";
    let shown = root.display().to_string();
    let drive: String = shown.trim_start_matches(r"\\?\").chars().take(2).collect();
    if drive.len() != 2 || !drive.ends_with(':') || !drive.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return check("disk", LABEL, "unknown", "The vault is not on a lettered drive, so BitLocker could not be read.");
    }
    let script = format!("(New-Object -ComObject Shell.Application).NameSpace('{drive}').Self.ExtendedProperty('System.Volume.BitLockerProtection')");
    let code = quiet("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .ok()
        .and_then(|out| String::from_utf8_lossy(&out.stdout).trim().parse::<u32>().ok());
    // Values of System.Volume.BitLockerProtection: 1 on, 3 encrypting, 5 and 6 suspended or locked.
    match code {
        Some(1) => check("disk", LABEL, "ok", format!("BitLocker is on for {drive}")),
        Some(3) => check("disk", LABEL, "warn", format!("BitLocker is still encrypting {drive}")),
        Some(5 | 6) => check("disk", LABEL, "warn", format!("BitLocker on {drive} is suspended or locked")),
        Some(_) => check("disk", LABEL, "warn", format!("{drive} is not encrypted. Anyone holding the disk can read the vault.")),
        None => check("disk", LABEL, "unknown", "Could not read BitLocker's state."),
    }
}

#[cfg(not(windows))]
fn disk_encryption(root: &Path) -> Check {
    const LABEL: &str = "Disk encryption";
    let run = |program: &str, args: &[&str]| {
        quiet(program).args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let Some(source) = run("findmnt", &["-no", "SOURCE", "-T", &root.display().to_string()]) else {
        return check("disk", LABEL, "unknown", "Could not find which disk holds the vault.");
    };
    // Every layer under the vault's filesystem, e.g. "lvm crypt part disk".
    match run("lsblk", &["-nso", "TYPE", &source]) {
        Some(layers) if layers.lines().any(|layer| layer.trim() == "crypt") => check("disk", LABEL, "ok", format!("{source} sits on an encrypted volume")),
        Some(_) => check("disk", LABEL, "warn", format!("{source} is not encrypted. Anyone holding the disk can read the vault.")),
        None => check("disk", LABEL, "unknown", format!("Could not read how {source} is built.")),
    }
}

#[cfg(unix)]
fn token_file(root: &Path) -> Check {
    use std::os::unix::fs::PermissionsExt;
    const LABEL: &str = "Token file";
    match std::fs::metadata(root.join(".castle").join("token")) {
        Ok(meta) if meta.permissions().mode() & 0o077 == 0 => check("token-file", LABEL, "ok", "Only the box's own account can read it."),
        Ok(_) => check("token-file", LABEL, "warn", "Other accounts on this machine can read the token."),
        Err(_) => check("token-file", LABEL, "unknown", "The token file could not be read."),
    }
}

#[cfg(not(unix))]
fn token_file(_root: &Path) -> Check {
    check("token-file", "Token file", "unknown", "On Windows the token file takes its folder's permissions; they are not checked yet.")
}

/// What the box can say about its own exposure right now.
pub struct Posture<'a> {
    pub root: &'a Path,
    pub listen: &'a str,
    pub loopback: bool,
    pub private_link: Option<String>,
}

pub fn checks(security: &Security, posture: &Posture) -> Vec<Check> {
    let verdict = security.verify();
    let denied = security.denied_last_day();
    vec![
        if posture.loopback {
            check("listen", "Who can reach the box", "ok", format!("Only this computer ({})", posture.listen))
        } else {
            check("listen", "Who can reach the box", "warn", format!("Any machine on the network, over plain HTTP ({})", posture.listen))
        },
        match &posture.private_link {
            Some(name) => check("link", "Private link", "ok", format!("{name} is connected")),
            None => check("link", "Private link", "idle", "No Tailscale or WireGuard link is up, so there is no private way in from outside."),
        },
        if verdict.intact {
            check("record", "Activity record", "ok", format!("{} entries, every one follows from the one before", verdict.entries))
        } else {
            check("record", "Activity record", "warn", format!("Broken at entry {}: it was changed or damaged", verdict.broken_at.unwrap_or(0)))
        },
        if denied == 0 {
            check("denied", "Refused calls", "ok", "No call with a wrong token in the last 24 hours")
        } else {
            check("denied", "Refused calls", "warn", format!("{denied} calls with a wrong or missing token in the last 24 hours"))
        },
        token_file(posture.root),
        disk_encryption(posture.root),
        check("accounts", "Accounts", "off", "One token opens everything. Per-person accounts are not built yet."),
        check("vault-encryption", "Vault encryption", "off", "Castle does not encrypt files itself yet; it relies on the disk's encryption."),
        check("witness", "Record held elsewhere", "off", "The record's newest hash is not copied to another device yet, so a full rewrite would go unnoticed."),
    ]
}

// ---------------------------------------------------------------------------------------
// Audits: read-only bash scripts, compiled in
// ---------------------------------------------------------------------------------------

struct AuditScript {
    id: &'static str,
    label: &'static str,
    about: &'static str,
    script: &'static str,
}

macro_rules! audit {
    ($id:literal, $label:literal, $about:literal) => {
        AuditScript { id: $id, label: $label, about: $about, script: include_str!(concat!("../../security/audits/", $id, ".sh")) }
    };
}

const AUDITS: [AuditScript; 10] = [
    audit!("users", "Accounts", "Every account on the box, its groups, and whether it can log in"),
    audit!("listening", "Open ports", "What is listening for connections, and whether other machines can reach it"),
    audit!("connections", "Connections", "Established connections, split into the outside world and the local network"),
    audit!("services", "Running services", "Services running right now"),
    audit!("startup", "Starts with the box", "Everything set to start on its own: enabled services, cron, autostart entries"),
    audit!("tasks", "Scheduled tasks", "Timers and cron jobs, and what each runs"),
    audit!("setuid", "Programs that run as root", "Programs any account can run with root's rights"),
    audit!("modules", "Kernel modules", "Drivers and other code loaded into the kernel"),
    audit!("hosts", "Hosts file", "Names this box resolves without asking DNS"),
    audit!("path", "Program search path", "Folders searched for programs, and whether other accounts can write to them"),
];

#[derive(Serialize)]
pub struct AuditInfo {
    id: &'static str,
    label: &'static str,
    about: &'static str,
}

#[derive(Serialize)]
pub struct AuditList {
    /// False where audits cannot run (not Linux, or no bash).
    available: bool,
    reason: &'static str,
    audits: Vec<AuditInfo>,
}

#[derive(Serialize)]
pub struct AuditReport {
    id: &'static str,
    at: u64,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    /// True when the box runs without root, so some audits see less.
    limited: bool,
}

fn audits_unavailable() -> Option<&'static str> {
    if !cfg!(target_os = "linux") {
        return Some("Audits are bash scripts for a Linux box. This box is not running Linux.");
    }
    let has_bash = quiet("bash").arg("--version").stdout(Stdio::null()).status().is_ok_and(|s| s.success());
    (!has_bash).then_some("bash is not installed on this box.")
}

pub fn audit_list() -> AuditList {
    let reason = audits_unavailable();
    AuditList {
        available: reason.is_none(),
        reason: reason.unwrap_or(""),
        audits: AUDITS.iter().map(|a| AuditInfo { id: a.id, label: a.label, about: a.about }).collect(),
    }
}

/// Splits an audit's output: a header line of column names, then one row per line, tabs between cells.
fn parse_table(output: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let mut lines = output.lines().filter(|line| !line.trim().is_empty());
    let split = |line: &str| line.split('\t').map(|cell| clean(cell.trim())).collect::<Vec<_>>();
    let columns = lines.next().map(split).unwrap_or_default();
    let rows = lines.take(MAX_AUDIT_ROWS).map(split).collect();
    (columns, rows)
}

pub fn run_audit(id: &str) -> SecurityResult<AuditReport> {
    let audit = AUDITS.iter().find(|a| a.id == id).ok_or("no such audit")?;
    if let Some(reason) = audits_unavailable() {
        return Err(reason.into());
    }
    let mut child = quiet("bash")
        .arg("-s")
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|_| "could not start bash".to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        // Windows checkouts may carry CRLF; bash needs LF.
        let _ = stdin.write_all(audit.script.replace("\r\n", "\n").as_bytes());
    }
    let mut stdout = child.stdout.take().ok_or("could not read the audit")?;
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = std::io::Read::read_to_string(&mut stdout, &mut text);
        text
    });
    let deadline = std::time::Instant::now() + AUDIT_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("the audit took too long and was stopped".into());
            }
            Ok(None) => std::thread::sleep(AUDIT_POLL),
            Err(_) => return Err("the audit could not be watched".into()),
        }
    }
    let output = reader.join().map_err(|_| "could not read the audit".to_string())?;
    let (columns, rows) = parse_table(&output);
    if columns.is_empty() {
        return Err("the audit produced nothing".into());
    }
    let limited = quiet("id").arg("-u").output().map(|o| String::from_utf8_lossy(&o.stdout).trim() != "0").unwrap_or(true);
    Ok(AuditReport { id: audit.id, at: now_ms(), columns, rows, limited })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("castled-security-{name}-{}", now_ms()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn record_chains_and_survives_a_restart() {
        let dir = temp_dir("chain");
        let security = Security::new(dir.clone()).unwrap();
        security.record("box.start", "");
        security.record("trash", "200 id=notes.txt");
        let verdict = security.verify();
        assert!(verdict.intact);
        assert_eq!(verdict.entries, 2);

        let reopened = Security::new(dir.clone()).unwrap();
        reopened.record("delete", "204 id=trash:abc");
        let verdict = reopened.verify();
        assert!(verdict.intact, "entries after a restart must continue the same chain");
        assert_eq!(verdict.entries, 3);
        assert_eq!(reopened.entries(2)[0].action, "delete");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_edited_entry_is_caught() {
        let dir = temp_dir("edit");
        let security = Security::new(dir.clone()).unwrap();
        for name in ["a", "b", "c"] {
            security.record("trash", name);
        }
        let file = dir.join(LOG_FILE);
        let edited = std::fs::read_to_string(&file).unwrap().replacen("\"detail\":\"b\"", "\"detail\":\"x\"", 1);
        std::fs::write(&file, edited).unwrap();
        let verdict = security.verify();
        assert!(!verdict.intact);
        assert_eq!(verdict.broken_at, Some(2));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_removed_entry_is_caught() {
        let dir = temp_dir("remove");
        let security = Security::new(dir.clone()).unwrap();
        for name in ["a", "b", "c"] {
            security.record("trash", name);
        }
        let file = dir.join(LOG_FILE);
        let text = std::fs::read_to_string(&file).unwrap();
        let kept: Vec<&str> = text.lines().enumerate().filter(|(i, _)| *i != 1).map(|(_, line)| line).collect();
        std::fs::write(&file, kept.join("\n")).unwrap();
        assert_eq!(security.verify().broken_at, Some(2));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn detail_is_kept_to_one_bounded_line() {
        assert_eq!(clean("a\nb\tc"), "abc");
        assert_eq!(clean(&"x".repeat(1000)).len(), MAX_DETAIL_CHARS);
    }

    #[test]
    fn denied_calls_are_counted_but_logged_sparingly() {
        let dir = temp_dir("denied");
        let security = Security::new(dir.clone()).unwrap();
        for _ in 0..50 {
            security.note_denied("/list");
        }
        assert_eq!(security.denied_last_day(), 50);
        assert_eq!(security.verify().entries, 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn audit_tables_split_on_tabs() {
        let (columns, rows) = parse_table("USER\tUID\nroot\t0\n\nalex\t1000\n");
        assert_eq!(columns, ["USER", "UID"]);
        assert_eq!(rows, [["root", "0"], ["alex", "1000"]]);
    }

    #[test]
    fn every_audit_has_a_script_with_a_header() {
        for audit in &AUDITS {
            assert!(audit.script.starts_with("#!/usr/bin/env bash"), "{} needs a bash shebang", audit.id);
            assert!(audit.script.contains("printf"), "{} must print a table", audit.id);
        }
        assert!(run_audit("nope").is_err());
    }
}
