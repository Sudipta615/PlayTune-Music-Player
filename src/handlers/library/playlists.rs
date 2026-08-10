use crate::app_state::{invalidate_loaded_filter, spawn_worker, ACTIVE_PLAYLIST_ID, GLOBAL_DB};
use crate::ffi_safe;
use crate::ui_sync::{refresh_playlists_view, refresh_ui};

fn cstr_to_string(ptr: *const std::ffi::c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let cstr = unsafe { std::ffi::CStr::from_ptr(ptr) };
    cstr.to_str().unwrap_or("").to_string()
}

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
