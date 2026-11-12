// DOGS AI desktop shell: one window showing the web app (../dist, or the Vite dev server under `tauri dev`), plus
// the local dogs engine, started and kept running for as long as the app is open.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod dogs_server;
mod save;

use tauri::{window::Color, RunEvent, Url, WebviewUrl, WebviewWindowBuilder};

// Same as the page background (--color-app in app.css), so the window never flashes white.
const BACKGROUND: Color = Color(2, 2, 3, 255);
// Where the traffic lights sit in the header, which is the title bar (keep in step with .titlebar in app.css).
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHTS: tauri::LogicalPosition<f64> = tauri::LogicalPosition { x: 18.0, y: 20.0 };

/// The app's own pages: tauri://localhost (macOS, Linux), http://tauri.localhost (Windows), the dev server.
fn is_app_url(url: &Url) -> bool {
    url.scheme() == "tauri"
        || url.host_str() == Some("tauri.localhost")
        || (cfg!(debug_assertions) && url.host_str() == Some("127.0.0.1") && url.port() == Some(5173))
}

fn main() {
    let engine = dogs_server::Engine::default();
    let app_engine = engine.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![save::save_file])
        .setup(move |app| {
            app_engine.spawn();
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("DOGS AI")
                .inner_size(1000.0, 800.0)
                .min_inner_size(360.0, 520.0)
                .background_color(BACKGROUND)
                // Security: the window only ever shows the app. Web links (e.g. Hugging Face) go to the system
                // browser instead of replacing it.
                .on_navigation(|url| {
                    if is_app_url(url) {
                        return true;
                    }
                    if matches!(url.scheme(), "http" | "https") {
                        let _ = tauri_plugin_opener::open_url(url.as_str(), None::<&str>);
                    }
                    false
                });
            // No system title bar: the app's header is the title bar. macOS keeps its traffic lights, inset into
            // the header; Windows and Linux get the header's own window buttons.
            #[cfg(target_os = "macos")]
            let window = window
                .title_bar_style(tauri::TitleBarStyle::Overlay)
                .hidden_title(true)
                .traffic_light_position(TRAFFIC_LIGHTS);
            #[cfg(not(target_os = "macos"))]
            let window = window.decorations(false);
            window.build()?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("could not start DOGS AI")
        .run(move |_app, event| {
            if let RunEvent::Exit = event {
                engine.stop();
            }
        });
}
