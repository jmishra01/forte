mod attachments;
mod export;
mod models;
mod notes;
mod pdfs;
mod state;
mod sync;

use state::{AppState, Dirs};
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let default_base = app.path().app_data_dir().expect("no app data dir");
            let config_path = app.path().app_config_dir().expect("no app config dir");
            std::fs::create_dir_all(&config_path)?;
            let config_path = config_path.join("config.json");

            let base = AppState::load_data_dir_override(&config_path).unwrap_or(default_base);
            let dirs = Dirs::new(base)?;

            app.manage(AppState {
                dirs: Mutex::new(dirs),
                config_path,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            notes::list_notes,
            notes::list_trashed_notes,
            notes::list_tags,
            notes::get_note,
            notes::create_note,
            notes::update_note,
            notes::rename_note,
            notes::set_note_tags,
            notes::set_note_parent,
            notes::link_pdf_to_note,
            notes::unlink_pdf_from_note,
            notes::delete_note,
            notes::restore_note,
            notes::permanently_delete_note,
            notes::empty_notes_trash,
            notes::get_backlinks,
            notes::list_note_history,
            notes::get_note_history_content,
            notes::restore_note_version,
            pdfs::list_pdfs,
            pdfs::list_trashed_pdfs,
            pdfs::add_pdf_from_path,
            pdfs::add_pdf_from_url,
            pdfs::get_pdf_path,
            pdfs::update_pdf_progress,
            pdfs::rename_pdf,
            pdfs::delete_pdf,
            pdfs::restore_pdf,
            pdfs::permanently_delete_pdf,
            pdfs::empty_pdfs_trash,
            pdfs::list_annotations,
            pdfs::add_annotation,
            pdfs::delete_annotation,
            attachments::import_attachment,
            attachments::get_attachment_path,
            export::write_file_bytes,
            export::export_all_notes,
            sync::get_data_dir,
            sync::choose_data_dir,
            sync::git_status,
            sync::git_init,
            sync::git_set_remote,
            sync::git_sync,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
