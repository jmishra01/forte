use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::State;

use crate::conflicts::has_markers;
use crate::vault::{root, AppState};

/// Where the vault stands with Git. Everything is limited to the vault folder,
/// which may be the whole repository or a folder inside one.
#[derive(Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    /// False when git isn't installed or the vault isn't in a repository.
    pub is_repo: bool,
    pub branch: Option<String>,
    /// Whether the branch tracks a remote branch to pull from and push to.
    pub has_upstream: bool,
    /// Uncommitted changed, added or deleted files in the vault.
    pub changes: usize,
    pub ahead: usize,
    pub behind: usize,
    /// Files from a merge that still contain conflict markers, relative to the vault.
    /// Once the markers are edited out, the next sync marks them resolved.
    pub conflicted: Vec<String>,
    /// A merge is in progress (finish it by resolving conflicts and syncing again).
    pub merging: bool,
}

/// `git <args>` in `dir`, never prompting: a missing credential fails instead of hanging.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("couldn't run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let msg = err.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("git failed");
        Err(msg.trim_start_matches("fatal: ").trim_start_matches("error: ").to_string())
    }
}

pub fn status(root: &Path) -> GitStatus {
    if git(root, &["rev-parse", "--is-inside-work-tree"]).is_err() {
        return GitStatus::default();
    }
    let mut st = GitStatus { is_repo: true, ..Default::default() };
    st.merging = git(root, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).is_ok();
    // Paths relative to the vault (the current directory), limited to it.
    let Ok(out) = git(root, &["-c", "status.relativePaths=true", "status", "--porcelain=v1", "-b", "--", "."]) else {
        return st;
    };
    for line in out.lines() {
        if let Some(head) = line.strip_prefix("## ") {
            parse_branch(head, &mut st);
        } else if line.len() > 3 {
            let (xy, path) = line.split_at(3);
            let path = path.trim_matches('"');
            let unmerged = matches!(xy.trim_end(), "UU" | "AA" | "DD" | "AU" | "UA" | "DU" | "UD");
            if unmerged && std::fs::read_to_string(root.join(path)).is_ok_and(|t| has_markers(&t)) {
                st.conflicted.push(path.to_string());
            }
            st.changes += 1;
        }
    }
    st
}

/// `main...origin/main [ahead 1, behind 2]`, or `No commits yet on main`.
fn parse_branch(head: &str, st: &mut GitStatus) {
    let head = head.strip_prefix("No commits yet on ").unwrap_or(head);
    let (names, counts) = head.split_once(" [").unwrap_or((head, ""));
    let (branch, upstream) = names.split_once("...").unwrap_or((names, ""));
    st.branch = Some(branch.to_string());
    st.has_upstream = !upstream.is_empty();
    for part in counts.trim_end_matches(']').split(", ") {
        if let Some(n) = part.strip_prefix("ahead ") {
            st.ahead = n.parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            st.behind = n.parse().unwrap_or(0);
        }
    }
}

/// Commits the vault's changes, merges in the remote branch, and pushes.
/// Stops with conflicts in place if the merge needs a person; syncing again
/// after fixing them finishes the merge.
pub fn sync(root: &Path, message: &str) -> Result<GitStatus, String> {
    let before = status(root);
    if !before.is_repo {
        return Err("this notes folder isn't in a Git repository".into());
    }
    if !before.conflicted.is_empty() {
        return Ok(before); // nothing to do until the conflicts are fixed
    }
    git(root, &["add", "-A", "--", "."])?;
    let staged = git(root, &["diff", "--cached", "--quiet"]).is_err();
    if before.merging {
        git(root, &["commit", "--no-edit"])?;
    } else if staged {
        git(root, &["commit", "-m", message, "--", "."])?;
    }
    if before.has_upstream {
        git(root, &["fetch", "--quiet"])?;
        // --ff: fast-forward when possible, merge when not, whatever pull.rebase/pull.ff say.
        if let Err(e) = git(root, &["pull", "--no-rebase", "--ff", "--no-edit"]) {
            let after = status(root);
            return if after.conflicted.is_empty() { Err(e) } else { Ok(after) };
        }
        if status(root).ahead > 0 {
            git(root, &["push", "--quiet"])?;
        }
    }
    Ok(status(root))
}

/// Starts a repository in the vault and commits what is there.
pub fn init(root: &Path) -> Result<GitStatus, String> {
    if status(root).is_repo {
        return Err("this notes folder is already in a Git repository".into());
    }
    git(root, &["init", "--quiet"])?;
    let ignore: PathBuf = root.join(".gitignore");
    if !ignore.exists() {
        std::fs::write(&ignore, ".trash/\n.DS_Store\n").map_err(|e| e.to_string())?;
    }
    git(root, &["add", "-A"])?;
    git(root, &["commit", "--quiet", "-m", "Start notes history"])?;
    Ok(status(root))
}

#[tauri::command]
pub async fn git_status(state: State<'_, AppState>) -> Result<GitStatus, String> {
    let root = root(&state);
    tauri::async_runtime::spawn_blocking(move || status(&root)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_sync(state: State<'_, AppState>, message: String) -> Result<GitStatus, String> {
    let root = root(&state);
    tauri::async_runtime::spawn_blocking(move || sync(&root, &message)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn git_init(state: State<'_, AppState>) -> Result<GitStatus, String> {
    let root = root(&state);
    tauri::async_runtime::spawn_blocking(move || init(&root)).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A repo with an identity, so commits work on machines without git config.
    fn repo(dir: &Path) {
        git(dir, &["init", "--quiet", "-b", "main"]).unwrap();
        git(dir, &["config", "user.name", "Test"]).unwrap();
        git(dir, &["config", "user.email", "test@example.com"]).unwrap();
        git(dir, &["config", "commit.gpgsign", "false"]).unwrap();
    }

    fn clone(remote: &Path, to: &Path) {
        git(remote.parent().unwrap(), &["clone", "--quiet", &remote.to_string_lossy(), &to.to_string_lossy()]).unwrap();
        git(to, &["config", "user.name", "Test"]).unwrap();
        git(to, &["config", "user.email", "test@example.com"]).unwrap();
        git(to, &["config", "commit.gpgsign", "false"]).unwrap();
    }

    #[test]
    fn parses_branch_lines() {
        let mut st = GitStatus::default();
        parse_branch("main...origin/main [ahead 2, behind 1]", &mut st);
        assert_eq!((st.branch.as_deref(), st.has_upstream, st.ahead, st.behind), (Some("main"), true, 2, 1));
        let mut st = GitStatus::default();
        parse_branch("No commits yet on main", &mut st);
        assert_eq!((st.branch.as_deref(), st.has_upstream), (Some("main"), false));
    }

    #[test]
    fn not_a_repo() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!status(dir.path()).is_repo);
        assert!(sync(dir.path(), "x").is_err());
    }

    #[test]
    fn init_commits_the_vault_and_ignores_trash() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("a.md"), "a").unwrap();
        fs::create_dir(root.join(".trash")).unwrap();
        fs::write(root.join(".trash/old.md"), "old").unwrap();
        // init commits with the machine's identity; tests can't rely on one being set.
        std::env::set_var("GIT_AUTHOR_NAME", "Test");
        std::env::set_var("GIT_AUTHOR_EMAIL", "test@example.com");
        std::env::set_var("GIT_COMMITTER_NAME", "Test");
        std::env::set_var("GIT_COMMITTER_EMAIL", "test@example.com");
        let st = init(root).unwrap();
        assert!(st.is_repo);
        assert_eq!(st.changes, 0);
        let tracked = git(root, &["ls-files"]).unwrap();
        assert!(tracked.contains("a.md") && !tracked.contains(".trash"));
        assert!(init(root).is_err(), "already a repo");
    }

    #[test]
    fn sync_commits_pulls_and_pushes_between_two_copies() {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("remote.git");
        fs::create_dir(&remote).unwrap();
        git(&remote, &["init", "--quiet", "--bare", "-b", "main"]).unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::create_dir(&a).unwrap();
        repo(&a);
        git(&a, &["remote", "add", "origin", &remote.to_string_lossy()]).unwrap();
        fs::write(a.join("Note.md"), "one\n").unwrap();
        git(&a, &["add", "-A"]).unwrap();
        git(&a, &["commit", "-qm", "first"]).unwrap();
        git(&a, &["push", "-q", "-u", "origin", "main"]).unwrap();
        clone(&remote, &b);

        // Edit in A, sync; then B syncs and gets it.
        fs::write(a.join("Note.md"), "one\ntwo\n").unwrap();
        fs::write(a.join("New.md"), "new").unwrap();
        let st = sync(&a, "from a").unwrap();
        assert_eq!((st.changes, st.ahead, st.behind), (0, 0, 0));
        let st = sync(&b, "from b").unwrap();
        assert_eq!(fs::read_to_string(b.join("Note.md")).unwrap(), "one\ntwo\n");
        assert!(b.join("New.md").exists());
        assert!(st.conflicted.is_empty());
    }

    #[test]
    fn sync_stops_on_conflicts_and_finishes_after_they_are_fixed() {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("remote.git");
        fs::create_dir(&remote).unwrap();
        git(&remote, &["init", "--quiet", "--bare", "-b", "main"]).unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::create_dir(&a).unwrap();
        repo(&a);
        git(&a, &["remote", "add", "origin", &remote.to_string_lossy()]).unwrap();
        fs::write(a.join("Note.md"), "base\n").unwrap();
        git(&a, &["add", "-A"]).unwrap();
        git(&a, &["commit", "-qm", "first"]).unwrap();
        git(&a, &["push", "-q", "-u", "origin", "main"]).unwrap();
        clone(&remote, &b);

        fs::write(a.join("Note.md"), "from a\n").unwrap();
        sync(&a, "a").unwrap();
        fs::write(b.join("Note.md"), "from b\n").unwrap();
        let st = sync(&b, "b").unwrap();
        assert_eq!(st.conflicted, ["Note.md"]);
        assert!(st.merging);
        let text = fs::read_to_string(b.join("Note.md")).unwrap();
        assert!(text.contains("<<<<<<<") && text.contains("from a") && text.contains("from b"));
        assert!(crate::conflicts::find(&b).iter().any(|c| c.kind == "markers"));

        // Syncing again before fixing changes nothing.
        assert_eq!(sync(&b, "b").unwrap().conflicted, ["Note.md"]);

        fs::write(b.join("Note.md"), "from a and b\n").unwrap();
        let st = sync(&b, "b").unwrap();
        assert!(!st.merging && st.conflicted.is_empty() && st.ahead == 0);
        sync(&a, "a").unwrap();
        assert_eq!(fs::read_to_string(a.join("Note.md")).unwrap(), "from a and b\n");
    }

    #[test]
    fn vault_in_a_subfolder_only_commits_the_vault() {
        let dir = tempfile::tempdir().unwrap();
        let repo_root = dir.path();
        repo(repo_root);
        fs::create_dir(repo_root.join("notes")).unwrap();
        fs::write(repo_root.join("notes/a.md"), "a").unwrap();
        fs::write(repo_root.join("code.rs"), "fn main() {}").unwrap();
        let vault = repo_root.join("notes");
        let st = status(&vault);
        assert_eq!(st.changes, 1, "only the vault's file counts");
        sync(&vault, "notes").unwrap();
        let tracked = git(repo_root, &["ls-files"]).unwrap();
        assert!(tracked.contains("notes/a.md") && !tracked.contains("code.rs"));
    }
}
