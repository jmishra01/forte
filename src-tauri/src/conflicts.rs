use serde::Serialize;
use std::fs;
use std::path::Path;
use tauri::State;

use crate::search::note_files;
use crate::trash::move_to_trash;
use crate::vault::{resolve, root, to_rel, unique_path, AppState};

/// A note that sync left in two versions.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub path: String,
    /// "copy": a second file a sync client wrote next to the note;
    /// "markers": one file containing `<<<<<<<`/`>>>>>>>` blocks from a Git merge.
    pub kind: &'static str,
    /// For copies, the note it is a version of, if that still exists.
    pub original: Option<String>,
}

/// The name a sync client's conflict copy was made from:
/// - Dropbox and Nextcloud: `Note (Ann's conflicted copy 2026-10-29).md`, `Note (conflicted copy …).md`
/// - Syncthing: `Note.sync-conflict-20261029-101500-ABCDEFG.md`
fn copy_of(stem: &str) -> Option<&str> {
    if let Some(i) = stem.find(".sync-conflict-") {
        return Some(&stem[..i]);
    }
    let open = stem.rfind(" (")?;
    (stem.ends_with(')') && stem[open..].contains("conflicted copy")).then(|| &stem[..open])
}

pub(crate) fn has_markers(text: &str) -> bool {
    let mut lines = text.lines();
    lines.any(|l| l.starts_with("<<<<<<< ") || l == "<<<<<<<")
        && lines.any(|l| l == "=======")
        && lines.any(|l| l.starts_with(">>>>>>> ") || l == ">>>>>>>")
}

pub fn find(root: &Path) -> Vec<Conflict> {
    let mut out = Vec::new();
    for file in note_files(root) {
        let path = to_rel(root, &file);
        let stem = file.file_stem().unwrap_or_default().to_string_lossy();
        if let Some(base) = copy_of(&stem) {
            let original = file.with_file_name(format!("{}.md", base.trim_end()));
            out.push(Conflict {
                path,
                kind: "copy",
                original: original.is_file().then(|| to_rel(root, &original)),
            });
        } else if fs::read_to_string(&file).is_ok_and(|t| has_markers(&t)) {
            out.push(Conflict { path, kind: "markers", original: None });
        }
    }
    out
}

/// Settles a conflict copy. `keep` is "original" (the copy goes to the trash),
/// "copy" (the original goes to the trash and the copy takes its place), or
/// "both" (the copy is renamed so it no longer looks like a conflict).
/// Returns the path of the note to show afterwards.
pub fn settle(root: &Path, copy: &str, keep: &str) -> Result<String, String> {
    let copy_path = resolve(root, copy)?;
    let stem = copy_path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let base = copy_of(&stem).ok_or("not a conflict copy")?.trim_end().to_string();
    let original = copy_path.with_file_name(format!("{base}.md"));
    match keep {
        "original" => {
            move_to_trash(root, copy)?;
            Ok(to_rel(root, &original))
        }
        "copy" => {
            if original.exists() {
                move_to_trash(root, &to_rel(root, &original))?;
            }
            fs::rename(&copy_path, &original).map_err(|e| e.to_string())?;
            Ok(to_rel(root, &original))
        }
        "both" => {
            let dir = copy_path.parent().unwrap_or(root);
            let to = unique_path(dir, &format!("{base} (other version)"), "md");
            fs::rename(&copy_path, &to).map_err(|e| e.to_string())?;
            Ok(to_rel(root, &to))
        }
        _ => Err(format!("unknown choice: {keep}")),
    }
}

#[tauri::command]
pub fn find_conflicts(state: State<AppState>) -> Vec<Conflict> {
    find(&root(&state))
}

#[tauri::command]
pub fn settle_conflict(state: State<AppState>, path: String, keep: String) -> Result<String, String> {
    settle(&root(&state), &path, &keep)
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
    fn recognises_sync_client_copies() {
        assert_eq!(copy_of("Plan (Ann's conflicted copy 2026-10-29)"), Some("Plan"));
        assert_eq!(copy_of("Plan (conflicted copy 2026-10-29 101500)"), Some("Plan"));
        assert_eq!(copy_of("Plan.sync-conflict-20261029-101500-ABCDEFG"), Some("Plan"));
        assert_eq!(copy_of("Plan (draft)"), None);
        assert_eq!(copy_of("Plan 2"), None);
    }

    #[test]
    fn finds_copies_and_marker_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "work/Plan.md", "mine");
        write(root, "work/Plan (Ann's conflicted copy 2026-10-29).md", "theirs");
        write(root, "Orphan.sync-conflict-20261029-101500-ABC.md", "x");
        write(root, "Merged.md", "a\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> main\nb\n");
        write(root, "Fine.md", "a ======= b\n");
        let mut found = find(root);
        found.sort_by(|a, b| a.path.cmp(&b.path));
        assert_eq!(
            found,
            vec![
                Conflict { path: "Merged.md".into(), kind: "markers", original: None },
                Conflict { path: "Orphan.sync-conflict-20261029-101500-ABC.md".into(), kind: "copy", original: None },
                Conflict {
                    path: "work/Plan (Ann's conflicted copy 2026-10-29).md".into(),
                    kind: "copy",
                    original: Some("work/Plan.md".into()),
                },
            ]
        );
    }

    #[test]
    fn settle_keeps_the_chosen_version_and_trashes_the_other() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let copy = "Plan (conflicted copy 2026-10-29).md";

        write(root, "Plan.md", "mine");
        write(root, copy, "theirs");
        assert_eq!(settle(root, copy, "original").unwrap(), "Plan.md");
        assert_eq!(fs::read_to_string(root.join("Plan.md")).unwrap(), "mine");
        assert!(!root.join(copy).exists());

        write(root, copy, "theirs");
        assert_eq!(settle(root, copy, "copy").unwrap(), "Plan.md");
        assert_eq!(fs::read_to_string(root.join("Plan.md")).unwrap(), "theirs");
        assert!(crate::trash::list(root).iter().any(|i| i.original.as_deref() == Some("Plan.md")));

        write(root, copy, "third");
        assert_eq!(settle(root, copy, "both").unwrap(), "Plan (other version).md");
        assert!(find(root).is_empty());

        assert!(settle(root, "Plan.md", "copy").is_err());
        assert!(settle(root, "../x (conflicted copy 1).md", "copy").is_err());
    }
}
