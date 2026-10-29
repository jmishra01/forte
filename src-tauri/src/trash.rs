use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

use crate::vault::{resolve, root, to_rel, unique_path, AppState};

/// Deleted notes and folders live in `<vault>/.trash`. Where each came from
/// is kept in `.trash/.origins.json` so it can be put back.
const DIR: &str = ".trash";
const ORIGINS: &str = ".origins.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Origin {
    path: String,
    /// Seconds since the Unix epoch.
    deleted_at: u64,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrashItem {
    /// File or folder name inside the trash.
    name: String,
    is_dir: bool,
    /// Where it was deleted from, if known (items trashed by older versions aren't).
    pub(crate) original: Option<String>,
    deleted_at: Option<u64>,
}

fn origins(root: &Path) -> BTreeMap<String, Origin> {
    fs::read_to_string(root.join(DIR).join(ORIGINS))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_origins(root: &Path, map: &BTreeMap<String, Origin>) -> Result<(), String> {
    let data = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    fs::write(root.join(DIR).join(ORIGINS), data).map_err(|e| e.to_string())
}

/// A trash entry by name; only a plain name, never a path.
fn entry(root: &Path, name: &str) -> Result<PathBuf, String> {
    let mut parts = Path::new(name).components();
    match (parts.next(), parts.next()) {
        (Some(Component::Normal(_)), None) if name != ORIGINS => Ok(root.join(DIR).join(name)),
        _ => Err(format!("invalid trash entry: {name}")),
    }
}

/// Moves `path` into the trash, remembering where it was.
pub fn move_to_trash(root: &Path, path: &str) -> Result<(), String> {
    let from = resolve(root, path)?;
    let dir = root.join(DIR);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let to = if from.is_dir() {
        unique_path(&dir, &from.file_name().ok_or("invalid path")?.to_string_lossy(), "")
    } else {
        let stem = from.file_stem().ok_or("invalid path")?.to_string_lossy();
        let ext = from.extension().map(|e| e.to_string_lossy()).unwrap_or_default();
        unique_path(&dir, &stem, &ext)
    };
    fs::rename(&from, &to).map_err(|e| e.to_string())?;
    let mut map = origins(root);
    let deleted_at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    map.insert(
        to.file_name().unwrap().to_string_lossy().into_owned(),
        Origin { path: to_rel(root, &from), deleted_at },
    );
    save_origins(root, &map)
}

/// What is in the trash, most recently deleted first.
pub fn list(root: &Path) -> Vec<TrashItem> {
    let map = origins(root);
    let Ok(rd) = fs::read_dir(root.join(DIR)) else { return vec![] };
    let mut items: Vec<TrashItem> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name == ORIGINS || name.starts_with('.') {
                return None;
            }
            let origin = map.get(&name);
            Some(TrashItem {
                is_dir: e.path().is_dir(),
                original: origin.map(|o| o.path.clone()),
                deleted_at: origin.map(|o| o.deleted_at),
                name,
            })
        })
        .collect();
    items.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at).then_with(|| a.name.cmp(&b.name)));
    items
}

/// Puts `name` back where it was deleted from (or the vault root if unknown),
/// under a free name if that spot is taken now. Returns its new path.
pub fn restore(root: &Path, name: &str) -> Result<String, String> {
    let from = entry(root, name)?;
    if !from.exists() {
        return Err(format!("\"{name}\" is no longer in the trash"));
    }
    let mut map = origins(root);
    let wanted = match map.get(name) {
        Some(o) => resolve(root, &o.path)?,
        None => root.join(name),
    };
    let parent = wanted.parent().unwrap_or(root).to_path_buf();
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    let to = if !wanted.exists() {
        wanted
    } else if from.is_dir() {
        unique_path(&parent, &wanted.file_name().unwrap().to_string_lossy(), "")
    } else {
        let stem = wanted.file_stem().unwrap_or_default().to_string_lossy();
        let ext = wanted.extension().map(|e| e.to_string_lossy()).unwrap_or_default();
        unique_path(&parent, &stem, &ext)
    };
    fs::rename(&from, &to).map_err(|e| e.to_string())?;
    map.remove(name);
    save_origins(root, &map)?;
    Ok(to_rel(root, &to))
}

/// Permanently deletes one entry from the trash.
pub fn delete_forever(root: &Path, name: &str) -> Result<(), String> {
    let p = entry(root, name)?;
    if p.is_dir() {
        fs::remove_dir_all(&p).map_err(|e| e.to_string())?;
    } else if p.exists() {
        fs::remove_file(&p).map_err(|e| e.to_string())?;
    }
    let mut map = origins(root);
    map.remove(name);
    save_origins(root, &map)
}

/// Permanently deletes everything in the trash.
pub fn empty(root: &Path) -> Result<(), String> {
    for item in list(root) {
        delete_forever(root, &item.name)?;
    }
    save_origins(root, &BTreeMap::new())
}

#[tauri::command]
pub fn list_trash(state: State<AppState>) -> Vec<TrashItem> {
    list(&root(&state))
}

#[tauri::command]
pub fn restore_from_trash(state: State<AppState>, name: String) -> Result<String, String> {
    restore(&root(&state), &name)
}

#[tauri::command]
pub fn delete_from_trash(state: State<AppState>, name: String) -> Result<(), String> {
    delete_forever(&root(&state), &name)
}

#[tauri::command]
pub fn empty_trash(state: State<AppState>) -> Result<(), String> {
    empty(&root(&state))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    #[test]
    fn restore_puts_notes_and_folders_back_where_they_were() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "work/plan.md", "plan");
        write(root, "old/a.md", "a");
        move_to_trash(root, "work/plan.md").unwrap();
        move_to_trash(root, "old").unwrap();
        fs::remove_dir(root.join("work")).unwrap(); // the folder it came from is gone too

        let items = list(root);
        assert_eq!(items.len(), 2);
        let plan = items.iter().find(|i| i.name == "plan.md").unwrap();
        assert_eq!(plan.original.as_deref(), Some("work/plan.md"));
        assert!(!plan.is_dir);
        assert!(items.iter().any(|i| i.name == "old" && i.is_dir));

        assert_eq!(restore(root, "plan.md").unwrap(), "work/plan.md");
        assert_eq!(fs::read_to_string(root.join("work/plan.md")).unwrap(), "plan");
        assert_eq!(restore(root, "old").unwrap(), "old");
        assert_eq!(fs::read_to_string(root.join("old/a.md")).unwrap(), "a");
        assert!(list(root).is_empty());
    }

    #[test]
    fn restore_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "n.md", "old");
        move_to_trash(root, "n.md").unwrap();
        write(root, "n.md", "new");
        assert_eq!(restore(root, "n.md").unwrap(), "n 1.md");
        assert_eq!(fs::read_to_string(root.join("n.md")).unwrap(), "new");
        assert_eq!(fs::read_to_string(root.join("n 1.md")).unwrap(), "old");
    }

    #[test]
    fn items_trashed_before_origins_existed_restore_to_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, ".trash/legacy.md", "x");
        let items = list(root);
        assert_eq!(items[0].original, None);
        assert_eq!(restore(root, "legacy.md").unwrap(), "legacy.md");
    }

    #[test]
    fn delete_forever_and_empty() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for n in ["a.md", "b.md", "c.md"] {
            write(root, n, n);
            move_to_trash(root, n).unwrap();
        }
        delete_forever(root, "a.md").unwrap();
        assert_eq!(list(root).len(), 2);
        empty(root).unwrap();
        assert!(list(root).is_empty());
        assert!(origins(root).is_empty());
        assert!(!root.join("b.md").exists());
    }

    #[test]
    fn entry_names_cannot_escape_the_trash() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "keep.md", "safe");
        for bad in ["../keep.md", "a/b.md", ".origins.json", "/etc/passwd", ".."] {
            assert!(delete_forever(root, bad).is_err(), "{bad}");
            assert!(restore(root, bad).is_err(), "{bad}");
        }
        assert!(root.join("keep.md").exists());
    }
}
