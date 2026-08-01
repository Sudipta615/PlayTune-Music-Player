use crate::app_state::spawn_worker;
use crate::bridge;
use crate::ffi_safe;
use crate::ui_sync::{
    next_nav_gen, populate_gui_state, refresh_albums_view, refresh_artists_view,
    refresh_folders_view, refresh_playlists_view, refresh_ui_gen,
};

pub extern "C" fn rust_nav_tab(tab_id: std::ffi::c_int) {
    ffi_safe!({
        rust_nav_tab_inner(tab_id);
    });
}

pub fn rust_nav_tab_inner(tab_id: std::ffi::c_int) {
    log::info!("Navigation tab switched: {}", tab_id);
    // Increment the generation counter BEFORE spawning a worker. Any
    // in-flight worker from a previous nav click will see the new value and
    // exit before issuing FFI calls, so we never push stale data.
    let gen = next_nav_gen();
    match tab_id {
        0 => {
            // Home → Songs Table (all tracks)
            // Switch view immediately for instant UI feedback.
            bridge::switch_view(0);
            // Load data in background to avoid freezing the GUI.
            spawn_worker("playtune-nav-home", move || {
                refresh_ui_gen("all", None, gen);
            });
        }
        1 => {
            // Albums tab — show the AlbumsViewWidget (page 3 of the stack).
            bridge::switch_view(3);
            spawn_worker("playtune-nav-albums", move || {
                refresh_albums_view();
            });
        }
        2 => {
            // Artists tab — show the ArtistsViewWidget (page 4 of the stack).
            bridge::switch_view(4);
            spawn_worker("playtune-nav-artists", move || {
                refresh_artists_view();
            });
        }
        3 => {
            // Folders view.
            // Previously this called refresh_ui("all") AND refresh_folders_view().
            // The refresh_ui("all") was redundant — the Home tab already
            // populated the song list; calling it again here triggered a
            // full DB query + 10 000-row FFI rebuild every time the user
            // clicked the Folders tab, causing visible CPU spikes.
            bridge::switch_view(2);
            spawn_worker("playtune-nav-folders", || {
                refresh_folders_view();
            });
        }
        4 => {
            // Settings view — no data loading needed, instant switch.
            bridge::switch_view(1);
        }
        5 => {
            // Favorites
            bridge::switch_view(0);
            spawn_worker("playtune-nav-favs", move || {
                refresh_ui_gen("favorites", None, gen);
            });
        }
        6 => {
            // Recently Played
            bridge::switch_view(0);
            spawn_worker("playtune-nav-recent", move || {
                refresh_ui_gen("recently_played", None, gen);
            });
        }
        7 => {
            // Most Played
            bridge::switch_view(0);
            spawn_worker("playtune-nav-most", move || {
                refresh_ui_gen("most_played", None, gen);
            });
        }
        _ => {}
    }
}

pub extern "C" fn rust_gui_ready() {
    ffi_safe!({
        log::info!("GUI reported ready. Populating initial state...");
        spawn_worker("playtune-gui-populate", || {
            populate_gui_state();
            // Push the initial playlists list to the sidebar too.
            refresh_playlists_view();
            // Load persisted settings for the new feature flags.
            load_persisted_feature_flags();
        });
    });
}

/// Load the persisted feature flags from the DB `settings` table and apply
/// them to the in-memory atomics. Called once at GUI-ready time.
fn load_persisted_feature_flags() {
    use std::sync::atomic::Ordering;
    let Some(db) = crate::app_state::GLOBAL_DB.get() else { return };
    if let Ok(Some(v)) = db.get_setting("notifications_enabled") {
        let on = v == "1";
        crate::app_state::NOTIFICATIONS_ENABLED.store(on, Ordering::SeqCst);
    }
    if let Ok(Some(v)) = db.get_setting("cursor_follows_playback") {
        let on = v == "1";
        crate::app_state::CURSOR_FOLLOWS_PLAYBACK.store(on, Ordering::SeqCst);
    }
    if let Ok(Some(v)) = db.get_setting("crossfade_enabled") {
        let on = v == "1";
        crate::app_state::CROSSFADE_ENABLED.store(on, Ordering::SeqCst);
        // Propagate to the engine.
        if let Some(engine_lock) = crate::app_state::GLOBAL_ENGINE.get() {
            if let Some(mut engine) = engine_lock.try_lock() {
                engine.pipeline_mut().mixer_mut().set_enabled(on);
            }
        }
    }
    if let Ok(Some(v)) = db.get_setting("crossfade_duration_ms") {
        if let Ok(ms) = v.parse::<u32>() {
            crate::app_state::CROSSFADE_DURATION_MS.store(ms, Ordering::SeqCst);
            if let Some(engine_lock) = crate::app_state::GLOBAL_ENGINE.get() {
                if let Some(mut engine) = engine_lock.try_lock() {
                    let sr = engine.config().sample_rate;
                    engine.pipeline_mut().mixer_mut().set_duration_ms(ms as u64, sr as f32);
                }
            }
        }
    }
    if let Ok(Some(v)) = db.get_setting("tray_enabled") {
        let on = v == "1";
        crate::app_state::TRAY_ENABLED.store(on, Ordering::SeqCst);
    }
    if let Ok(Some(v)) = db.get_setting("minimize_to_tray") {
        let on = v == "1";
        crate::app_state::MINIMIZE_TO_TRAY.store(on, Ordering::SeqCst);
    }
    // Restore playback speed.
    if let Ok(Some(v)) = db.get_setting("playback_speed") {
        if let Ok(s) = v.parse::<f32>() {
            let clamped = s.clamp(0.25, 4.0);
            if let Some(tx) = crate::app_state::ENGINE_CMD_TX.get() {
                let _ = tx.send(engine::buffer::EngineCommand::SetSpeed(clamped));
            }
            bridge::set_speed_label(clamped as f64);
        }
    }
}
