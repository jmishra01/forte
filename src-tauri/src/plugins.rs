//! Plugins are folders in `<vault>/.forte/plugins/<id>/` with a `manifest.json`
//! and a script. This only finds and reads them; the page runs each approved
//! one in a sandboxed Web Worker and decides what it may do.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use tauri::State;

use crate::vault::{root, AppState};

const MAX_CODE_BYTES: u64 = 1024 * 1024;
/// What a plugin may ask for; anything else in a manifest is ignored.
const KNOWN_PERMISSIONS: &[&str] = &["editor", "notes:read", "notes:write"];

#[derive(Deserialize)]
struct Manifest {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    permissions: Vec<String>,
    #[serde(default = "default_main")]
    main: String,
}

fn default_main() -> String {
    "plugin.js".into()
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Plugin {
    /// The plugin's folder name.
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub permissions: Vec<String>,
    pub code: String,
    /// SHA-256 of the code, so approval can be tied to exactly what was reviewed.
    pub hash: String,
    /// Why it can't be loaded, if it can't.
    pub error: Option<String>,
}

pub fn list(root: &Path) -> Vec<Plugin> {
    let Ok(rd) = fs::read_dir(root.join(".forte").join("plugins")) else { return vec![] };
    let mut out: Vec<Plugin> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            (!id.starts_with('.')).then(|| load(&e.path(), id))
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn load(dir: &Path, id: String) -> Plugin {
    let broken = |error: String| Plugin {
        name: id.clone(),
        id: id.clone(),
        description: String::new(),
        version: String::new(),
        permissions: vec![],
        code: String::new(),
        hash: String::new(),
        error: Some(error),
    };
    let manifest: Manifest = match fs::read_to_string(dir.join("manifest.json")).map(|s| serde_json::from_str(&s)) {
        Ok(Ok(m)) => m,
        Ok(Err(e)) => return broken(format!("manifest.json isn't valid: {e}")),
        Err(_) => return broken("manifest.json is missing".into()),
    };
    // The script must be a plain file name inside the plugin's folder.
    if manifest.main.contains(['/', '\\']) || manifest.main.starts_with('.') {
        return broken(format!("\"main\" must be a file in the plugin folder, not {}", manifest.main));
    }
    let script = dir.join(&manifest.main);
    match fs::metadata(&script) {
        Ok(m) if m.len() > MAX_CODE_BYTES => return broken("the script is larger than 1 MB".into()),
        Ok(_) => {}
        Err(_) => return broken(format!("{} is missing", manifest.main)),
    }
    let Ok(code) = fs::read_to_string(&script) else { return broken(format!("{} isn't readable text", manifest.main)) };
    let hash = format!("{:x}", Sha256::digest(code.as_bytes()));
    Plugin {
        id,
        name: manifest.name,
        description: manifest.description,
        version: manifest.version,
        permissions: manifest.permissions.into_iter().filter(|p| KNOWN_PERMISSIONS.contains(&p.as_str())).collect(),
        code,
        hash,
        error: None,
    }
}

#[tauri::command]
pub fn list_plugins(state: State<AppState>) -> Vec<Plugin> {
    list(&root(&state))
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
    fn lists_plugins_with_a_hash_and_known_permissions_only() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, ".forte/plugins/wc/manifest.json", r#"{"name":"Word count","permissions":["editor","root:shell"]}"#);
        write(root, ".forte/plugins/wc/plugin.js", "forte.registerCommand('x', () => {})");
        let plugins = list(root);
        assert_eq!(plugins.len(), 1);
        let p = &plugins[0];
        assert_eq!((p.id.as_str(), p.name.as_str(), p.error.as_deref()), ("wc", "Word count", None));
        assert_eq!(p.permissions, ["editor"]);
        assert_eq!(p.hash.len(), 64);
        // Any change to the code changes the hash.
        write(root, ".forte/plugins/wc/plugin.js", "forte.registerCommand('y', () => {})");
        assert_ne!(list(root)[0].hash, p.hash);
    }

    #[test]
    fn broken_plugins_say_why() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, ".forte/plugins/a/manifest.json", "{not json");
        write(root, ".forte/plugins/b/manifest.json", r#"{"name":"B","main":"../../../secret.js"}"#);
        write(root, ".forte/plugins/c/manifest.json", r#"{"name":"C"}"#);
        let errors: Vec<_> = list(root).into_iter().map(|p| (p.id, p.error.is_some(), p.code)).collect();
        assert!(errors.iter().all(|(_, broken, code)| *broken && code.is_empty()), "{errors:?}");
        assert!(list(&root.join("nowhere")).is_empty());
    }
}
