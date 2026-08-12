use std::ffi::c_double;
use std::sync::atomic::Ordering;

use engine::buffer::EngineCommand;
use platform::{MprisPlaybackStatus, MprisTrackInfo};

use crate::app_state::{
    apply_play_state, cached_cover_path, send_track_info_and_lyrics, sync_shuffle_order,
    CURRENT_INDEX, CURRENT_TRACK_LIST, CURRENT_VOLUME, ELAPSED_SECONDS, ENGINE_CMD_TX, IS_PLAYING,
    LAST_RECORDED_TRACK_ID, PLATFORM, QUEUE_CLEARED_BY_USER, REPEAT_ENABLED, SHUFFLE_ENABLED,
    SHUFFLE_ORDER, SHUFFLE_POS, USER_SELECT_GEN,
};
use crate::bridge;
use crate::ffi_safe;
use crate::ui_sync::{refresh_up_next_queue, save_session_state};

pub extern "C" fn rust_play_pause() {
    ffi_safe!({
        for _ in 0..4 {
            let old_state = IS_PLAYING.load(Ordering::SeqCst);
            let new_state = !old_state;
            if IS_PLAYING
                .compare_exchange(old_state, new_state, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                log::info!("Play/Pause toggled. New playing state: {}", new_state);
                apply_play_state(new_state);
                break;
            }
        }
    });
}

pub extern "C" fn rust_prev() {
    ffi_safe!({
        rust_prev_inner();
    });
}

pub fn rust_prev_inner() {
    QUEUE_CLEARED_BY_USER.store(false, Ordering::SeqCst);
    let track_opt = if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
        if let Some(list) = list_lock.try_lock() {
            if !list.is_empty() {
                let mut idx = CURRENT_INDEX.lock();
                let mut found = false;
                if REPEAT_ENABLED.load(Ordering::SeqCst) {
                    // keep *idx unchanged
                } else if SHUFFLE_ENABLED.load(Ordering::SeqCst) {
                    sync_shuffle_order(*idx, list.len());
                    let order = SHUFFLE_ORDER.lock();
                    let mut pos = SHUFFLE_POS.lock();
                    if !order.is_empty() {
                        let mut attempts = 0;
                        while attempts < order.len() {
                            *pos = if *pos > 0 { *pos - 1 } else { order.len() - 1 };
                            let candidate = order[*pos];
                            if candidate < list.len() && list[candidate].rating != -1 {
                                *idx = candidate;
                                break;
                            }
                            attempts += 1;
                        }
                    }
                } else {
                    let start_idx = *idx;
                    for _ in 0..list.len() {
                        if *idx > 0 {
                            *idx -= 1;
                        } else {
                            *idx = list.len() - 1;
                        }
                        if list[*idx].rating != -1 {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        *idx = start_idx;
                    }
                }
                Some((list[*idx].clone(), *idx))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if let Some((track, _new_idx)) = track_opt {
        USER_SELECT_GEN.fetch_add(1, Ordering::SeqCst);
        log::info!("Prev Track clicked. Playing track: {}", track.title);
        *ELAPSED_SECONDS.lock() = 0.0;

        let cover_path = cached_cover_path(&track.path).unwrap_or_default();
        send_track_info_and_lyrics(&track, &cover_path);
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
                platform.set_mpris_status(MprisPlaybackStatus::Playing);
            }
        }

        bridge::set_active_index(track.id as i32);
        // Issue 5: sync IS_PLAYING flag so Play/Pause button works on first press
        IS_PLAYING.store(true, Ordering::SeqCst);
        bridge::set_play_state(true);
        bridge::set_playback_progress(0.0, track.duration_secs);
        // Issue 6: reset play-count tracker for the new track
        LAST_RECORDED_TRACK_ID.store(0, Ordering::SeqCst);
        refresh_up_next_queue();

        if let Some(tx) = ENGINE_CMD_TX.get() {
            let _ = tx.send(EngineCommand::OpenUri(track.path.clone()));
            let _ = tx.send(EngineCommand::Play);
        }
        save_session_state();
    }
}

pub extern "C" fn rust_next() {
    ffi_safe!({
        rust_next_inner();
    });
}

pub fn rust_next_inner() {
    QUEUE_CLEARED_BY_USER.store(false, Ordering::SeqCst);
    let track_opt = if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
        if let Some(list) = list_lock.try_lock() {
            if !list.is_empty() {
                let mut idx = CURRENT_INDEX.lock();
                let mut found = false;
                if REPEAT_ENABLED.load(Ordering::SeqCst) {
                    // keep *idx unchanged — replay same track
                } else if SHUFFLE_ENABLED.load(Ordering::SeqCst) {
                    sync_shuffle_order(*idx, list.len());
                    let order = SHUFFLE_ORDER.lock();
                    let mut pos = SHUFFLE_POS.lock();
                    if !order.is_empty() {
                        let mut attempts = 0;
                        while attempts < order.len() {
                            *pos = (*pos + 1) % order.len();
                            let candidate = order[*pos];
                            if candidate < list.len() && list[candidate].rating != -1 {
                                *idx = candidate;
                                break;
                            }
                            attempts += 1;
                        }
                    }
                } else {
                    let start_idx = *idx;
                    for _ in 0..list.len() {
                        *idx = (*idx + 1) % list.len();
                        if list[*idx].rating != -1 {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        *idx = start_idx; // Fallback if all are disliked
                    }
                }
                Some((list[*idx].clone(), *idx))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if let Some((track, _new_idx)) = track_opt {
        USER_SELECT_GEN.fetch_add(1, Ordering::SeqCst);
        log::info!("Next Track clicked. Playing track: {}", track.title);
        *ELAPSED_SECONDS.lock() = 0.0;

        let cover_path = cached_cover_path(&track.path).unwrap_or_default();
        send_track_info_and_lyrics(&track, &cover_path);
        // notify_track_change is called inside send_track_info_and_lyrics.
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
                platform.set_mpris_status(MprisPlaybackStatus::Playing);
            }
        }

        bridge::set_active_index(track.id as i32);
        // Issue 5: sync IS_PLAYING flag so Play/Pause button works on first press
        IS_PLAYING.store(true, Ordering::SeqCst);
        bridge::set_play_state(true);
        bridge::set_playback_progress(0.0, track.duration_secs);
        // Issue 6: reset play-count tracker for the new track
        LAST_RECORDED_TRACK_ID.store(0, Ordering::SeqCst);
        refresh_up_next_queue();

        if let Some(tx) = ENGINE_CMD_TX.get() {
            let _ = tx.send(EngineCommand::OpenUri(track.path.clone()));
            let _ = tx.send(EngineCommand::Play);
        }
        save_session_state();
    }
}

pub extern "C" fn rust_seek(seconds: c_double) {
    ffi_safe!({
        rust_seek_inner(seconds);
    });
}

pub fn rust_seek_inner(seconds: c_double) {
    let mut elapsed = ELAPSED_SECONDS.lock();
    *elapsed = seconds;
    log::info!("Seek requested to {:.2} seconds", seconds);
    if let Some(tx) = ENGINE_CMD_TX.get() {
        let _ = tx.send(EngineCommand::Seek(seconds as f32));
    }
    if let Some(platform_lock) = PLATFORM.get() {
        if let Some(mut platform) = platform_lock.try_lock() {
            platform.set_mpris_seek((seconds * 1_000_000.0) as i64);
        }
    }
}

pub extern "C" fn rust_volume(volume: c_double) {
    ffi_safe!({
        rust_volume_inner(volume);
    });
}

pub fn rust_volume_inner(volume: c_double) {
    if !volume.is_finite() {
        log::warn!("SetVolume ignored: non-finite value {}", volume);
        return;
    }
    let stored = volume.clamp(0.0, 2.0);
    let engine_volume = volume.clamp(0.0, 1.0);
    CURRENT_VOLUME.store(((stored * 100.0).round() as u32).min(200), Ordering::SeqCst);
    log::debug!(
        "Volume changed to {:.2}% (engine gain: {:.2}%)",
        stored * 100.0,
        engine_volume * 100.0
    );
    if let Some(tx) = ENGINE_CMD_TX.get() {
        let _ = tx.send(EngineCommand::SetVolume(engine_volume as f32));
    }
    if let Some(platform_lock) = PLATFORM.get() {
        if let Some(mut platform) = platform_lock.try_lock() {
            platform.set_mpris_volume(engine_volume as f32);
        }
    }
}

pub extern "C" fn rust_stop() {
    ffi_safe!({
        rust_stop_inner();
    });
}

pub fn rust_stop_inner() {
    IS_PLAYING.store(false, Ordering::SeqCst);
    log::info!("Stop requested");
    bridge::set_play_state(false);
    if let Some(tx) = ENGINE_CMD_TX.get() {
        let _ = tx.send(EngineCommand::Stop);
    }
    if let Some(platform_lock) = PLATFORM.get() {
        if let Some(mut platform) = platform_lock.try_lock() {
            platform.set_mpris_status(MprisPlaybackStatus::Stopped);
        }
    }
}

pub fn rust_open_uri(uri: &str) {
    log::info!("OpenUri requested: {}", uri);
    let path_str = if let Some(stripped) = uri.strip_prefix("file://") {
        percent_decode_path(stripped)
    } else {
        uri.to_string()
    };
    if let Some(tx) = ENGINE_CMD_TX.get() {
        let _ = tx.send(EngineCommand::OpenUri(path_str.clone()));
        let _ = tx.send(EngineCommand::Play);
    }
}

pub fn percent_decode_path(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_digit(bytes[i + 1]);
            let lo = hex_digit(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn rust_set_loop_status(status: &str) {
    match status {
        "None" | "Track" | "Playlist" => {
            REPEAT_ENABLED.store(status != "None", Ordering::SeqCst);
            if let Some(tx) = ENGINE_CMD_TX.get() {
                let _ = tx.send(EngineCommand::SetLoopStatus(status.to_string()));
            }
        }
        other => {
            log::warn!("rust_set_loop_status: invalid status '{}'", other);
        }
    }
}

pub extern "C" fn rust_select_song(song_idx: i32) {
    ffi_safe!({
        if song_idx < 0 {
            log::warn!("rust_select_song: negative song_idx {song_idx}");
            return;
        }
        rust_select_song_inner(song_idx);
    });
}

pub fn rust_select_song_inner(song_idx: i32) {
    USER_SELECT_GEN.fetch_add(1, Ordering::SeqCst);
    QUEUE_CLEARED_BY_USER.store(false, Ordering::SeqCst);
    let target_id = song_idx as i64;
    let (track_opt, list_len) = if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
        if let Some(list) = list_lock.try_lock() {
            if !list.is_empty() {
                // First try matching by track ID (e.g. when table is sorted/filtered)
                if let Some(pos) = list.iter().position(|t| t.id == target_id) {
                    (Some(list[pos].clone()), list.len())
                } else {
                    // Fallback to 0-based list index if within bounds
                    let idx = (song_idx as usize) % list.len();
                    (list.get(idx).cloned(), list.len())
                }
            } else {
                (None, 0)
            }
        } else {
            (None, 0)
        }
    } else {
        (None, 0)
    };

    if let Some(track) = track_opt {
        let track_path = std::path::Path::new(&track.path);
        if !track_path.is_file() {
            log::warn!("Selected track file does not exist: {}", track.path);
            crate::app_state::notify_track_change(&track);
            bridge::show_desktop_notification(
                "Track Unavailable",
                &format!("File not found: {}", track.title),
            );
            IS_PLAYING.store(false, Ordering::SeqCst);
            bridge::set_play_state(false);
            crate::app_state::invalidate_all_views();
            crate::ui_sync::refresh_ui("all", None);
            return;
        }

        let selected_idx = if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
            if let Some(list) = list_lock.try_lock() {
                list.iter().position(|t| t.id == track.id).unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        };
        if let Some(mut curr_idx) = CURRENT_INDEX.try_lock() {
            *curr_idx = selected_idx;
        }
        if SHUFFLE_ENABLED.load(Ordering::SeqCst) && list_len > 0 {
            sync_shuffle_order(selected_idx, list_len);
        }
        log::info!("Song selected from list: {} - {}", track.title, track.artist);
        *ELAPSED_SECONDS.lock() = 0.0;

        let cover_path = cached_cover_path(&track.path).unwrap_or_default();
        send_track_info_and_lyrics(&track, &cover_path);
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
                platform.set_mpris_status(MprisPlaybackStatus::Playing);
            }
        }

        bridge::set_active_index(track.id as i32);
        bridge::set_play_state(true);
        IS_PLAYING.store(true, Ordering::SeqCst);
        bridge::set_playback_progress(0.0, track.duration_secs);
        // Issue 6: reset play-count tracker for the newly selected track
        LAST_RECORDED_TRACK_ID.store(0, Ordering::SeqCst);
        refresh_up_next_queue();

        if let Some(tx) = ENGINE_CMD_TX.get() {
            let _ = tx.send(EngineCommand::OpenUri(track.path.clone()));
            let _ = tx.send(EngineCommand::Play);
        }
        save_session_state();
    }
}
