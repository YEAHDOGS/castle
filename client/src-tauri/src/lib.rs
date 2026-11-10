//! The Castle desktop app. Besides showing the interface, it runs a box on this computer:
//! the `castled` daemon that ships next to the app. The window asks for `local_box` on
//! start and connects to it, so there is no address or token to type.

use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{Manager, RunEvent, State};

const DEFAULT_PORT: u16 = 8787;
const DEFAULT_VAULT_FOLDER: &str = "Castle"; // inside the home folder
const CONFIG_FILE: &str = "box.json";
const LOG_FILE: &str = "box.log";
const TOKEN_PATH: [&str; 2] = [".castle", "token"];
const START_TIMEOUT: Duration = Duration::from_secs(15);
const PROBE_TIMEOUT: Duration = Duration::from_millis(300);
const PROBE_PAUSE: Duration = Duration::from_millis(150);
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000; // no console window flashes up

/// What `box.json` in the app's config folder may set. Every field is optional.
#[derive(Deserialize, Default)]
struct BoxConfig {
  /// The vault folder. Default: `Castle` in the home folder.
  data: Option<PathBuf>,
  /// A games folder (Consoles/ + Roms/) for Arcade.
  arcade: Option<PathBuf>,
  /// Castle's clonewars folder, for Clones.
  clonewars: Option<PathBuf>,
  port: Option<u16>,
}

#[derive(Serialize)]
struct LocalBox {
  url: String,
  token: String,
}

/// The daemon this app started, if it started one. Stopped when the app exits.
#[derive(Default)]
struct Daemon(Mutex<Option<Child>>);

fn is_listening(port: u16) -> bool {
  TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], port)), PROBE_TIMEOUT).is_ok()
}

fn daemon_path() -> Option<PathBuf> {
  let name = if cfg!(windows) { "castled.exe" } else { "castled" };
  let path = std::env::current_exe().ok()?.parent()?.join(name);
  path.is_file().then_some(path)
}

fn read_config(app: &tauri::AppHandle) -> Result<(PathBuf, BoxConfig), String> {
  let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
  std::fs::create_dir_all(&dir).map_err(|e| format!("Can't create {}: {e}", dir.display()))?;
  let config = match std::fs::read(dir.join(CONFIG_FILE)) {
    Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("{CONFIG_FILE} is not valid: {e}"))?,
    Err(_) => BoxConfig::default(),
  };
  Ok((dir, config))
}

fn spawn_daemon(exe: &Path, config_dir: &Path, config: &BoxConfig, data: &Path, port: u16) -> Result<Child, String> {
  let log = std::fs::File::create(config_dir.join(LOG_FILE)).map_err(|e| e.to_string())?;
  let mut command = Command::new(exe);
  command.arg("--data").arg(data).arg("--bind").arg(format!("127.0.0.1:{port}"));
  // A folder that is not there right now (an unplugged drive) is left out, so the box still starts.
  if let Some(dir) = config.arcade.as_ref().filter(|d| d.is_dir()) {
    command.arg("--arcade").arg(dir);
  }
  if let Some(dir) = config.clonewars.as_ref().filter(|d| d.is_dir()) {
    command.arg("--clonewars").arg(dir);
  }
  command.stdin(Stdio::null()).stdout(log.try_clone().map_err(|e| e.to_string())?).stderr(log);
  #[cfg(windows)]
  {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
  }
  command.spawn().map_err(|e| format!("Can't start the box: {e}"))
}

/// The box on this computer: starts the bundled daemon unless one is already listening,
/// then hands the window its address and token.
#[tauri::command(async)]
fn local_box(app: tauri::AppHandle, daemon: State<'_, Daemon>) -> Result<LocalBox, String> {
  let (config_dir, config) = read_config(&app)?;
  let port = config.port.unwrap_or(DEFAULT_PORT);
  let data = match &config.data {
    Some(dir) => dir.clone(),
    None => app.path().home_dir().map_err(|e| e.to_string())?.join(DEFAULT_VAULT_FOLDER),
  };

  let mut running = daemon.0.lock().map_err(|_| "The box state is unavailable".to_string())?;
  if !is_listening(port) {
    let exe = daemon_path().ok_or("This build of Castle has no box inside it")?;
    std::fs::create_dir_all(&data).map_err(|e| format!("Can't create {}: {e}", data.display()))?;
    *running = Some(spawn_daemon(&exe, &config_dir, &config, &data, port)?);
    let deadline = Instant::now() + START_TIMEOUT;
    while !is_listening(port) {
      if Instant::now() > deadline {
        return Err(format!("The box did not start. See {}", config_dir.join(LOG_FILE).display()));
      }
      std::thread::sleep(PROBE_PAUSE);
    }
  }

  let token_file = TOKEN_PATH.iter().fold(data.clone(), |path, part| path.join(part));
  let token = std::fs::read_to_string(&token_file)
    .map_err(|_| format!("A box is running on port {port}, but its token is not in {}", data.display()))?;
  Ok(LocalBox { url: format!("http://127.0.0.1:{port}"), token: token.trim().to_string() })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .manage(Daemon::default())
    .invoke_handler(tauri::generate_handler![local_box])
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .build(tauri::generate_context!())
    .expect("error while building tauri application")
    .run(|app, event| {
      if let RunEvent::Exit = event {
        // Only a daemon this app started is stopped; one that was already running is left alone.
        if let Ok(mut running) = app.state::<Daemon>().0.lock() {
          if let Some(mut child) = running.take() {
            let _ = child.kill();
            let _ = child.wait();
          }
        }
      }
    });
}
