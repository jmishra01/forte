use crate::models::{PdfFolder, PdfMeta};
use crate::state::{AppState, Dirs};
use chrono::Utc;
use std::fs;
use std::path::Path;
use tauri::State;
use uuid::Uuid;

fn read_index(dirs: &Dirs) -> Vec<PdfMeta> {
    let path = dirs.pdfs_dir.join("index.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|data| serde_json::from_str(&data).ok())
        .unwrap_or_default()
}

fn write_index(dirs: &Dirs, index: &[PdfMeta]) -> Result<(), String> {
    let path = dirs.pdfs_dir.join("index.json");
    let data = serde_json::to_string_pretty(index).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())
}

fn read_folders_index(dirs: &Dirs) -> Vec<PdfFolder> {
    let path = dirs.pdfs_dir.join("folders.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|data| serde_json::from_str(&data).ok())
        .unwrap_or_default()
}

fn write_folders_index(dirs: &Dirs, index: &[PdfFolder]) -> Result<(), String> {
    let path = dirs.pdfs_dir.join("folders.json");
    let data = serde_json::to_string_pretty(index).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())
}

fn save_pdf(
    dirs: &Dirs,
    bytes: Vec<u8>,
    title: String,
    source: String,
    source_url: Option<String>,
) -> Result<PdfMeta, String> {
    let id = Uuid::new_v4().to_string();
    let file_name = format!("{}.pdf", id);
    fs::write(dirs.pdfs_dir.join(&file_name), &bytes).map_err(|e| e.to_string())?;
    let meta = PdfMeta {
        id,
        title,
        source,
        source_url,
        added_at: Utc::now().to_rfc3339(),
        file_name,
        last_page: 1,
        completed: false,
        trashed_at: None,
        folder_id: None,
        tags: Vec::new(),
    };
    let mut index = read_index(dirs);
    index.push(meta.clone());
    write_index(dirs, &index)?;
    Ok(meta)
}

#[tauri::command]
pub fn list_pdfs(state: State<AppState>) -> Result<Vec<PdfMeta>, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index: Vec<PdfMeta> = read_index(&dirs).into_iter().filter(|p| p.trashed_at.is_none()).collect();
    index.sort_by(|a, b| b.added_at.cmp(&a.added_at));
    Ok(index)
}

#[tauri::command]
pub fn list_trashed_pdfs(state: State<AppState>) -> Result<Vec<PdfMeta>, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index: Vec<PdfMeta> = read_index(&dirs).into_iter().filter(|p| p.trashed_at.is_some()).collect();
    index.sort_by(|a, b| b.trashed_at.cmp(&a.trashed_at));
    Ok(index)
}

#[tauri::command]
pub fn add_pdf_from_path(
    state: State<AppState>,
    path: String,
    title: Option<String>,
) -> Result<PdfMeta, String> {
    let src = Path::new(&path);
    let is_pdf_ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false);
    if !is_pdf_ext {
        return Err("Selected file is not a PDF".into());
    }
    let bytes = fs::read(src).map_err(|e| format!("Could not read file: {e}"))?;
    if !bytes.starts_with(b"%PDF") {
        return Err("Selected file does not look like a valid PDF".into());
    }
    let derived_title = title.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| {
        src.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string()
    });
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    save_pdf(&dirs, bytes, derived_title, "local".into(), None)
}

#[tauri::command]
pub fn add_pdf_from_url(
    state: State<AppState>,
    url: String,
    title: Option<String>,
) -> Result<PdfMeta, String> {
    let trimmed = url.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err("URL must start with http:// or https://".into());
    }
    let response =
        reqwest::blocking::get(trimmed).map_err(|e| format!("Download failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("Server returned status {}", response.status()));
    }
    let content_type_ok = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("pdf"))
        .unwrap_or(false);
    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed reading response: {e}"))?
        .to_vec();
    if !content_type_ok && !bytes.starts_with(b"%PDF") {
        return Err("The URL did not return a PDF file".into());
    }
    let derived_title = title.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| {
        trimmed
            .rsplit('/')
            .find(|seg| !seg.is_empty())
            .unwrap_or("Untitled")
            .to_string()
    });
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    save_pdf(&dirs, bytes, derived_title, "url".into(), Some(url))
}

#[tauri::command]
pub fn get_pdf_path(state: State<AppState>, id: String) -> Result<String, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let index = read_index(&dirs);
    let meta = index
        .into_iter()
        .find(|p| p.id == id)
        .ok_or("PDF not found")?;
    dirs.pdfs_dir
        .join(&meta.file_name)
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid path".into())
}

#[tauri::command]
pub fn set_pdf_completed(state: State<AppState>, id: String, completed: bool) -> Result<PdfMeta, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    let entry = index.iter_mut().find(|p| p.id == id).ok_or("PDF not found")?;
    entry.completed = completed;
    let meta = entry.clone();
    write_index(&dirs, &index)?;
    Ok(meta)
}

#[tauri::command]
pub fn rename_pdf(state: State<AppState>, id: String, title: String) -> Result<PdfMeta, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    let entry = index.iter_mut().find(|p| p.id == id).ok_or("PDF not found")?;
    let trimmed = title.trim();
    if !trimmed.is_empty() {
        entry.title = trimmed.to_string();
    }
    let meta = entry.clone();
    write_index(&dirs, &index)?;
    Ok(meta)
}

#[tauri::command]
pub fn delete_pdf(state: State<AppState>, id: String) -> Result<(), String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    let entry = index.iter_mut().find(|p| p.id == id).ok_or("PDF not found")?;
    entry.trashed_at = Some(Utc::now().to_rfc3339());
    write_index(&dirs, &index)
}

#[tauri::command]
pub fn restore_pdf(state: State<AppState>, id: String) -> Result<PdfMeta, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    let entry = index.iter_mut().find(|p| p.id == id).ok_or("PDF not found")?;
    entry.trashed_at = None;
    let meta = entry.clone();
    write_index(&dirs, &index)?;
    Ok(meta)
}

#[tauri::command]
pub fn permanently_delete_pdf(state: State<AppState>, id: String) -> Result<(), String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    if let Some(meta) = index.iter().find(|p| p.id == id) {
        let _ = fs::remove_file(dirs.pdfs_dir.join(&meta.file_name));
    }
    index.retain(|p| p.id != id);
    write_index(&dirs, &index)
}

#[tauri::command]
pub fn empty_pdfs_trash(state: State<AppState>) -> Result<(), String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    let (trashed, kept): (Vec<PdfMeta>, Vec<PdfMeta>) =
        index.drain(..).partition(|p| p.trashed_at.is_some());
    for p in &trashed {
        let _ = fs::remove_file(dirs.pdfs_dir.join(&p.file_name));
    }
    write_index(&dirs, &kept)
}

#[tauri::command]
pub fn list_pdf_folders(state: State<AppState>) -> Result<Vec<PdfFolder>, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut folders = read_folders_index(&dirs);
    folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(folders)
}

#[tauri::command]
pub fn create_pdf_folder(state: State<AppState>, name: String) -> Result<PdfFolder, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Folder name can't be empty".into());
    }
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut folders = read_folders_index(&dirs);
    let folder = PdfFolder {
        id: Uuid::new_v4().to_string(),
        name: trimmed.to_string(),
        created_at: Utc::now().to_rfc3339(),
    };
    folders.push(folder.clone());
    write_folders_index(&dirs, &folders)?;
    Ok(folder)
}

#[tauri::command]
pub fn rename_pdf_folder(state: State<AppState>, id: String, name: String) -> Result<PdfFolder, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Folder name can't be empty".into());
    }
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut folders = read_folders_index(&dirs);
    let entry = folders.iter_mut().find(|f| f.id == id).ok_or("Folder not found")?;
    entry.name = trimmed.to_string();
    let folder = entry.clone();
    write_folders_index(&dirs, &folders)?;
    Ok(folder)
}

#[tauri::command]
pub fn delete_pdf_folder(state: State<AppState>, id: String) -> Result<(), String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut folders = read_folders_index(&dirs);
    folders.retain(|f| f.id != id);
    write_folders_index(&dirs, &folders)?;

    let mut index = read_index(&dirs);
    for p in index.iter_mut().filter(|p| p.folder_id.as_deref() == Some(id.as_str())) {
        p.folder_id = None;
    }
    write_index(&dirs, &index)
}

#[tauri::command]
pub fn set_pdf_folder(state: State<AppState>, id: String, folder_id: Option<String>) -> Result<PdfMeta, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    if let Some(fid) = &folder_id {
        let folders = read_folders_index(&dirs);
        if !folders.iter().any(|f| &f.id == fid) {
            return Err("Folder not found".into());
        }
    }
    let mut index = read_index(&dirs);
    let entry = index.iter_mut().find(|p| p.id == id).ok_or("PDF not found")?;
    entry.folder_id = folder_id;
    let meta = entry.clone();
    write_index(&dirs, &index)?;
    Ok(meta)
}

#[tauri::command]
pub fn set_pdf_tags(state: State<AppState>, id: String, tags: Vec<String>) -> Result<PdfMeta, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    let entry = index.iter_mut().find(|p| p.id == id).ok_or("PDF not found")?;
    entry.tags = tags;
    let meta = entry.clone();
    write_index(&dirs, &index)?;
    Ok(meta)
}

#[tauri::command]
pub fn list_pdf_tags(state: State<AppState>) -> Result<Vec<String>, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut tags: Vec<String> = read_index(&dirs)
        .into_iter()
        .filter(|p| p.trashed_at.is_none())
        .flat_map(|p| p.tags)
        .collect();
    tags.sort();
    tags.dedup();
    Ok(tags)
}
