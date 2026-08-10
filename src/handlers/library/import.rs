use std::sync::atomic::Ordering;

use crate::app_state::{invalidate_all_views, spawn_worker, GLOBAL_DB, SHUTDOWN};
use crate::ffi_safe;
use crate::ui_sync::refresh_ui;

pub extern "C" fn rust_import_files(paths: *const *const std::ffi::c_char, count: std::ffi::c_int) {
    ffi_safe!({
        const MAX_IMPORT_FILES: usize = 65_536;
        if paths.is_null() || count <= 0 || (count as usize) > MAX_IMPORT_FILES {
            log::warn!("rust_import_files: rejecting count={}", count);
            return;
        }
        log::info!("Importing {} files...", count);

        let mut path_vec: Vec<String> = Vec::with_capacity(count as usize);
        let slice = unsafe { std::slice::from_raw_parts(paths, count as usize) };
        for &ptr in slice {
            if !ptr.is_null() {
                let cstr = unsafe { std::ffi::CStr::from_ptr(ptr) };
                if let Ok(path_str) = cstr.to_str() {
                    path_vec.push(path_str.to_string());
                }
            }
        }

        if let Some(db_arc) = GLOBAL_DB.get() {
            let db_clone = std::sync::Arc::clone(db_arc);
            spawn_worker("playtune-import-files", move || {
                for path_str in &path_vec {
                    if SHUTDOWN.load(Ordering::SeqCst) {
                        log::info!("rust_import_files: SHUTDOWN detected, aborting import");
                        return;
                    }
                    let p = std::path::Path::new(path_str);
                    let (title, artist, album, duration_secs, duration_str) =
                        engine::extract_track_metadata(p);
                    let _ = engine::extract_cover_art_to_cache(p);
                    let mtime = std::fs::metadata(p)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    let _ = db_clone.add_or_update_track(
                        path_str,
                        &title,
                        &artist,
                        &album,
                        duration_secs,
                        &duration_str,
                        None,
                        mtime,
                    );
                }
                if SHUTDOWN.load(Ordering::SeqCst) {
                    return;
                }
                invalidate_all_views();
                refresh_ui("all", None);
            });
        }
    });
}
