use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::{Mutex, RwLock};

use db::{PlayTuneDb, TrackRecord as DbTrack};
use engine::{
    buffer::{EngineCommand, PlaybackInfo},
    AudioEngine,
};
use library::LibraryManager;
use platform::{MprisPlaybackStatus, PlatformIntegration};

use crate::bridge;

#[macro_export]
macro_rules! ffi_safe {
    ($body:block) => {{
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body));
        if result.is_err() {
            log::error!("panic inside extern \"C\" callback (contained by ffi_safe!)");
        }
    }};
}

pub static WORKER_HANDLES: Mutex<Vec<std::thread::JoinHandle<()>>> =
    parking_lot::const_mutex(Vec::new());

pub fn spawn_worker<F>(name: &str, f: F)
where
    F: FnOnce() + Send + 'static,
{
    match thread::Builder::new().name(name.to_string()).spawn(f) {
        Ok(handle) => {
            let mut handles = WORKER_HANDLES.lock();
            // Drain finished handles before appending the new one. Without
            // this, every nav click or search that spawns a worker adds a
            // permanently-dead JoinHandle to the Vec. After 1 000 tab
            // switches the Vec grows to 1 000 entries; the shutdown join
            // loop then takes proportionally longer and the mutex is held
            // for a write on every spawn even for a read-heavy check.
            //
            // `is_finished()` is O(1) and does not block. We use
            // `retain` (in-place filter) so no reallocation is needed
            // when the Vec is already at capacity with live handles.
            handles.retain(|h| !h.is_finished());
            handles.push(handle);
        }
        Err(e) => {
            log::error!(
                "Failed to spawn worker thread '{}': {}. \
                 This worker's work will not be performed.",
                name,
                e
            );
        }
    }
}

// Global atomic playback states and engine handles
pub static IS_PLAYING: AtomicBool = AtomicBool::new(false);
pub static CURRENT_VOLUME: AtomicU32 = AtomicU32::new(75);
pub static CURRENT_INDEX: Mutex<usize> = Mutex::new(0);
pub static ELAPSED_SECONDS: Mutex<f64> = Mutex::new(0.0);
pub static SHUTDOWN: AtomicBool = AtomicBool::new(false);
pub static SHUFFLE_ENABLED: AtomicBool = AtomicBool::new(false);
pub static REPEAT_ENABLED: AtomicBool = AtomicBool::new(false);
pub static QUEUE_CLEARED_BY_USER: AtomicBool = AtomicBool::new(false);
pub static SHUFFLE_ORDER: Mutex<Vec<usize>> = parking_lot::const_mutex(Vec::new());
pub static SHUFFLE_POS: Mutex<usize> = parking_lot::const_mutex(0);
pub static USER_SELECT_GEN: AtomicU64 = AtomicU64::new(0);

/// Track ID for which a play event has already been recorded this session
/// (reset to 0 when a new track starts, set to track.id after >= 10s).
pub static LAST_RECORDED_TRACK_ID: AtomicI64 = AtomicI64::new(0);

/// ISO standard 10-band graphic EQ centre frequencies (Hz).
pub const EQ_BAND_FREQS: [f32; 10] =
    [31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

pub static GLOBAL_ENGINE: OnceLock<Mutex<AudioEngine>> = OnceLock::new();
pub static ENGINE_CMD_TX: OnceLock<crossbeam::channel::Sender<EngineCommand>> = OnceLock::new();
pub static PLAYBACK_INFO: OnceLock<Arc<arc_swap::ArcSwap<PlaybackInfo>>> = OnceLock::new();
pub static VISUALIZER_TAP: OnceLock<Arc<engine::analysis::FftVisualizerTap>> = OnceLock::new();
pub static GLOBAL_DB: OnceLock<Arc<PlayTuneDb>> = OnceLock::new();
pub static LIBRARY_MANAGER: OnceLock<Arc<LibraryManager>> = OnceLock::new();
pub static PLATFORM: OnceLock<Mutex<PlatformIntegration>> = OnceLock::new();
pub static CURRENT_TRACK_LIST: OnceLock<Mutex<Vec<DbTrack>>> = OnceLock::new();

pub static LOUDNESS_SCAN_CANCELLED: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// New feature flags & state (essential feature set)
// ---------------------------------------------------------------------------

/// When `Some(instant)`, the play-ticker thread will fade out and pause
/// playback when the deadline is reached. Set by `on_sleep_timer` and
/// cleared either by `on_sleep_timer(0)` or after firing.
pub static SLEEP_TIMER_DEADLINE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();

/// Total duration of the currently active sleep timer, used to compute the
/// remaining-seconds readout pushed to the GUI.
pub static SLEEP_TIMER_TOTAL_SECS: AtomicU32 = AtomicU32::new(0);

/// Whether desktop notifications should be shown on track change. Defaults
/// to true (matches AIMP/Foobar2000 behavior). Toggled via Settings.
pub static NOTIFICATIONS_ENABLED: AtomicBool = AtomicBool::new(true);

/// Whether the songs table should auto-scroll to keep the playing row
/// visible. Defaults to false (matches Foobar2000's opt-in behavior).
/// Toggled via Settings; the C++ side also reads a QSettings flag so the
/// UI works even before the bridge is wired.
pub static CURSOR_FOLLOWS_PLAYBACK: AtomicBool = AtomicBool::new(false);

/// Whether crossfade is enabled (the inverse of "gapless"). When false,
/// the TrackMixer jumps straight to the next track (true gapless). When
/// true, it fades between tracks over `CROSSFADE_DURATION_MS`.
pub static CROSSFADE_ENABLED: AtomicBool = AtomicBool::new(false);
pub static CROSSFADE_DURATION_MS: AtomicU32 = AtomicU32::new(3000);

/// Whether the system tray icon is visible.
pub static TRAY_ENABLED: AtomicBool = AtomicBool::new(false);

/// Whether closing the main window should hide to tray instead of quitting.
pub static MINIMIZE_TO_TRAY: AtomicBool = AtomicBool::new(false);

/// ID of the playlist currently shown in the songs table, or `None` if a
/// non-playlist filter (all / favorites / album / artist / etc.) is active.
/// Used by the "Export current view as M3U" feature.
pub static ACTIVE_PLAYLIST_ID: Mutex<Option<i64>> = parking_lot::const_mutex(None);

/// Helper: get-or-init the sleep timer deadline slot.
pub fn sleep_timer_deadline() -> &'static Mutex<Option<Instant>> {
    SLEEP_TIMER_DEADLINE.get_or_init(|| Mutex::new(None))
}

/// Start a sleep timer that will pause playback after `minutes` minutes.
/// A `minutes` value of 0 cancels any active timer.
pub fn set_sleep_timer(minutes: u32) {
    let mut guard = sleep_timer_deadline().lock();
    if minutes == 0 {
        *guard = None;
        SLEEP_TIMER_TOTAL_SECS.store(0, std::sync::atomic::Ordering::SeqCst);
        bridge::set_sleep_timer_remaining(0);
        log::info!("Sleep timer cancelled");
        return;
    }
    let total_secs = minutes * 60;
    *guard = Some(Instant::now() + Duration::from_secs(total_secs as u64));
    SLEEP_TIMER_TOTAL_SECS.store(total_secs, std::sync::atomic::Ordering::SeqCst);
    bridge::set_sleep_timer_remaining(total_secs as i32);
    log::info!("Sleep timer set for {} minutes", minutes);
}

/// Tick the sleep timer. Called from the main 30ms ticker. Returns true
/// if the timer just fired (so the caller can perform the fade-out + pause).
pub fn tick_sleep_timer() -> bool {
    let (fired, remaining_to_push) = {
        let mut guard = sleep_timer_deadline().lock();
        if let Some(deadline) = *guard {
            let now = Instant::now();
            if now >= deadline {
                *guard = None;
                SLEEP_TIMER_TOTAL_SECS.store(0, std::sync::atomic::Ordering::SeqCst);
                (true, Some(0i32))
            } else {
                let remaining = (deadline - now).as_secs() as i32;
                // Only push the update once per second to avoid flooding the GUI.
                static LAST_PUSHED_SECS: AtomicU32 = AtomicU32::new(0);
                let last = LAST_PUSHED_SECS.load(std::sync::atomic::Ordering::Relaxed);
                if last != remaining as u32 {
                    LAST_PUSHED_SECS.store(remaining as u32, std::sync::atomic::Ordering::Relaxed);
                    (false, Some(remaining))
                } else {
                    (false, None)
                }
            }
        } else {
            (false, None)
        }
    };
    // Now the lock is released — safe to make the FFI call.
    if let Some(remaining) = remaining_to_push {
        bridge::set_sleep_timer_remaining(remaining);
    }
    fired
}

/// Returns the remaining seconds of the active sleep timer, or 0 if none.
#[allow(dead_code)]
pub fn sleep_timer_remaining_secs() -> u32 {
    let guard = sleep_timer_deadline().lock();
    if let Some(deadline) = *guard {
        let now = Instant::now();
        if now >= deadline {
            return 0;
        }
        return (deadline - now).as_secs() as u32;
    }
    0
}

/// Send a desktop notification if the user has them enabled. The actual
/// dispatch is async (fire-and-forget on a worker thread).
pub fn notify_track_change(track: &DbTrack) {
    if !NOTIFICATIONS_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let title = track.title.clone();
    let body = if track.artist.is_empty() && track.album.is_empty() {
        String::new()
    } else if track.album.is_empty() {
        track.artist.to_string()
    } else {
        format!("{} — {}", track.artist, track.album)
    };
    // Prefer the native platform notification (notify-send / osascript / PowerShell toast).
    if let Some(platform_lock) = PLATFORM.get() {
        if let Some(platform) = platform_lock.try_lock() {
            platform.send_notification(&title, &body);
            return;
        }
    }
    // Fall back to the in-app toast via the bridge (Linux without notify-send, etc.)
    bridge::show_desktop_notification(&title, &body);
    // Also push to the system tray icon if it's visible.
    if TRAY_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
        bridge::show_tray_message(&title, &body);
    }
}

// Performance: in-memory cover-path cache + last-loaded filter tracking.
pub static COVER_PATH_CACHE: OnceLock<RwLock<std::collections::HashMap<String, Option<String>>>> =
    OnceLock::new();
pub static LAST_LOADED_FILTER: Mutex<String> = parking_lot::const_mutex(String::new());

/// Dirty flags for secondary views that are not covered by LAST_LOADED_FILTER.
/// Set to true when the library is modified, false after the view is populated.
/// This prevents re-populating albums/artists grids on every tab switch.
pub static ALBUMS_VIEW_DIRTY: AtomicBool = AtomicBool::new(true);
pub static ARTISTS_VIEW_DIRTY: AtomicBool = AtomicBool::new(true);
pub static FOLDERS_VIEW_DIRTY: AtomicBool = AtomicBool::new(true);

/// Initialize the cover-path cache lazily; safe to call from anywhere.
pub fn cover_cache() -> &'static RwLock<std::collections::HashMap<String, Option<String>>> {
    COVER_PATH_CACHE.get_or_init(|| RwLock::new(std::collections::HashMap::new()))
}

/// Look up a cover path in the in-memory cache; on miss, fall back to
/// `extract_cover_art_to_cache` and store the result.
pub fn cached_cover_path(track_path: &str) -> Option<String> {
    {
        let cache = cover_cache().read();
        if let Some(v) = cache.get(track_path) {
            return v.clone();
        }
    }
    // Miss: resolve and store.
    let resolved = engine::extract_cover_art_to_cache(std::path::Path::new(track_path));
    let mut cache = cover_cache().write();
    cache.insert(track_path.to_string(), resolved.clone());
    resolved
}

/// Invalidate a single entry (e.g. after the tag editor rewrites the file).
pub fn invalidate_cover_cache(track_path: &str) {
    if let Some(cache) = COVER_PATH_CACHE.get() {
        let mut w = cache.write();
        w.remove(track_path);
    }
}

/// Force the next `refresh_ui` call to actually re-query the DB and rebuild
/// the table, even if the same filter is currently loaded. Call this after
/// any modification to the library: import, delete, tag edit, favorite
/// toggle, loudness scan, etc.
pub fn invalidate_loaded_filter() {
    let mut last = LAST_LOADED_FILTER.lock();
    last.clear();
}

pub fn invalidate_shuffle_order() {
    let mut order = SHUFFLE_ORDER.lock();
    order.clear();
    let mut pos = SHUFFLE_POS.lock();
    *pos = 0;
}

pub fn sync_shuffle_order(current_idx: usize, list_len: usize) {
    if list_len == 0 {
        invalidate_shuffle_order();
        return;
    }
    let mut order = SHUFFLE_ORDER.lock();
    let mut pos = SHUFFLE_POS.lock();

    let cur_idx_clamped = current_idx % list_len;
    let is_valid = order.len() == list_len && order.iter().all(|&x| x < list_len);
    if !is_valid {
        let mut candidates: Vec<usize> = (0..list_len).filter(|&i| i != cur_idx_clamped).collect();
        let mut seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as usize)
            .unwrap_or(12345)
            .wrapping_add(cur_idx_clamped)
            .wrapping_mul(2654435761);

        for i in (1..candidates.len()).rev() {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let j = (seed >> 16) % (i + 1);
            candidates.swap(i, j);
        }
        candidates.insert(0, cur_idx_clamped);
        *order = candidates;
        *pos = 0;
    } else {
        if order.get(*pos) != Some(&cur_idx_clamped) {
            if let Some(found_p) = order.iter().position(|&x| x == cur_idx_clamped) {
                *pos = found_p;
            } else {
                order[*pos] = cur_idx_clamped;
            }
        }
    }
}

/// Mark ALL views as dirty so the next tab switch triggers a full
/// re-population. Call this after any library mutation: import, delete,
/// tag edit, etc.
pub fn invalidate_all_views() {
    invalidate_loaded_filter();
    invalidate_shuffle_order();
    ALBUMS_VIEW_DIRTY.store(true, std::sync::atomic::Ordering::SeqCst);
    ARTISTS_VIEW_DIRTY.store(true, std::sync::atomic::Ordering::SeqCst);
    FOLDERS_VIEW_DIRTY.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub fn apply_play_state(new_state: bool) {
    log::info!("Play state -> {}", new_state);
    bridge::set_play_state(new_state);
    if let Some(platform_lock) = PLATFORM.get() {
        if let Some(mut platform) = platform_lock.try_lock() {
            platform.set_mpris_status(if new_state {
                MprisPlaybackStatus::Playing
            } else {
                MprisPlaybackStatus::Paused
            });
        }
    }
    if let Some(tx) = ENGINE_CMD_TX.get() {
        let cmd = if new_state { EngineCommand::Play } else { EngineCommand::Pause };
        let _ = tx.send(cmd);
    }
}

pub fn send_track_info_and_lyrics(track: &DbTrack, cover_path: &str) {
    bridge::set_track_info(&track.title, &track.artist, &track.album, cover_path);
    bridge::update_track_lyrics(
        track.id as i32,
        track.lyrics_synced.as_deref(),
        track.lyrics_unsynced.as_deref(),
    );
    // Fire a desktop notification (if enabled) and tray message (if visible).
    notify_track_change(track);
}
