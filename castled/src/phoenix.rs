//! Phoenix: setting a Windows PC up from nothing, and keeping one clean.
//!
//! Phoenix is its own project (github.com/YEAHDOGS/phoenix), imported here as the git
//! submodule `app/phoenix/`, and it is also a standalone app. Everything it does goes
//! through one PowerShell engine, `scripts/engine/phoenix.ps1`, which takes a command and a
//! JSON file of arguments and prints one JSON event per line, the last of them the result.
//! The box runs that engine and hands the result on; it builds no PowerShell itself.
//!
//! What the section uses:
//! - the cleaner: a scan that deletes nothing and lists what each rule would free, then a
//!   run of only the rules a person picked, on this computer or on an Android phone on USB;
//! - the app list: Chocolatey packages to install on a fresh machine, searched in the
//!   community feed, and the hardening choices;
//! - the two things made from that list: a set-up script to run by hand, and a Windows
//!   answer file that runs it at the first logon.
//!
//! Which copy runs: CASTLE_PHOENIX_DIR when set; otherwise Castle's own `app/phoenix/`, or
//! else a `phoenix` checkout beside Castle. Every copy shares one config file, the one the
//! standalone app keeps (%APPDATA%\Phoenix\phoenix.config.json), so the app list is the
//! same in both.
//!
//! The engine is Windows PowerShell, so a box on Linux has no Phoenix. Scanning is allowed
//! from any device; deleting files, saving the list and making an answer file (which takes
//! account passwords) only from the computer the box runs on.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
};

use serde_json::{Value, json};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// One engine run at a time: two cleans at once would count each other's files.
static ENGINE: Mutex<()> = Mutex::new(());

pub type PhoenixResult<T> = Result<T, String>;

fn looks_like_phoenix(dir: &Path) -> bool {
    dir.join("scripts").join("engine").join("phoenix.ps1").is_file() && dir.join("config").join("clean").is_dir()
}

fn candidates() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::env::var_os("CASTLE_PHOENIX_DIR").map(PathBuf::from).into_iter().collect();
    for start in [std::env::current_exe().ok(), std::env::current_dir().ok()].into_iter().flatten() {
        for dir in start.ancestors() {
            found.push(dir.join("app").join("phoenix"));
            found.push(dir.join("phoenix"));
        }
    }
    found
}

/// The copy to run, or None when there is none on this computer.
pub fn find() -> Option<PathBuf> {
    candidates().into_iter().find(|d| looks_like_phoenix(d)).map(|d| crate::plain_path(&d.canonicalize().unwrap_or(d)))
}

/// Why Phoenix cannot run here, in words for the app, or the folder it runs from.
fn kit() -> PhoenixResult<PathBuf> {
    if !cfg!(windows) {
        return Err("Phoenix sets up and cleans Windows PCs, and its engine is Windows PowerShell. This box runs on Linux, so it has none.".into());
    }
    find().ok_or_else(|| "Phoenix is not on this computer. In Castle's folder, run: git submodule update --init app/phoenix".to_string())
}

/// adb, when Castle's own Android tools are installed (Clones puts them there). Without it
/// the engine looks in its own places and on the PATH.
fn adb(kit: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) { "adb.exe" } else { "adb" };
    let data = std::env::var_os("CASTLE_CLONEWARS_DATA").map(PathBuf::from).or_else(|| kit.parent().map(|app| app.join("data").join("clonewars")))?;
    let path = data.join("android-sdk").join("platform-tools").join(name);
    path.is_file().then_some(path)
}

/// Phoenix's answer-file generator, which is built, not checked in: the copy that runs may
/// not have it while another copy on this computer does (a `phoenix` checkout beside Castle
/// where it was built). The engine looks in its own folder; this is handed to it as well.
fn unattend_exe() -> Option<PathBuf> {
    if let Some(exe) = std::env::var_os("PHOENIX_UNATTEND_EXE").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(exe);
    }
    candidates().into_iter().map(|dir| dir.join("app").join("unattend").join("bin").join("phoenix-unattend.exe")).find(|exe| exe.is_file())
}

/// The config every copy of Phoenix shares.
fn config_path() -> Option<PathBuf> {
    std::env::var_os("PHOENIX_CONFIG").map(PathBuf::from).or_else(|| std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Phoenix").join("phoenix.config.json")))
}

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    // Windows PowerShell writes UTF-8 with a byte-order mark.
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

/// The shared config, or Phoenix's own example when nobody has saved one yet.
fn config(kit: &Path) -> Value {
    config_path().and_then(|p| read_json(&p)).or_else(|| read_json(&kit.join("config").join("phoenix.example.json"))).unwrap_or_else(|| json!({ "schema": 1 }))
}

/// Runs one engine command and returns its result data, or the engine's own error text.
fn run(kit: &Path, work: &Path, command: &str, args: &Value) -> PhoenixResult<Value> {
    let _one = ENGINE.lock().map_err(|_| "Phoenix is in a bad state. Restart the box.".to_string())?;
    std::fs::create_dir_all(work).map_err(|e| format!("Castle could not make its work folder: {e}"))?;
    let args_path = work.join(format!("args-{}.json", crate::random_hex(8)));
    std::fs::write(&args_path, serde_json::to_vec(args).map_err(|e| e.to_string())?).map_err(|e| format!("Castle could not write the engine's arguments: {e}"))?;
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(kit.join("scripts").join("engine").join("phoenix.ps1"))
        .arg(command)
        .arg("-ArgsPath")
        .arg(&args_path)
        // Started from PowerShell 7, the box carries its module path, and Windows PowerShell
        // then cannot find its own commands (Get-FileHash). Without it, it uses its default.
        .env_remove("PSModulePath");
    if let Some(exe) = unattend_exe() {
        cmd.env("PHOENIX_UNATTEND_EXE", exe);
    }
    cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd.output();
    let _ = std::fs::remove_file(&args_path);
    let output = output.map_err(|e| format!("Windows PowerShell did not start: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let result = text
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim().trim_start_matches('\u{feff}')).ok())
        .find(|event| event["t"] == "result")
        .ok_or_else(|| "Phoenix's engine stopped without an answer.".to_string())?;
    if result["ok"] == true {
        Ok(result["data"].clone())
    } else {
        Err(result["data"]["error"].as_str().unwrap_or("Phoenix's engine failed.").to_string())
    }
}

pub struct Phoenix {
    /// Where argument files and answer files are written for a moment: the vault's private folder.
    work: PathBuf,
}

fn platform(name: &str) -> PhoenixResult<&'static str> {
    match name {
        "" | "windows" => Ok("windows"),
        "android" => Ok("android"),
        _ => Err("Phoenix cleans Windows and Android.".into()),
    }
}

/// A rule id, an app id or a phone's serial: nothing that could be more than a name.
fn plain(text: &str, max: usize) -> bool {
    !text.is_empty() && text.len() <= max && text.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':'))
}

fn account_name(text: &str) -> bool {
    !text.is_empty() && text.len() <= 20 && text.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ' ')) && !text.starts_with(' ') && !text.ends_with(' ')
}

impl Phoenix {
    pub fn new(meta: &Path) -> Self {
        Self { work: meta.join("phoenix") }
    }

    /// Whether Phoenix can run here, and as what.
    pub fn status(&self) -> Value {
        match kit() {
            Err(reason) => json!({ "available": false, "reason": reason }),
            Ok(kit) => match run(&kit, &self.work, "version", &json!({})) {
                Ok(v) => json!({ "available": true, "folder": kit, "engine": v["engineVersion"], "isAdmin": v["isAdmin"], "os": v["os"], "android": adb(&kit).is_some() }),
                Err(reason) => json!({ "available": false, "reason": reason }),
            },
        }
    }

    fn clean_args(&self, kit: &Path, platform_name: &str, serial: &str) -> PhoenixResult<Value> {
        let mut args = json!({ "platform": platform(platform_name)? });
        if !serial.is_empty() {
            if !plain(serial, 64) {
                return Err("That is not a device serial.".into());
            }
            args["serial"] = json!(serial);
        }
        if let Some(adb) = adb(kit) {
            args["adbPath"] = json!(adb);
        }
        Ok(args)
    }

    /// Android phones on USB.
    pub fn devices(&self) -> PhoenixResult<Value> {
        let kit = kit()?;
        run(&kit, &self.work, "clean-devices", &self.clean_args(&kit, "android", "")?)
    }

    /// What could be cleaned. Deletes nothing.
    pub fn scan(&self, platform_name: &str, serial: &str) -> PhoenixResult<Value> {
        let kit = kit()?;
        run(&kit, &self.work, "clean-scan", &self.clean_args(&kit, platform_name, serial)?)
    }

    /// Deletes what the named rules cover, and only those.
    pub fn clean(&self, platform_name: &str, serial: &str, ids: &[String]) -> PhoenixResult<Value> {
        if ids.is_empty() {
            return Err("Pick what to clean first.".into());
        }
        if ids.len() > 200 || !ids.iter().all(|id| plain(id, 64)) {
            return Err("That is not a list of cleaning rules.".into());
        }
        let kit = kit()?;
        let mut args = self.clean_args(&kit, platform_name, serial)?;
        args["ids"] = json!(ids);
        run(&kit, &self.work, "clean-run", &args)
    }

    /// The app list, Phoenix's own catalog, and the hardening choices, for the Set up tab.
    pub fn setup(&self) -> PhoenixResult<Value> {
        let kit = kit()?;
        let cfg = config(&kit);
        let catalog = read_json(&kit.join("config").join("catalog").join("apps.json")).unwrap_or_else(|| json!([]));
        let harden = read_json(&kit.join("config").join("sources").join("sos-harden.json")).unwrap_or(Value::Null);
        let first = &cfg["firstBoot"];
        Ok(json!({
            "apps": cfg["apps"].as_array().cloned().unwrap_or_default(),
            "catalog": catalog,
            "computerName": cfg["machine"]["computerName"].as_str().unwrap_or(""),
            "adminName": cfg["accounts"]["admin"]["name"].as_str().unwrap_or(""),
            "userName": cfg["accounts"]["lead"]["name"].as_str().unwrap_or(""),
            "harden": {
                "enabled": first["harden"]["enabled"] == true,
                "chosen": first["harden"]["options"].as_object().cloned().unwrap_or_default(),
                "name": harden["name"], "description": harden["description"], "homepage": harden["homepage"],
                "commit": harden["commit"], "options": harden["options"].as_array().cloned().unwrap_or_default(),
            },
            "saved": config_path().is_some_and(|p| p.is_file()),
        }))
    }

    /// Saves the app list and the rest of the Set up tab into the shared config.
    pub fn save_setup(&self, body: &Value) -> PhoenixResult<Value> {
        let kit = kit()?;
        let path = config_path().ok_or("This computer has no place for Phoenix's settings.")?;
        let mut cfg = config(&kit);
        if !cfg.is_object() {
            cfg = json!({ "schema": 1 });
        }
        if let Some(apps) = body["apps"].as_array() {
            let mut ids: Vec<String> = Vec::new();
            for app in apps {
                let id = app.as_str().unwrap_or("").trim().to_string();
                if !plain(&id, 100) || id.contains(':') {
                    return Err(format!("\"{id}\" is not a Chocolatey package name."));
                }
                if !ids.iter().any(|have| have.eq_ignore_ascii_case(&id)) {
                    ids.push(id);
                }
            }
            if ids.len() > 300 {
                return Err("That is more apps than one machine should install by itself.".into());
            }
            cfg["apps"] = json!(ids);
        }
        if let Some(name) = body["computerName"].as_str() {
            let name = name.trim();
            if name.is_empty() || name.len() > 15 || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err("A computer name is up to 15 letters, digits and dashes.".into());
            }
            cfg["machine"]["computerName"] = json!(name);
        }
        for (key, slot) in [("adminName", "admin"), ("userName", "lead")] {
            if let Some(name) = body[key].as_str() {
                let name = name.trim();
                if name.is_empty() && slot == "lead" {
                    if let Some(accounts) = cfg["accounts"].as_object_mut() {
                        accounts.remove("lead");
                    }
                    continue;
                }
                if !account_name(name) {
                    return Err("An account name is up to 20 letters, digits, spaces, dots and dashes.".into());
                }
                cfg["accounts"][slot]["name"] = json!(name);
            }
        }
        if let Some(harden) = body["harden"].as_object() {
            let mut chosen = serde_json::Map::new();
            for (name, on) in harden.get("chosen").and_then(Value::as_object).into_iter().flatten() {
                if name.len() <= 40 && name.chars().all(|c| c.is_ascii_alphanumeric()) {
                    chosen.insert(name.clone(), json!(on == &json!(true)));
                }
            }
            cfg["firstBoot"]["harden"] = json!({ "enabled": harden.get("enabled") == Some(&json!(true)), "options": chosen });
        }
        // An answer file made here always sets the machine up at first boot; that is the point of it.
        cfg["firstBoot"]["enabled"] = json!(true);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("Castle could not save Phoenix's settings: {e}"))?;
        }
        let text = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| format!("Castle could not save Phoenix's settings: {e}"))?;
        self.setup()
    }

    /// Chocolatey's community feed, or Phoenix's catalog when the feed cannot be reached.
    pub fn search(&self, query: &str) -> PhoenixResult<Value> {
        let query = query.trim();
        if query.len() > 80 {
            return Err("That is too long to search for.".into());
        }
        let kit = kit()?;
        run(&kit, &self.work, "choco-search", &json!({ "query": query, "top": 30 }))
    }

    /// The set-up script for the saved list: Chocolatey, the apps, and the hardening if chosen.
    pub fn script(&self) -> PhoenixResult<Value> {
        let kit = kit()?;
        let cfg = config(&kit);
        let args = json!({ "apps": cfg["apps"].as_array().cloned().unwrap_or_default(), "harden": cfg["firstBoot"]["harden"] });
        run(&kit, &self.work, "setup-script", &args)
    }

    /// A Windows answer file for the saved settings. The passwords go to the engine in a
    /// file it deletes as it reads, and into the answer file, which the format requires;
    /// the answer file is handed back and not kept.
    pub fn answer_file(&self, admin_password: &str, user_password: &str) -> PhoenixResult<Value> {
        let kit = kit()?;
        if admin_password.is_empty() || admin_password.len() > 127 || user_password.len() > 127 {
            return Err("The administrator needs a password, up to 127 characters.".into());
        }
        let cfg_path = config_path().filter(|p| p.is_file()).ok_or("Save the set-up first.")?;
        std::fs::create_dir_all(&self.work).map_err(|e| format!("Castle could not make its work folder: {e}"))?;
        let token = crate::random_hex(8);
        let secrets = self.work.join(format!("secrets-{token}.json"));
        let out = self.work.join(format!("answer-{token}.xml"));
        std::fs::write(&secrets, serde_json::to_vec(&json!({ "admin": admin_password, "lead": user_password })).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Castle could not hand the passwords to Phoenix: {e}"))?;
        let result = run(&kit, &self.work, "unattend", &json!({ "configPath": cfg_path, "secretsPath": secrets, "outPath": out }));
        let _ = std::fs::remove_file(&secrets);
        let xml = std::fs::read_to_string(&out);
        let _ = std::fs::remove_file(&out);
        let data = result?;
        let xml = xml.map_err(|_| "Phoenix made no answer file.".to_string())?;
        Ok(json!({ "xml": xml, "sha256": data["sha256"], "accounts": data["accounts"], "apps": data["firstBootApps"], "warnings": data["warnings"] }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_only_names() {
        assert!(plain("win.temp.user", 64));
        assert!(plain("emulator-5554", 64));
        assert!(plain("127.0.0.1:5555", 64));
        assert!(!plain("", 64));
        assert!(!plain("a b", 64));
        assert!(!plain("x'; Remove-Item C:\\", 64));
        assert!(!plain("../up", 64));
        assert!(account_name("Big Man"));
        assert!(!account_name(" x"));
        assert!(!account_name("a\"b"));
    }

    #[test]
    fn platforms() {
        assert_eq!(platform(""), Ok("windows"));
        assert_eq!(platform("android"), Ok("android"));
        assert!(platform("ios").is_err());
    }

    #[test]
    fn a_folder_is_phoenix_only_with_its_engine_and_rules() {
        let dir = std::env::temp_dir().join(format!("castle-phoenix-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scripts").join("engine")).unwrap();
        assert!(!looks_like_phoenix(&dir));
        std::fs::write(dir.join("scripts").join("engine").join("phoenix.ps1"), "").unwrap();
        assert!(!looks_like_phoenix(&dir));
        std::fs::create_dir_all(dir.join("config").join("clean")).unwrap();
        assert!(looks_like_phoenix(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_config_written_by_windows_powershell_is_read() {
        let file = std::env::temp_dir().join(format!("castle-phoenix-bom-{}.json", std::process::id()));
        std::fs::write(&file, "\u{feff}{\"apps\":[\"7zip\"]}").unwrap();
        assert_eq!(read_json(&file).unwrap()["apps"][0], "7zip");
        let _ = std::fs::remove_file(&file);
    }
}
