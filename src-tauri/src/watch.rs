use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Runtime};

/// Event sent to the frontend with the vault-relative paths that changed on disk.
pub const EVENT: &str = "vault-changed";

/// Changes arriving within this window are sent as one event, so a burst
/// (a sync client, `git checkout`, our own save) causes a single refresh.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// Starts watching `root`. Dropping the returned watcher stops it, and its
/// background thread exits with it. Returns `None` if the folder can't be watched;
/// the app still works, it just won't notice outside edits.
pub fn start<R: Runtime>(app: AppHandle<R>, root: &Path) -> Option<RecommendedWatcher> {
    // Some platforms report canonical paths (e.g. /private/var on macOS).
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let (tx, rx) = mpsc::channel::<PathBuf>();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        if matches!(event.kind, EventKind::Access(_)) {
            return;
        }
        for path in event.paths {
            let _ = tx.send(path);
        }
    })
    .ok()?;
    watcher.watch(&root, RecursiveMode::Recursive).ok()?;

    std::thread::spawn(move || {
        // Ends when the watcher (and with it `tx`) is dropped.
        while let Ok(first) = rx.recv() {
            let mut batch = BTreeSet::new();
            batch.extend(relevant(&root, &first));
            while let Ok(p) = rx.recv_timeout(DEBOUNCE) {
                batch.extend(relevant(&root, &p));
            }
            if !batch.is_empty() {
                let _ = app.emit(EVENT, batch.into_iter().collect::<Vec<_>>());
            }
        }
    });
    Some(watcher)
}

/// The vault-relative path of `p`, unless it is outside the vault or hidden
/// (`.trash`, `.git`, editor swap files, ...), which the app never shows.
fn relevant(root: &Path, p: &Path) -> Option<String> {
    let rel = p.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for c in rel.components() {
        let Component::Normal(s) = c else { return None };
        let s = s.to_string_lossy();
        if s.starts_with('.') {
            return None;
        }
        parts.push(s.into_owned());
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relevant_paths_are_vault_relative() {
        let root = Path::new("/v");
        assert_eq!(relevant(root, Path::new("/v/a/b.md")).as_deref(), Some("a/b.md"));
        assert_eq!(relevant(root, Path::new("/v/a")).as_deref(), Some("a"));
    }

    #[test]
    fn hidden_and_outside_paths_are_ignored() {
        let root = Path::new("/v");
        assert_eq!(relevant(root, Path::new("/v/.trash/x.md")), None);
        assert_eq!(relevant(root, Path::new("/v/a/.b.md.swp")), None);
        assert_eq!(relevant(root, Path::new("/elsewhere/x.md")), None);
        assert_eq!(relevant(root, Path::new("/v")), None);
    }

    #[test]
    fn emits_vault_relative_paths_on_change() {
        use tauri::Listener;
        let app = tauri::test::mock_app();
        let (tx, rx) = mpsc::channel::<Vec<String>>();
        app.listen_any(EVENT, move |e| {
            let _ = tx.send(serde_json::from_str(e.payload()).unwrap());
        });
        let dir = tempfile::tempdir().unwrap();
        let _watcher = start(app.handle().clone(), dir.path()).expect("watcher");
        std::thread::sleep(Duration::from_millis(300)); // let the OS watcher settle

        std::fs::write(dir.path().join("a.md"), "x").unwrap();
        std::fs::write(dir.path().join("b.md"), "y").unwrap();
        std::fs::create_dir(dir.path().join(".trash")).unwrap();

        let mut seen = BTreeSet::new();
        while let Ok(paths) = rx.recv_timeout(Duration::from_secs(3)) {
            seen.extend(paths);
            if seen.contains("a.md") && seen.contains("b.md") {
                break;
            }
        }
        assert!(seen.contains("a.md") && seen.contains("b.md"), "saw {seen:?}");
        assert!(seen.iter().all(|p| !p.starts_with('.')), "saw {seen:?}");
    }
}
