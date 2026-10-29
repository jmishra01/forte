//! End-to-end encrypted sync through a folder some other service already syncs
//! (iCloud Drive, Dropbox, a USB stick). Only encrypted files go there:
//!
//! - `forte-sync.json`: the key-derivation settings and salt, and a check value
//!   that tells a wrong passphrase from a right one;
//! - `objects/<id>.bin`: one file per note or attachment. `<id>` is a keyed hash
//!   of the path, so file names don't reveal note names, and the contents
//!   (path, data, or "deleted") are encrypted with XChaCha20-Poly1305 using the
//!   id as associated data, so files can't be swapped or edited unnoticed.
//!
//! The key comes from the passphrase with Argon2id. The passphrase is kept in
//! the OS keychain; what each file looked like at the last sync is kept in the
//! app's config folder, never in the vault.

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use chacha20poly1305::aead::rand_core::RngCore;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use tauri::State;

use crate::vault::{root, unique_path, AppState};

const HEADER: &str = "forte-sync.json";
const OBJECTS: &str = "objects";
const CHECK: &[u8] = b"forte encrypted sync";
const KEYCHAIN_SERVICE: &str = "com.jitendra.forte";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
struct Kdf {
    /// Memory in KiB, passes, and lanes for Argon2id.
    m: u32,
    t: u32,
    p: u32,
}

/// Strong enough to make guessing a passphrase slow, quick enough (~0.5 s) to unlock.
const DEFAULT_KDF: Kdf = Kdf { m: 64 * 1024, t: 3, p: 1 };

#[derive(Serialize, Deserialize)]
struct Header {
    format: u32,
    kdf: Kdf,
    salt: String,
    check: String,
}

/// What one object holds once decrypted.
#[derive(Serialize, Deserialize)]
struct Object {
    path: String,
    /// Base64 of the file, or None if the file was deleted.
    data: Option<String>,
}

/// The two keys derived from the passphrase.
#[derive(Clone)]
struct Keys {
    enc: [u8; 32],
    ids: [u8; 32],
}

/// Plaintext hash of each file as it was at the last sync, for one vault.
#[derive(Serialize, Deserialize, Default)]
struct SyncState {
    remote: PathBuf,
    synced: BTreeMap<String, String>,
}

#[derive(Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub sent: usize,
    pub received: usize,
    pub deleted: usize,
    /// Notes changed on both sides; the other side's version was saved next to them.
    pub conflicts: Vec<String>,
    /// Remote files that couldn't be decrypted (damaged, or changed by someone without the key).
    pub unreadable: usize,
}

fn mac(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("any key length works");
    m.update(msg);
    m.finalize().into_bytes().into()
}

fn derive(passphrase: &str, salt: &[u8], kdf: Kdf) -> Result<Keys, String> {
    let params = Params::new(kdf.m, kdf.t, kdf.p, Some(32)).map_err(|e| e.to_string())?;
    let mut master = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, &mut master)
        .map_err(|e| e.to_string())?;
    Ok(Keys { enc: mac(&master, b"forte/encrypt"), ids: mac(&master, b"forte/ids") })
}

fn seal(keys: &Keys, aad: &[u8], plain: &[u8]) -> Vec<u8> {
    let cipher = XChaCha20Poly1305::new((&keys.enc).into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let mut out = nonce.to_vec();
    out.extend(cipher.encrypt(&nonce, Payload { msg: plain, aad }).expect("encryption can't fail"));
    out
}

fn open(keys: &Keys, aad: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
    if sealed.len() < 24 {
        return None;
    }
    let (nonce, body) = sealed.split_at(24);
    XChaCha20Poly1305::new((&keys.enc).into())
        .decrypt(XNonce::from_slice(nonce), Payload { msg: body, aad })
        .ok()
}

fn object_id(keys: &Keys, path: &str) -> String {
    mac(&keys.ids, path.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Writes via a temporary file, so a half-written file is never seen by sync clients.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("tmp-forte");
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Unlocks `remote` with `passphrase`, creating it if it is empty.
fn unlock(remote: &Path, passphrase: &str, new_kdf: Kdf) -> Result<Keys, String> {
    let header_path = remote.join(HEADER);
    if let Ok(text) = fs::read_to_string(&header_path) {
        let h: Header = serde_json::from_str(&text).map_err(|_| "the sync folder's forte-sync.json is damaged")?;
        if h.format != 1 {
            return Err("this sync folder was made by a newer version of Forte".into());
        }
        let salt = B64.decode(h.salt).map_err(|e| e.to_string())?;
        let keys = derive(passphrase, &salt, h.kdf)?;
        let check = B64.decode(h.check).map_err(|e| e.to_string())?;
        return match open(&keys, b"check", &check) {
            Some(c) if c == CHECK => Ok(keys),
            _ => Err("wrong passphrase for this sync folder".into()),
        };
    }
    if fs::read_dir(remote).map(|mut d| d.next().is_some()).unwrap_or(false) && !remote.join(OBJECTS).exists() {
        return Err("choose an empty folder, or one Forte already syncs to".into());
    }
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let keys = derive(passphrase, &salt, new_kdf)?;
    let header = Header { format: 1, kdf: new_kdf, salt: B64.encode(salt), check: B64.encode(seal(&keys, b"check", CHECK)) };
    write_atomic(&header_path, serde_json::to_string_pretty(&header).unwrap().as_bytes())?;
    fs::create_dir_all(remote.join(OBJECTS)).map_err(|e| e.to_string())?;
    Ok(keys)
}

/// Every non-hidden file in the vault (notes and attachments), by relative path.
fn local_files(root: &Path) -> BTreeMap<String, PathBuf> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else if p.extension().and_then(|x| x.to_str()) != Some("tmp-forte") {
                out.insert(crate::vault::to_rel(root, &p), p);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// A path from the remote side, which must stay inside the vault and out of hidden folders.
fn safe_path(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty()
        && p.components().all(|c| matches!(c, Component::Normal(s) if !s.to_string_lossy().starts_with('.')))
}

/// Syncs `root` with the encrypted folder `remote`.
fn run(root: &Path, remote: &Path, keys: &Keys, state: &mut SyncState, device: &str) -> Result<Summary, String> {
    let mut summary = Summary::default();
    let objects = remote.join(OBJECTS);
    fs::create_dir_all(&objects).map_err(|e| e.to_string())?;

    // What's on the remote side now: path -> (hash, or None if deleted; and the data).
    let mut theirs: BTreeMap<String, (Option<String>, Option<Vec<u8>>)> = BTreeMap::new();
    for e in fs::read_dir(&objects).map_err(|e| e.to_string())?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(id) = name.strip_suffix(".bin") else { continue };
        let opened = fs::read(e.path()).ok().and_then(|b| open(keys, id.as_bytes(), &b));
        let parsed = opened.and_then(|plain| serde_json::from_slice::<Object>(&plain).ok());
        match parsed {
            // The id must match the path, or the file was moved here from another note.
            Some(obj) if object_id(keys, &obj.path) == id && safe_path(&obj.path) => {
                let data = obj.data.and_then(|d| B64.decode(d).ok());
                theirs.insert(obj.path, (data.as_deref().map(sha), data));
            }
            _ => summary.unreadable += 1,
        }
    }

    let mine = local_files(root);
    let paths: BTreeSet<String> = mine.keys().chain(theirs.keys()).chain(state.synced.keys()).cloned().collect();
    let send = |path: &str, data: Option<&[u8]>| -> Result<(), String> {
        let obj = Object { path: path.to_string(), data: data.map(|d| B64.encode(d)) };
        let id = object_id(keys, path);
        let sealed = seal(keys, id.as_bytes(), &serde_json::to_vec(&obj).unwrap());
        write_atomic(&objects.join(format!("{id}.bin")), &sealed)
    };

    for path in paths {
        let local = mine.get(&path).and_then(|p| fs::read(p).ok());
        let l = local.as_deref().map(sha);
        let (r, remote_data) = theirs.remove(&path).unwrap_or((None, None));
        let base = state.synced.get(&path).cloned();
        if l == r {
            // Same on both sides (including deleted on both).
        } else if r == base {
            // Changed here only.
            send(&path, local.as_deref())?;
            if l.is_some() { summary.sent += 1 } else { summary.deleted += 1 }
        } else if l == base {
            // Changed there only.
            let target = root.join(&path);
            match &remote_data {
                Some(data) => {
                    write_atomic(&target, data)?;
                    summary.received += 1;
                }
                None => {
                    if target.exists() {
                        crate::trash::move_to_trash(root, &path)?;
                    }
                    summary.deleted += 1;
                }
            }
        } else {
            // Changed on both sides.
            match (&local, &remote_data) {
                (Some(_), Some(data)) => {
                    // Keep ours in place, save theirs next to it, then send ours.
                    let target = root.join(&path);
                    let dir = target.parent().unwrap_or(root);
                    let stem = target.file_stem().unwrap_or_default().to_string_lossy();
                    let ext = target.extension().map(|e| e.to_string_lossy()).unwrap_or_default();
                    let date = format_date();
                    let copy = unique_path(dir, &format!("{stem} ({device}'s conflicted copy {date})"), &ext);
                    write_atomic(&copy, data)?;
                    // Send the copy now too, so the other device sees both versions straight away.
                    let copy_rel = crate::vault::to_rel(root, &copy);
                    send(&copy_rel, Some(data))?;
                    state.synced.insert(copy_rel, sha(data));
                    send(&path, local.as_deref())?;
                    summary.conflicts.push(path.clone());
                }
                (None, Some(data)) => {
                    // Deleted here but edited there: keep the edit.
                    write_atomic(&root.join(&path), data)?;
                    summary.received += 1;
                    state.synced.insert(path.clone(), sha(data));
                    continue;
                }
                (Some(_), None) => {
                    // Edited here but deleted there: keep the edit.
                    send(&path, local.as_deref())?;
                    summary.sent += 1;
                }
                (None, None) => {}
            }
            match &l {
                Some(h) => state.synced.insert(path, h.clone()),
                None => state.synced.remove(&path),
            };
            continue;
        }
        // Both sides now agree on `l` (sent) or `r` (received).
        let now = if r == base { l } else { r };
        match now {
            Some(h) => state.synced.insert(path, h),
            None => state.synced.remove(&path),
        };
    }
    Ok(summary)
}

fn format_date() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    // Days since the epoch to a calendar date (UTC), without a date library.
    let days = (secs / 86_400) as i64;
    let (z, era) = { let z = days + 719_468; (z, z.div_euclid(146_097)) };
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

// --- commands ------------------------------------------------------------------

/// Keys for the current session, so the slow key derivation runs once.
static UNLOCKED: Mutex<Option<(PathBuf, Keys)>> = Mutex::new(None);

fn state_path(state: &AppState, vault: &Path) -> PathBuf {
    let name = format!("encsync-{}.json", &sha(vault.to_string_lossy().as_bytes())[..16]);
    state.config_path.with_file_name(name)
}

fn load_state(path: &Path) -> Option<SyncState> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn keychain(remote: &Path) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, &format!("encrypted-sync:{}", remote.display())).map_err(|e| e.to_string())
}

fn device_name() -> String {
    let name = gethostname::gethostname().to_string_lossy().into_owned();
    let name = name.trim_end_matches(".local");
    let clean: String = name.chars().map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c }).collect();
    if clean.is_empty() { "another device".into() } else { clean }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncSyncStatus {
    /// The sync folder, if this vault has one.
    remote: Option<String>,
}

/// "existing" if `remote` already holds Forte's encrypted notes, "new" if it is
/// empty, or "other" if it has other files in it (and can't be used).
fn probe(remote: &Path) -> &'static str {
    if remote.join(HEADER).is_file() {
        "existing"
    } else if fs::read_dir(remote).map(|mut d| d.next().is_none()).unwrap_or(false) {
        "new"
    } else {
        "other"
    }
}

#[tauri::command]
pub fn enc_sync_probe(remote: String) -> &'static str {
    probe(Path::new(&remote))
}

#[tauri::command]
pub fn enc_sync_status(state: State<AppState>) -> EncSyncStatus {
    let vault = root(&state);
    EncSyncStatus { remote: load_state(&state_path(&state, &vault)).map(|s| s.remote.to_string_lossy().into_owned()) }
}

/// Connects this vault to `remote`, creating the encrypted folder if it's empty.
#[tauri::command]
pub async fn enc_sync_connect(state: State<'_, AppState>, remote: String, passphrase: String) -> Result<(), String> {
    if passphrase.chars().count() < 8 {
        return Err("use a passphrase of at least 8 characters".into());
    }
    let vault = root(&state);
    let remote = PathBuf::from(remote);
    if remote.starts_with(&vault) || vault.starts_with(&remote) {
        return Err("the sync folder can't be inside the notes folder, or the other way round".into());
    }
    let (r, p) = (remote.clone(), passphrase.clone());
    let keys = tauri::async_runtime::spawn_blocking(move || unlock(&r, &p, DEFAULT_KDF)).await.map_err(|e| e.to_string())??;
    keychain(&remote)?.set_password(&passphrase).map_err(|e| e.to_string())?;
    let state_file = state_path(&state, &vault);
    // A different folder starts from scratch, so nothing is mistaken for deleted.
    let fresh = SyncState { remote: remote.clone(), synced: BTreeMap::new() };
    write_atomic(&state_file, serde_json::to_string(&fresh).unwrap().as_bytes())?;
    *UNLOCKED.lock().unwrap() = Some((remote, keys));
    Ok(())
}

#[tauri::command]
pub fn enc_sync_disconnect(state: State<AppState>) -> Result<(), String> {
    let path = state_path(&state, &root(&state));
    if let Some(s) = load_state(&path) {
        let _ = keychain(&s.remote).and_then(|k| k.delete_credential().map_err(|e| e.to_string()));
    }
    *UNLOCKED.lock().unwrap() = None;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn enc_sync_run(state: State<'_, AppState>) -> Result<Summary, String> {
    let vault = root(&state);
    let state_file = state_path(&state, &vault);
    let mut sync = load_state(&state_file).ok_or("encrypted sync isn't set up for this notes folder")?;
    let cached = UNLOCKED.lock().unwrap().clone().filter(|(r, _)| *r == sync.remote).map(|(_, k)| k);
    let remote = sync.remote.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let keys = match cached {
            Some(k) => k,
            None => {
                let pass = keychain(&remote)?.get_password().map_err(|_| "the sync passphrase is missing; set up sync again")?;
                let k = unlock(&remote, &pass, DEFAULT_KDF)?;
                *UNLOCKED.lock().unwrap() = Some((remote.clone(), k.clone()));
                k
            }
        };
        if !remote.join(HEADER).exists() {
            return Err("the sync folder isn't available (is the drive connected?)".to_string());
        }
        let summary = run(&vault, &remote, &keys, &mut sync, &device_name())?;
        write_atomic(&state_file, serde_json::to_string(&sync).unwrap().as_bytes())?;
        Ok(summary)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fast settings: the tests check behaviour, not resistance to guessing.
    const TEST_KDF: Kdf = Kdf { m: 256, t: 1, p: 1 };

    struct Device {
        vault: tempfile::TempDir,
        state: SyncState,
        keys: Keys,
        name: &'static str,
    }

    impl Device {
        fn new(remote: &Path, name: &'static str) -> Self {
            let keys = unlock(remote, "correct horse battery", TEST_KDF).unwrap();
            Device { vault: tempfile::tempdir().unwrap(), state: SyncState { remote: remote.into(), ..Default::default() }, keys, name }
        }
        fn write(&self, rel: &str, text: &str) {
            write_atomic(&self.vault.path().join(rel), text.as_bytes()).unwrap();
        }
        fn read(&self, rel: &str) -> Option<String> {
            fs::read_to_string(self.vault.path().join(rel)).ok()
        }
        fn sync(&mut self, remote: &Path) -> Summary {
            run(self.vault.path(), remote, &self.keys, &mut self.state, self.name).unwrap()
        }
    }

    #[test]
    fn edits_flow_both_ways_and_deletions_follow() {
        let remote = tempfile::tempdir().unwrap();
        let mut a = Device::new(remote.path(), "Mac");
        let mut b = Device::new(remote.path(), "Laptop");
        a.write("Plan.md", "v1");
        a.write("work/Ideas.md", "ideas");
        a.write("attachments/pic.png", "PNG");
        assert_eq!(a.sync(remote.path()).sent, 3);
        assert_eq!(b.sync(remote.path()).received, 3);
        assert_eq!(b.read("work/Ideas.md").as_deref(), Some("ideas"));

        b.write("Plan.md", "v2");
        b.sync(remote.path());
        assert_eq!(a.sync(remote.path()).received, 1);
        assert_eq!(a.read("Plan.md").as_deref(), Some("v2"));

        fs::remove_file(a.vault.path().join("work/Ideas.md")).unwrap();
        assert_eq!(a.sync(remote.path()).deleted, 1);
        assert_eq!(b.sync(remote.path()).deleted, 1);
        assert!(b.read("work/Ideas.md").is_none());
        assert!(crate::trash::list(b.vault.path()).iter().any(|i| i.original.as_deref() == Some("work/Ideas.md")), "deleted notes go to the trash");

        // Nothing new: nothing to do.
        assert_eq!(a.sync(remote.path()), Summary::default());
    }

    #[test]
    fn edits_on_both_sides_keep_both_versions() {
        let remote = tempfile::tempdir().unwrap();
        let mut a = Device::new(remote.path(), "Mac");
        let mut b = Device::new(remote.path(), "Laptop");
        a.write("Plan.md", "base");
        a.sync(remote.path());
        b.sync(remote.path());
        a.write("Plan.md", "from a");
        b.write("Plan.md", "from b");
        a.sync(remote.path());
        let s = b.sync(remote.path());
        assert_eq!(s.conflicts, ["Plan.md"]);
        assert_eq!(b.read("Plan.md").as_deref(), Some("from b"));
        let copy = local_files(b.vault.path()).into_keys().find(|p| p.contains("Laptop's conflicted copy")).unwrap();
        assert_eq!(b.read(&copy).as_deref(), Some("from a"));
        assert!(crate::conflicts::find(b.vault.path()).iter().any(|c| c.original.as_deref() == Some("Plan.md")), "the conflict screen sees it");
        // A then gets B's version and the copy; both sides agree afterwards.
        a.sync(remote.path());
        assert_eq!(a.read("Plan.md").as_deref(), Some("from b"));
        assert_eq!(a.read(&copy).as_deref(), Some("from a"));
        assert_eq!(b.sync(remote.path()), Summary::default());
    }

    #[test]
    fn an_edit_beats_a_deletion() {
        let remote = tempfile::tempdir().unwrap();
        let mut a = Device::new(remote.path(), "Mac");
        let mut b = Device::new(remote.path(), "Laptop");
        a.write("Plan.md", "base");
        a.sync(remote.path());
        b.sync(remote.path());
        fs::remove_file(a.vault.path().join("Plan.md")).unwrap();
        b.write("Plan.md", "edited");
        a.sync(remote.path());
        b.sync(remote.path());
        a.sync(remote.path());
        assert_eq!(a.read("Plan.md").as_deref(), Some("edited"));
        assert_eq!(b.read("Plan.md").as_deref(), Some("edited"));
    }

    #[test]
    fn the_remote_folder_reveals_nothing_and_rejects_tampering() {
        let remote = tempfile::tempdir().unwrap();
        let mut a = Device::new(remote.path(), "Mac");
        a.write("Secret plans.md", "launch codes: 1234");
        a.sync(remote.path());
        for e in walkdir(remote.path()) {
            let name = e.file_name().unwrap().to_string_lossy().into_owned();
            assert!(!name.contains("Secret"), "file name leaks: {name}");
            let bytes = fs::read(&e).unwrap();
            assert!(!String::from_utf8_lossy(&bytes).contains("launch codes"), "content leaks in {name}");
        }
        // Flip one byte of the note's object: it is refused, not trusted.
        let obj = walkdir(&remote.path().join(OBJECTS)).into_iter().next().unwrap();
        let mut bytes = fs::read(&obj).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&obj, bytes).unwrap();
        let mut b = Device::new(remote.path(), "Laptop");
        let s = b.sync(remote.path());
        assert_eq!((s.unreadable, s.received), (1, 0));
    }

    #[test]
    fn wrong_passphrase_and_unsafe_paths_are_refused() {
        let remote = tempfile::tempdir().unwrap();
        unlock(remote.path(), "correct horse battery", TEST_KDF).unwrap();
        // (Keys deliberately aren't Debug, so they can't end up in logs; match instead of unwrap_err.)
        let Err(e) = unlock(remote.path(), "wrong", TEST_KDF) else { panic!("a wrong passphrase was accepted") };
        assert!(e.contains("wrong passphrase"));
        let busy = tempfile::tempdir().unwrap();
        fs::write(busy.path().join("photo.jpg"), "x").unwrap();
        assert!(unlock(busy.path(), "correct horse battery", TEST_KDF).is_err(), "won't take over a folder with other files");
        assert!(!safe_path("../outside.md") && !safe_path(".forte/plugins/x.js") && !safe_path("/abs.md"));
        assert!(safe_path("work/Plan.md"));
    }

    #[test]
    fn probing_tells_new_existing_and_unusable_folders_apart() {
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(probe(empty.path()), "new");
        unlock(empty.path(), "correct horse battery", TEST_KDF).unwrap();
        assert_eq!(probe(empty.path()), "existing");
        let busy = tempfile::tempdir().unwrap();
        fs::write(busy.path().join("a.txt"), "x").unwrap();
        assert_eq!(probe(busy.path()), "other");
    }

    #[test]
    fn dates_are_formatted_without_a_date_library() {
        let d = format_date();
        assert_eq!(d.len(), 10);
        assert!(d.starts_with("20"));
    }

    fn walkdir(dir: &Path) -> Vec<PathBuf> {
        let mut out = vec![];
        for e in fs::read_dir(dir).unwrap().flatten() {
            if e.path().is_dir() {
                out.extend(walkdir(&e.path()));
            } else {
                out.push(e.path());
            }
        }
        out
    }
}
