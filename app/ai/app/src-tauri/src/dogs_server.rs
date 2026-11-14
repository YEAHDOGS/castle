// Runs the local dogs engine (server/bin/dogs.mjs serve) for the desktop app, so nobody has to start a server:
// it starts when the app opens and is restarted if it ever stops. It runs on the Node that ships in the dogs
// CLI archive (<home>/node/), else the Node on PATH.
use std::{
    env,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const DEFAULT_PORT: u16 = 8787;
const HEALTH_TIMEOUT: Duration = Duration::from_millis(1500);
const START_TIMEOUT: Duration = Duration::from_secs(20);
const START_POLL: Duration = Duration::from_millis(300);
const WATCH: Duration = Duration::from_secs(5);

#[derive(Clone, Default)]
pub struct Engine {
    child: Arc<Mutex<Option<Child>>>,
    stopped: Arc<AtomicBool>,
}

impl Engine {
    /// Start the engine if needed, then keep it running for as long as the app is open.
    pub fn spawn(&self) {
        let engine = self.clone();
        thread::spawn(move || {
            while !engine.stopped.load(Ordering::SeqCst) {
                if !up(port()) {
                    engine.start();
                }
                thread::sleep(WATCH);
            }
        });
    }

    /// Stop watching, and stop the engine if this app started it.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn start(&self) {
        let Some(home) = dogs_home() else {
            eprintln!("[dogs] project folder not found; set DOGS_HOME or start `dogs serve` yourself");
            return;
        };
        let mut cmd = Command::new(node(&home));
        cmd.arg(home.join("server").join("bin").join("dogs.mjs"))
            .arg("serve")
            .current_dir(&home)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        // Windows can refuse a spawn (low memory, security software): log it and let the watcher try again.
        let child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                eprintln!("[dogs] could not start the engine, retrying: {e}");
                return;
            }
        };
        #[cfg(windows)]
        kill_with_app::tie(&child);
        *self.child.lock().unwrap() = Some(child);

        let t0 = Instant::now();
        while t0.elapsed() < START_TIMEOUT && !self.stopped.load(Ordering::SeqCst) && !up(port()) {
            // It exited (port taken, bad Node): stop waiting, the watcher retries.
            let exited = match self.child.lock().unwrap().as_mut() {
                Some(child) => !matches!(child.try_wait(), Ok(None)),
                None => true,
            };
            if exited {
                self.child.lock().unwrap().take();
                return;
            }
            thread::sleep(START_POLL);
        }
    }
}

fn port() -> u16 {
    env::var("DOGS_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(DEFAULT_PORT)
}

/// GET /api/health answers 2xx. Plain HTTP over a socket: one request does not need an HTTP client.
fn up(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut s) = TcpStream::connect_timeout(&addr, HEALTH_TIMEOUT) else {
        return false;
    };
    let _ = s.set_read_timeout(Some(HEALTH_TIMEOUT));
    let _ = s.set_write_timeout(Some(HEALTH_TIMEOUT));
    let req = format!("GET /api/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    if s.write_all(req.as_bytes()).is_err() {
        return false;
    }
    // "HTTP/1.1 200" is 12 bytes.
    let mut head = Vec::with_capacity(16);
    let mut buf = [0u8; 16];
    while head.len() < 12 {
        match s.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => head.extend_from_slice(&buf[..n]),
        }
    }
    head.len() >= 12 && head.starts_with(b"HTTP/1.") && head[9] == b'2'
}

fn has_dogs(dir: &Path) -> bool {
    dir.join("server").join("bin").join("dogs.mjs").is_file()
}

fn dogs_home() -> Option<PathBuf> {
    let user_home = env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    [
        env::var_os("DOGS_HOME").map(PathBuf::from),
        // Recorded by build.rs; release (CI) builds record nothing.
        option_env!("DOGS_BUILD_HOME").map(PathBuf::from),
        // Release builds expect the dogs CLI archive (or a clone) extracted to ~/dogs-ai.
        user_home.map(|h| h.join("dogs-ai")),
    ]
    .into_iter()
    .flatten()
    .find(|d| has_dogs(d))
}

/// The CLI archive's bundled Node, else Node from PATH. Apps opened from the macOS Finder get a bare PATH, so
/// the usual Homebrew / installer locations are tried too.
fn node(home: &Path) -> PathBuf {
    let exe = if cfg!(windows) { "node.exe" } else { "node" };
    let bundled = home.join("node").join(exe);
    if bundled.is_file() {
        return bundled;
    }
    let on_path = env::var_os("PATH")
        .map(|p| env::split_paths(&p).map(|d| d.join(exe)).collect::<Vec<_>>())
        .unwrap_or_default();
    let usual = ["/opt/homebrew/bin/node", "/usr/local/bin/node", "/usr/bin/node"].map(PathBuf::from);
    on_path
        .into_iter()
        .chain(if cfg!(windows) { vec![] } else { usual.to_vec() })
        .find(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from(exe))
}

/// Windows does not end a process's children with it, and `stop` only runs on a clean exit. A killed or crashed app
/// (or `tauri dev` restarting it) would leave the engine running old code, and the next app would reuse it. A job
/// object closes when the app's process ends, however it ends, and takes the engine with it. Silent breakaway keeps
/// what the engine starts in turn (ComfyUI) out of the job, so ComfyUI outlives the app as before.
#[cfg(windows)]
mod kill_with_app {
    use std::{os::windows::io::AsRawHandle, process::Child, sync::OnceLock};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK,
    };

    // The handle as usize (raw handles are not Send). Never closed: closing it is what kills the engine.
    static JOB: OnceLock<usize> = OnceLock::new();

    pub fn tie(child: &Child) {
        let job = *JOB.get_or_init(|| unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return 0;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK;
            let size = std::mem::size_of_val(&info) as u32;
            SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info as *const _ as *const _, size);
            job as usize
        });
        if job != 0 {
            unsafe { AssignProcessToJobObject(job as _, child.as_raw_handle() as _) };
        }
    }
}