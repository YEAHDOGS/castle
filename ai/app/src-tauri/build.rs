// Records where the dogs-ai project lives (DOGS_HOME, else two folders up), so the packaged app can start
// `dogs serve` from it. A CI runner's checkout means nothing on the machine that installs the app, so CI builds
// record nothing unless DOGS_HOME is set. Override at runtime with the DOGS_HOME environment variable.
use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=DOGS_HOME");
    println!("cargo:rerun-if-env-changed=CI");
    let home = env::var("DOGS_HOME").ok().or_else(|| {
        if env::var_os("CI").is_some() {
            return None;
        }
        let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").ok()?);
        Some(manifest.parent()?.parent()?.to_string_lossy().into_owned())
    });
    if let Some(home) = home {
        println!("cargo:rustc-env=DOGS_BUILD_HOME={home}");
    }
    tauri_build::build()
}
