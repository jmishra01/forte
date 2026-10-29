mod ai;
mod conflicts;
mod git;
mod search;
mod trash;
mod vault;
mod watch;

use std::sync::Mutex;
use tauri::Manager;
use vault::{AppState, Config};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(desktop)]
            {
                app.handle().plugin(tauri_plugin_process::init())?;
                // Updates are signed; the updater only runs once a signing key's public half
                // is configured under plugins.updater (see docs/releasing.md).
                if app.config().plugins.0.contains_key("updater") {
                    app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
                }
            }

            let data_dir = app.path().app_data_dir().expect("no app data dir");
            let config_dir = app.path().app_config_dir().expect("no app config dir");
            std::fs::create_dir_all(&config_dir)?;
            let config_path = config_dir.join("config.json");
            // Folders are named after the bundle identifier, which used to be this.
            let old = |dir: &std::path::Path| dir.with_file_name("com.jitendra.notespdfapp");
            vault::adopt_old_install(&config_path, &old(&config_dir), &old(&data_dir));
            let cfg = Config::load(&config_path);

            let root = cfg.vault.unwrap_or_else(|| data_dir.join("vault"));
            if !root.exists() {
                std::fs::create_dir_all(&root)?;
                vault::migrate_legacy(cfg.data_dir.as_deref().unwrap_or(&data_dir), &root);
            }

            vault::allow_assets(app.handle(), &root);
            let watcher = watch::start(app.handle().clone(), &root);
            app.manage(AppState {
                root: Mutex::new(root),
                config_path,
                watcher: Mutex::new(watcher),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            vault::get_vault,
            vault::set_vault,
            vault::list_tree,
            vault::read_note,
            vault::write_note,
            vault::create_note,
            vault::create_folder,
            vault::rename_entry,
            vault::move_entry,
            vault::delete_entry,
            trash::list_trash,
            trash::restore_from_trash,
            trash::delete_from_trash,
            trash::empty_trash,
            conflicts::find_conflicts,
            conflicts::settle_conflict,
            git::git_status,
            git::git_sync,
            git::git_init,
            ai::ai_has_key,
            ai::ai_set_key,
            ai::ai_clear_key,
            ai::ai_run,
            ai::ai_stop,
            vault::search_notes,
            vault::note_index,
            vault::get_settings,
            vault::set_settings,
            vault::export_html,
            vault::save_attachment,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
