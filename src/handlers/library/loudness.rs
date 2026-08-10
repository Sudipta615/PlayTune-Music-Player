use std::sync::atomic::Ordering;

use crate::app_state::{GLOBAL_DB, LOUDNESS_SCAN_CANCELLED};
use crate::bridge;

#[no_mangle]
pub extern "C" fn playtune_cancel_loudness_scan() {
    LOUDNESS_SCAN_CANCELLED.store(true, Ordering::SeqCst);
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn playtune_start_loudness_scan(
    track_ids: *const std::ffi::c_int,
    count: std::ffi::c_int,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        LOUDNESS_SCAN_CANCELLED.store(false, Ordering::SeqCst);

        let target_ids: Option<Vec<i64>> = unsafe {
            if !track_ids.is_null() && count > 0 {
                let slice = std::slice::from_raw_parts(track_ids, count as usize);
                Some(slice.iter().map(|&id| id as i64).collect())
            } else {
                None
            }
        };

        std::thread::spawn(move || {
            let _ =
                thread_priority::set_current_thread_priority(thread_priority::ThreadPriority::Min);

            let db = match GLOBAL_DB.get() {
                Some(db) => db,
                None => {
                    bridge::loudness_scan_finished(false, "Database not initialized");
                    return;
                }
            };

            let tracks = match target_ids {
                Some(ids) => {
                    let mut vec = Vec::new();
                    for id in ids {
                        if let Ok(Some(t)) = db.get_track(id) {
                            vec.push(t);
                        }
                    }
                    vec
                }
                None => match db.get_all_tracks() {
                    Ok(t) => t,
                    Err(e) => {
                        bridge::loudness_scan_finished(
                            false,
                            &format!("Database query failed: {}", e),
                        );
                        return;
                    }
                },
            };

            let total = tracks.len() as i32;
            if total == 0 {
                bridge::loudness_scan_finished(true, "");
                return;
            }

            for (i, track) in tracks.iter().enumerate() {
                if LOUDNESS_SCAN_CANCELLED.load(Ordering::SeqCst) {
                    bridge::loudness_scan_finished(false, "Scan cancelled by user");
                    return;
                }

                bridge::loudness_scan_progress(i as i32, total, &track.title);

                let path = std::path::Path::new(&track.path);
                if let Ok(res) =
                    library::loudness_scanner::scan_track_loudness(track.id, path, &track.title)
                {
                    if LOUDNESS_SCAN_CANCELLED.load(Ordering::SeqCst) {
                        bridge::loudness_scan_finished(false, "Scan cancelled by user");
                        return;
                    }
                    bridge::loudness_scan_track_result(
                        res.track_id as i32,
                        res.lufs,
                        res.peak,
                        res.rg_gain_db,
                        res.r128_gain_db,
                    );
                }

                bridge::loudness_scan_progress((i + 1) as i32, total, &track.title);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }

            bridge::loudness_scan_finished(true, "");
        });
    }));
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn playtune_write_loudness_results(
    items: *const bridge::FfiLoudnessWriteItem,
    count: std::ffi::c_int,
) -> std::ffi::c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if items.is_null() || count <= 0 {
            return 0;
        }
        let db = match GLOBAL_DB.get() {
            Some(db) => db,
            None => {
                log::error!("Database not initialized when writing loudness results");
                return 0;
            }
        };

        let slice = unsafe { std::slice::from_raw_parts(items, count as usize) };
        let mut success_count = 0;

        for item in slice {
            if let Ok(Some(track)) = db.get_track(item.track_id as i64) {
                if let Err(e) = db.update_track_loudness(
                    item.track_id as i64,
                    Some(item.rg_gain_db),
                    Some(item.peak),
                    None,
                    None,
                    Some(item.lufs),
                    Some(item.peak),
                ) {
                    log::warn!("Failed to update DB for track {}: {}", item.track_id, e);
                    continue;
                }

                let path = std::path::Path::new(&track.path);
                if let Err(e) = library::loudness_scanner::write_loudness_tags(
                    path,
                    item.rg_gain_db,
                    item.peak,
                    None,
                    None,
                    item.r128_gain_db,
                ) {
                    log::warn!("Failed to write file tags for track {}: {}", item.track_id, e);
                } else {
                    success_count += 1;
                }
            }
        }
        if success_count == count {
            1
        } else {
            0
        }
    }));
    match result {
        Ok(ret) => ret,
        Err(_) => {
            log::error!("panic inside playtune_write_loudness_results");
            0
        }
    }
}
