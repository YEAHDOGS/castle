// Saves a file the page made (converter output) through the system "Save as" dialog. The page sends the bytes
// as the raw request body and the suggested name, percent-encoded, in the x-file-name header.
use tauri::{ipc::InvokeBody, AppHandle};
use tauri_plugin_dialog::DialogExt;

/// Returns false when the user cancels the dialog.
#[tauri::command]
pub async fn save_file(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<bool, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected the file's bytes".into());
    };
    let name = request
        .headers()
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode)
        .unwrap_or_else(|| "dogs-ai".into());
    let Some(path) = app.dialog().file().set_file_name(&name).blocking_save_file() else {
        return Ok(false);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| format!("could not save {}: {e}", path.display()))?;
    Ok(true)
}

/// Undoes encodeURIComponent.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = (b[i] == b'%')
            .then(|| b.get(i + 1..i + 3))
            .flatten()
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match hex {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(b[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
