use crate::models::GitStatus;
use crate::state::{copy_dir_recursive, AppState, Dirs};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::State;

#[tauri::command]
pub fn get_data_dir(state: State<AppState>) -> Result<String, String> {
    let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
    Ok(dirs.base.to_string_lossy().to_string())
}

#[tauri::command]
pub fn choose_data_dir(state: State<AppState>, path: String, migrate: bool) -> Result<String, String> {
    let new_base = PathBuf::from(&path);
    if !new_base.is_dir() {
        return Err("Selected path is not a directory".into());
    }
    let mut dirs_guard = state.dirs.lock().map_err(|e| e.to_string())?;

    if migrate {
        copy_dir_recursive(&dirs_guard.base, &new_base).map_err(|e| e.to_string())?;
    }

    let new_dirs = Dirs::new(new_base.clone()).map_err(|e| e.to_string())?;
    *dirs_guard = new_dirs;
    state.save_data_dir(&new_base)?;
    Ok(new_base.to_string_lossy().to_string())
}

fn run_git(dir: &Path, args: &[&str]) -> Result<(String, bool), String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("git not available: {e}"))?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok((combined, output.status.success()))
}

fn git_available() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

#[tauri::command]
pub fn git_status(state: State<AppState>) -> Result<GitStatus, String> {
    let base = {
        let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
        dirs.base.clone()
    };

    if !git_available() {
        return Ok(GitStatus {
            available: false,
            is_repo: false,
            has_remote: false,
            remote_url: None,
            dirty: false,
            last_commit_at: None,
        });
    }

    if !base.join(".git").is_dir() {
        return Ok(GitStatus {
            available: true,
            is_repo: false,
            has_remote: false,
            remote_url: None,
            dirty: false,
            last_commit_at: None,
        });
    }

    let (remote_out, remote_ok) = run_git(&base, &["remote", "get-url", "origin"])?;
    let has_remote = remote_ok && !remote_out.trim().is_empty();
    let remote_url = has_remote.then(|| remote_out.trim().to_string());

    let (status_out, _) = run_git(&base, &["status", "--porcelain"])?;
    let dirty = !status_out.trim().is_empty();

    let (log_out, log_ok) = run_git(&base, &["log", "-1", "--format=%cI"])?;
    let last_commit_at = (log_ok && !log_out.trim().is_empty()).then(|| log_out.trim().to_string());

    Ok(GitStatus {
        available: true,
        is_repo: true,
        has_remote,
        remote_url,
        dirty,
        last_commit_at,
    })
}

#[tauri::command]
pub fn git_init(state: State<AppState>) -> Result<String, String> {
    let base = {
        let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
        dirs.base.clone()
    };
    if !git_available() {
        return Err("git is not installed or not on PATH".into());
    }
    let (out, ok) = run_git(&base, &["init"])?;
    if !ok {
        return Err(out);
    }
    let _ = run_git(&base, &["add", "-A"])?;
    let (commit_out, commit_ok) = run_git(&base, &["commit", "-m", "Initial commit", "--allow-empty"])?;
    if !commit_ok {
        return Err(commit_out);
    }
    Ok("Initialized git repository in the data folder".into())
}

#[tauri::command]
pub fn git_set_remote(state: State<AppState>, url: String) -> Result<String, String> {
    let base = {
        let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
        dirs.base.clone()
    };
    if !base.join(".git").is_dir() {
        return Err("Not a git repository yet — initialize it first".into());
    }
    let (_, has_remote) = run_git(&base, &["remote", "get-url", "origin"])?;
    let args: [&str; 4] = if has_remote {
        ["remote", "set-url", "origin", url.as_str()]
    } else {
        ["remote", "add", "origin", url.as_str()]
    };
    let (out, ok) = run_git(&base, &args)?;
    if !ok {
        return Err(out);
    }
    Ok("Remote configured".into())
}

#[tauri::command]
pub fn git_sync(state: State<AppState>) -> Result<String, String> {
    let base = {
        let dirs = state.dirs.lock().map_err(|e| e.to_string())?;
        dirs.base.clone()
    };
    if !base.join(".git").is_dir() {
        return Err("Not a git repository yet — initialize it first".into());
    }

    let mut log = String::new();

    let (add_out, add_ok) = run_git(&base, &["add", "-A"])?;
    log.push_str(&add_out);
    if !add_ok {
        return Err(log);
    }

    let (status_out, _) = run_git(&base, &["status", "--porcelain"])?;
    if !status_out.trim().is_empty() {
        let (commit_out, commit_ok) = run_git(&base, &["commit", "-m", "Sync from Notes & PDFs app"])?;
        log.push_str(&commit_out);
        if !commit_ok {
            return Err(log);
        }
    }

    let (_, has_remote) = run_git(&base, &["remote", "get-url", "origin"])?;
    if has_remote {
        let (pull_out, pull_ok) = run_git(&base, &["pull", "--rebase", "--autostash"])?;
        log.push_str(&pull_out);
        if !pull_ok {
            return Err(log);
        }
        let (push_out, push_ok) = run_git(&base, &["push", "-u", "origin", "HEAD"])?;
        log.push_str(&push_out);
        if !push_ok {
            return Err(log);
        }
    }

    Ok(if log.trim().is_empty() {
        "Already up to date".into()
    } else {
        log
    })
}
