use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

use crate::{search, trash, watch};

/// The vault is a plain folder on disk: sub-folders are folders, `.md` files are notes.
/// Every path crossing the IPC boundary is relative to the vault root, using `/`.
pub struct AppState {
    pub root: Mutex<PathBuf>,
    pub config_path: PathBuf,
    /// Watches `root` for changes made outside the app; replaced when the vault changes.
    pub watcher: Mutex<Option<notify::RecommendedWatcher>>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub vault: Option<PathBuf>,
    /// Data dir of the pre-rewrite app, only read for migration.
    #[serde(default)]
    pub data_dir: Option<PathBuf>,
    #[serde(default)]
    pub settings: serde_json::Value,
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

pub(crate) fn root(state: &State<AppState>) -> PathBuf {
    state.root.lock().unwrap().clone()
}

pub(crate) fn resolve(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = Path::new(rel);
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(format!("invalid path: {}", rel.display()));
    }
    Ok(root.join(rel))
}

pub(crate) fn to_rel(root: &Path, p: &Path) -> String {
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
pub(crate) fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
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

/// Lets the preview load images from the vault (and only the vault) over the asset protocol.
pub fn allow_assets(app: &AppHandle, root: &Path) {
    let _ = app.asset_protocol_scope().allow_directory(root, true);
}

#[tauri::command]
pub fn get_vault(state: State<AppState>) -> String {
    root(&state).to_string_lossy().into_owned()
}

#[tauri::command]
pub fn set_vault(app: AppHandle, state: State<AppState>, path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    let mut cfg = Config::load(&state.config_path);
    cfg.vault = Some(path.clone());
    cfg.save(&state.config_path)?;
    allow_assets(&app, &path);
    *state.watcher.lock().unwrap() = watch::start(app, &path);
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
    write(&root(&state), &path, &content)
}

#[tauri::command]
pub fn create_note(state: State<AppState>, dir: String, name: String) -> Result<String, String> {
    create_note_in(&root(&state), &dir, &name)
}

#[tauri::command]
pub fn create_folder(state: State<AppState>, dir: String, name: String) -> Result<String, String> {
    create_folder_in(&root(&state), &dir, &name)
}

#[tauri::command]
pub fn rename_entry(state: State<AppState>, path: String, name: String) -> Result<String, String> {
    rename(&root(&state), &path, &name)
}

#[tauri::command]
pub fn move_entry(state: State<AppState>, path: String, dir: String) -> Result<String, String> {
    move_to(&root(&state), &path, &dir)
}

#[tauri::command]
pub fn delete_entry(state: State<AppState>, path: String) -> Result<(), String> {
    trash::move_to_trash(&root(&state), &path)
}

#[tauri::command]
pub fn search_notes(state: State<AppState>, query: String) -> Vec<search::Hit> {
    search::search(&root(&state), &query, 200)
}

#[tauri::command]
pub fn note_index(state: State<AppState>) -> Vec<search::NoteInfo> {
    search::index(&root(&state))
}

/// UI preferences, kept as an opaque JSON object in the config file.
#[tauri::command]
pub fn get_settings(state: State<AppState>) -> serde_json::Value {
    Config::load(&state.config_path).settings
}

#[tauri::command]
pub fn set_settings(state: State<AppState>, settings: serde_json::Value) -> Result<(), String> {
    let mut cfg = Config::load(&state.config_path);
    cfg.settings = settings;
    cfg.save(&state.config_path)
}

/// Asks where to save, then writes `html` there. Images written as
/// `src="vault:<path>"` are embedded from the vault. Returns where it was saved,
/// or None if the user cancelled. The path never comes from the webview.
#[tauri::command]
pub async fn export_html(app: AppHandle, name: String, html: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let root = root(&app.state::<AppState>());
    let Some(target) = app
        .dialog()
        .file()
        .add_filter("Web page", &["html"])
        .set_file_name(format!("{}.html", clean_name(&name).unwrap_or_else(|_| "Note".into())))
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let target = target.into_path().map_err(|e| e.to_string())?;
    fs::write(&target, embed_images(&root, &html)).map_err(|e| e.to_string())?;
    Ok(Some(target.to_string_lossy().into_owned()))
}

/// Saves the request body (an image pasted or dropped into a note) under
/// `<vault>/attachments` and returns its vault-relative path. The file name
/// comes in the `x-name` header, URL-encoded.
#[tauri::command]
pub fn save_attachment(state: State<AppState>, request: tauri::ipc::Request) -> Result<String, String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected raw bytes".into());
    };
    let name = request
        .headers()
        .get("x-name")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode)
        .unwrap_or_default();
    save_attachment_in(&root(&state), &name, bytes)
}

// The commands above are thin wrappers so the logic below can be tested
// against a plain folder.

fn write(root: &Path, path: &str, content: &str) -> Result<(), String> {
    let p = resolve(root, path)?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(p, content).map_err(|e| e.to_string())
}

fn create_note_in(root: &Path, dir: &str, name: &str) -> Result<String, String> {
    let d = resolve(root, dir)?;
    fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let p = unique_path(&d, &clean_name(name)?, "md");
    fs::write(&p, "").map_err(|e| e.to_string())?;
    Ok(to_rel(root, &p))
}

fn create_folder_in(root: &Path, dir: &str, name: &str) -> Result<String, String> {
    let p = unique_path(&resolve(root, dir)?, &clean_name(name)?, "");
    fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    Ok(to_rel(root, &p))
}

fn rename(root: &Path, path: &str, name: &str) -> Result<String, String> {
    let from = resolve(root, path)?;
    let mut name = clean_name(name)?;
    if from.is_file() {
        name.push_str(".md");
    }
    let to = from.with_file_name(name);
    if to == from {
        return Ok(path.to_string());
    }
    if to.exists() {
        return Err("a file with that name already exists".into());
    }
    fs::rename(&from, &to).map_err(|e| e.to_string())?;
    Ok(to_rel(root, &to))
}

fn move_to(root: &Path, path: &str, dir: &str) -> Result<String, String> {
    let from = resolve(root, path)?;
    let target_dir = resolve(root, dir)?;
    if target_dir.starts_with(&from) {
        return Err("cannot move a folder into itself".into());
    }
    let to = target_dir.join(from.file_name().ok_or("invalid path")?);
    if to == from {
        return Ok(path.to_string());
    }
    if to.exists() {
        return Err("a file with that name already exists there".into());
    }
    fs::rename(&from, &to).map_err(|e| e.to_string())?;
    Ok(to_rel(root, &to))
}

fn save_attachment_in(root: &Path, name: &str, bytes: &[u8]) -> Result<String, String> {
    let dir = root.join("attachments");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = Path::new(name);
    let ext = file.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = clean_name(&stem).unwrap_or_else(|_| "attachment".into());
    let ext = if ext.chars().all(|c| c.is_ascii_alphanumeric()) { ext } else { String::new() };
    let p = unique_path(&dir, &stem, &ext);
    fs::write(&p, bytes).map_err(|e| e.to_string())?;
    Ok(to_rel(root, &p))
}

/// Replaces `src="vault:<path>"` with the file's contents as a data URL.
/// Paths outside the vault, missing files and non-images are left out.
fn embed_images(root: &Path, html: &str) -> String {
    const MARK: &str = "src=\"vault:";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(i) = rest.find(MARK) {
        out.push_str(&rest[..i]);
        rest = &rest[i + MARK.len()..];
        let end = rest.find('"').unwrap_or(rest.len());
        let rel = percent_decode(&rest[..end]);
        rest = &rest[(end + 1).min(rest.len())..];
        out.push_str("src=\"");
        if let Some(data) = data_url(root, &rel) {
            out.push_str(&data);
        }
        out.push('"');
    }
    out.push_str(rest);
    out
}

fn data_url(root: &Path, rel: &str) -> Option<String> {
    use base64::Engine;
    let p = resolve(root, rel).ok()?;
    let mime = match p.extension()?.to_str()?.to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        _ => return None,
    };
    let bytes = fs::read(p).ok()?;
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = (b[i] == b'%' && i + 2 < b.len())
            .then(|| std::str::from_utf8(&b[i + 1..i + 3]).ok())
            .flatten()
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match hex {
            Some(v) => {
                out.push(v);
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

/// First launch under the current app identifier: pick up the config and
/// vault of an install made under the old one (their folders are named after
/// the identifier). Nothing is moved or deleted; the old folders stay as they are.
pub fn adopt_old_install(config_path: &Path, old_config_dir: &Path, old_data_dir: &Path) {
    if config_path.exists() {
        return;
    }
    let mut cfg = Config::load(&old_config_dir.join("config.json"));
    if cfg.vault.is_none() && old_data_dir.join("vault").is_dir() {
        cfg.vault = Some(old_data_dir.join("vault"));
    }
    // Notes from before the markdown rewrite are imported from here if no vault exists yet.
    if cfg.data_dir.is_none() && old_data_dir.is_dir() {
        cfg.data_dir = Some(old_data_dir.to_path_buf());
    }
    if cfg.vault.is_some() || cfg.data_dir.is_some() {
        let _ = cfg.save(config_path);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn vault() -> (TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        (dir, root)
    }

    fn names(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn resolve_rejects_paths_leaving_the_vault() {
        let root = Path::new("/v");
        assert_eq!(resolve(root, "a/b.md").unwrap(), root.join("a/b.md"));
        for bad in ["../x.md", "a/../../x.md", "/etc/passwd", "./x.md"] {
            assert!(resolve(root, bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn clean_name_strips_extension_and_unsafe_characters() {
        assert_eq!(clean_name("  Plan.md ").unwrap(), "Plan");
        assert_eq!(clean_name("a/b:c?").unwrap(), "a-b-c-");
        assert!(clean_name("   ").is_err());
        assert!(clean_name(".hidden").is_err());
    }

    #[test]
    fn create_note_picks_a_free_name() {
        let (_d, root) = vault();
        assert_eq!(create_note_in(&root, "", "Idea").unwrap(), "Idea.md");
        assert_eq!(create_note_in(&root, "", "Idea").unwrap(), "Idea 1.md");
        assert_eq!(create_note_in(&root, "work/q4", "Idea").unwrap(), "work/q4/Idea.md");
        assert!(create_note_in(&root, "..", "Idea").is_err());
    }

    #[test]
    fn write_then_read_round_trips() {
        let (_d, root) = vault();
        write(&root, "deep/dir/n.md", "hello").unwrap();
        assert_eq!(fs::read_to_string(root.join("deep/dir/n.md")).unwrap(), "hello");
        assert!(write(&root, "../escape.md", "x").is_err());
    }

    #[test]
    fn tree_lists_folders_first_and_hides_dotfiles_and_non_notes() {
        let (_d, root) = vault();
        write(&root, "b.md", "").unwrap();
        write(&root, "A.md", "").unwrap();
        write(&root, "image.png", "").unwrap();
        write(&root, ".trash/old.md", "").unwrap();
        write(&root, "zfolder/inner.md", "").unwrap();
        let tree = read_tree(&root, &root);
        assert_eq!(names(&tree), ["zfolder", "A", "b"]);
        assert_eq!(tree[0].children[0].path, "zfolder/inner.md");
    }

    #[test]
    fn rename_keeps_extension_and_refuses_to_overwrite() {
        let (_d, root) = vault();
        write(&root, "a.md", "A").unwrap();
        write(&root, "b.md", "B").unwrap();
        assert_eq!(rename(&root, "a.md", "c").unwrap(), "c.md");
        assert!(rename(&root, "c.md", "b").is_err());
        assert_eq!(fs::read_to_string(root.join("b.md")).unwrap(), "B");
        assert_eq!(rename(&root, "c.md", "c").unwrap(), "c.md");
    }

    #[test]
    fn move_refuses_overwrite_and_folder_into_itself() {
        let (_d, root) = vault();
        write(&root, "n.md", "top").unwrap();
        write(&root, "f/n.md", "nested").unwrap();
        create_folder_in(&root, "f", "sub").unwrap();
        assert!(move_to(&root, "n.md", "f").is_err());
        assert!(move_to(&root, "f", "f/sub").is_err());
        assert_eq!(move_to(&root, "f/n.md", "f/sub").unwrap(), "f/sub/n.md");
        assert_eq!(move_to(&root, "n.md", "f").unwrap(), "f/n.md");
    }

    #[test]
    fn delete_moves_to_trash_without_overwriting() {
        let (_d, root) = vault();
        write(&root, "n.md", "first").unwrap();
        trash::move_to_trash(&root, "n.md").unwrap();
        write(&root, "n.md", "second").unwrap();
        trash::move_to_trash(&root, "n.md").unwrap();
        assert!(!root.join("n.md").exists());
        assert_eq!(fs::read_to_string(root.join(".trash/n.md")).unwrap(), "first");
        assert_eq!(fs::read_to_string(root.join(".trash/n 1.md")).unwrap(), "second");
    }

    #[test]
    fn attachments_get_clean_unique_names() {
        let (_d, root) = vault();
        assert_eq!(save_attachment_in(&root, "image.PNG", b"1").unwrap(), "attachments/image.png");
        assert_eq!(save_attachment_in(&root, "image.png", b"2").unwrap(), "attachments/image 1.png");
        assert_eq!(save_attachment_in(&root, "../../evil.sh", b"3").unwrap(), "attachments/evil.sh");
        assert_eq!(save_attachment_in(&root, "", b"4").unwrap(), "attachments/attachment");
        assert_eq!(fs::read(root.join("attachments/image 1.png")).unwrap(), b"2");
    }

    #[test]
    fn percent_decode_handles_utf8_and_bad_escapes() {
        assert_eq!(percent_decode("caf%C3%A9%20menu.png"), "café menu.png");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn export_embeds_vault_images_only() {
        let (_d, root) = vault();
        write(&root, "attachments/a b.png", "PNG").unwrap();
        let html = r#"<img src="vault:attachments/a%20b.png"><img src="vault:../secret.png"><img src="https://x/y.png">"#;
        assert_eq!(
            embed_images(&root, html),
            r#"<img src="data:image/png;base64,UE5H"><img src=""><img src="https://x/y.png">"#
        );
    }

    #[test]
    fn adopts_old_install_config_and_default_vault() {
        let (_d, base) = vault();
        let (new_cfg, old_cfg, old_data) = (base.join("new/config.json"), base.join("oldcfg"), base.join("olddata"));
        fs::create_dir_all(new_cfg.parent().unwrap()).unwrap();

        // Nothing old: nothing written.
        adopt_old_install(&new_cfg, &old_cfg, &old_data);
        assert!(!new_cfg.exists());

        // Old default vault, no old config.
        fs::create_dir_all(old_data.join("vault")).unwrap();
        adopt_old_install(&new_cfg, &old_cfg, &old_data);
        let cfg = Config::load(&new_cfg);
        assert_eq!(cfg.vault, Some(old_data.join("vault")));
        assert_eq!(cfg.data_dir, Some(old_data.clone()));

        // An existing new config is never replaced.
        fs::write(&new_cfg, r#"{"vault":"/mine"}"#).unwrap();
        adopt_old_install(&new_cfg, &old_cfg, &old_data);
        assert_eq!(Config::load(&new_cfg).vault, Some(PathBuf::from("/mine")));
    }

    #[test]
    fn adopts_old_config_with_custom_vault_and_settings() {
        let (_d, base) = vault();
        let (new_cfg, old_cfg, old_data) = (base.join("config.json"), base.join("oldcfg"), base.join("nodata"));
        fs::create_dir_all(&old_cfg).unwrap();
        fs::write(old_cfg.join("config.json"), r#"{"vault":"/Users/me/Notes","settings":{"theme":"dark"}}"#).unwrap();
        adopt_old_install(&new_cfg, &old_cfg, &old_data);
        let cfg = Config::load(&new_cfg);
        assert_eq!(cfg.vault, Some(PathBuf::from("/Users/me/Notes")));
        assert_eq!(cfg.settings["theme"], "dark");
        assert_eq!(cfg.data_dir, None);
    }

    #[test]
    fn migrate_legacy_imports_untrashed_notes_by_title() {
        let (_d, base) = vault();
        let notes = base.join("notes");
        fs::create_dir_all(&notes).unwrap();
        fs::write(
            notes.join("index.json"),
            r#"[{"id":"1","title":"Kept"},{"id":"2","title":"Gone","trashedAt":"2025-01-01"}]"#,
        )
        .unwrap();
        fs::write(notes.join("1.md"), "kept body").unwrap();
        fs::write(notes.join("2.md"), "gone body").unwrap();
        let vault = base.join("vault");
        fs::create_dir_all(&vault).unwrap();
        migrate_legacy(&base, &vault);
        assert_eq!(fs::read_to_string(vault.join("Kept.md")).unwrap(), "kept body");
        assert!(!vault.join("Gone.md").exists());
    }
}
