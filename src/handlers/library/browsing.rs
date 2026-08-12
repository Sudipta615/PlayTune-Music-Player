use std::sync::atomic::Ordering;

use crate::app_state::{
    invalidate_all_views, sync_shuffle_order, CURRENT_INDEX, CURRENT_TRACK_LIST, GLOBAL_DB,
    QUEUE_CLEARED_BY_USER, SHUFFLE_ENABLED, SHUFFLE_ORDER, SHUFFLE_POS,
};
use crate::bridge;
use crate::ffi_safe;
use crate::handlers::playback::rust_stop_inner;
use crate::ui_sync::{
    refresh_albums_for_artist, refresh_folders_view, refresh_ui, refresh_up_next_queue,
    save_session_state,
};

pub extern "C" fn rust_clear_queue() {
    ffi_safe!({
        log::info!("Clear Queue clicked");
        QUEUE_CLEARED_BY_USER.store(true, Ordering::SeqCst);
        bridge::clear_queue();
        save_session_state();
    });
}

pub extern "C" fn rust_remove_from_library(track_id: std::ffi::c_int) {
    ffi_safe!({
        let track_id_removed = track_id as i64;

        let old_track_id = CURRENT_TRACK_LIST.get().and_then(|l| l.try_lock()).and_then(|list| {
            let idx = *CURRENT_INDEX.lock();
            list.get(idx).map(|t| t.id)
        });

        if let Some(db) = GLOBAL_DB.get() {
            if let Err(e) = db.delete_track(track_id_removed) {
                log::error!("delete_track failed: {}", e);
            }
        }
        invalidate_all_views();
        refresh_ui("all", None);
        refresh_folders_view();

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
        if let Some(db) = GLOBAL_DB.get() {
            if let Ok(Some(track)) = db.get_track(artist_id as i64) {
                refresh_albums_for_artist(&track.artist);
            }
        }
    });
}

pub extern "C" fn rust_set_rating(track_id: std::ffi::c_int, _rating: std::ffi::c_int) {
    ffi_safe!({
        let Some(db) = GLOBAL_DB.get() else { return };
        let current_rating =
            db.get_track(track_id as i64).ok().flatten().map(|t| t.rating).unwrap_or(0);
        let new_rating = if current_rating == -1 { 0 } else { -1 };
        if let Err(e) = db.set_track_rating(track_id as i64, new_rating) {
            log::error!("Failed to toggle dislike for track {}: {}", track_id, e);
        } else {
            log::info!("Track {} dislike toggled: {} → {}", track_id, current_rating, new_rating);
            bridge::set_rating_for_row(track_id, new_rating);
        }
    });
}

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
