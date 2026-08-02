#![allow(clippy::needless_range_loop, clippy::manual_map)]

use std::ffi::c_double;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use config::LibraryConfig;
use db::PlayTuneDb;
use engine::{buffer::EngineCommand, buffer::PlaybackState, AudioEngine};
use library::LibraryManager;
use platform::{MediaKeyAction, PlatformIntegration};

mod app_state;
mod bridge;
mod handlers;
mod ui_sync;

use app_state::*;
use handlers::eq::rust_slider_param;
pub use handlers::library::{
    playtune_cancel_loudness_scan, playtune_get_track_lyrics, playtune_get_track_tags,
    playtune_start_loudness_scan, playtune_update_track_tags, playtune_write_loudness_results,
};
use handlers::playback::{
    rust_next, rust_open_uri, rust_play_pause, rust_prev, rust_seek, rust_set_loop_status,
    rust_stop, rust_volume,
};
use ui_sync::{refresh_folders_view, refresh_ui, save_session_state};

#[cfg(target_os = "linux")]
unsafe extern "C" fn silent_alsa_error_handler(
    _file: *const std::ffi::c_char,
    _line: std::ffi::c_int,
    _function: *const std::ffi::c_char,
    _err: std::ffi::c_int,
    _fmt: *const std::ffi::c_char,
) {
    // No-op: silences ALSA lib stderr warnings during device enumeration
}

#[cfg(target_os = "linux")]
unsafe fn silence_alsa_logs() {
    #[link(name = "asound")]
    extern "C" {
        fn snd_lib_error_set_handler(
            handler: unsafe extern "C" fn(
                *const std::ffi::c_char,
                std::ffi::c_int,
                *const std::ffi::c_char,
                std::ffi::c_int,
                *const std::ffi::c_char,
            ),
        ) -> std::ffi::c_int;
    }
    let _ = snd_lib_error_set_handler(silent_alsa_error_handler);
}

#[cfg(not(target_os = "linux"))]
unsafe fn silence_alsa_logs() {}

fn main() {
    env_logger::init();
    unsafe {
        silence_alsa_logs();
    }
    log::info!("Initializing PlayTune Music Engine...");

    let _ = CURRENT_TRACK_LIST.set(parking_lot::Mutex::new(Vec::new()));

    if let Ok((platform, media_rx)) = PlatformIntegration::new() {
        let mut platform = platform;
        let _ = &media_rx;
        let _ = platform.register_mpris("PlayTune");
        let _ = PLATFORM.set(parking_lot::Mutex::new(platform));

        spawn_worker("playtune-media-keys", move || loop {
            if SHUTDOWN.load(Ordering::SeqCst) {
                break;
            }
            let _ = &media_rx;
            match media_rx.recv_timeout(Duration::from_millis(200)) {
                Ok(action) => match action {
                    MediaKeyAction::PlayPause => rust_play_pause(),
                    MediaKeyAction::Play => {
                        if IS_PLAYING
                            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                            .is_ok()
                        {
                            apply_play_state(true);
                        }
                    }
                    MediaKeyAction::Pause => {
                        if IS_PLAYING
                            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
                            .is_ok()
                        {
                            apply_play_state(false);
                        }
                    }
                    MediaKeyAction::Next => rust_next(),
                    MediaKeyAction::Previous => rust_prev(),
                    MediaKeyAction::Stop => rust_stop(),
                    MediaKeyAction::Seek(us) => rust_seek(us as f64 / 1_000_000.0),
                    MediaKeyAction::SetPosition { position_us, .. } => {
                        rust_seek(position_us as f64 / 1_000_000.0)
                    }
                    MediaKeyAction::SetVolume(v) => rust_volume(v as c_double),
                    MediaKeyAction::VolumeUp => {
                        let cur = CURRENT_VOLUME.load(Ordering::SeqCst) as f64 / 100.0;
                        rust_volume((cur + 0.05).min(2.0));
                    }
                    MediaKeyAction::VolumeDown => {
                        let cur = CURRENT_VOLUME.load(Ordering::SeqCst) as f64 / 100.0;
                        rust_volume((cur - 0.05).max(0.0));
                    }
                    MediaKeyAction::OpenUri(uri) => rust_open_uri(&uri),
                    MediaKeyAction::SetShuffle(b) => {
                        rust_slider_param(6, if b { 1.0 } else { 0.0 })
                    }
                    MediaKeyAction::SetLoopStatus(s) => rust_set_loop_status(&s),
                    MediaKeyAction::SetRate(r) => rust_slider_param(7, r as c_double),
                    MediaKeyAction::ToggleShuffle => {
                        let cur = SHUFFLE_ENABLED.load(Ordering::SeqCst);
                        rust_slider_param(6, if !cur { 1.0 } else { 0.0 });
                    }
                    MediaKeyAction::ToggleRepeat => {
                        let cur = REPEAT_ENABLED.load(Ordering::SeqCst);
                        rust_slider_param(5, if !cur { 1.0 } else { 0.0 });
                    }
                    MediaKeyAction::GlobalSearch => {
                        log::debug!("GlobalSearch media-key action received");
                    }
                    MediaKeyAction::Quit => {
                        bridge::request_quit();
                        SHUTDOWN.store(true, Ordering::SeqCst);
                    }
                    MediaKeyAction::Mute | MediaKeyAction::Raise => {}
                },
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    log::warn!("Media key channel disconnected; media key thread exiting.");
                    break;
                }
            }
        });
    } else {
        log::warn!("PlatformIntegration::new() failed; media keys are unavailable.");
    }

    match AudioEngine::new_default() {
        Ok(mut engine) => {
            let cmd_tx = engine.send_command_channel();
            let pb_info = engine.playback_info_arc();
            let vis_tap = engine.visualizer_tap();
            let _ = ENGINE_CMD_TX.set(cmd_tx);
            let _ = PLAYBACK_INFO.set(pb_info);
            let _ = VISUALIZER_TAP.set(vis_tap);
            match engine.start() {
                Ok(_) => log::info!("AudioEngine started successfully with output stream."),
                Err(e) => log::warn!(
                    "AudioEngine initialized without output stream (offline/dummy mode): {:?}",
                    e
                ),
            }
            let _ = GLOBAL_ENGINE.set(parking_lot::Mutex::new(engine));
        }
        Err(e) => {
            log::error!("Failed to initialize AudioEngine: {:?}", e);
        }
    }

    // Spawn engine ticker and playback synchronization thread
    spawn_worker("playtune-ticker", || {
        let _ = engine::buffer::enable_flush_zero_denormals_on_current_thread();

        let mut visualizer_phase = 0.0f32;
        let mut mock_spectrum = [0.1f32; 65];
        let mut prev_engine_state = PlaybackState::Stopped;
        let mut last_ui_update = std::time::Instant::now();
        let mut last_db_save = std::time::Instant::now();

        loop {
            if SHUTDOWN.load(Ordering::SeqCst) {
                break;
            }
            // 1. Tick the audio engine
            let mut has_pending = false;
            let mut current_engine_state = PlaybackState::Stopped;

            if let Some(engine_lock) = GLOBAL_ENGINE.get() {
                if let Some(mut engine) = engine_lock.try_lock() {
                    engine.tick();
                    has_pending = engine.has_pending_chunk();
                    current_engine_state = engine.current_state();
                }
            }

            let is_playing_engine = current_engine_state == PlaybackState::Playing;
            let sleep_ms = if is_playing_engine && !has_pending { 10 } else { 100 };

            let ui_sync_interval_ms = if is_playing_engine { 30u64 } else { 200u64 };

            // 2. Read PlaybackInfo lock-free and sync UI state
            if last_ui_update.elapsed() >= Duration::from_millis(ui_sync_interval_ms) {
                last_ui_update = std::time::Instant::now();

                if let Some(info_swap) = PLAYBACK_INFO.get() {
                    let info = info_swap.load();

                    if prev_engine_state == PlaybackState::Playing
                        && info.state == PlaybackState::Stopped
                        && info.duration_secs > 0.0
                        && IS_PLAYING.load(Ordering::SeqCst)
                    {
                        log::info!("Track finished (engine EOS). Auto-playing next track.");
                        rust_next();
                    }
                    prev_engine_state = info.state;

                    let is_playing =
                        info.state == PlaybackState::Playing || IS_PLAYING.load(Ordering::SeqCst);

                    static LAST_PUSHED_PLAY_STATE: std::sync::atomic::AtomicBool =
                        std::sync::atomic::AtomicBool::new(false);
                    if LAST_PUSHED_PLAY_STATE.swap(is_playing, std::sync::atomic::Ordering::Relaxed)
                        != is_playing
                    {
                        bridge::set_play_state(is_playing);
                    }

                    if info.duration_secs > 0.0 {
                        let pos_tenths = (info.position_secs * 10.0) as i64;
                        let dur_tenths = (info.duration_secs * 10.0) as i64;
                        static LAST_PUSHED_POS: std::sync::atomic::AtomicI64 =
                            std::sync::atomic::AtomicI64::new(i64::MIN);
                        static LAST_PUSHED_DUR: std::sync::atomic::AtomicI64 =
                            std::sync::atomic::AtomicI64::new(i64::MIN);
                        let prev_pos =
                            LAST_PUSHED_POS.swap(pos_tenths, std::sync::atomic::Ordering::Relaxed);
                        let prev_dur =
                            LAST_PUSHED_DUR.swap(dur_tenths, std::sync::atomic::Ordering::Relaxed);
                        if prev_pos != pos_tenths || prev_dur != dur_tenths {
                            bridge::set_playback_progress(
                                info.position_secs as f64,
                                info.duration_secs as f64,
                            );
                        }
                        if let Some(mut elapsed) = ELAPSED_SECONDS.try_lock() {
                            *elapsed = info.position_secs as f64;
                        }
                        let pos_us = (info.position_secs * 1_000_000.0) as i64;
                        static LAST_PUSHED_MPRIS_POS: std::sync::atomic::AtomicI64 =
                            std::sync::atomic::AtomicI64::new(i64::MIN);
                        let prev_mpris_pos = LAST_PUSHED_MPRIS_POS
                            .swap(pos_us, std::sync::atomic::Ordering::Relaxed);
                        if prev_mpris_pos != pos_us {
                            if let Some(platform_lock) = PLATFORM.get() {
                                if let Some(mut platform) = platform_lock.try_lock() {
                                    platform.set_mpris_position(pos_us);
                                }
                            }
                        }

                        let track_id_opt = if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
                            if let Some(list) = list_lock.try_lock() {
                                let idx = *CURRENT_INDEX.lock();
                                list.get(idx % list.len().max(1)).map(|t| (t.id, t.duration_secs))
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        if let Some((track_id, dur_secs)) = track_id_opt {
                            // 10s, but never more than 50% of the track (so
                            // a 5s jingle still needs ~2.5s of listening,
                            // not the full 10s which is impossible).
                            let threshold = {
                                let half = dur_secs * 0.5;
                                10.0_f64.min(half).max(1.0)
                            };
                            let already_recorded =
                                LAST_RECORDED_TRACK_ID.load(Ordering::SeqCst) == track_id;
                            if !already_recorded && info.position_secs as f64 >= threshold {
                                if let Some(db) = GLOBAL_DB.get() {
                                    let _ = db.record_play(track_id);
                                    LAST_RECORDED_TRACK_ID.store(track_id, Ordering::SeqCst);
                                    log::info!(
                                        "Recorded play for track {} (pos {:.1}s / {:.1}s, threshold {:.1}s)",
                                        track_id,
                                        info.position_secs,
                                        dur_secs,
                                        threshold
                                    );
                                }
                            }
                        }
                    } else if is_playing {
                        let track_opt = if let Some(list_lock) = CURRENT_TRACK_LIST.get() {
                            if let Some(list) = list_lock.try_lock() {
                                if !list.is_empty() {
                                    let idx = CURRENT_INDEX.lock();
                                    Some((
                                        list[*idx % list.len()].clone(),
                                        *idx % list.len(),
                                        list.len(),
                                    ))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        if let Some((track, _current_idx, list_len)) = track_opt {
                            {
                                let elapsed_val = { *ELAPSED_SECONDS.lock() };
                                let already_recorded =
                                    LAST_RECORDED_TRACK_ID.load(Ordering::SeqCst) == track.id;
                                if !already_recorded && elapsed_val >= 10.0 {
                                    if let Some(db) = GLOBAL_DB.get() {
                                        let _ = db.record_play(track.id);
                                        LAST_RECORDED_TRACK_ID.store(track.id, Ordering::SeqCst);
                                        log::info!(
                                            "Recorded play for track {} (elapsed {:.1}s)",
                                            track.id,
                                            elapsed_val
                                        );
                                    }
                                }
                            }

                            let (current_elapsed, finished) = {
                                let mut elapsed = ELAPSED_SECONDS.lock();
                                *elapsed += 0.03;
                                if *elapsed >= track.duration_secs && track.duration_secs > 0.0 {
                                    *elapsed = 0.0;
                                    (0.0, true)
                                } else {
                                    (*elapsed, false)
                                }
                            };

                            if finished && list_len > 0 {
                                if QUEUE_CLEARED_BY_USER.load(Ordering::SeqCst) {
                                    log::info!(
                                        "[Rust] Track finished and Up Next queue is empty (cleared by user). Stopping playback."
                                    );
                                    if let Some(tx) = ENGINE_CMD_TX.get() {
                                        let _ = tx.send(EngineCommand::Stop);
                                    }
                                    IS_PLAYING.store(false, Ordering::SeqCst);
                                    bridge::set_play_state(false);
                                    bridge::set_playback_progress(0.0, track.duration_secs);
                                } else {
                                    log::info!("[Rust] Track finished. Auto-playing next track.");
                                    rust_next();
                                }
                            }
                            bridge::set_playback_progress(current_elapsed, track.duration_secs);
                        }
                    }

                    // Push visualizer data
                    static PAUSED_SPECTRUM_PUSHED: std::sync::atomic::AtomicBool =
                        std::sync::atomic::AtomicBool::new(false);
                    if is_playing {
                        PAUSED_SPECTRUM_PUSHED.store(false, std::sync::atomic::Ordering::Relaxed);
                        let mut bars = [0.0f32; 65];
                        let n = if let Some(tap) = VISUALIZER_TAP.get() {
                            tap.get_bars_into(&mut bars)
                        } else {
                            0
                        };

                        let has_real_audio = bars[..n].iter().any(|&b| b > 0.001);
                        if has_real_audio && n == 65 {
                            bridge::update_visualizer(&bars);
                            // Reset the mock-phase tracker so the next time we
                            // fall back to mock data, it starts fresh.
                            visualizer_phase = 0.0;
                        } else {
                            // To keep the visible animation speed unchanged
                            // despite 5× fewer recompute ticks, advance the
                            // phase by 5× the per-tick delta on each
                            // recompute. The static counter gates the
                            // recompute + FFI call to every 5th tick; the
                            // other 4 ticks do zero work.
                            static MOCK_TICK: std::sync::atomic::AtomicU32 =
                                std::sync::atomic::AtomicU32::new(0);
                            let tick = MOCK_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if tick % 5 == 0 {
                                visualizer_phase += 0.15 * 5.0;
                                for i in 0..65 {
                                    let frac = i as f32 / 65.0;
                                    let val = 0.15
                                        + 0.40 * (frac * std::f32::consts::PI).sin()
                                        + 0.25
                                            * (frac * 4.0 * std::f32::consts::PI
                                                + visualizer_phase)
                                                .sin()
                                            * (frac * 2.5 * std::f32::consts::PI).cos()
                                        + 0.10
                                            * (frac * 12.0 * std::f32::consts::PI
                                                + visualizer_phase * 2.0)
                                                .cos();
                                    mock_spectrum[i] = val.abs().clamp(0.1, 0.95);
                                }
                                bridge::update_visualizer(&mock_spectrum);
                            }
                        }
                    } else {
                        if !PAUSED_SPECTRUM_PUSHED.swap(true, std::sync::atomic::Ordering::Relaxed)
                        {
                            mock_spectrum.fill(0.05);
                            bridge::update_visualizer(&mock_spectrum);
                        }
                        visualizer_phase = 0.0;
                    }
                    if last_db_save.elapsed() >= Duration::from_secs(1) {
                        last_db_save = std::time::Instant::now();
                        save_session_state();
                    }
                }

                if crate::app_state::tick_sleep_timer() {
                    log::info!("Sleep timer fired — pausing playback.");
                    crate::app_state::apply_play_state(false);
                    IS_PLAYING.store(false, Ordering::SeqCst);
                }
            }
            thread::sleep(Duration::from_millis(sleep_ms));
        }
    });

    // Initialize Database
    match PlayTuneDb::open_default() {
        Ok(db) => {
            log::info!("Database initialized successfully.");
            let arc_db = std::sync::Arc::new(db);
            let _ = GLOBAL_DB.set(arc_db.clone());

            let db_for_cleanup = std::sync::Arc::clone(&arc_db);
            spawn_worker("playtune-mock-cleanup", move || {
                if SHUTDOWN.load(Ordering::SeqCst) {
                    return;
                }
                match db_for_cleanup.delete_mock_tracks() {
                    Ok(n) if n > 0 => log::info!("Cleaned up {} mock tracks", n),
                    Ok(_) => {}
                    Err(e) => log::warn!("Mock-track cleanup failed: {}", e),
                }
            });

            let mut lib_config = LibraryConfig::default();
            let has_stored_folders = if let Ok(stored_folders) = arc_db.get_all_folders() {
                for folder in &stored_folders {
                    let p = std::path::PathBuf::from(&folder.path);
                    if p.is_dir() && !lib_config.watch_dirs.contains(&p) {
                        log::info!("Restoring watched folder from DB: {}", folder.path);
                        lib_config.watch_dirs.push(p);
                    }
                }
                !stored_folders.is_empty()
            } else {
                false
            };
            // Check if the user has ever explicitly configured folders via the UI
            let folders_configured = arc_db
                .get_setting("folders_configured")
                .ok()
                .flatten()
                .map(|v| v == "1")
                .unwrap_or(false);
            if !has_stored_folders && !folders_configured {
                if let Some(doc_dir) = dirs::audio_dir() {
                    if !lib_config.watch_dirs.contains(&doc_dir) {
                        lib_config.watch_dirs.push(doc_dir);
                    }
                } else if let Some(mut home) = dirs::home_dir() {
                    home.push("Music");
                    if !lib_config.watch_dirs.contains(&home) {
                        lib_config.watch_dirs.push(home);
                    }
                }
            }
            let lib_mgr = LibraryManager::new(arc_db, lib_config);
            let lib_mgr_arc = std::sync::Arc::new(lib_mgr);
            let _ = LIBRARY_MANAGER.set(lib_mgr_arc.clone());

            let mgr_for_scan = lib_mgr_arc.clone();
            spawn_worker("playtune-startup-scan", move || {
                log::info!("Starting background library scan on startup...");
                let _ = mgr_for_scan.scan(|p| {
                    if SHUTDOWN.load(Ordering::SeqCst) {
                        return;
                    }
                    if p.files_processed % 100 == 0 {
                        log::info!(
                            "Startup scan: {}/{} files processed",
                            p.files_processed,
                            p.files_found
                        );
                    }
                });
                log::info!("Background library scan complete.");
                if SHUTDOWN.load(Ordering::SeqCst) {
                    return;
                }
                crate::app_state::invalidate_loaded_filter();
                refresh_ui("all", None);
                if SHUTDOWN.load(Ordering::SeqCst) {
                    return;
                }
                refresh_folders_view();
            });
        }
        Err(e) => {
            log::info!("Failed to open PlayTuneDb: {:?}", e);
        }
    }

    let callbacks = handlers::create_callbacks();

    log::info!("Launching C++ Qt6 GUI...");

    let args: Vec<String> = std::env::args_os()
        .map(|os| os.into_string().unwrap_or_else(|_| String::from("<invalid-utf8>")))
        .collect();

    let _ = ctrlc::set_handler(|| {
        log::info!("SIGINT received; initiating graceful shutdown");
        SHUTDOWN.store(true, Ordering::SeqCst);
        bridge::request_quit();
    });

    extern "C" fn qt_shutdown_hook() {
        SHUTDOWN.store(true, Ordering::SeqCst);
    }
    bridge::install_shutdown_hook(qt_shutdown_hook);

    let exit_code = bridge::start_gui(args, callbacks);

    SHUTDOWN.store(true, Ordering::SeqCst);
    save_session_state();
    let mut handles = WORKER_HANDLES.lock();
    for handle in handles.drain(..) {
        let worker_name = handle.thread().name().unwrap_or("unnamed-worker").to_string();
        std::thread::Builder::new()
            .name(format!("playtune-join-watcher-{}", worker_name))
            .spawn(move || {
                let start = std::time::Instant::now();
                let deadline = start + std::time::Duration::from_secs(2);
                while !handle.is_finished() {
                    if std::time::Instant::now() >= deadline {
                        log::warn!(
                            "Worker thread '{}' did not shut down within 2s; \
                             detaching (it will exit when it next checks SHUTDOWN)",
                            worker_name
                        );
                        std::mem::forget(handle);
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                let _ = handle.join();
                log::debug!("Worker thread '{}' shut down in {:?}", worker_name, start.elapsed());
            })
            .ok();
    }
    drop(handles);

    log::info!("GUI closed. Exit code: {}", exit_code);
}
