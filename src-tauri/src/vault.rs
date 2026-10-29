use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use tauri::State;

/// The vault is a plain folder on disk: sub-folders are folders, `.md` files are notes.
/// Every path crossing the IPC boundary is relative to the vault root, using `/`.
pub struct AppState {
    pub root: Mutex<PathBuf>,
    pub config_path: PathBuf,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub vault: Option<PathBuf>,
    /// Data dir of the pre-rewrite app, only read for migration.
    #[serde(default)]
    pub data_dir: Option<PathBuf>,
}

impl Config {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self, path: &Path) -> Result<(), String> {
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, data).map_err(|e| e.to_string())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    name: String,
    path: String,
    is_dir: bool,
    children: Vec<Entry>,
}

fn root(state: &State<AppState>) -> PathBuf {
    state.root.lock().unwrap().clone()
}

fn resolve(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = Path::new(rel);
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(format!("invalid path: {}", rel.display()));
    }
    Ok(root.join(rel))
}

fn to_rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn clean_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    let name = name.strip_suffix(".md").unwrap_or(name).trim();
    if name.is_empty() || name.starts_with('.') {
        return Err("invalid name".into());
    }
    Ok(name
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c })
        .collect())
}

/// `dir/stem.ext`, or `dir/stem 1.ext`, `dir/stem 2.ext`, ... if taken.
fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let make = |n: u32| {
        let base = if n == 0 { stem.to_string() } else { format!("{stem} {n}") };
        dir.join(if ext.is_empty() { base } else { format!("{base}.{ext}") })
    };
    (0..).map(make).find(|p| !p.exists()).unwrap()
}

fn read_tree(root: &Path, dir: &Path) -> Vec<Entry> {
    let Ok(rd) = fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<Entry> = rd
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let file_name = e.file_name().to_string_lossy().into_owned();
            if file_name.starts_with('.') {
                return None;
            }
            let is_dir = p.is_dir();
            if !is_dir && p.extension().and_then(|x| x.to_str()) != Some("md") {
                return None;
            }
            Some(Entry {
                name: if is_dir {
                    file_name
                } else {
                    p.file_stem()?.to_string_lossy().into_owned()
                },
                path: to_rel(root, &p),
                children: if is_dir { read_tree(root, &p) } else { vec![] },
                is_dir,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

#[tauri::command]
pub fn get_vault(state: State<AppState>) -> String {
    root(&state).to_string_lossy().into_owned()
}

#[tauri::command]
pub fn set_vault(state: State<AppState>, path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    let mut cfg = Config::load(&state.config_path);
    cfg.vault = Some(path.clone());
    cfg.save(&state.config_path)?;
    *state.root.lock().unwrap() = path;
    Ok(())
}

#[tauri::command]
pub fn list_tree(state: State<AppState>) -> Vec<Entry> {
    let root = root(&state);
    read_tree(&root, &root)
}

#[tauri::command]
pub fn read_note(state: State<AppState>, path: String) -> Result<String, String> {
    fs::read_to_string(resolve(&root(&state), &path)?).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_note(state: State<AppState>, path: String, content: String) -> Result<(), String> {
    let p = resolve(&root(&state), &path)?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(p, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_note(state: State<AppState>, dir: String, name: String) -> Result<String, String> {
    let root = root(&state);
    let d = resolve(&root, &dir)?;
    fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let p = unique_path(&d, &clean_name(&name)?, "md");
    fs::write(&p, "").map_err(|e| e.to_string())?;
    Ok(to_rel(&root, &p))
}

#[tauri::command]
pub fn create_folder(state: State<AppState>, dir: String, name: String) -> Result<String, String> {
    let root = root(&state);
    let p = unique_path(&resolve(&root, &dir)?, &clean_name(&name)?, "");
    fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    Ok(to_rel(&root, &p))
}

#[tauri::command]
pub fn rename_entry(state: State<AppState>, path: String, name: String) -> Result<String, String> {
    let root = root(&state);
    let from = resolve(&root, &path)?;
    let mut name = clean_name(&name)?;
    if from.is_file() {
        name.push_str(".md");
    }
    let to = from.with_file_name(name);
    if to == from {
        return Ok(path);
    }
    if to.exists() {
        return Err("a file with that name already exists".into());
    }
    fs::rename(&from, &to).map_err(|e| e.to_string())?;
    Ok(to_rel(&root, &to))
}

#[tauri::command]
pub fn move_entry(state: State<AppState>, path: String, dir: String) -> Result<String, String> {
    let root = root(&state);
    let from = resolve(&root, &path)?;
    let target_dir = resolve(&root, &dir)?;
    if target_dir.starts_with(&from) {
        return Err("cannot move a folder into itself".into());
    }
    let to = target_dir.join(from.file_name().ok_or("invalid path")?);
    if to == from {
        return Ok(path);
    }
    if to.exists() {
        return Err("a file with that name already exists there".into());
    }
    fs::rename(&from, &to).map_err(|e| e.to_string())?;
    Ok(to_rel(&root, &to))
}

/// Deleted entries go to `<vault>/.trash` instead of being removed for good.
#[tauri::command]
pub fn delete_entry(state: State<AppState>, path: String) -> Result<(), String> {
    let root = root(&state);
    let from = resolve(&root, &path)?;
    let trash = root.join(".trash");
    fs::create_dir_all(&trash).map_err(|e| e.to_string())?;
    let stem = from.file_stem().ok_or("invalid path")?.to_string_lossy();
    let ext = from.extension().map(|e| e.to_string_lossy()).unwrap_or_default();
    fs::rename(&from, unique_path(&trash, &stem, &ext)).map_err(|e| e.to_string())
}

/// One-time import of notes from the old app format (`<id>.md` + `index.json`)
/// into the new vault, named by title.
pub fn migrate_legacy(old_base: &Path, vault: &Path) {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct OldMeta {
        id: String,
        title: String,
        #[serde(default)]
        trashed_at: Option<String>,
    }
    let notes_dir = old_base.join("notes");
    let Ok(data) = fs::read_to_string(notes_dir.join("index.json")) else { return };
    let Ok(index) = serde_json::from_str::<Vec<OldMeta>>(&data) else { return };
    for meta in index.into_iter().filter(|m| m.trashed_at.is_none()) {
        let Ok(content) = fs::read_to_string(notes_dir.join(format!("{}.md", meta.id))) else {
            continue;
        };
        let stem = clean_name(&meta.title).unwrap_or_else(|_| "Untitled".into());
        let _ = fs::write(unique_path(vault, &stem, "md"), content);
    }
}
