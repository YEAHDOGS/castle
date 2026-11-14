//! The Castle desktop app. Besides showing the interface, it runs a box on this computer:
//! the `castled` daemon that ships next to the app. The window asks for `local_box` on
//! start and connects to it, so there is no address or token to type.
//!
//! While it runs it has an icon in the tray (Open, Quit). For now, closing the app stops
//! everything: the box it is connected to is asked to stop, and the box stops what it
//! started (Clones' machines, DOGS AI, ComfyUI) before it goes. The app waits for that.

use std::io::{Read, Write};
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
/// How long the box gets to stop itself and what it started when the app closes. VMs get
/// 20 s to shut down and containers 20 s to stop, so this leaves room for both.
const STOP_TIMEOUT: Duration = Duration::from_secs(60);
const TRAY_ID: &str = "castle";
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

/// The daemon this app started, if it started one, and the box the window connected to
/// (its port and token), which is asked to stop when the app exits.
#[derive(Default)]
struct Daemon {
  child: Mutex<Option<Child>>,
  connected: Mutex<Option<(u16, String)>>,
}

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

  let mut running = daemon.child.lock().map_err(|_| "The box state is unavailable".to_string())?;
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
  let token = token.trim().to_string();
  if let Ok(mut connected) = daemon.connected.lock() {
    *connected = Some((port, token.clone()));
  }
  Ok(LocalBox { url: format!("http://127.0.0.1:{port}"), token })
}

/// Asks the box on `port` to stop (POST /api/v1/box/stop). It answers at once and then stops
/// what it started; `stop_box` waits for that.
fn ask_box_to_stop(port: u16, token: &str) -> bool {
  let addr = SocketAddr::from(([127, 0, 0, 1], port));
  let Ok(mut stream) = TcpStream::connect_timeout(&addr, PROBE_TIMEOUT) else { return false };
  let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
  let request = format!(
    "POST /api/v1/box/stop HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
  );
  if stream.write_all(request.as_bytes()).is_err() {
    return false;
  }
  let mut answer = Vec::new();
  let _ = stream.take(4096).read_to_end(&mut answer);
  answer.starts_with(b"HTTP/1.1 200")
}

/// Stops the box this app is connected to, and waits until it has stopped everything it
/// started. A box the app started itself that does not stop in time is ended.
fn stop_box(app: &tauri::AppHandle) {
  let daemon = app.state::<Daemon>();
  let connected = daemon.connected.lock().ok().and_then(|c| c.clone());
  let ours = daemon.child.lock().ok().and_then(|mut c| c.take());
  let deadline = Instant::now() + STOP_TIMEOUT;
  let asked = connected.is_some_and(|(port, token)| {
    let asked = ask_box_to_stop(port, &token);
    while asked && is_listening(port) && Instant::now() < deadline {
      std::thread::sleep(PROBE_PAUSE);
    }
    asked
  });
  if let Some(mut child) = ours {
    // It stops by itself once asked; ended at once if it could not be asked, or at the deadline.
    while asked && matches!(child.try_wait(), Ok(None)) && Instant::now() < deadline {
      std::thread::sleep(PROBE_PAUSE);
    }
    let _ = child.kill();
    let _ = child.wait();
  }
}

#[cfg(desktop)]
fn show_window(app: &tauri::AppHandle) {
  if let Some(window) = app.get_webview_window("main") {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
  }
}

/// The tray icon: a click opens the window; its menu opens it or quits Castle.
#[cfg(desktop)]
fn tray(app: &tauri::App) -> tauri::Result<()> {
  use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
  use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

  let open = MenuItem::with_id(app, "open", "Open Castle", true, None::<&str>)?;
  let quit = MenuItem::with_id(app, "quit", "Quit Castle (stops the box)", true, None::<&str>)?;
  let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;
  let mut builder = TrayIconBuilder::with_id(TRAY_ID)
    .tooltip("Castle")
    .menu(&menu)
    .show_menu_on_left_click(false)
    .on_menu_event(|app, event| match event.id.as_ref() {
      "open" => show_window(app),
      "quit" => app.exit(0),
      _ => {}
    })
    .on_tray_icon_event(|tray, event| {
      if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
        show_window(tray.app_handle());
      }
    });
  if let Some(icon) = app.default_window_icon() {
    builder = builder.icon(icon.clone());
  }
  builder.build(app)?;
  Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .manage(Daemon::default())
    .invoke_handler(tauri::generate_handler![local_box])
    .setup(|app| {
      #[cfg(desktop)]
      tray(app)?;
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
        // For now everything stops with the app: the box, whoever started it, and all it runs.
        #[cfg(desktop)]
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
          let _ = tray.set_tooltip(Some("Castle: stopping the box"));
        }
        stop_box(app);
      }
    });
}
