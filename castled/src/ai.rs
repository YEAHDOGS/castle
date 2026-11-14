//! AI is DOGS AI: local image, video, audio and chat generation, with its own app. It is its
//! own project (github.com/YEAHDOGS/dogs-ai), imported here as the git submodule `app/ai/`, and
//! it is also a standalone app. The box runs its server, which also serves its app; Castle's AI
//! section shows that app as it is.
//!
//! Every copy of DOGS AI on a computer shares one data folder (models, history, chats; see the
//! dogs-ai README) and one engine: one that is already running, started by the standalone app
//! or the CLI, is used as it is (DOGS AI records it in server.json in that folder). Castle
//! passes the Ollama address from its settings (ai.ollamaUrl), which DOGS AI keeps for every copy.
//!
//! Which copy runs: CASTLE_AI_DIR when set; otherwise Castle's own `app/ai/`, or else a
//! `dogs-ai` checkout beside Castle. Other copies that still keep their data inside their own
//! folder are named in DOGS_AI_LEGACY, so DOGS AI moves that data into the shared folder.
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
/// The dogs server waits up to 10 s for ComfyUI to go before it answers a stop.
const COMFY_STOP_TIMEOUT: Duration = Duration::from_secs(12);

fn looks_like_dogs(dir: &Path) -> bool {
    dir.join("server").join("bin").join("dogs.mjs").is_file() && dir.join("stack.json").is_file()
}

/// The data folder every copy of DOGS AI shares, as DOGS AI decides it (server/src/data-dir.mjs):
/// DOGS_AI_DATA, else %LOCALAPPDATA%\DOGS AI, ~/Library/Application Support/DOGS AI or
/// $XDG_DATA_HOME/dogs-ai.
pub fn data_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(dir) = var("DOGS_AI_DATA") {
        return Some(dir);
    }
    let home = var(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    if cfg!(windows) {
        return var("LOCALAPPDATA").or_else(|| home.map(|h| h.join("AppData").join("Local"))).map(|d| d.join("DOGS AI"));
    }
    if cfg!(target_os = "macos") {
        return home.map(|h| h.join("Library").join("Application Support").join("DOGS AI"));
    }
    var("XDG_DATA_HOME").or_else(|| home.map(|h| h.join(".local").join("share"))).map(|d| d.join("dogs-ai"))
}

/// A folder with something in it (a lone .gitkeep does not count).
fn has_content(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut entries| entries.any(|e| e.is_ok_and(|e| e.file_name() != ".gitkeep")))
}

/// ComfyUI is installed: in the shared data folder, or still inside an older copy's own folder.
fn has_runtime(dir: &Path) -> bool {
    runtime_in(dir, data_dir().as_deref())
}

fn runtime_in(dir: &Path, data: Option<&Path>) -> bool {
    data.into_iter().chain([dir]).any(|d| d.join("runtime").join("ComfyUI").is_dir())
}

/// Copies other than `chosen` that still keep runtime/, models/ or outputs/ inside their own
/// folder, from before DOGS AI shared one data folder. DOGS AI moves that data across.
fn older_copies(chosen: &Path) -> Vec<PathBuf> {
    let chosen = chosen.canonicalize().unwrap_or_else(|_| chosen.to_path_buf());
    let mut found: Vec<PathBuf> = Vec::new();
    for dir in candidates().into_iter().filter(|d| looks_like_dogs(d)) {
        let dir = dir.canonicalize().unwrap_or(dir);
        if dir != chosen && !found.contains(&dir) && ["runtime", "models", "outputs"].iter().any(|n| has_content(&dir.join(n))) {
            found.push(dir);
        }
    }
    found.into_iter().map(|d| crate::plain_path(&d)).collect()
}

/// The port of the engine that is running, from server.json in the shared data folder.
fn recorded_port() -> Option<u16> {
    let text = std::fs::read_to_string(data_dir()?.join("server.json")).ok()?;
    let found: serde_json::Value = serde_json::from_str(&text).ok()?;
    found["port"].as_u64().and_then(|p| u16::try_from(p).ok())
}

/// Every place a copy of DOGS AI may be, in order.
fn candidates() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::env::var_os("CASTLE_AI_DIR").map(PathBuf::from).into_iter().collect();
    for start in [std::env::current_exe().ok(), std::env::current_dir().ok()].into_iter().flatten() {
        for dir in start.ancestors() {
            found.push(dir.join("ai"));
            found.push(dir.join("app").join("ai"));
            found.push(dir.join("dogs-ai"));
        }
    }
    found
}

/// The copy to run: CASTLE_AI_DIR if it is one, else the first there is (Castle's own app/ai
/// before a checkout beside it). Every copy uses the same data, so none is preferred for it.
pub fn find() -> Option<PathBuf> {
    candidates().into_iter().find(|d| looks_like_dogs(d)).map(|d| crate::plain_path(&d.canonicalize().unwrap_or(d)))
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

    /// Starts the dogs server, unless one is already running: Castle's, or one the standalone app
    /// or the CLI started (recorded in the shared data folder, or on a nearby port). `ollama` is
    /// the Ollama address from Castle's settings, which DOGS AI keeps for every copy.
    pub fn start(&self, ollama: Option<&str>) -> Result<(), String> {
        if self.ours_alive() || self.answering() {
            self.set_ollama(ollama);
            return Ok(());
        }
        // One already running is used as it is; otherwise the first free port.
        let ports = self.first_port..self.first_port.saturating_add(PORTS_TRIED);
        if let Some(port) = recorded_port().into_iter().chain(ports.clone()).find(|p| self.dogs_on(*p)) {
            *self.port.lock().map_err(|_| "AI is unavailable".to_string())? = port;
            self.set_ollama(ollama);
            return Ok(());
        }
        let port = ports.into_iter().find(|p| self.port_free(*p)).ok_or("Every port DOGS AI could use is taken.")?;
        *self.port.lock().map_err(|_| "AI is unavailable".to_string())? = port;
        let dir = find().ok_or("DOGS AI was not found. Castle's copy is the ai folder of the Castle checkout.")?;
        let node = node().ok_or("DOGS AI needs Node.js 22 or newer. Install it from nodejs.org.")?;
        let mut log = std::fs::OpenOptions::new().create(true).append(true).open(&self.log).map_err(|_| "The AI log could not be opened.".to_string())?;
        let _ = writeln!(log, "---- starting dogs server from {} on {}:{port}", dir.display(), self.host);
        let older = older_copies(&dir);
        let older = (!older.is_empty()).then(|| std::env::join_paths(&older).ok()).flatten();
        if let Some(joined) = &older {
            let _ = writeln!(log, "---- older copies whose data DOGS AI moves into its shared folder: {}", joined.to_string_lossy());
        }
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
        if let Some(url) = ollama.map(str::trim).filter(|u| !u.is_empty()) {
            cmd.env("DOGS_AI_OLLAMA", url);
        }
        if let Some(joined) = older {
            cmd.env("DOGS_AI_LEGACY", joined);
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

    /// Stops the dogs server if Castle started it, and the ComfyUI it started, which would
    /// otherwise outlive it. One started some other way is left alone.
    pub fn stop(&self) {
        if !self.ours_alive() {
            return;
        }
        self.stop_comfy();
        if let Ok(mut child) = self.child.lock() {
            if let Some(mut c) = child.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }

    /// Gives the running engine the Ollama address from Castle's settings (its POST
    /// /api/connections), which it keeps for every copy of DOGS AI. Nothing when there is none.
    pub fn set_ollama(&self, ollama: Option<&str>) {
        let Some(url) = ollama.map(str::trim).filter(|u| !u.is_empty()) else { return };
        let body = serde_json::json!({ "ollama": url }).to_string();
        let _ = self.post("/api/connections", &body, PROBE_TIMEOUT * 5);
    }

    /// One POST with a JSON body to the dogs server; true when it answered 2xx.
    fn post(&self, path: &str, body: &str, wait: Duration) -> bool {
        let Ok(addr) = format!("{}:{}", self.probe_host(), self.port()).parse::<SocketAddr>() else { return false };
        let Ok(mut stream) = TcpStream::connect_timeout(&addr, PROBE_TIMEOUT) else { return false };
        let _ = stream.set_read_timeout(Some(wait));
        let auth = if self.key.is_empty() { String::new() } else { format!("Authorization: Bearer {}\r\n", self.key) };
        let request = format!(
            "POST {path} HTTP/1.1\r\nHost: {addr}\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        if stream.write_all(request.as_bytes()).is_err() {
            return false;
        }
        let mut answer = Vec::new();
        let _ = stream.take(4096).read_to_end(&mut answer);
        answer.starts_with(b"HTTP/1.1 2")
    }

    /// Asks the dogs server to stop ComfyUI (its own POST /api/comfy/stop), and waits for it.
    fn stop_comfy(&self) {
        let Ok(addr) = format!("{}:{}", self.probe_host(), self.port()).parse::<SocketAddr>() else { return };
        let Ok(mut stream) = TcpStream::connect_timeout(&addr, PROBE_TIMEOUT) else { return };
        let _ = stream.set_read_timeout(Some(COMFY_STOP_TIMEOUT));
        let auth = if self.key.is_empty() { String::new() } else { format!("Authorization: Bearer {}\r\n", self.key) };
        let request = format!("POST /api/comfy/stop HTTP/1.1\r\nHost: {addr}\r\n{auth}Content-Length: 0\r\nConnection: close\r\n\r\n");
        if stream.write_all(request.as_bytes()).is_err() {
            return;
        }
        let mut answer = Vec::new();
        let _ = stream.take(4096).read_to_end(&mut answer);
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
        assert!(!runtime_in(&dir, None));
        // An older copy keeps it inside; the shared folder has it once DOGS AI moved it there.
        std::fs::create_dir_all(dir.join("runtime").join("ComfyUI")).unwrap();
        assert!(runtime_in(&dir, None));
        assert!(has_content(&dir.join("runtime")));
        std::fs::create_dir_all(dir.join("models")).unwrap();
        std::fs::write(dir.join("models").join(".gitkeep"), b"").unwrap();
        assert!(!has_content(&dir.join("models")));
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
