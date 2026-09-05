use crate::models::NoteMeta;
use crate::state::AppState;
use std::fs;
use std::fs::File;
use std::io::Write;
use tauri::State;
use zip::write::SimpleFileOptions;

#[tauri::command]
pub fn write_file_bytes(path: String, data: Vec<u8>) -> Result<(), String> {
    fs::write(path, data).map_err(|e| e.to_string())
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    cleaned.trim().to_string()
}

#[tauri::command]
pub fn export_all_notes(state: State<AppState>, path: String) -> Result<usize, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let index_path = dirs.notes_dir.join("index.json");
    let notes: Vec<NoteMeta> = fs::read_to_string(&index_path)
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default();

    let file = File::create(&path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut used_names: Vec<String> = Vec::new();
    let mut count = 0;
    for note in notes.iter().filter(|n| n.trashed_at.is_none()) {
        let content =
            fs::read_to_string(dirs.notes_dir.join(format!("{}.md", note.id))).unwrap_or_default();
        let mut base = sanitize_filename(&note.title);
        if base.is_empty() {
            base = "Untitled".to_string();
        }
        let mut name = format!("{}.md", base);
        let mut suffix = 1;
        while used_names.contains(&name) {
            suffix += 1;
            name = format!("{}-{}.md", base, suffix);
        }
        used_names.push(name.clone());

        zip.start_file(&name, options).map_err(|e| e.to_string())?;
        zip.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
        count += 1;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(count)
}
