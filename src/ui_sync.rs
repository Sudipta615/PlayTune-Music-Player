use std::sync::atomic::Ordering;

use crate::app_state::{
    cached_cover_path, send_track_info_and_lyrics, sync_shuffle_order, ACTIVE_PLAYLIST_ID,
    ALBUMS_VIEW_DIRTY, ARTISTS_VIEW_DIRTY, CURRENT_INDEX, CURRENT_TRACK_LIST, ELAPSED_SECONDS,
    ENGINE_CMD_TX, FOLDERS_VIEW_DIRTY, GLOBAL_DB, GLOBAL_ENGINE, IS_PLAYING, LAST_LOADED_FILTER,
    PLATFORM, QUEUE_CLEARED_BY_USER, SHUFFLE_ENABLED, SHUFFLE_ORDER, SHUFFLE_POS, SHUTDOWN,
};
use crate::bridge;
use config::AudioBackend;
use engine::buffer::EngineCommand;
use platform::{MprisPlaybackStatus, MprisTrackInfo};

pub fn refresh_up_next_queue() {
    if QUEUE_CLEARED_BY_USER.load(Ordering::SeqCst) {
        bridge::clear_queue();
        return;
    }

    let list_lock = if let Some(l) = CURRENT_TRACK_LIST.get() { l } else { return };
    let list = match list_lock.try_lock() {
        Some(l) => l,
        None => return,
    };
    let len = list.len();
    if len == 0 {
        drop(list);
        bridge::clear_queue();
        return;
    }
    let curr = *CURRENT_INDEX.lock() % len;
    bridge::clear_queue();

    let count = 10.min(len);
    let mut indices: [usize; 10] = [0; 10];
    let mut indices_len = 0usize;

    if SHUFFLE_ENABLED.load(Ordering::SeqCst) {
        sync_shuffle_order(curr, len);
        let order = SHUFFLE_ORDER.lock();
        let pos = *SHUFFLE_POS.lock();
        let order_len = order.len();
        if order_len > 0 {
            let take = count.min(order_len.saturating_sub(1).max(1));
            for k in 1..=take {
                indices[indices_len] = order[(pos + k) % order_len];
                indices_len += 1;
            }
        }
    } else {
        for i in 0..count {
            indices[indices_len] = (curr + 1 + i) % len;
            indices_len += 1;
        }
    }

    for k in 0..indices_len {
        let idx = indices[k];
        if let Some(track) = list.get(idx) {
            let cover_path = cached_cover_path(&track.path).unwrap_or_default();
            bridge::add_queue_song(
                idx as i32,
                &track.title,
                &track.artist,
                &track.duration_str,
                &cover_path,
            );
        }
    }
}

pub fn push_audio_devices_to_gui(backend: AudioBackend) {
    let devices = engine::output::CpalOutput::enumerate_devices(backend);
    let current_device = if let Some(engine_lock) = GLOBAL_ENGINE.get() {
        if let Some(engine) = engine_lock.try_lock() {
            engine.config().output_device.clone()
        } else {
            None
        }
    } else {
        None
    };

    bridge::clear_audio_devices();
    bridge::add_audio_device(
        "Default / Automatic",
        current_device.is_none() || current_device.as_deref() == Some("Default / Automatic"),
    );
    for dev in devices {
        let is_cur = current_device.as_deref() == Some(dev.as_str());
        bridge::add_audio_device(&dev, is_cur);
    }
}

/// Monotonically-increasing counter incremented on each navigation event.
/// Background `refresh_ui` workers capture it at spawn time and bail out
/// before making FFI calls if the counter has advanced (the user already
/// navigated away). This eliminates redundant table rebuilds when the
/// user clicks tabs rapidly.
pub static NAV_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Increment the nav generation and return the new value. Call this at the
/// top of every nav handler so in-flight workers for the previous tab exit.
pub fn next_nav_gen() -> u64 {
    NAV_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
}

pub fn refresh_ui(filter_type: &str, filter_id: Option<i64>) {
    refresh_ui_gen(filter_type, filter_id, NAV_GENERATION.load(std::sync::atomic::Ordering::SeqCst))
}

/// Like `refresh_ui` but the caller supplies the expected generation value.
/// If `NAV_GENERATION` has been incremented beyond `expected_gen` by the
/// time we are about to emit FFI calls, we discard the result and return.
pub fn refresh_ui_gen(filter_type: &str, filter_id: Option<i64>, expected_gen: u64) {
    static REFRESH_LOCK: parking_lot::Mutex<()> = parking_lot::const_mutex(());
    let _refresh_guard = REFRESH_LOCK.lock();
    static LAST_PUSHED_ACTIVE_INDEX: std::sync::atomic::AtomicI32 =
        std::sync::atomic::AtomicI32::new(i32::MIN);

    // NOTE: we clone the lock's contents and drop the guard before making
    // any FFI call so we never hold a Rust lock across the FFI boundary
    // (which could deadlock if the GUI thread also tries to acquire it).

    let canonical_key = match filter_type {
        "folder" => format!("folder:{}", filter_id.unwrap_or(0)),
        "album" => format!("album:{}", filter_id.unwrap_or(0)),
        "artist" => format!("artist:{}", filter_id.unwrap_or(0)),
        "playlist" => format!("playlist:{}", filter_id.unwrap_or(0)),
        other => other.to_string(),
    };
    let already_loaded = {
        let last = LAST_LOADED_FILTER.lock();
        *last == canonical_key && !canonical_key.is_empty()
    };
    if already_loaded {
        // Same view is already materialized. Just re-emit the active
        // highlight so the table does not lose the "playing" indicator.
        let active = *CURRENT_INDEX.lock() as i32;
        let prev = LAST_PUSHED_ACTIVE_INDEX.swap(active, std::sync::atomic::Ordering::Relaxed);
        if prev != active {
            bridge::set_active_index(active);
        }
        return;
    }

    // For playlist / album / artist filters we need to look up the actual
    // album/artist name from a stable track id. We do this once, before
    // entering the DB query branch below, since the get_*_tracks methods
    // take a name string rather than an id.
    let album_name_opt: Option<String> = if filter_type == "album" {
        filter_id.and_then(|id| {
            GLOBAL_DB.get().and_then(|db| db.get_track(id).ok().flatten().map(|t| t.album))
        })
    } else {
        None
    };
    let artist_name_opt: Option<String> = if filter_type == "artist" {
        filter_id.and_then(|id| {
            GLOBAL_DB.get().and_then(|db| db.get_track(id).ok().flatten().map(|t| t.artist))
        })
    } else {
        None
    };

    // Update the ACTIVE_PLAYLIST_ID state so the "Export current view as
    // M3U" feature knows whether the current view is a playlist.
    {
        let mut active_pl = ACTIVE_PLAYLIST_ID.lock();
        *active_pl = if filter_type == "playlist" { filter_id } else { None };
    }

    let tracks_opt = if let Some(db) = GLOBAL_DB.get() {
        match filter_type {
            "favorites" => db.get_favorite_tracks().ok(),
            "recently_played" => db.get_recently_played_tracks(50).ok(),
            "most_played" => db.get_most_played_tracks(50).ok(),
            "folder" => db.get_tracks_by_folder(filter_id.unwrap_or(0)).ok(),
            "album" => album_name_opt.as_deref().and_then(|name| db.get_tracks_by_album(name).ok()),
            "artist" => {
                artist_name_opt.as_deref().and_then(|name| db.get_tracks_by_artist(name).ok())
            }
            "playlist" => db.get_tracks_by_playlist(filter_id.unwrap_or(0)).ok(),
            _ => db.get_all_tracks().ok(),
        }
    } else {
        None
    };

    if let Some(tracks) = tracks_opt {
        // Bail early if a newer navigation event has superseded us.
        // We check AFTER the DB query (which is the expensive part) so we
        // don't hold locks or issue FFI calls for a stale result.
        if NAV_GENERATION.load(std::sync::atomic::Ordering::SeqCst) != expected_gen {
            log::debug!(
                "refresh_ui: gen {} superseded by {}, discarding result",
                expected_gen,
                NAV_GENERATION.load(std::sync::atomic::Ordering::SeqCst)
            );
            return;
        }

        let mut playing_track_id: i64 = if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
            if let Some(list) = list_lock.try_lock() {
                let idx = *CURRENT_INDEX.lock();
                list.get(idx).map(|t| t.id).unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        };

        if playing_track_id == 0 {
            if let Some(db) = GLOBAL_DB.get() {
                if let Ok(Some(track_id_str)) = db.get_setting("last_played_track_id") {
                    if let Ok(tid) = track_id_str.parse::<i64>() {
                        if tid > 0 {
                            playing_track_id = tid;
                        }
                    }
                }
            }
        }

        // Build the FFI payload. Cover paths are intentionally omitted —
        // sending them would require N calls to `cached_cover_path()`
        // which each acquire a write lock and may do disk I/O on cache
        // miss. Instead we send an empty string and let the C++ CoverLoader
        // resolve covers lazily as rows scroll into view (it already does
        // this; the cover path stored on the QTableWidgetItem via UserRole
        // is retrieved by the delegate when painting).
        //
        // The C++ side was already updated to handle empty cover_path by
        // requesting an async load via CoverLoader::requestAsync, so the
        // only visible effect is that the first paint shows the default art
        // for ~1 frame before the async load delivers the real cover.
        let mood_map = if let Some(db) = GLOBAL_DB.get() {
            db.get_top_moods_batch(0.50).unwrap_or_default()
        } else {
            std::collections::HashMap::new()
        };

        let ffi_rows: Vec<bridge::SongRowArg> = tracks
            .iter()
            .enumerate()
            .map(|(i, track)| bridge::SongRowArg {
                display_index: if matches!(filter_type, "album") {
                    track.track_number.filter(|&n| n > 0).unwrap_or((i + 1) as i32)
                } else {
                    (i + 1) as i32
                },
                song_id: track.id as i32,
                is_favorite: track.is_favorite,
                title: track.title.clone(),
                artist: track.artist.clone(),
                album: track.album.clone(),
                duration: track.duration_str.clone(),
                cover_path: cached_cover_path(&track.path).unwrap_or_default(),
                mood: mood_map.get(&track.id).cloned().unwrap_or_default(),
            })
            .collect();

        // Move tracks into the global list (no clone).
        if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
            if let Some(mut list) = list_lock.try_lock() {
                *list = tracks;
            }
        }

        // Single FFI round-trip instead of N. The C++ side does a
        // transactional rebuild of the active songs table; covers are
        // resolved lazily by the CoverLoader when each row scrolls into
        // view. This cuts a 10 000-track refresh from ~5 s of UI freeze
        // to a single ~100 ms transaction.
        bridge::set_songs_batch(&ffi_rows);

        {
            let mut last = LAST_LOADED_FILTER.lock();
            *last = canonical_key;
        }
        // Re-borrow the global list to find the active track position.
        // (We can't use `tracks` here because we just moved it.)
        let new_active = if playing_track_id != 0 {
            if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
                if let Some(list) = list_lock.try_lock() {
                    list.iter()
                        .position(|t| t.id == playing_track_id)
                        .unwrap_or(*CURRENT_INDEX.lock())
                } else {
                    *CURRENT_INDEX.lock()
                }
            } else {
                *CURRENT_INDEX.lock()
            }
        } else {
            *CURRENT_INDEX.lock()
        };
        {
            let mut idx = CURRENT_INDEX.lock();
            *idx = new_active;
        }
        let active_song_id = if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
            if let Some(list) = list_lock.try_lock() {
                list.get(new_active).map(|t| t.id as i32).unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        };
        LAST_PUSHED_ACTIVE_INDEX.store(active_song_id, std::sync::atomic::Ordering::Relaxed);
        bridge::set_active_index(active_song_id);
    }
}

pub fn refresh_folders_view() {
    FOLDERS_VIEW_DIRTY.store(false, std::sync::atomic::Ordering::SeqCst);
    let folders_opt = if let Some(db) = GLOBAL_DB.get() { db.get_all_folders().ok() } else { None };

    if let Some(folders) = folders_opt {
        bridge::clear_folders();
        for f in folders {
            bridge::add_folder(f.id as i32, &f.path, &f.name, f.track_count);
        }
    }
}

/// Push the current list of user playlists from the DB to the GUI sidebar.
pub fn refresh_playlists_view() {
    let playlists_opt =
        if let Some(db) = GLOBAL_DB.get() { db.get_all_playlists().ok() } else { None };
    bridge::clear_playlists();
    if let Some(playlists) = playlists_opt {
        for p in playlists {
            bridge::add_playlist(p.id as i32, &p.name, p.track_count, p.duration_secs);
        }
    }
}

/// Push the album grid to the GUI.
pub fn refresh_albums_view() {
    if !ALBUMS_VIEW_DIRTY.swap(false, std::sync::atomic::Ordering::SeqCst) {
        log::debug!("Albums view already populated, skipping refresh");
        return;
    }
    let albums_opt = if let Some(db) = GLOBAL_DB.get() { db.get_all_albums().ok() } else { None };
    bridge::clear_albums();
    if let Some(albums) = albums_opt {
        let track_paths: std::collections::HashMap<i64, String> = if let Some(db) = GLOBAL_DB.get()
        {
            db.get_track_paths_batch(&albums.iter().map(|a| a.id).collect::<Vec<_>>())
                .unwrap_or_default()
        } else {
            std::collections::HashMap::new()
        };
        for a in albums {
            let year = a.year.unwrap_or(0);
            let cover_path =
                track_paths.get(&a.id).and_then(|path| cached_cover_path(path)).unwrap_or_default();
            bridge::add_album(
                a.id as i32,
                &a.album,
                &a.album_artist,
                a.track_count,
                a.duration_secs,
                year,
                &cover_path,
            );
        }
    }
}

/// Push the artist list to the GUI.
pub fn refresh_artists_view() {
    if !ARTISTS_VIEW_DIRTY.swap(false, std::sync::atomic::Ordering::SeqCst) {
        log::debug!("Artists view already populated, skipping refresh");
        return;
    }
    let artists_opt = if let Some(db) = GLOBAL_DB.get() { db.get_all_artists().ok() } else { None };
    bridge::clear_artists();
    if let Some(artists) = artists_opt {
        let track_paths: std::collections::HashMap<i64, String> = if let Some(db) = GLOBAL_DB.get()
        {
            db.get_track_paths_batch(&artists.iter().map(|a| a.id).collect::<Vec<_>>())
                .unwrap_or_default()
        } else {
            std::collections::HashMap::new()
        };
        for a in artists {
            let cover_path =
                track_paths.get(&a.id).and_then(|path| cached_cover_path(path)).unwrap_or_default();
            bridge::add_artist(a.id as i32, &a.artist, a.album_count, a.track_count, &cover_path);
        }
    }
}

/// Push the albums of a specific artist to the GUI (used by the Artists
/// view's right panel).
pub fn refresh_albums_for_artist(artist_name: &str) {
    let albums_opt = if let Some(db) = GLOBAL_DB.get() {
        db.get_albums_by_artist(artist_name).ok()
    } else {
        None
    };
    bridge::clear_albums_in_artist();
    if let Some(albums) = albums_opt {
        for a in albums {
            bridge::add_album_to_artist(
                a.id as i32,
                &a.album,
                &a.album_artist,
                a.track_count,
                a.duration_secs,
            );
        }
    }
}

pub fn save_session_state() {
    // The full queue_ids_str is only built when the hash differs (i.e.,
    // when we're actually going to write to the DB). This eliminates
    // ~1 000 string allocations/sec during idle and ~1 000/sec during
    // playback (when elapsed changes every second but queue content
    // doesn't).
    static LAST_SAVED_HASH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static LAST_SAVED_INDEX: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static LAST_SAVED_TRACK_ID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
    static LAST_SAVED_ELAPSED: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    if let Some(db) = GLOBAL_DB.get() {
        // Compute the queue hash and read the active track id in a single
        // lock scope (we need to lock CURRENT_TRACK_LIST once anyway).
        // We also read the active_index here so we can use it for both the
        // dedup check and the DB write.
        let active_index = *CURRENT_INDEX.lock();

        let (queue_hash, active_track_id) = if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
            if let Some(list) = list_lock.try_lock() {
                let mut hasher = DefaultHasher::new();
                for track in list.iter() {
                    track.id.hash(&mut hasher);
                }
                let tid = list.get(active_index).map(|t| t.id).unwrap_or(0);
                (hasher.finish(), tid)
            } else {
                // Could not lock — use the previous hash to skip this
                // save. The next save attempt will retry.
                let prev_hash = LAST_SAVED_HASH.load(std::sync::atomic::Ordering::Relaxed);
                let prev_tid = LAST_SAVED_TRACK_ID.load(std::sync::atomic::Ordering::Relaxed);
                (prev_hash, prev_tid)
            }
        } else {
            (0, 0)
        };

        let elapsed = *ELAPSED_SECONDS.lock();
        let elapsed_rounded = elapsed as i64;

        // Dedup check: compare hash + index + track_id + elapsed_rounded.
        // If all match the last-saved values, skip the DB writes.
        let prev_hash = LAST_SAVED_HASH.load(std::sync::atomic::Ordering::Relaxed);
        let prev_index = LAST_SAVED_INDEX.load(std::sync::atomic::Ordering::Relaxed);
        let prev_track_id = LAST_SAVED_TRACK_ID.load(std::sync::atomic::Ordering::Relaxed);
        let prev_elapsed = LAST_SAVED_ELAPSED.load(std::sync::atomic::Ordering::Relaxed);

        let unchanged = queue_hash == prev_hash
            && active_index as u64 == prev_index
            && active_track_id == prev_track_id
            && elapsed_rounded == prev_elapsed;

        if unchanged {
            return;
        }

        // We pre-allocate the String with enough capacity for the worst
        // case (queue_len × 20 chars per i64 + queue_len commas). This
        // avoids reallocation during the join.
        let queue_ids_str = if queue_hash != prev_hash {
            // Queue content changed — rebuild the full string.
            if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
                if let Some(list) = list_lock.try_lock() {
                    // Pre-size: each i64 is at most 20 digits, plus a comma.
                    let estimated_cap = list.len() * 21;
                    let mut s = String::with_capacity(estimated_cap);
                    let mut first = true;
                    for track in list.iter() {
                        if !first {
                            s.push(',');
                        }
                        use std::fmt::Write;
                        let _ = write!(s, "{}", track.id);
                        first = false;
                    }
                    s
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            // We choose to skip: if the queue content (hash) hasn't
            // changed, the DB's queue_ids_str column is already correct.
            // We only write the 3 changed settings (index, track_id,
            // elapsed). This saves one SQLite UPDATE per save when only
            // the position changes (the common case during playback).
            String::new() // sentinel: empty string = "don't write queue_ids_str"
        };

        // Optimistically update the dedup atomics BEFORE the writes. If
        // the writes fail, we'll just retry on the next changed-state call.
        LAST_SAVED_HASH.store(queue_hash, std::sync::atomic::Ordering::Relaxed);
        LAST_SAVED_INDEX.store(active_index as u64, std::sync::atomic::Ordering::Relaxed);
        LAST_SAVED_TRACK_ID.store(active_track_id, std::sync::atomic::Ordering::Relaxed);
        LAST_SAVED_ELAPSED.store(elapsed_rounded, std::sync::atomic::Ordering::Relaxed);

        // Only write queue_ids_str if it was rebuilt (queue content changed).
        if !queue_ids_str.is_empty() {
            let _ = db.set_setting("last_played_queue_ids", &queue_ids_str);
        }
        let _ = db.set_setting("last_played_index", &active_index.to_string());
        if active_track_id > 0 {
            let _ = db.set_setting("last_played_track_id", &active_track_id.to_string());
        }
        let _ = db.set_setting("last_played_elapsed", &elapsed.to_string());
    }
}

pub fn populate_gui_state() {
    if SHUTDOWN.load(Ordering::SeqCst) {
        return;
    }
    log::info!("Populating track data into Qt UI...");

    // Load saved session state from DB settings FIRST before refreshing UI.
    let mut restored_track_id: i64 = 0;
    if let Some(db) = GLOBAL_DB.get() {
        if let Ok(Some(track_id_str)) = db.get_setting("last_played_track_id") {
            if let Ok(track_id) = track_id_str.parse::<i64>() {
                if track_id > 0 {
                    restored_track_id = track_id;
                }
            }
        }
        if let Ok(Some(elapsed_str)) = db.get_setting("last_played_elapsed") {
            if let Ok(elapsed) = elapsed_str.parse::<f64>() {
                let mut curr_elapsed = ELAPSED_SECONDS.lock();
                *curr_elapsed = elapsed;
            }
        }
    }

    // Force invalidation of filter cache so refresh_ui populates all tracks on launch
    crate::app_state::invalidate_loaded_filter();

    // 1. Populate main Songs table from DB
    refresh_ui("all", None);
    if SHUTDOWN.load(Ordering::SeqCst) {
        return;
    }

    if restored_track_id > 0 {
        if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
            if let Some(list) = list_lock.try_lock() {
                if let Some(pos) = list.iter().position(|t| t.id == restored_track_id) {
                    let mut idx = CURRENT_INDEX.lock();
                    *idx = pos;
                }
            }
        }
    }

    refresh_folders_view();
    if SHUTDOWN.load(Ordering::SeqCst) {
        return;
    }

    // 2. Populate right-side Up Next Queue from real tracks
    refresh_up_next_queue();

    let tracks_opt = if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
        if let Some(list) = list_lock.try_lock() {
            Some(list.clone())
        } else {
            None
        }
    } else {
        None
    };

    if let Some(tracks) = tracks_opt {
        if !tracks.is_empty() {
            let idx = *CURRENT_INDEX.lock() % tracks.len();
            let track = &tracks[idx];
            let cover_path = cached_cover_path(&track.path).unwrap_or_default();
            send_track_info_and_lyrics(track, &cover_path);
            if let Some(platform_lock) = PLATFORM.get() {
                if let Some(mut platform) = platform_lock.try_lock() {
                    platform.set_mpris_track(MprisTrackInfo {
                        title: Some(track.title.clone()),
                        artist: Some(track.artist.clone()),
                        album: Some(track.album.clone()),
                        art_url: Some(format!("file://{}", cover_path)),
                        length_microseconds: Some((track.duration_secs * 1_000_000.0) as i64),
                        track_id: Some(format!("/org/playtune/track/{}", track.id)),
                        ..Default::default()
                    });
                    platform.set_mpris_status(MprisPlaybackStatus::Stopped);
                }
            }

            // Open the track in the engine so it's loaded and paused
            if let Some(tx) = ENGINE_CMD_TX.get() {
                let vol = crate::app_state::CURRENT_VOLUME.load(Ordering::SeqCst) as f32 / 100.0;
                let _ = tx.send(EngineCommand::SetVolume(vol));
                let _ = tx.send(EngineCommand::OpenUri(track.path.clone()));
                let elapsed = *ELAPSED_SECONDS.lock();
                if elapsed > 0.0 {
                    let _ = tx.send(EngineCommand::Seek(elapsed as f32));
                }
                let _ = tx.send(EngineCommand::Pause);
            }

            bridge::set_active_index(track.id as i32);
            bridge::set_play_state(false);
            IS_PLAYING.store(false, Ordering::SeqCst);
            bridge::set_playback_progress(*ELAPSED_SECONDS.lock(), track.duration_secs);
        } else {
            bridge::set_track_info(
                "No track loaded",
                "Import music to start playing",
                "PlayTune Library",
                "",
            );
            bridge::set_play_state(false);
            IS_PLAYING.store(false, Ordering::SeqCst);
            bridge::set_playback_progress(0.0, 0.0);
        }
    }

    let current_backend = if let Some(engine_lock) = GLOBAL_ENGINE.get() {
        if let Some(engine) = engine_lock.try_lock() {
            engine.config().output_backend
        } else {
            AudioBackend::Auto
        }
    } else {
        AudioBackend::Auto
    };
    crate::app_state::spawn_worker("playtune-audio-devs", move || {
        push_audio_devices_to_gui(current_backend);
    });
}
