#![allow(clippy::too_many_arguments)]
use std::ffi::{c_char, c_double, c_float, c_int, CString};

/// Helper: convert a &str to CString, logging a warning if the string
/// contains an interior NUL byte.
fn cstring_or_warn(s: &str, context: &str) -> CString {
    match CString::new(s) {
        Ok(c) => c,
        Err(_) => {
            log::warn!("{} contains a NUL byte; replacing with empty string", context);
            CString::default()
        }
    }
}

#[repr(C)]
pub struct Callbacks {
    pub on_play_pause: Option<extern "C" fn()>,
    pub on_prev: Option<extern "C" fn()>,
    pub on_next: Option<extern "C" fn()>,
    pub on_seek: Option<extern "C" fn(seconds: c_double)>,
    pub on_volume: Option<extern "C" fn(volume: c_double)>,
    pub on_eq_band: Option<extern "C" fn(band_idx: c_int, gain_db: c_double)>,
    pub on_eq_enabled: Option<extern "C" fn(enabled: c_int)>,
    pub on_select_song: Option<extern "C" fn(song_idx: c_int)>,
    pub on_preset_selected: Option<extern "C" fn(preset_idx: c_int)>,
    pub on_reset_eq: Option<extern "C" fn()>,
    pub on_slider_param: Option<extern "C" fn(param_idx: c_int, value: c_double)>,
    pub on_clear_queue: Option<extern "C" fn()>,
    pub on_import_files: Option<extern "C" fn(paths: *const *const c_char, count: c_int)>,
    pub on_import_folder: Option<extern "C" fn(folder_path: *const c_char)>,
    pub on_delete_folder: Option<extern "C" fn(folder_id: c_int)>,
    pub on_toggle_favorite: Option<extern "C" fn(song_id: c_int)>,
    pub on_nav_tab: Option<extern "C" fn(tab_id: c_int)>,
    pub on_filter_folder: Option<extern "C" fn(folder_id: c_int)>,
    pub on_search: Option<extern "C" fn(query: *const c_char)>,
    pub on_eq_advanced_band: Option<
        extern "C" fn(
            band_idx: c_int,
            freq: c_double,
            gain_db: c_double,
            q: c_double,
            filter_type: c_int,
        ),
    >,
    pub on_set_resampler_quality: Option<extern "C" fn(quality: c_int)>,
    pub on_set_output_backend: Option<extern "C" fn(backend: c_int)>,
    pub on_set_output_device: Option<extern "C" fn(device_name: *const c_char)>,
    pub on_gui_ready: Option<extern "C" fn()>,
    // ===== New callbacks for the essential feature set =====
    /// Create a new playlist. The callback returns the new playlist id
    /// (or -1 on failure).
    pub on_create_playlist: Option<extern "C" fn(name: *const c_char) -> c_int>,
    /// Rename an existing playlist. Returns 1 on success, 0 if not found.
    pub on_rename_playlist:
        Option<extern "C" fn(playlist_id: c_int, new_name: *const c_char) -> c_int>,
    /// Delete a playlist by id.
    pub on_delete_playlist: Option<extern "C" fn(playlist_id: c_int)>,
    /// Add a track to a playlist. Returns the new total track count.
    pub on_add_track_to_playlist:
        Option<extern "C" fn(playlist_id: c_int, track_id: c_int) -> c_int>,
    /// Remove a track from a playlist.
    pub on_remove_track_from_playlist: Option<extern "C" fn(playlist_id: c_int, track_id: c_int)>,
    /// Show the tracks of a playlist (replaces the songs table content).
    pub on_filter_playlist: Option<extern "C" fn(playlist_id: c_int)>,
    /// Filter the songs table by album (album_id is the stable track id).
    pub on_filter_album: Option<extern "C" fn(album_id: c_int)>,
    /// Filter the songs table by artist (artist_id is the stable track id).
    pub on_filter_artist: Option<extern "C" fn(artist_id: c_int)>,
    /// Set the user rating of a track (0-5 stars).
    pub on_set_rating: Option<extern "C" fn(track_id: c_int, rating: c_int)>,
    /// Toggle the gapless-playback / crossfade mode.
    /// `enabled`=1 means crossfade is enabled (gapless is the opposite).
    pub on_crossfade_toggled: Option<extern "C" fn(enabled: c_int)>,
    /// Set the crossfade duration in milliseconds (only meaningful when
    /// crossfade is enabled).
    pub on_set_crossfade_duration: Option<extern "C" fn(duration_ms: c_int)>,
    /// Toggle desktop notifications on track change.
    pub on_notifications_toggled: Option<extern "C" fn(enabled: c_int)>,
    /// Toggle the "cursor follows playback" feature.
    pub on_cursor_follows_playback: Option<extern "C" fn(enabled: c_int)>,
    /// Set the playback speed (0.25..=4.0). 1.0 = normal speed.
    pub on_set_speed: Option<extern "C" fn(speed: c_double)>,
    /// Start a sleep timer that will pause playback after `minutes`.
    /// Pass 0 to cancel any active sleep timer.
    pub on_sleep_timer: Option<extern "C" fn(minutes: c_int)>,
    /// Import an M3U/M3U8 playlist file from disk. The imported tracks
    /// will be added to a new (or existing) playlist named `playlist_name`.
    /// If `playlist_name` is null, the file basename is used.
    pub on_import_m3u:
        Option<extern "C" fn(path: *const c_char, playlist_name: *const c_char) -> c_int>,
    /// Export the given playlist's tracks to an M3U8 file at `path`.
    /// Returns 1 on success, 0 on failure.
    pub on_export_m3u: Option<extern "C" fn(playlist_id: c_int, path: *const c_char) -> c_int>,
    /// Toggle the system tray icon visibility.
    pub on_tray_toggled: Option<extern "C" fn(enabled: c_int)>,
    /// Minimize the main window to the system tray instead of quitting
    /// when the close button is clicked.
    pub on_minimize_to_tray_toggled: Option<extern "C" fn(enabled: c_int)>,
    pub on_reorder_queue: Option<extern "C" fn(from_idx: c_int, to_idx: c_int)>,
    pub on_remove_from_library: Option<extern "C" fn(track_id: c_int)>,
}

#[repr(C)]
pub struct FfiTagEditRequest {
    pub track_id: c_int,
    pub title: *const c_char,
    pub artist: *const c_char,
    pub album: *const c_char,
    pub album_artist: *const c_char,
    pub genre: *const c_char,
    pub year: u32,
    pub track_number: u32,
    pub disc_number: u32,
    pub cover_image_path: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FfiLoudnessWriteItem {
    pub track_id: c_int,
    pub lufs: c_float,
    pub peak: c_float,
    pub rg_gain_db: c_float,
    pub r128_gain_db: c_float,
}

/// C-ABI song row used by `set_songs_batch`. Must match the C++ side
/// `SongRowFfi` struct in `gui_bridge.cpp`:
///
/// ```c
/// struct SongRowFfi {
///     int display_index;
///     int song_id;
///     int is_favorite;
///     int _pad;            // explicit 4-byte padding (kept for clarity)
///     const char* title;
///     const char* artist;
///     const char* album;
///     const char* duration;
///     const char* cover_path;
/// };
/// ```
///
/// The struct is laid out identically on x86-64 and aarch64 (LP64). The
/// explicit `_pad` field makes the 8-byte alignment of the first pointer
/// (title) visible; the C++ side has a matching `int _pad` field plus a
/// `static_assert` to catch layout drift.
#[repr(C)]
pub struct FfiSongRow {
    pub display_index: c_int,
    pub song_id: c_int,
    pub is_favorite: c_int,
    pub _pad: c_int,
    pub title: *const c_char,
    pub artist: *const c_char,
    pub album: *const c_char,
    pub duration: *const c_char,
    pub cover_path: *const c_char,
}

// Sanity check: the struct must be exactly 5 pointers + 4 ints.
const _: () = assert!(
    std::mem::size_of::<FfiSongRow>()
        == 5 * std::mem::size_of::<*const c_char>() + 4 * std::mem::size_of::<c_int>(),
    "FfiSongRow layout mismatch — check C++ SongRowFfi"
);

mod ffi {
    use super::*;
    extern "C" {
        pub fn run_qt_app(argc: c_int, argv: *mut *mut c_char, callbacks: Callbacks) -> c_int;
        pub fn set_play_state(playing: c_int);
        pub fn set_playback_progress(elapsed: c_double, total: c_double);
        pub fn update_track_metadata(
            track_id: c_int,
            title: *const c_char,
            artist: *const c_char,
            album: *const c_char,
            duration: *const c_char,
            cover_path: *const c_char,
        );
        pub fn update_track_lyrics(
            track_id: c_int,
            synced_lrc: *const c_char,
            unsynced_lyrics: *const c_char,
        );
        pub fn set_track_info(
            title: *const c_char,
            artist: *const c_char,
            album: *const c_char,
            cover_path: *const c_char,
        );
        pub fn set_active_index(index: c_int);
        #[allow(dead_code)]
        pub fn clear_songs();
        #[allow(dead_code)]
        pub fn add_song(
            index: c_int,
            song_id: c_int,
            is_favorite: c_int,
            title: *const c_char,
            artist: *const c_char,
            album: *const c_char,
            duration: *const c_char,
            cover_path: *const c_char,
        );
        /// Batch replace the entire songs table.
        ///
        /// `rows` points to `count` contiguous `FfiSongRow` entries.
        /// See `set_songs_batch()` below.
        pub fn set_songs_batch(rows: *const FfiSongRow, count: c_int);
        pub fn clear_folders();
        pub fn add_folder(id: c_int, path: *const c_char, name: *const c_char, track_count: c_int);
        pub fn switch_view(view_index: c_int);
        pub fn clear_queue();
        pub fn add_queue_song(
            index: c_int,
            title: *const c_char,
            artist: *const c_char,
            duration: *const c_char,
            cover_path: *const c_char,
        );
        pub fn update_visualizer(data: *const c_float, size: c_int);
        pub fn request_quit();
        pub fn install_shutdown_hook(hook: extern "C" fn());
        pub fn clear_audio_devices();
        pub fn add_audio_device(name: *const c_char, is_current: c_int);
        pub fn loudness_scan_progress(current: c_int, total: c_int, current_file: *const c_char);
        pub fn loudness_scan_track_result(
            track_id: c_int,
            lufs: c_float,
            peak: c_float,
            rg_gain_db: c_float,
            r128_gain_db: c_float,
        );
        pub fn loudness_scan_finished(success: c_int, error_msg: *const c_char);
        // New ABI for the essential feature set
        pub fn clear_playlists();
        pub fn add_playlist(
            playlist_id: c_int,
            name: *const c_char,
            track_count: c_int,
            duration_secs: c_double,
        );
        pub fn clear_albums();
        pub fn add_album(
            album_id: c_int,
            name: *const c_char,
            artist: *const c_char,
            track_count: c_int,
            duration_secs: c_double,
            year: c_int,
            cover_path: *const c_char,
        );
        pub fn clear_artists();
        pub fn add_artist(
            artist_id: c_int,
            name: *const c_char,
            album_count: c_int,
            track_count: c_int,
            cover_path: *const c_char,
        );
        pub fn clear_albums_in_artist();
        pub fn add_album_to_artist(
            album_id: c_int,
            name: *const c_char,
            artist: *const c_char,
            track_count: c_int,
            duration_secs: c_double,
        );
        pub fn set_speed_label(speed: c_double);
        pub fn set_sleep_timer_remaining(seconds_remaining: c_int);
        pub fn show_tray_message(title: *const c_char, body: *const c_char);
        pub fn scroll_songs_table_to_active();
        pub fn show_desktop_notification(title: *const c_char, body: *const c_char);
        pub fn set_rating_for_row(track_id: c_int, rating: c_int);
    }
}

/// Start the Qt application. Blocks until GUI window closes.
pub fn start_gui(args: Vec<String>, callbacks: Callbacks) -> i32 {
    // Allocate each argument as an owned *mut c_char on the heap. Qt's
    // QApplication may reorder/remove Qt-specific arguments from the
    // pointer array, but it does not write through the string pointers,
    // so heap-ownership stays with us and we reclaim it after exec returns.
    let mut c_args: Vec<*mut c_char> = args
        .into_iter()
        .map(|arg| {
            let cstring = cstring_or_warn(&arg, "arg");
            CString::into_raw(cstring)
        })
        .collect();

    let exit_code =
        unsafe { ffi::run_qt_app(c_args.len() as c_int, c_args.as_mut_ptr(), callbacks) as i32 };

    // Reclaim the heap-allocated CStrings to avoid leaking argv on every
    // start_gui call (which is once per process, but cleanliness matters).
    for ptr in c_args {
        // CString::from_raw would panic on a null pointer; guard with a check.
        if !ptr.is_null() {
            // Safety: ptr was allocated by CString::into_raw above and has
            // not been freed yet. Qt does not free argv strings.
            let _ = unsafe { CString::from_raw(ptr) };
        }
    }

    exit_code
}

pub fn set_play_state(playing: bool) {
    unsafe {
        ffi::set_play_state(if playing { 1 } else { 0 });
    }
}

pub fn set_playback_progress(elapsed: f64, total: f64) {
    unsafe {
        ffi::set_playback_progress(elapsed, total);
    }
}

pub fn update_track_metadata(
    track_id: i32,
    title: &str,
    artist: &str,
    album: &str,
    duration: &str,
    cover_path: &str,
) {
    let c_title = cstring_or_warn(title, "title");
    let c_artist = cstring_or_warn(artist, "artist");
    let c_album = cstring_or_warn(album, "album");
    let c_dur = cstring_or_warn(duration, "duration");
    let c_cover = cstring_or_warn(cover_path, "cover_path");

    unsafe {
        ffi::update_track_metadata(
            track_id,
            c_title.as_ptr(),
            c_artist.as_ptr(),
            c_album.as_ptr(),
            c_dur.as_ptr(),
            c_cover.as_ptr(),
        );
    }
}

pub fn update_track_lyrics(track_id: i32, synced: Option<&str>, unsynced: Option<&str>) {
    let c_synced = synced.map(|s| cstring_or_warn(s, "update_track_lyrics synced"));
    let c_unsynced = unsynced.map(|s| cstring_or_warn(s, "update_track_lyrics unsynced"));
    unsafe {
        ffi::update_track_lyrics(
            track_id,
            c_synced.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            c_unsynced.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
        );
    }
}

pub fn set_track_info(title: &str, artist: &str, album: &str, cover_path: &str) {
    let c_title = cstring_or_warn(title, "title");
    let c_artist = cstring_or_warn(artist, "artist");
    let c_album = cstring_or_warn(album, "album");
    let c_cover = cstring_or_warn(cover_path, "cover_path");

    unsafe {
        ffi::set_track_info(
            c_title.as_ptr(),
            c_artist.as_ptr(),
            c_album.as_ptr(),
            c_cover.as_ptr(),
        );
    }
}

pub fn set_active_index(index: i32) {
    unsafe {
        ffi::set_active_index(index);
    }
}

#[allow(dead_code)]
pub fn clear_songs() {
    unsafe {
        ffi::clear_songs();
    }
}

#[allow(dead_code)]
pub fn add_song(
    index: i32,
    song_id: i32,
    is_favorite: bool,
    title: &str,
    artist: &str,
    album: &str,
    duration: &str,
    cover_path: &str,
) {
    let c_title = cstring_or_warn(title, "title");
    let c_artist = cstring_or_warn(artist, "artist");
    let c_album = cstring_or_warn(album, "album");
    let c_duration = cstring_or_warn(duration, "duration");
    let c_cover = cstring_or_warn(cover_path, "cover_path");

    unsafe {
        ffi::add_song(
            index,
            song_id,
            if is_favorite { 1 } else { 0 },
            c_title.as_ptr(),
            c_artist.as_ptr(),
            c_album.as_ptr(),
            c_duration.as_ptr(),
            c_cover.as_ptr(),
        );
    }
}

/// Batch-replace the songs table. Each `SongRowArg` is converted to an
/// `FfiSongRow` (with the strings backed by owned `CString`s whose
/// lifetimes are tied to the local `cstrings` vector) and passed across
/// the FFI in a single call.
///
/// Performance: this is the bulk-load fast path. The previous code
/// called `add_song` once per track, which on the C++ side meant:
///   * one cross-thread Qt signal emission per track (each ~30 µs),
///   * one `clearSongs` + `insertRow` + `setCellWidget` per track,
///   * one synchronous cover load + decode per track on the GUI thread.
///
/// For 10 000 tracks that summed to ~5 s of UI freeze + ~676 MB of
/// pixmap memory. With `set_songs_batch`, the entire payload crosses
/// the FFI once, the C++ side does a single transactional rebuild
/// (signals blocked, updates disabled), and covers are loaded lazily
/// by the CoverLoader when each row scrolls into view.
///
pub fn set_songs_batch(rows: &[SongRowArg]) {
    if rows.is_empty() {
        unsafe { ffi::set_songs_batch(std::ptr::null(), 0) };
        return;
    }
    // Build the CString backing storage first so the pointers stay
    // valid for the duration of the FFI call. We keep them in a Vec to
    // prevent them from being dropped early.
    let mut cstrings: Vec<CString> = Vec::with_capacity(rows.len() * 5);
    let mut ffi_rows: Vec<FfiSongRow> = Vec::with_capacity(rows.len());

    for r in rows {
        let c_title = cstring_or_warn(&r.title, "title");
        let c_artist = cstring_or_warn(&r.artist, "artist");
        let c_album = cstring_or_warn(&r.album, "album");
        let c_duration = cstring_or_warn(&r.duration, "duration");
        let c_cover = cstring_or_warn(&r.cover_path, "cover_path");

        let title_ptr = c_title.as_ptr();
        let artist_ptr = c_artist.as_ptr();
        let album_ptr = c_album.as_ptr();
        let duration_ptr = c_duration.as_ptr();
        let cover_ptr = c_cover.as_ptr();

        // Push the CStrings BEFORE pushing the FfiSongRow that holds
        // their pointers, so the order of drop (reverse of push) at end
        // of scope drops the FfiSongRow first, then the CStrings.
        cstrings.push(c_title);
        cstrings.push(c_artist);
        cstrings.push(c_album);
        cstrings.push(c_duration);
        cstrings.push(c_cover);

        ffi_rows.push(FfiSongRow {
            display_index: r.display_index,
            song_id: r.song_id,
            is_favorite: if r.is_favorite { 1 } else { 0 },
            _pad: 0,
            title: title_ptr,
            artist: artist_ptr,
            album: album_ptr,
            duration: duration_ptr,
            cover_path: cover_ptr,
        });
    }

    unsafe {
        ffi::set_songs_batch(ffi_rows.as_ptr(), ffi_rows.len() as c_int);
    }
}

/// Argument bundle for `set_songs_batch`. Owned by the caller; the
/// `&str` fields are copied into CStrings inside the FFI wrapper.
#[derive(Debug, Clone, Default)]
pub struct SongRowArg {
    pub display_index: i32,
    pub song_id: i32,
    pub is_favorite: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: String,
    pub cover_path: String,
}

pub fn clear_folders() {
    unsafe {
        ffi::clear_folders();
    }
}

pub fn add_folder(id: i32, path: &str, name: &str, track_count: i32) {
    let c_path = cstring_or_warn(path, "path");
    let c_name = cstring_or_warn(name, "name");
    unsafe {
        ffi::add_folder(id, c_path.as_ptr(), c_name.as_ptr(), track_count);
    }
}

pub fn switch_view(view_index: i32) {
    unsafe {
        ffi::switch_view(view_index);
    }
}

pub fn clear_queue() {
    unsafe {
        ffi::clear_queue();
    }
}

pub fn add_queue_song(index: i32, title: &str, artist: &str, duration: &str, cover_path: &str) {
    let c_title = cstring_or_warn(title, "title");
    let c_artist = cstring_or_warn(artist, "artist");
    let c_duration = cstring_or_warn(duration, "duration");
    let c_cover = cstring_or_warn(cover_path, "cover_path");

    unsafe {
        ffi::add_queue_song(
            index,
            c_title.as_ptr(),
            c_artist.as_ptr(),
            c_duration.as_ptr(),
            c_cover.as_ptr(),
        );
    }
}

pub fn update_visualizer(data: &[f32]) {
    if data.len() > (c_int::MAX as usize) {
        log::warn!("update_visualizer: data.len() {} exceeds c_int::MAX; truncating", data.len());
        let truncated = &data[..c_int::MAX as usize];
        unsafe {
            ffi::update_visualizer(truncated.as_ptr(), c_int::MAX);
        }
        return;
    }
    unsafe {
        ffi::update_visualizer(data.as_ptr(), data.len() as c_int);
    }
}

/// Request that the Qt application quit gracefully by emitting a signal
/// that calls QApplication::quit via a QueuedConnection. Safe to call
/// from any thread.
pub fn request_quit() {
    unsafe {
        ffi::request_quit();
    }
}

pub fn install_shutdown_hook(hook: extern "C" fn()) {
    unsafe {
        ffi::install_shutdown_hook(hook);
    }
}

pub fn clear_audio_devices() {
    unsafe {
        ffi::clear_audio_devices();
    }
}

pub fn add_audio_device(name: &str, is_current: bool) {
    let c_name = cstring_or_warn(name, "device_name");
    unsafe {
        ffi::add_audio_device(c_name.as_ptr(), if is_current { 1 } else { 0 });
    }
}

pub fn loudness_scan_progress(current: i32, total: i32, current_file: &str) {
    let c_file = cstring_or_warn(current_file, "current_file");
    unsafe {
        ffi::loudness_scan_progress(current, total, c_file.as_ptr());
    }
}

pub fn loudness_scan_track_result(
    track_id: i32,
    lufs: f32,
    peak: f32,
    rg_gain_db: f32,
    r128_gain_db: f32,
) {
    unsafe {
        ffi::loudness_scan_track_result(track_id, lufs, peak, rg_gain_db, r128_gain_db);
    }
}

pub fn loudness_scan_finished(success: bool, error_msg: &str) {
    let c_msg = cstring_or_warn(error_msg, "error_msg");
    unsafe {
        ffi::loudness_scan_finished(if success { 1 } else { 0 }, c_msg.as_ptr());
    }
}

// ========================================================================
// New safe wrappers for the essential feature set
// ========================================================================

pub fn clear_playlists() {
    unsafe { ffi::clear_playlists() }
}

pub fn add_playlist(playlist_id: i32, name: &str, track_count: i32, duration_secs: f64) {
    let c_name = cstring_or_warn(name, "playlist name");
    unsafe { ffi::add_playlist(playlist_id, c_name.as_ptr(), track_count, duration_secs) }
}

pub fn clear_albums() {
    unsafe { ffi::clear_albums() }
}

pub fn add_album(
    album_id: i32,
    name: &str,
    artist: &str,
    track_count: i32,
    duration_secs: f64,
    year: i32,
    cover_path: &str,
) {
    let c_name = cstring_or_warn(name, "album name");
    let c_artist = cstring_or_warn(artist, "album artist");
    let c_cover = cstring_or_warn(cover_path, "cover path");
    unsafe {
        ffi::add_album(
            album_id,
            c_name.as_ptr(),
            c_artist.as_ptr(),
            track_count,
            duration_secs,
            year,
            c_cover.as_ptr(),
        )
    }
}

pub fn clear_artists() {
    unsafe { ffi::clear_artists() }
}

pub fn add_artist(
    artist_id: i32,
    name: &str,
    album_count: i32,
    track_count: i32,
    cover_path: &str,
) {
    let c_name = cstring_or_warn(name, "artist name");
    let c_cover = cstring_or_warn(cover_path, "cover path");
    unsafe {
        ffi::add_artist(artist_id, c_name.as_ptr(), album_count, track_count, c_cover.as_ptr())
    }
}

pub fn clear_albums_in_artist() {
    unsafe { ffi::clear_albums_in_artist() }
}

pub fn add_album_to_artist(
    album_id: i32,
    name: &str,
    artist: &str,
    track_count: i32,
    duration_secs: f64,
) {
    let c_name = cstring_or_warn(name, "album name");
    let c_artist = cstring_or_warn(artist, "artist");
    unsafe {
        ffi::add_album_to_artist(
            album_id,
            c_name.as_ptr(),
            c_artist.as_ptr(),
            track_count,
            duration_secs,
        )
    }
}

pub fn set_speed_label(speed: f64) {
    unsafe { ffi::set_speed_label(speed) }
}

pub fn set_sleep_timer_remaining(seconds_remaining: i32) {
    unsafe { ffi::set_sleep_timer_remaining(seconds_remaining) }
}

pub fn show_tray_message(title: &str, body: &str) {
    let c_title = cstring_or_warn(title, "tray title");
    let c_body = cstring_or_warn(body, "tray body");
    unsafe { ffi::show_tray_message(c_title.as_ptr(), c_body.as_ptr()) }
}

pub fn scroll_songs_table_to_active() {
    unsafe { ffi::scroll_songs_table_to_active() }
}

pub fn show_desktop_notification(title: &str, body: &str) {
    let c_title = cstring_or_warn(title, "notification title");
    let c_body = cstring_or_warn(body, "notification body");
    unsafe { ffi::show_desktop_notification(c_title.as_ptr(), c_body.as_ptr()) }
}

pub fn set_rating_for_row(track_id: i32, rating: i32) {
    unsafe { ffi::set_rating_for_row(track_id, rating) }
}
