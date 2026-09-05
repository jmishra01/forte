use crate::models::{Note, NoteMeta, NoteVersion, NoteVersionContent};
use crate::state::{AppState, Dirs};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use tauri::State;
use uuid::Uuid;

const HISTORY_MIN_GAP_SECONDS: i64 = 300;

fn read_index(dirs: &Dirs) -> Vec<NoteMeta> {
    let path = dirs.notes_dir.join("index.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|data| serde_json::from_str(&data).ok())
        .unwrap_or_default()
}

fn write_index(dirs: &Dirs, index: &[NoteMeta]) -> Result<(), String> {
    let path = dirs.notes_dir.join("index.json");
    let data = serde_json::to_string_pretty(index).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())
}

fn note_path(dirs: &Dirs, id: &str) -> std::path::PathBuf {
    dirs.notes_dir.join(format!("{}.md", id))
}

fn history_dir_for(dirs: &Dirs, id: &str) -> std::path::PathBuf {
    dirs.history_dir.join(id)
}

/// All descendants of `id` (children, grandchildren, ...), not including `id` itself.
fn collect_descendants(index: &[NoteMeta], id: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut frontier = vec![id.to_string()];
    while let Some(current) = frontier.pop() {
        for n in index {
            if n.parent_id.as_deref() == Some(current.as_str()) {
                result.push(n.id.clone());
                frontier.push(n.id.clone());
            }
        }
    }
    result
}

#[derive(Serialize, Deserialize)]
struct HistoryEntry {
    title: String,
    content: String,
}

fn safe_timestamp_filename(ts: &str) -> String {
    ts.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Snapshot the note's current on-disk state into history before it gets overwritten,
/// but only if enough time has passed since the last snapshot — otherwise continuous
/// autosaves while typing would create a version per keystroke pause.
fn maybe_snapshot(dirs: &Dirs, meta: &NoteMeta) -> Result<(), String> {
    let dir = history_dir_for(dirs, &meta.id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let history_is_empty = fs::read_dir(&dir)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(true);

    let gap_large_enough = DateTime::parse_from_rfc3339(&meta.updated_at)
        .map(|t| (Utc::now() - t.with_timezone(&Utc)).num_seconds() >= HISTORY_MIN_GAP_SECONDS)
        .unwrap_or(true);

    if !history_is_empty && !gap_large_enough {
        return Ok(());
    }

    let existing_content = fs::read_to_string(note_path(dirs, &meta.id)).unwrap_or_default();
    let entry = HistoryEntry {
        title: meta.title.clone(),
        content: existing_content,
    };
    let file_name = format!("{}.json", safe_timestamp_filename(&meta.updated_at));
    let data = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
    fs::write(dir.join(file_name), data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_notes(state: State<AppState>) -> Result<Vec<NoteMeta>, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index: Vec<NoteMeta> = read_index(&dirs).into_iter().filter(|n| n.trashed_at.is_none()).collect();
    index.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(index)
}

#[tauri::command]
pub fn list_trashed_notes(state: State<AppState>) -> Result<Vec<NoteMeta>, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index: Vec<NoteMeta> = read_index(&dirs).into_iter().filter(|n| n.trashed_at.is_some()).collect();
    index.sort_by(|a, b| b.trashed_at.cmp(&a.trashed_at));
    Ok(index)
}

#[tauri::command]
pub fn list_tags(state: State<AppState>) -> Result<Vec<String>, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut tags: Vec<String> = read_index(&dirs)
        .into_iter()
        .filter(|n| n.trashed_at.is_none())
        .flat_map(|n| n.tags)
        .collect();
    tags.sort();
    tags.dedup();
    Ok(tags)
}

#[tauri::command]
pub fn get_note(state: State<AppState>, id: String) -> Result<Note, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let index = read_index(&dirs);
    let meta = index.into_iter().find(|n| n.id == id).ok_or("Note not found")?;
    let content = fs::read_to_string(note_path(&dirs, &id)).map_err(|e| e.to_string())?;
    Ok(Note { meta, content })
}

#[tauri::command]
pub fn create_note(
    state: State<AppState>,
    title: String,
    parent_id: Option<String>,
) -> Result<Note, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let title = if title.trim().is_empty() { "Untitled".to_string() } else { title };
    let meta = NoteMeta {
        id: id.clone(),
        title: title.clone(),
        created_at: now.clone(),
        updated_at: now,
        tags: Vec::new(),
        parent_id,
        linked_pdf_ids: Vec::new(),
        trashed_at: None,
    };
    let content = format!("# {}\n\n", title);
    fs::write(note_path(&dirs, &id), &content).map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    index.push(meta.clone());
    write_index(&dirs, &index)?;
    Ok(Note { meta, content })
}

#[tauri::command]
pub fn update_note(state: State<AppState>, id: String, title: String, content: String) -> Result<NoteMeta, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    let entry = index.iter_mut().find(|n| n.id == id).ok_or("Note not found")?;

    maybe_snapshot(&dirs, entry)?;

    entry.title = if title.trim().is_empty() { "Untitled".to_string() } else { title };
    entry.updated_at = Utc::now().to_rfc3339();
    let meta = entry.clone();
    fs::write(note_path(&dirs, &id), &content).map_err(|e| e.to_string())?;
    write_index(&dirs, &index)?;
    Ok(meta)
}

#[tauri::command]
pub fn set_note_tags(state: State<AppState>, id: String, tags: Vec<String>) -> Result<NoteMeta, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    let entry = index.iter_mut().find(|n| n.id == id).ok_or("Note not found")?;
    entry.tags = tags;
    let meta = entry.clone();
    write_index(&dirs, &index)?;
    Ok(meta)
}

#[tauri::command]
pub fn set_note_parent(
    state: State<AppState>,
    id: String,
    parent_id: Option<String>,
) -> Result<NoteMeta, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);

    if let Some(pid) = &parent_id {
        if pid == &id {
            return Err("A page can't be its own parent".into());
        }
        if collect_descendants(&index, &id).contains(pid) {
            return Err("Can't move a page into one of its own sub-pages".into());
        }
        if !index.iter().any(|n| &n.id == pid) {
            return Err("Parent page not found".into());
        }
    }

    let entry = index.iter_mut().find(|n| n.id == id).ok_or("Note not found")?;
    entry.parent_id = parent_id;
    let meta = entry.clone();
    write_index(&dirs, &index)?;
    Ok(meta)
}

#[tauri::command]
pub fn link_pdf_to_note(state: State<AppState>, note_id: String, pdf_id: String) -> Result<NoteMeta, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    let entry = index.iter_mut().find(|n| n.id == note_id).ok_or("Note not found")?;
    if !entry.linked_pdf_ids.contains(&pdf_id) {
        entry.linked_pdf_ids.push(pdf_id);
    }
    let meta = entry.clone();
    write_index(&dirs, &index)?;
    Ok(meta)
}

#[tauri::command]
pub fn unlink_pdf_from_note(state: State<AppState>, note_id: String, pdf_id: String) -> Result<NoteMeta, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    let entry = index.iter_mut().find(|n| n.id == note_id).ok_or("Note not found")?;
    entry.linked_pdf_ids.retain(|p| p != &pdf_id);
    let meta = entry.clone();
    write_index(&dirs, &index)?;
    Ok(meta)
}

#[tauri::command]
pub fn delete_note(state: State<AppState>, id: String) -> Result<(), String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    if !index.iter().any(|n| n.id == id) {
        return Err("Note not found".into());
    }
    // Trashing a page also trashes its sub-pages, mirroring how the tree is presented.
    let mut ids = collect_descendants(&index, &id);
    ids.push(id);
    let now = Utc::now().to_rfc3339();
    for n in index.iter_mut().filter(|n| ids.contains(&n.id)) {
        n.trashed_at = Some(now.clone());
    }
    write_index(&dirs, &index)
}

#[tauri::command]
pub fn restore_note(state: State<AppState>, id: String) -> Result<NoteMeta, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    let mut ids = collect_descendants(&index, &id);
    ids.push(id.clone());
    for n in index.iter_mut().filter(|n| ids.contains(&n.id)) {
        n.trashed_at = None;
    }
    let meta = index
        .iter()
        .find(|n| n.id == id)
        .cloned()
        .ok_or("Note not found")?;
    write_index(&dirs, &index)?;
    Ok(meta)
}

#[tauri::command]
pub fn permanently_delete_note(state: State<AppState>, id: String) -> Result<(), String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    let mut ids = collect_descendants(&index, &id);
    ids.push(id);
    for note_id in &ids {
        let _ = fs::remove_file(note_path(&dirs, note_id));
        let _ = fs::remove_dir_all(history_dir_for(&dirs, note_id));
    }
    index.retain(|n| !ids.contains(&n.id));
    write_index(&dirs, &index)
}

#[tauri::command]
pub fn empty_notes_trash(state: State<AppState>) -> Result<(), String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    let (trashed, kept): (Vec<NoteMeta>, Vec<NoteMeta>) =
        index.drain(..).partition(|n| n.trashed_at.is_some());
    for n in &trashed {
        let _ = fs::remove_file(note_path(&dirs, &n.id));
        let _ = fs::remove_dir_all(history_dir_for(&dirs, &n.id));
    }
    write_index(&dirs, &kept)
}

#[tauri::command]
pub fn get_backlinks(state: State<AppState>, title: String) -> Result<Vec<NoteMeta>, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let index: Vec<NoteMeta> = read_index(&dirs).into_iter().filter(|n| n.trashed_at.is_none()).collect();
    let needle = format!("[[{}]]", title).to_lowercase();
    let mut results = Vec::new();
    for meta in index {
        if let Ok(content) = fs::read_to_string(note_path(&dirs, &meta.id)) {
            if content.to_lowercase().contains(&needle) {
                results.push(meta);
            }
        }
    }
    Ok(results)
}

#[tauri::command]
pub fn list_note_history(state: State<AppState>, id: String) -> Result<Vec<NoteVersion>, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let dir = history_dir_for(&dirs, &id);
    let mut versions = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let Some(timestamp) = file_name.strip_suffix(".json").map(|s| s.to_string()) else {
                continue;
            };
            if let Ok(data) = fs::read_to_string(entry.path()) {
                if let Ok(parsed) = serde_json::from_str::<HistoryEntry>(&data) {
                    versions.push(NoteVersion { timestamp, title: parsed.title });
                }
            }
        }
    }
    versions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(versions)
}

#[tauri::command]
pub fn get_note_history_content(state: State<AppState>, id: String, timestamp: String) -> Result<NoteVersionContent, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let path = history_dir_for(&dirs, &id).join(format!("{}.json", timestamp));
    let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let parsed: HistoryEntry = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    Ok(NoteVersionContent { title: parsed.title, content: parsed.content })
}

#[tauri::command]
pub fn restore_note_version(state: State<AppState>, id: String, timestamp: String) -> Result<Note, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    let mut index = read_index(&dirs);
    let entry = index.iter_mut().find(|n| n.id == id).ok_or("Note not found")?;

    // Snapshot the current state first so restoring doesn't lose it permanently.
    maybe_snapshot(&dirs, entry)?;

    let path = history_dir_for(&dirs, &id).join(format!("{}.json", timestamp));
    let data = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let parsed: HistoryEntry = serde_json::from_str(&data).map_err(|e| e.to_string())?;

    entry.title = parsed.title.clone();
    entry.updated_at = Utc::now().to_rfc3339();
    let meta = entry.clone();
    fs::write(note_path(&dirs, &id), &parsed.content).map_err(|e| e.to_string())?;
    write_index(&dirs, &index)?;
    Ok(Note { meta, content: parsed.content })
}
