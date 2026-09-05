use crate::models::AttachmentMeta;
use crate::state::AppState;
use std::fs;
use std::path::Path;
use tauri::State;
use uuid::Uuid;

const ALLOWED_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];

#[tauri::command]
pub fn import_attachment(state: State<AppState>, path: String) -> Result<AttachmentMeta, String> {
    let src = Path::new(&path);
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .ok_or("File has no extension")?;
    if !ALLOWED_EXTS.contains(&ext.as_str()) {
        return Err("Only image files can be imported as attachments".into());
    }
    let bytes = fs::read(src).map_err(|e| format!("Could not read file: {e}"))?;
    let id = Uuid::new_v4().to_string();
    let file_name = format!("{}.{}", id, ext);
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    fs::write(dirs.attachments_dir.join(&file_name), &bytes).map_err(|e| e.to_string())?;
    Ok(AttachmentMeta { id, file_name })
}

#[tauri::command]
pub fn get_attachment_path(state: State<AppState>, file_name: String) -> Result<String, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let safe_name = Path::new(&file_name)
        .file_name()
        .ok_or("Invalid attachment name")?;
    let path = dirs.attachments_dir.join(safe_name);
    if !path.is_file() {
        return Err("Attachment not found".into());
    }
    path.to_str().map(|s| s.to_string()).ok_or_else(|| "Invalid path".into())
}
