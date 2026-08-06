use std::sync::atomic::Ordering;

use db::TrackRecord as DbTrack;
use library::LibraryManager;

use crate::app_state::{
    cached_cover_path, invalidate_all_views, invalidate_cover_cache, invalidate_loaded_filter,
    spawn_worker, sync_shuffle_order, ACTIVE_PLAYLIST_ID, CURRENT_INDEX, CURRENT_TRACK_LIST,
    GLOBAL_DB, LOUDNESS_SCAN_CANCELLED, QUEUE_CLEARED_BY_USER, SHUFFLE_ENABLED, SHUFFLE_ORDER,
    SHUFFLE_POS, SHUTDOWN,
};
use crate::bridge;
use crate::ffi_safe;
use crate::handlers::playback::rust_stop_inner;
use crate::ui_sync::{
    refresh_albums_for_artist, refresh_folders_view, refresh_playlists_view, refresh_ui,
    refresh_up_next_queue, save_session_state,
};

pub extern "C" fn rust_clear_queue() {
    ffi_safe!({
        log::info!("Clear Queue clicked");
        QUEUE_CLEARED_BY_USER.store(true, Ordering::SeqCst);
        bridge::clear_queue();
        save_session_state();
    });
}

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
                    // Mark that the user has explicitly configured folders
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

        // Capture the currently playing track's ID before deletion
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

        // Check if the current track was in the deleted folder
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

pub extern "C" fn rust_remove_from_library(track_id: std::ffi::c_int) {
    ffi_safe!({
        let track_id_removed = track_id as i64;

        // Capture the currently playing track's ID before deletion
        let old_track_id = CURRENT_TRACK_LIST.get().and_then(|l| l.try_lock()).and_then(|list| {
            let idx = *CURRENT_INDEX.lock();
            list.get(idx).map(|t| t.id)
        });

        if let Some(db) = GLOBAL_DB.get() {
            let _ = db.delete_track(track_id_removed);
        }
        invalidate_all_views();
        refresh_ui("all", None);
        refresh_folders_view();

        // If the deleted track was the currently playing one, stop playback
        let was_current = old_track_id == Some(track_id_removed);
        if was_current
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

pub extern "C" fn rust_toggle_favorite(song_id: std::ffi::c_int) {
    ffi_safe!({
        log::info!("Toggling favorite for song ID: {}", song_id);
        if let Some(db) = GLOBAL_DB.get() {
            let _ = db.toggle_favorite(song_id as i64);
        }
    });
}

pub extern "C" fn rust_filter_folder(folder_id: std::ffi::c_int) {
    ffi_safe!({
        log::info!("Filtering by folder ID: {}", folder_id);
        refresh_ui("folder", Some(folder_id as i64));
    });
}

/// Monotonically-increasing counter used to cancel stale search workers.
/// Each call to `rust_search` increments this; a worker that finds the
/// counter has changed since it started simply discards its result and
/// returns without emitting FFI calls.
static SEARCH_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub extern "C" fn rust_search(query: *const std::ffi::c_char) {
    ffi_safe!({
        // Convert the C string on the GUI thread (the pointer is only
        // valid while the callback is executing).
        let query_str = if query.is_null() {
            String::new()
        } else {
            let cstr = unsafe { std::ffi::CStr::from_ptr(query) };
            cstr.to_str().unwrap_or("").to_string()
        };

        // Increment the generation counter. Any previously-spawned search
        // worker that checks this counter will see the new value and bail
        // out before emitting FFI calls, so stale results are discarded.
        let gen = SEARCH_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;

        // Run the actual DB query + filter on a background worker so the
        // GUI thread is never blocked waiting on SQLite.
        spawn_worker("playtune-search", move || {
            rust_search_worker(query_str, gen);
        });
    });
}

/// Background worker for search. All heavy work (DB query, string filter,
/// FFI payload construction) happens here, not on the GUI thread.
fn rust_search_worker(query_str: String, gen: u64) {
    // Early exit if a newer search has already been queued.
    if SEARCH_GENERATION.load(std::sync::atomic::Ordering::SeqCst) != gen {
        return;
    }

    let query_lower = query_str.to_lowercase();
    log::debug!("Search worker (gen={}) query: {:?}", gen, query_lower);

    let tracks_opt = if let Some(db) = GLOBAL_DB.get() { db.get_all_tracks().ok() } else { None };
    let Some(tracks) = tracks_opt else { return };

    // Check again after the potentially-slow DB query.
    if SEARCH_GENERATION.load(std::sync::atomic::Ordering::SeqCst) != gen {
        return;
    }

    let filtered: Vec<DbTrack> = if query_lower.is_empty() {
        tracks
    } else {
        tracks
            .into_iter()
            .filter(|t| {
                t.title.to_lowercase().contains(&query_lower)
                    || t.artist.to_lowercase().contains(&query_lower)
                    || t.album.to_lowercase().contains(&query_lower)
            })
            .collect()
    };

    // One more generation check before we do any FFI.
    if SEARCH_GENERATION.load(std::sync::atomic::Ordering::SeqCst) != gen {
        return;
    }

    // Build the FFI payload. Cover paths are intentionally omitted here —
    // the C++ CoverLoader resolves them lazily as rows scroll into view,
    // avoiding N `cached_cover_path()` calls (each may hit disk) on the
    // background thread under a write lock.
    let ffi_rows: Vec<crate::bridge::SongRowArg> = filtered
        .iter()
        .enumerate()
        .map(|(i, track)| crate::bridge::SongRowArg {
            display_index: (i + 1) as i32,
            song_id: track.id as i32,
            is_favorite: track.is_favorite,
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            duration: track.duration_str.clone(),
            cover_path: cached_cover_path(&track.path).unwrap_or_default(),
        })
        .collect();

    if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
        if let Some(mut list) = list_lock.try_lock() {
            *list = filtered;
        }
    }

    invalidate_loaded_filter();

    // Single FFI round-trip — the C++ side does a transactional rebuild.
    crate::bridge::set_songs_batch(&ffi_rows);
}

/// Inner synchronous search, kept for callers that already run on a
/// background thread (e.g., the initial populate path). Prefer the async
/// `rust_search` / `rust_search_worker` path for GUI-triggered searches.
#[allow(dead_code)]
pub fn rust_search_inner(query: *const std::ffi::c_char) {
    let query_str = if query.is_null() {
        String::new()
    } else {
        let cstr = unsafe { std::ffi::CStr::from_ptr(query) };
        cstr.to_str().unwrap_or("").to_string()
    };
    let gen = SEARCH_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    rust_search_worker(query_str, gen);
}

#[no_mangle]
pub extern "C" fn playtune_get_track_lyrics(track_id: std::ffi::c_int) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Some(db) = GLOBAL_DB.get() {
            if let Ok(Some(track)) = db.get_track(track_id as i64) {
                bridge::update_track_lyrics(
                    track.id as i32,
                    track.lyrics_synced.as_deref(),
                    track.lyrics_unsynced.as_deref(),
                );
            }
        }
    }));
    if result.is_err() {
        log::error!("panic inside playtune_get_track_lyrics");
    }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn playtune_update_track_tags(
    req: *const bridge::FfiTagEditRequest,
) -> std::ffi::c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if req.is_null() {
            return 0;
        }
        let req = unsafe { &*req };
        let db = match GLOBAL_DB.get() {
            Some(db) => db,
            None => {
                log::error!("Database not initialized when updating tags");
                return 0;
            }
        };

        let title_str = unsafe {
            if req.title.is_null() {
                "".to_string()
            } else {
                std::ffi::CStr::from_ptr(req.title).to_string_lossy().to_string()
            }
        };
        let artist_str = unsafe {
            if req.artist.is_null() {
                "".to_string()
            } else {
                std::ffi::CStr::from_ptr(req.artist).to_string_lossy().to_string()
            }
        };
        let album_str = unsafe {
            if req.album.is_null() {
                "".to_string()
            } else {
                std::ffi::CStr::from_ptr(req.album).to_string_lossy().to_string()
            }
        };
        let album_artist_str = unsafe {
            if req.album_artist.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(req.album_artist).to_string_lossy().to_string())
            }
        };
        let genre_str = unsafe {
            if req.genre.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(req.genre).to_string_lossy().to_string())
            }
        };
        let cover_path_opt = unsafe {
            if req.cover_image_path.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(req.cover_image_path).to_string_lossy().to_string())
            }
        };

        let tag_req = library::TagEditRequest {
            track_id: req.track_id as i64,
            title: title_str,
            artist: artist_str,
            album: album_str,
            album_artist: album_artist_str,
            genre: genre_str,
            year: Some(req.year),
            track_number: Some(req.track_number),
            disc_number: Some(req.disc_number),
            cover_image_path: cover_path_opt,
        };

        match library::update_track_tags(db, tag_req) {
            Ok(updated_track) => {
                log::info!("Successfully updated tags for track ID {}", updated_track.id);
                invalidate_cover_cache(&updated_track.path);
                invalidate_all_views();
                let cover_path = {
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut hasher = DefaultHasher::new();
                    updated_track.path.hash(&mut hasher);
                    let hash_id = hasher.finish();
                    let base =
                        dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
                    let covers_dir = base.join("playtune").join("covers");
                    let jpg = covers_dir.join(format!("{}.jpg", hash_id));
                    let png = covers_dir.join(format!("{}.png", hash_id));
                    if jpg.exists() {
                        jpg.to_string_lossy().to_string()
                    } else if png.exists() {
                        png.to_string_lossy().to_string()
                    } else {
                        // Fall back to the in-memory cover cache which may
                        // have just been re-populated by
                        // `extract_cover_art_to_cache` inside
                        // `library::update_track_tags`.
                        crate::app_state::cached_cover_path(&updated_track.path).unwrap_or_default()
                    }
                };

                bridge::update_track_metadata(
                    updated_track.id as i32,
                    &updated_track.title,
                    &updated_track.artist,
                    &updated_track.album,
                    &updated_track.duration_str,
                    &cover_path,
                );
                1
            }
            Err(e) => {
                log::error!("Failed to update track tags: {}", e);
                0
            }
        }
    }));
    match result {
        Ok(ret) => ret,
        Err(_) => {
            log::error!("panic inside playtune_update_track_tags");
            0
        }
    }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn playtune_get_track_tags(
    track_id: std::ffi::c_int,
    title_buf: *mut std::ffi::c_char,
    title_len: std::ffi::c_int,
    artist_buf: *mut std::ffi::c_char,
    artist_len: std::ffi::c_int,
    album_buf: *mut std::ffi::c_char,
    album_len: std::ffi::c_int,
    album_artist_buf: *mut std::ffi::c_char,
    album_artist_len: std::ffi::c_int,
    genre_buf: *mut std::ffi::c_char,
    genre_len: std::ffi::c_int,
    year_out: *mut std::ffi::c_uint,
    track_num_out: *mut std::ffi::c_uint,
    disc_num_out: *mut std::ffi::c_uint,
    cover_buf: *mut std::ffi::c_char,
    cover_len: std::ffi::c_int,
) -> std::ffi::c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let db = match crate::GLOBAL_DB.get() {
            Some(db) => db,
            None => return 0,
        };

        let tag_req = match library::get_track_tags(db, track_id as i64) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("playtune_get_track_tags failed for {}: {}", track_id, e);
                return 0;
            }
        };

        unsafe {
            let copy_str = |s: &str, buf: *mut std::ffi::c_char, max_len: std::ffi::c_int| {
                if !buf.is_null() && max_len > 0 {
                    let bytes = s.as_bytes();
                    let to_copy = bytes.len().min((max_len - 1) as usize);
                    std::ptr::copy_nonoverlapping(
                        bytes.as_ptr() as *const std::ffi::c_char,
                        buf,
                        to_copy,
                    );
                    *buf.add(to_copy) = 0;
                }
            };

            copy_str(&tag_req.title, title_buf, title_len);
            copy_str(&tag_req.artist, artist_buf, artist_len);
            copy_str(&tag_req.album, album_buf, album_len);
            copy_str(
                tag_req.album_artist.as_deref().unwrap_or(""),
                album_artist_buf,
                album_artist_len,
            );
            copy_str(tag_req.genre.as_deref().unwrap_or(""), genre_buf, genre_len);

            if !year_out.is_null() {
                *year_out = tag_req.year.unwrap_or(0);
            }
            if !track_num_out.is_null() {
                *track_num_out = tag_req.track_number.unwrap_or(0);
            }
            if !disc_num_out.is_null() {
                *disc_num_out = tag_req.disc_number.unwrap_or(0);
            }

            copy_str(tag_req.cover_image_path.as_deref().unwrap_or(""), cover_buf, cover_len);
        }

        1
    }));

    result.unwrap_or(0)
}

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

// ========================================================================
// New handlers: Custom Playlists, Ratings, M3U Import/Export, Album/Artist nav
// ========================================================================

/// Helper: convert a C `*const c_char` to an owned `String`, defaulting
/// to empty on null or invalid UTF-8.
fn cstr_to_string(ptr: *const std::ffi::c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let cstr = unsafe { std::ffi::CStr::from_ptr(ptr) };
    cstr.to_str().unwrap_or("").to_string()
}

// ----- Custom Playlists -----

pub extern "C" fn rust_create_playlist(name: *const std::ffi::c_char) -> std::ffi::c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let name = cstr_to_string(name);
        if name.trim().is_empty() {
            log::warn!("rust_create_playlist: empty name");
            return -1;
        }
        let Some(db) = GLOBAL_DB.get() else { return -1 };
        match db.create_playlist(name.trim()) {
            Ok(id) => {
                refresh_playlists_view();
                id as i32
            }
            Err(e) => {
                log::error!("Failed to create playlist: {}", e);
                -1
            }
        }
    }));
    result.unwrap_or(-1)
}

pub extern "C" fn rust_rename_playlist(
    playlist_id: std::ffi::c_int,
    new_name: *const std::ffi::c_char,
) -> std::ffi::c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let new_name = cstr_to_string(new_name);
        if new_name.trim().is_empty() {
            return 0;
        }
        let Some(db) = GLOBAL_DB.get() else { return 0 };
        match db.rename_playlist(playlist_id as i64, new_name.trim()) {
            Ok(ok) => {
                if ok {
                    refresh_playlists_view();
                }
                if ok {
                    1
                } else {
                    0
                }
            }
            Err(e) => {
                log::error!("Failed to rename playlist {}: {}", playlist_id, e);
                0
            }
        }
    }));
    result.unwrap_or(0)
}

pub extern "C" fn rust_delete_playlist(playlist_id: std::ffi::c_int) {
    ffi_safe!({
        let Some(db) = GLOBAL_DB.get() else { return };
        if let Err(e) = db.delete_playlist(playlist_id as i64) {
            log::error!("Failed to delete playlist {}: {}", playlist_id, e);
            return;
        }
        // If the deleted playlist was the active view, fall back to "all".
        let was_active = {
            let active = ACTIVE_PLAYLIST_ID.lock();
            *active == Some(playlist_id as i64)
        };
        if was_active {
            invalidate_loaded_filter();
            refresh_ui("all", None);
        }
        refresh_playlists_view();
    });
}

pub extern "C" fn rust_add_track_to_playlist(
    playlist_id: std::ffi::c_int,
    track_id: std::ffi::c_int,
) -> std::ffi::c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(db) = GLOBAL_DB.get() else { return 0 };
        match db.add_track_to_playlist(playlist_id as i64, track_id as i64) {
            Ok(count) => {
                refresh_playlists_view();
                count
            }
            Err(e) => {
                log::error!("Failed to add track {} to playlist {}: {}", track_id, playlist_id, e);
                0
            }
        }
    }));
    result.unwrap_or(0)
}

pub extern "C" fn rust_remove_track_from_playlist(
    playlist_id: std::ffi::c_int,
    track_id: std::ffi::c_int,
) {
    ffi_safe!({
        let Some(db) = GLOBAL_DB.get() else { return };
        if let Err(e) = db.remove_track_from_playlist(playlist_id as i64, track_id as i64) {
            log::error!("Failed to remove track {} from playlist {}: {}", track_id, playlist_id, e);
            return;
        }
        refresh_playlists_view();
        // If the active view is this playlist, refresh it too.
        let active = ACTIVE_PLAYLIST_ID.lock();
        if *active == Some(playlist_id as i64) {
            drop(active);
            invalidate_loaded_filter();
            refresh_ui("playlist", Some(playlist_id as i64));
        }
    });
}

pub extern "C" fn rust_filter_playlist(playlist_id: std::ffi::c_int) {
    ffi_safe!({
        log::info!("Filtering by playlist ID: {}", playlist_id);
        refresh_ui("playlist", Some(playlist_id as i64));
    });
}

// ----- Album & Artist navigation -----

pub extern "C" fn rust_filter_album(album_id: std::ffi::c_int) {
    ffi_safe!({
        log::info!("Filtering by album ID: {}", album_id);
        refresh_ui("album", Some(album_id as i64));
    });
}

pub extern "C" fn rust_filter_artist(artist_id: std::ffi::c_int) {
    ffi_safe!({
        log::info!("Filtering by artist ID: {}", artist_id);
        refresh_ui("artist", Some(artist_id as i64));
        // Also push the albums of this artist to the GUI for the right panel.
        if let Some(db) = GLOBAL_DB.get() {
            if let Ok(Some(track)) = db.get_track(artist_id as i64) {
                refresh_albums_for_artist(&track.artist);
            }
        }
    });
}

// ----- Dislike Toggle (repurposed from Ratings) -----

pub extern "C" fn rust_set_rating(track_id: std::ffi::c_int, _rating: std::ffi::c_int) {
    ffi_safe!({
        let Some(db) = GLOBAL_DB.get() else { return };
        // Toggle dislike: if already disliked (-1) → clear (0), else → dislike (-1).
        let current_rating =
            db.get_track(track_id as i64).ok().flatten().map(|t| t.rating).unwrap_or(0);
        let new_rating = if current_rating == -1 { 0 } else { -1 };
        if let Err(e) = db.set_track_rating(track_id as i64, new_rating) {
            log::error!("Failed to toggle dislike for track {}: {}", track_id, e);
        } else {
            log::info!("Track {} dislike toggled: {} → {}", track_id, current_rating, new_rating);
            // Push the new state back to the GUI so the button updates.
            bridge::set_rating_for_row(track_id, new_rating);
        }
    });
}

// ----- M3U Import / Export -----

pub extern "C" fn playtune_import_m3u(
    path: *const std::ffi::c_char,
    playlist_name: *const std::ffi::c_char,
) -> std::ffi::c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let path_str = cstr_to_string(path);
        if path_str.is_empty() {
            return 0;
        }
        let name_str = cstr_to_string(playlist_name);
        let playlist_name_owned = if name_str.trim().is_empty() {
            // Use the file basename without extension as the playlist name.
            std::path::Path::new(&path_str)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Imported Playlist")
                .to_string()
        } else {
            name_str.trim().to_string()
        };

        let path_owned = path_str.clone();
        let name_owned = playlist_name_owned.clone();
        spawn_worker("playtune-import-m3u", move || {
            let path = std::path::Path::new(&path_owned);
            let entries = match library::playlist_io::read_m3u_file(path) {
                Ok(e) => e,
                Err(e) => {
                    log::error!("M3U import failed: {}", e);
                    return;
                }
            };
            let Some(db) = GLOBAL_DB.get() else { return };
            let playlist_file_dir = path.parent();
            // Gather watch_dirs from the LibraryManager config if available.
            let watch_dirs: Vec<std::path::PathBuf> =
                if let Some(mgr) = crate::app_state::LIBRARY_MANAGER.get() {
                    mgr.config().watch_dirs.clone()
                } else {
                    Vec::new()
                };
            let result = match library::playlist_io::resolve_entries(
                &entries,
                db,
                playlist_file_dir,
                &watch_dirs,
            ) {
                Ok(r) => r,
                Err(e) => {
                    log::error!("M3U resolve failed: {}", e);
                    return;
                }
            };
            if result.resolved_track_ids.is_empty() {
                log::warn!(
                    "M3U import: no tracks from '{}' could be matched to the library",
                    path_owned
                );
                return;
            }
            // Create the playlist and add all resolved tracks in one transaction.
            let pid = match db.create_playlist(&name_owned) {
                Ok(id) => id,
                Err(e) => {
                    log::error!("Failed to create playlist '{}': {}", name_owned, e);
                    return;
                }
            };
            if let Err(e) = db.add_tracks_to_playlist(pid, &result.resolved_track_ids) {
                log::error!("Failed to add tracks to playlist {}: {}", pid, e);
            }
            log::info!(
                "M3U import: added {} tracks to playlist '{}' (skipped {})",
                result.resolved_track_ids.len(),
                name_owned,
                result.skipped_entries.len()
            );
            refresh_playlists_view();
        });
        1
    }));
    result.unwrap_or(0)
}

pub extern "C" fn playtune_export_m3u(
    playlist_id: std::ffi::c_int,
    path: *const std::ffi::c_char,
) -> std::ffi::c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let path_str = cstr_to_string(path);
        if path_str.is_empty() || playlist_id < 0 {
            return 0;
        }
        let Some(db) = GLOBAL_DB.get() else { return 0 };
        let tracks = match db.get_tracks_by_playlist(playlist_id as i64) {
            Ok(t) => t,
            Err(e) => {
                log::error!("Failed to fetch tracks for playlist {}: {}", playlist_id, e);
                return 0;
            }
        };
        let playlist_name =
            db.get_playlist(playlist_id as i64).ok().flatten().map(|p| p.name).unwrap_or_default();
        let path = std::path::PathBuf::from(&path_str);
        match library::playlist_io::write_m3u_file(&path, &tracks, Some(&playlist_name)) {
            Ok(()) => 1,
            Err(e) => {
                log::error!("Failed to write M3U file {}: {}", path_str, e);
                0
            }
        }
    }));
    result.unwrap_or(0)
}

// ----- Sleep timer / Notifications / Cursor / Tray / Speed -----

pub extern "C" fn rust_sleep_timer(minutes: std::ffi::c_int) {
    ffi_safe!({
        if minutes < 0 {
            return;
        }
        crate::app_state::set_sleep_timer(minutes as u32);
    });
}

pub extern "C" fn rust_notifications_toggled(enabled: std::ffi::c_int) {
    ffi_safe!({
        let on = enabled != 0;
        crate::app_state::NOTIFICATIONS_ENABLED.store(on, Ordering::SeqCst);
        if let Some(db) = GLOBAL_DB.get() {
            let _ = db.set_setting("notifications_enabled", if on { "1" } else { "0" });
        }
    });
}

pub extern "C" fn rust_cursor_follows_playback(enabled: std::ffi::c_int) {
    ffi_safe!({
        let on = enabled != 0;
        crate::app_state::CURSOR_FOLLOWS_PLAYBACK.store(on, Ordering::SeqCst);
        if let Some(db) = GLOBAL_DB.get() {
            let _ = db.set_setting("cursor_follows_playback", if on { "1" } else { "0" });
        }
        if on {
            bridge::scroll_songs_table_to_active();
        }
    });
}

pub extern "C" fn rust_crossfade_toggled(enabled: std::ffi::c_int) {
    ffi_safe!({
        let on = enabled != 0;
        crate::app_state::CROSSFADE_ENABLED.store(on, Ordering::SeqCst);
        if let Some(db) = GLOBAL_DB.get() {
            let _ = db.set_setting("crossfade_enabled", if on { "1" } else { "0" });
        }
        // Propagate to the engine's TrackMixer: enabled=on → crossfade,
        // enabled=off → gapless (mixer.set_enabled(false)).
        if let Some(engine_lock) = crate::app_state::GLOBAL_ENGINE.get() {
            if let Some(mut engine) = engine_lock.try_lock() {
                engine.pipeline_mut().mixer_mut().set_enabled(on);
            }
        }
        if let Some(db) = GLOBAL_DB.get() {
            let _ = db.set_setting("crossfade_enabled", if on { "1" } else { "0" });
        }
    });
}

pub extern "C" fn rust_set_crossfade_duration(duration_ms: std::ffi::c_int) {
    ffi_safe!({
        let ms = duration_ms.max(0) as u32;
        crate::app_state::CROSSFADE_DURATION_MS.store(ms, Ordering::SeqCst);
        if let Some(db) = GLOBAL_DB.get() {
            let _ = db.set_setting("crossfade_duration_ms", &ms.to_string());
        }
        if let Some(engine_lock) = crate::app_state::GLOBAL_ENGINE.get() {
            if let Some(mut engine) = engine_lock.try_lock() {
                let sr = engine.config().sample_rate;
                engine.pipeline_mut().mixer_mut().set_duration_ms(ms as u64, sr as f32);
            }
        }
    });
}

pub extern "C" fn rust_set_speed(speed: std::ffi::c_double) {
    ffi_safe!({
        let clamped = (speed as f32).clamp(0.25, 4.0);
        if let Some(tx) = crate::app_state::ENGINE_CMD_TX.get() {
            let _ = tx.send(engine::buffer::EngineCommand::SetSpeed(clamped));
        }
        // Push the speed label to the GUI so the Now-Playing card can
        // display "1.00×" etc.
        bridge::set_speed_label(clamped as f64);
        if let Some(db) = GLOBAL_DB.get() {
            let _ = db.set_setting("playback_speed", &format!("{:.3}", clamped));
        }
    });
}

pub extern "C" fn rust_tray_toggled(enabled: std::ffi::c_int) {
    ffi_safe!({
        let on = enabled != 0;
        crate::app_state::TRAY_ENABLED.store(on, Ordering::SeqCst);
        if let Some(db) = GLOBAL_DB.get() {
            let _ = db.set_setting("tray_enabled", if on { "1" } else { "0" });
        }
    });
}

pub extern "C" fn rust_minimize_to_tray_toggled(enabled: std::ffi::c_int) {
    ffi_safe!({
        let on = enabled != 0;
        crate::app_state::MINIMIZE_TO_TRAY.store(on, Ordering::SeqCst);
        if let Some(db) = GLOBAL_DB.get() {
            let _ = db.set_setting("minimize_to_tray", if on { "1" } else { "0" });
        }
    });
}

#[allow(clippy::missing_transmute_annotations)]
#[used]
static _EXPORTED_SYMBOLS: [unsafe extern "C" fn(); 8] = [
    unsafe { std::mem::transmute(playtune_get_track_lyrics as extern "C" fn(std::ffi::c_int)) },
    unsafe {
        std::mem::transmute(
            playtune_update_track_tags
                as extern "C" fn(*const bridge::FfiTagEditRequest) -> std::ffi::c_int,
        )
    },
    unsafe {
        std::mem::transmute(
            playtune_get_track_tags
                as extern "C" fn(
                    std::ffi::c_int,
                    *mut std::ffi::c_char,
                    std::ffi::c_int,
                    *mut std::ffi::c_char,
                    std::ffi::c_int,
                    *mut std::ffi::c_char,
                    std::ffi::c_int,
                    *mut std::ffi::c_char,
                    std::ffi::c_int,
                    *mut std::ffi::c_char,
                    std::ffi::c_int,
                    *mut std::ffi::c_uint,
                    *mut std::ffi::c_uint,
                    *mut std::ffi::c_uint,
                    *mut std::ffi::c_char,
                    std::ffi::c_int,
                ) -> std::ffi::c_int,
        )
    },
    unsafe { std::mem::transmute(playtune_cancel_loudness_scan as extern "C" fn()) },
    unsafe {
        std::mem::transmute(
            playtune_start_loudness_scan as extern "C" fn(*const std::ffi::c_int, std::ffi::c_int),
        )
    },
    unsafe {
        std::mem::transmute(
            playtune_write_loudness_results
                as extern "C" fn(
                    *const bridge::FfiLoudnessWriteItem,
                    std::ffi::c_int,
                ) -> std::ffi::c_int,
        )
    },
    unsafe {
        std::mem::transmute(
            playtune_import_m3u
                as extern "C" fn(
                    *const std::ffi::c_char,
                    *const std::ffi::c_char,
                ) -> std::ffi::c_int,
        )
    },
    unsafe {
        std::mem::transmute(
            playtune_export_m3u
                as extern "C" fn(std::ffi::c_int, *const std::ffi::c_char) -> std::ffi::c_int,
        )
    },
];

pub extern "C" fn rust_reorder_queue(from_idx: std::ffi::c_int, to_idx: std::ffi::c_int) {
    ffi_safe!({
        let from_row = from_idx as usize;
        let to_row = to_idx as usize;
        log::info!("Reordering queue row: {} -> {}", from_row, to_row);
        let list_len = CURRENT_TRACK_LIST.get().and_then(|l| l.try_lock()).map_or(0, |l| l.len());
        if list_len > 1 {
            let curr = *CURRENT_INDEX.lock() % list_len;
            if SHUFFLE_ENABLED.load(Ordering::SeqCst) {
                sync_shuffle_order(curr, list_len);
                let mut order = SHUFFLE_ORDER.lock();
                let pos = *SHUFFLE_POS.lock();
                let order_len = order.len();
                if order_len > 1 {
                    let from_pos = (pos + 1 + from_row) % order_len;
                    let to_pos = (pos + 1 + to_row) % order_len;
                    if from_pos < order_len && to_pos < order_len && from_pos != to_pos {
                        let item = order.remove(from_pos);
                        order.insert(to_pos, item);
                    }
                }
            } else if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
                if let Some(mut list) = list_lock.try_lock() {
                    let len = list.len();
                    let from_pos = (curr + 1 + from_row) % len;
                    let to_pos = (curr + 1 + to_row) % len;
                    if from_pos < len && to_pos < len && from_pos != to_pos {
                        let track = list.remove(from_pos);
                        list.insert(to_pos, track);

                        let mut c_idx = CURRENT_INDEX.lock();
                        if *c_idx == from_pos {
                            *c_idx = to_pos;
                        } else if from_pos < *c_idx && to_pos >= *c_idx {
                            *c_idx = c_idx.saturating_sub(1);
                        } else if from_pos > *c_idx && to_pos <= *c_idx {
                            *c_idx = c_idx.saturating_add(1);
                        }
                    }
                }
            }
        }
        refresh_up_next_queue();
        save_session_state();
    });
}
