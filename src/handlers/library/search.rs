use db::TrackRecord as DbTrack;

use crate::app_state::{
    cached_cover_path, invalidate_loaded_filter, spawn_worker, CURRENT_TRACK_LIST, GLOBAL_DB,
};
use crate::ffi_safe;

static SEARCH_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub extern "C" fn rust_search(query: *const std::ffi::c_char) {
    ffi_safe!({
        let query_str = if query.is_null() {
            String::new()
        } else {
            let cstr = unsafe { std::ffi::CStr::from_ptr(query) };
            cstr.to_str().unwrap_or("").to_string()
        };

        let gen = SEARCH_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;

        spawn_worker("playtune-search", move || {
            rust_search_worker(query_str, gen);
        });
    });
}

fn rust_search_worker(query_str: String, gen: u64) {
    if SEARCH_GENERATION.load(std::sync::atomic::Ordering::SeqCst) != gen {
        return;
    }

    let query_lower = query_str.to_lowercase();
    log::debug!("Search worker (gen={}) query: {:?}", gen, query_lower);

    let tracks_opt = if let Some(db) = GLOBAL_DB.get() { db.get_all_tracks().ok() } else { None };
    let Some(tracks) = tracks_opt else { return };

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

    if SEARCH_GENERATION.load(std::sync::atomic::Ordering::SeqCst) != gen {
        return;
    }

    let mood_map = if let Some(db) = GLOBAL_DB.get() {
        db.get_top_moods_batch(0.50).unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    let ffi_rows: Vec<crate::bridge::SongRowArg> = filtered
        .iter()
        .enumerate()
        .map(|(i, track)| crate::bridge::SongRowArg {
            display_index: (i + 1) as i32,
            song_id: track.id as i32,
            is_favorite: track.is_favorite,
            title: track.title.to_string(),
            artist: track.artist.to_string(),
            album: track.album.to_string(),
            duration: track.duration_str.to_string(),
            cover_path: cached_cover_path(&track.path).unwrap_or_default(),
            mood: mood_map.get(&track.id).cloned().unwrap_or_default(),
        })
        .collect();

    if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
        if let Some(mut list) = list_lock.try_lock() {
            *list = filtered;
        }
    }

    invalidate_loaded_filter();
    crate::bridge::set_songs_batch(&ffi_rows);
}

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
