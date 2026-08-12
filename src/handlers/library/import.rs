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
                use rayon::prelude::*;

                let extracted: Vec<_> = path_vec
                    .par_iter()
                    .filter_map(|path_str| {
                        if SHUTDOWN.load(Ordering::SeqCst) {
                            return None;
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
                        Some((path_str.clone(), title, artist, album, duration_secs, duration_str, mtime))
                    })
                    .collect();

                if SHUTDOWN.load(Ordering::SeqCst) || extracted.is_empty() {
                    return;
                }

                let batch_inputs: Vec<_> = extracted
                    .iter()
                    .map(|(path, title, artist, album, dur_secs, dur_str, mtime)| {
                        (
                            path.as_str(),
                            title.as_str(),
                            artist.as_str(),
                            album.as_str(),
                            *dur_secs,
                            dur_str.as_str(),
                            None,
                            *mtime,
                            None,
                            None,
                            None,
                        )
                    })
                    .collect();

                let _ = db_clone.with_transaction(|tx| {
                    db::PlayTuneDb::insert_tracks_batch_tx(tx, &batch_inputs)?;
                    Ok(())
                });

                if SHUTDOWN.load(Ordering::SeqCst) {
                    return;
                }
                invalidate_all_views();
                refresh_ui("all", None);
            });
        }
    });
}
