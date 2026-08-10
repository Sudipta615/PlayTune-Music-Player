use library::LibraryManager;
use std::sync::atomic::Ordering;

use crate::app_state::{
    invalidate_all_views, spawn_worker, CURRENT_INDEX, CURRENT_TRACK_LIST, GLOBAL_DB, SHUTDOWN,
};
use crate::bridge;
use crate::ffi_safe;
use crate::handlers::playback::rust_stop_inner;
use crate::ui_sync::{refresh_folders_view, refresh_ui, refresh_up_next_queue, save_session_state};

pub extern "C" fn rust_import_folder(folder_path: *const std::ffi::c_char) {
    ffi_safe!({
        if folder_path.is_null() {
            return;
        }
        let cstr = unsafe { std::ffi::CStr::from_ptr(folder_path) };
        if let Ok(path_str) = cstr.to_str() {
            log::info!("Importing folder via library scanner: {}", path_str);
            if let Some(db_arc) = GLOBAL_DB.get() {
                let db_clone = std::sync::Arc::clone(db_arc);
                let path_string = path_str.to_string();
                spawn_worker("playtune-import-folder", move || {
                    let mut config = config::LibraryConfig::default();
                    config.watch_dirs.push(std::path::PathBuf::from(&path_string));
                    let temp_mgr = LibraryManager::new(db_clone.clone(), config);
                    let _ = temp_mgr.scan(|progress| {
                        if SHUTDOWN.load(Ordering::SeqCst) {
                            return;
                        }
                        if progress.files_processed % 50 == 0 {
                            log::info!(
                                "{}/{} files...",
                                progress.files_processed,
                                progress.files_found
                            );
                        }
                    });
                    if SHUTDOWN.load(Ordering::SeqCst) {
                        return;
                    }
                    let _ = db_clone.set_setting("folders_configured", "1");
                    refresh_folders_view();
                    if SHUTDOWN.load(Ordering::SeqCst) {
                        return;
                    }
                    invalidate_all_views();
                    refresh_ui("all", None);
                });
            }
        }
    });
}

pub extern "C" fn rust_delete_folder(folder_id: std::ffi::c_int) {
    ffi_safe!({
        log::info!("Deleting folder ID: {}", folder_id);

        let old_track_id = CURRENT_TRACK_LIST.get().and_then(|l| l.try_lock()).and_then(|list| {
            let idx = *CURRENT_INDEX.lock();
            list.get(idx).map(|t| t.id)
        });

        if let Some(db) = GLOBAL_DB.get() {
            let _ = db.delete_folder(folder_id as i64);
        }
        invalidate_all_views();
        refresh_folders_view();
        refresh_ui("all", None);

        let current_track_gone = old_track_id.is_some_and(|old_id| {
            CURRENT_TRACK_LIST
                .get()
                .and_then(|l| l.try_lock())
                .map_or(true, |list| !list.iter().any(|t| t.id == old_id))
        });

        if current_track_gone
            || CURRENT_TRACK_LIST
                .get()
                .and_then(|l| l.try_lock())
                .map(|l| l.is_empty())
                .unwrap_or(true)
        {
            rust_stop_inner();
            bridge::set_track_info("", "", "", "");
            bridge::clear_queue();
        } else {
            refresh_up_next_queue();
        }
        save_session_state();
    });
}

pub extern "C" fn rust_filter_folder(folder_id: std::ffi::c_int) {
    ffi_safe!({
        log::info!("Filtering by folder ID: {}", folder_id);
        refresh_ui("folder", Some(folder_id as i64));
    });
}
