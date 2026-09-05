use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct Dirs {
    pub base: PathBuf,
    pub notes_dir: PathBuf,
    pub history_dir: PathBuf,
    pub pdfs_dir: PathBuf,
    pub annotations_dir: PathBuf,
    pub attachments_dir: PathBuf,
}

impl Dirs {
    pub fn new(base: PathBuf) -> std::io::Result<Self> {
        let notes_dir = base.join("notes");
        let history_dir = notes_dir.join(".history");
        let pdfs_dir = base.join("pdfs");
        let annotations_dir = pdfs_dir.join(".annotations");
        let attachments_dir = base.join("attachments");
        fs::create_dir_all(&notes_dir)?;
        fs::create_dir_all(&history_dir)?;
        fs::create_dir_all(&pdfs_dir)?;
        fs::create_dir_all(&annotations_dir)?;
        fs::create_dir_all(&attachments_dir)?;
        Ok(Self {
            base,
            notes_dir,
            history_dir,
            pdfs_dir,
            annotations_dir,
            attachments_dir,
        })
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StoredConfig {
    data_dir: Option<String>,
}

pub struct AppState {
    pub dirs: Mutex<Dirs>,
    pub config_path: PathBuf,
}

impl AppState {
    pub fn load_data_dir_override(config_path: &Path) -> Option<PathBuf> {
        let data = fs::read_to_string(config_path).ok()?;
        let cfg: StoredConfig = serde_json::from_str(&data).ok()?;
        cfg.data_dir.map(PathBuf::from)
    }

    pub fn save_data_dir(&self, data_dir: &Path) -> Result<(), String> {
        let cfg = StoredConfig {
            data_dir: Some(data_dir.to_string_lossy().to_string()),
        };
        let data = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
        fs::write(&self.config_path, data).map_err(|e| e.to_string())
    }
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}
