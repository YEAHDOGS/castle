//! AI is DOGS AI: local image, video, audio and chat generation, with its own app. Its code
//! lives in this repository under `ai/` (imported from the dogs-ai project), and the box runs
//! its server, which also serves its app. Castle's AI section shows that app as it is.
//!
//! Which copy runs: CASTLE_AI_DIR when set; otherwise Castle's own `ai/` folder, or a
//! `dogs-ai` checkout beside Castle. A copy that already has its runtime (ComfyUI) installed
//! is preferred, so models downloaded before are there.
//!
//! The server listens on this computer only unless the box itself is reachable from other
//! machines; then it gets a key of its own (DOGS_API_KEY), kept in the vault's private folder,
//! because the dogs server refuses the network without one.

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::Duration,
};

use serde::Serialize;

/// Castle's own API has 8787, the dogs server's usual port, so DOGS AI moves up: to the first
/// of these ports that is free, or the one where a dogs server already answers.
pub const DEFAULT_PORT: u16 = 8788;
const PORTS_TRIED: u16 = 12;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const PROBE_TIMEOUT: Duration = Duration::from_millis(400);
const LOG_TAIL_BYTES: usize = 4000;

fn looks_like_dogs(dir: &Path) -> bool {
    dir.join("server").join("bin").join("dogs.mjs").is_file() && dir.join("stack.json").is_file()
}

/// Castle's copy keeps its runtime in `data/ai` beside it (app/data/ai, git-ignored), or in
/// DOGS_AI_DATA; a dogs-ai checkout of its own keeps it inside.
fn has_runtime(dir: &Path) -> bool {
    let data = match std::env::var_os("DOGS_AI_DATA") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => dir.parent().unwrap_or(dir).join("data").join("ai"),
    };
    [data.join("runtime"), dir.join("runtime")].iter().any(|r| r.join("ComfyUI").is_dir())
}

/// Every place a copy of DOGS AI may be, in order.
fn candidates() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::env::var_os("CASTLE_AI_DIR").map(PathBuf::from).into_iter().collect();
    for start in [std::env::current_exe().ok(), std::env::current_dir().ok()].into_iter().flatten() {
        for dir in start.ancestors() {
            found.push(dir.join("ai"));
            found.push(dir.join("dogs-ai"));
        }
    }
    found
}

/// The copy to run: CASTLE_AI_DIR if it is one, else the first with its runtime installed,
/// else the first there is.
pub fn find() -> Option<PathBuf> {
    let all: Vec<PathBuf> = candidates().into_iter().filter(|d| looks_like_dogs(d)).collect();
    if let Some(chosen) = std::env::var_os("CASTLE_AI_DIR").map(PathBuf::from).filter(|d| looks_like_dogs(d)) {
        return Some(chosen);
    }
    all.iter().find(|d| has_runtime(d)).or(all.first()).map(|d| crate::plain_path(&d.canonicalize().unwrap_or_else(|_| d.clone())))
}

/// Node.js, which the dogs server needs: CASTLE_NODE, then the system's path.
fn node() -> Option<PathBuf> {
    if let Some(node) = std::env::var_os("CASTLE_NODE").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(node);
    }
    let name = if cfg!(windows) { "node.exe" } else { "node" };
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(name)).find(|p| p.is_file()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// The dogs server answers.
    pub running: bool,
    /// Castle started it (rather than finding one already running).
    pub ours: bool,
    pub port: u16,
    pub dir: Option<String>,
    /// Its app has been built, so the server can show it.
    pub built: bool,
    /// Its runtime (ComfyUI) is installed, for making images and video.
    pub installed: bool,
    pub node: bool,
    /// The key the app needs when the box is reached over the network; empty on this computer.
    pub key: String,
    pub log: String,
}

pub struct Ai {
    child: Mutex<Option<Child>>,
    /// The first port to try, and the one in use once a server is running.
    first_port: u16,
    port: Mutex<u16>,
    host: String,
    key: String,
    origins: String,
    log: PathBuf,
}

impl Ai {
    /// `listen` is where the box's own API listens; `key_file` holds the network key.
    pub fn new(listen: SocketAddr, private: &Path) -> Self {
        let port = std::env::var("CASTLE_AI_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(DEFAULT_PORT);
        let loopback = listen.ip().is_loopback();
        let host = if loopback { "127.0.0.1".to_string() } else { listen.ip().to_string() };
        let key = if loopback {
            String::new()
        } else {
            let file = private.join("ai-key");
            std::fs::read_to_string(&file).ok().map(|k| k.trim().to_string()).filter(|k| k.len() >= 32).unwrap_or_else(|| {
                let key = crate::random_hex(24);
                let _ = std::fs::write(&file, &key);
                key
            })
        };
        // Pages that may call the dogs API from elsewhere: Castle's app wherever it runs.
        let mut origins: Vec<String> = crate::APP_ORIGINS.iter().map(|o| o.to_string()).collect();
        origins.push(format!("http://{listen}"));
        Self { child: Mutex::new(None), first_port: port, port: Mutex::new(port), host, key, origins: origins.join(","), log: private.join("ai.log") }
    }

    fn probe_host(&self) -> &str {
        if self.host == "0.0.0.0" { "127.0.0.1" } else { &self.host }
    }

    /// Is the dogs server on `port`? Asked properly: another program on the port is not it.
    fn dogs_on(&self, port: u16) -> bool {
        let Ok(addr) = format!("{}:{port}", self.probe_host()).parse::<SocketAddr>() else { return false };
        let Ok(mut stream) = TcpStream::connect_timeout(&addr, PROBE_TIMEOUT) else { return false };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(1500)));
        if write!(stream, "GET /api HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n").is_err() {
            return false;
        }
        let mut answer = Vec::new();
        let _ = stream.take(16 * 1024).read_to_end(&mut answer);
        let text = String::from_utf8_lossy(&answer);
        // Its index names it; with a key set it refuses, but says which key it wants.
        text.contains("\"dogs-ai\"") || text.contains("DOGS_API_KEY")
    }

    fn port_free(&self, port: u16) -> bool {
        std::net::TcpListener::bind((self.probe_host(), port)).is_ok()
    }

    fn port(&self) -> u16 {
        self.port.lock().map(|p| *p).unwrap_or(self.first_port)
    }

    fn answering(&self) -> bool {
        self.dogs_on(self.port())
    }

    fn ours_alive(&self) -> bool {
        let Ok(mut child) = self.child.lock() else { return false };
        match child.as_mut().map(|c| c.try_wait()) {
            Some(Ok(None)) => true,
            Some(_) => {
                *child = None;
                false
            }
            None => false,
        }
    }

    pub fn status(&self) -> Status {
        let dir = find();
        let log = std::fs::read(&self.log).ok().map(|b| {
            let start = b.len().saturating_sub(LOG_TAIL_BYTES);
            String::from_utf8_lossy(&b[start..]).to_string()
        });
        Status {
            running: self.answering(),
            ours: self.ours_alive(),
            port: self.port(),
            built: dir.as_ref().is_some_and(|d| d.join("app").join("dist").join("index.html").is_file()),
            installed: dir.as_ref().is_some_and(|d| has_runtime(d)),
            dir: dir.map(|d| d.to_string_lossy().to_string()),
            node: node().is_some(),
            key: self.key.clone(),
            log: log.unwrap_or_default(),
        }
    }

    /// Starts the dogs server, unless one already answers on its port.
    pub fn start(&self) -> Result<(), String> {
        if self.ours_alive() || self.answering() {
            return Ok(());
        }
        // One already running on a nearby port is used as it is; otherwise the first free port.
        let ports = self.first_port..self.first_port.saturating_add(PORTS_TRIED);
        if let Some(port) = ports.clone().find(|p| self.dogs_on(*p)) {
            *self.port.lock().map_err(|_| "AI is unavailable".to_string())? = port;
            return Ok(());
        }
        let port = ports.into_iter().find(|p| self.port_free(*p)).ok_or("Every port DOGS AI could use is taken.")?;
        *self.port.lock().map_err(|_| "AI is unavailable".to_string())? = port;
        let dir = find().ok_or("DOGS AI was not found. Castle's copy is the ai folder of the Castle checkout.")?;
        let node = node().ok_or("DOGS AI needs Node.js 22 or newer. Install it from nodejs.org.")?;
        let mut log = std::fs::OpenOptions::new().create(true).append(true).open(&self.log).map_err(|_| "The AI log could not be opened.".to_string())?;
        let _ = writeln!(log, "---- starting dogs server from {} on {}:{port}", dir.display(), self.host);
        let mut cmd = Command::new(node);
        cmd.arg(dir.join("server").join("bin").join("dogs.mjs"))
            .arg("serve")
            .current_dir(&dir)
            .env("DOGS_PORT", port.to_string())
            .env("DOGS_HOST", &self.host)
            .env("DOGS_ORIGINS", &self.origins)
            .stdin(Stdio::null())
            .stdout(log.try_clone().map(Stdio::from).unwrap_or_else(|_| Stdio::null()))
            .stderr(Stdio::from(log));
        if !self.key.is_empty() {
            cmd.env("DOGS_API_KEY", &self.key);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let child = cmd.spawn().map_err(|e| format!("The DOGS AI server did not start: {e}"))?;
        *self.child.lock().map_err(|_| "AI is unavailable".to_string())? = Some(child);
        Ok(())
    }

    /// Stops the dogs server if Castle started it. One started some other way is left alone.
    pub fn stop(&self) {
        if let Ok(mut child) = self.child.lock() {
            if let Some(mut c) = child.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dogs_folder_is_recognised() {
        let dir = std::env::temp_dir().join(format!("castled-ai-{}", crate::random_hex(6)));
        assert!(!looks_like_dogs(&dir));
        std::fs::create_dir_all(dir.join("server").join("bin")).unwrap();
        std::fs::write(dir.join("server").join("bin").join("dogs.mjs"), b"").unwrap();
        std::fs::write(dir.join("stack.json"), b"{}").unwrap();
        assert!(looks_like_dogs(&dir));
        assert!(!has_runtime(&dir));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn another_program_on_the_port_is_not_taken_for_dogs() {
        use std::io::{BufRead, BufReader};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for (i, stream) in listener.incoming().flatten().enumerate() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                    line.clear();
                }
                let body = if i == 0 { "<title>some other app</title>" } else { "{\"name\":\"dogs-ai\",\"endpoints\":[]}" };
                let mut stream = stream;
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len());
            }
        });
        let dir = std::env::temp_dir().join(format!("castled-ai-{}", crate::random_hex(6)));
        std::fs::create_dir_all(&dir).unwrap();
        let ai = Ai::new("127.0.0.1:8787".parse().unwrap(), &dir);
        assert!(!ai.dogs_on(port), "a page that is not dogs");
        assert!(ai.dogs_on(port), "the dogs index");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn castles_own_copy_is_found_from_a_checkout() {
        // Tests run in castled/, so the checkout's ai/ folder is one level up.
        assert!(find().is_some(), "the ai folder of this checkout");
    }

    #[test]
    fn the_network_gets_a_key_and_this_computer_does_not() {
        let dir = std::env::temp_dir().join(format!("castled-ai-{}", crate::random_hex(6)));
        std::fs::create_dir_all(&dir).unwrap();
        let local = Ai::new("127.0.0.1:8787".parse().unwrap(), &dir);
        assert!(local.key.is_empty());
        assert_eq!(local.host, "127.0.0.1");
        let lan = Ai::new("192.168.1.5:8787".parse().unwrap(), &dir);
        assert_eq!(lan.key.len(), 48);
        assert_eq!(Ai::new("192.168.1.5:8787".parse().unwrap(), &dir).key, lan.key, "the key is kept");
        assert!(lan.origins.contains("http://192.168.1.5:8787"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
