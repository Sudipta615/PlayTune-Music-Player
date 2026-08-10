use std::ffi::c_char;

/// Helper: convert a &str to CString, logging a warning if the string
/// contains an interior NUL byte.
pub(super) fn cstring_or_warn(s: &str, context: &str) -> std::ffi::CString {
    match std::ffi::CString::new(s) {
        Ok(c) => c,
        Err(_) => {
            log::warn!("{} contains a NUL byte; replacing with empty string", context);
            std::ffi::CString::default()
        }
    }
}

/// C-ABI callbacks struct — one field per user-initiated action that the
/// Qt/C++ side can invoke into the Rust backend.
#[repr(C)]
pub struct Callbacks {
    pub on_play_pause: Option<extern "C" fn()>,
    pub on_prev: Option<extern "C" fn()>,
    pub on_next: Option<extern "C" fn()>,
    pub on_seek: Option<extern "C" fn(seconds: std::ffi::c_double)>,
    pub on_volume: Option<extern "C" fn(volume: std::ffi::c_double)>,
    pub on_eq_band: Option<extern "C" fn(band_idx: std::ffi::c_int, gain_db: std::ffi::c_double)>,
    pub on_eq_enabled: Option<extern "C" fn(enabled: std::ffi::c_int)>,
    pub on_select_song: Option<extern "C" fn(song_idx: std::ffi::c_int)>,
    pub on_preset_selected: Option<extern "C" fn(preset_idx: std::ffi::c_int)>,
    pub on_reset_eq: Option<extern "C" fn()>,
    pub on_slider_param:
        Option<extern "C" fn(param_idx: std::ffi::c_int, value: std::ffi::c_double)>,
    pub on_clear_queue: Option<extern "C" fn()>,
    pub on_import_files: Option<extern "C" fn(paths: *const *const c_char, count: std::ffi::c_int)>,
    pub on_import_folder: Option<extern "C" fn(folder_path: *const c_char)>,
    pub on_delete_folder: Option<extern "C" fn(folder_id: std::ffi::c_int)>,
    pub on_toggle_favorite: Option<extern "C" fn(song_id: std::ffi::c_int)>,
    pub on_nav_tab: Option<extern "C" fn(tab_id: std::ffi::c_int)>,
    pub on_filter_folder: Option<extern "C" fn(folder_id: std::ffi::c_int)>,
    pub on_search: Option<extern "C" fn(query: *const c_char)>,
    pub on_eq_advanced_band: Option<
        extern "C" fn(
            band_idx: std::ffi::c_int,
            freq: std::ffi::c_double,
            gain_db: std::ffi::c_double,
            q: std::ffi::c_double,
            filter_type: std::ffi::c_int,
        ),
    >,
    pub on_set_resampler_quality: Option<extern "C" fn(quality: std::ffi::c_int)>,
    pub on_set_output_backend: Option<extern "C" fn(backend: std::ffi::c_int)>,
    pub on_set_output_device: Option<extern "C" fn(device_name: *const c_char)>,
    pub on_gui_ready: Option<extern "C" fn()>,
    // ===== New callbacks for the essential feature set =====
    /// Create a new playlist. The callback returns the new playlist id
    /// (or -1 on failure).
    pub on_create_playlist: Option<extern "C" fn(name: *const c_char) -> std::ffi::c_int>,
    /// Rename an existing playlist. Returns 1 on success, 0 if not found.
    pub on_rename_playlist: Option<
        extern "C" fn(playlist_id: std::ffi::c_int, new_name: *const c_char) -> std::ffi::c_int,
    >,
    /// Delete a playlist by id.
    pub on_delete_playlist: Option<extern "C" fn(playlist_id: std::ffi::c_int)>,
    /// Add a track to a playlist. Returns the new total track count.
    pub on_add_track_to_playlist: Option<
        extern "C" fn(playlist_id: std::ffi::c_int, track_id: std::ffi::c_int) -> std::ffi::c_int,
    >,
    /// Remove a track from a playlist.
    pub on_remove_track_from_playlist:
        Option<extern "C" fn(playlist_id: std::ffi::c_int, track_id: std::ffi::c_int)>,
    /// Show the tracks of a playlist (replaces the songs table content).
    pub on_filter_playlist: Option<extern "C" fn(playlist_id: std::ffi::c_int)>,
    /// Filter the songs table by album (album_id is the stable track id).
    pub on_filter_album: Option<extern "C" fn(album_id: std::ffi::c_int)>,
    /// Filter the songs table by artist (artist_id is the stable track id).
    pub on_filter_artist: Option<extern "C" fn(artist_id: std::ffi::c_int)>,
    /// Set the user rating of a track (0-5 stars).
    pub on_set_rating: Option<extern "C" fn(track_id: std::ffi::c_int, rating: std::ffi::c_int)>,
    /// Toggle the gapless-playback / crossfade mode.
    /// `enabled`=1 means crossfade is enabled (gapless is the opposite).
    pub on_crossfade_toggled: Option<extern "C" fn(enabled: std::ffi::c_int)>,
    /// Set the crossfade duration in milliseconds (only meaningful when
    /// crossfade is enabled).
    pub on_set_crossfade_duration: Option<extern "C" fn(duration_ms: std::ffi::c_int)>,
    /// Toggle desktop notifications on track change.
    pub on_notifications_toggled: Option<extern "C" fn(enabled: std::ffi::c_int)>,
    /// Toggle the "cursor follows playback" feature.
    pub on_cursor_follows_playback: Option<extern "C" fn(enabled: std::ffi::c_int)>,
    /// Set the playback speed (0.25..=4.0). 1.0 = normal speed.
    pub on_set_speed: Option<extern "C" fn(speed: std::ffi::c_double)>,
    /// Start a sleep timer that will pause playback after `minutes`.
    /// Pass 0 to cancel any active sleep timer.
    pub on_sleep_timer: Option<extern "C" fn(minutes: std::ffi::c_int)>,
    /// Import an M3U/M3U8 playlist file from disk. The imported tracks
    /// will be added to a new (or existing) playlist named `playlist_name`.
    /// If `playlist_name` is null, the file basename is used.
    pub on_import_m3u:
        Option<extern "C" fn(path: *const c_char, playlist_name: *const c_char) -> std::ffi::c_int>,
    /// Export the given playlist's tracks to an M3U8 file at `path`.
    /// Returns 1 on success, 0 on failure.
    pub on_export_m3u:
        Option<extern "C" fn(playlist_id: std::ffi::c_int, path: *const c_char) -> std::ffi::c_int>,
    /// Toggle the system tray icon visibility.
    pub on_tray_toggled: Option<extern "C" fn(enabled: std::ffi::c_int)>,
    /// Minimize the main window to the system tray instead of quitting
    /// when the close button is clicked.
    pub on_minimize_to_tray_toggled: Option<extern "C" fn(enabled: std::ffi::c_int)>,
    pub on_reorder_queue: Option<extern "C" fn(from_idx: std::ffi::c_int, to_idx: std::ffi::c_int)>,
    pub on_remove_from_library: Option<extern "C" fn(track_id: std::ffi::c_int)>,
}

#[repr(C)]
pub struct FfiTagEditRequest {
    pub track_id: std::ffi::c_int,
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
    pub track_id: std::ffi::c_int,
    pub lufs: std::ffi::c_float,
    pub peak: std::ffi::c_float,
    pub rg_gain_db: std::ffi::c_float,
    pub r128_gain_db: std::ffi::c_float,
}

/// C-ABI song row used by `set_songs_batch`. Must match the C++ side
/// `SongRowFfi` struct in `gui_bridge.cpp`.
#[repr(C)]
pub struct FfiSongRow {
    pub display_index: std::ffi::c_int,
    pub song_id: std::ffi::c_int,
    pub is_favorite: std::ffi::c_int,
    pub _pad: std::ffi::c_int,
    pub title: *const c_char,
    pub artist: *const c_char,
    pub album: *const c_char,
    pub duration: *const c_char,
    pub cover_path: *const c_char,
    pub mood: *const c_char,
}

// Sanity check: the struct must be exactly 6 pointers + 4 ints.
const _: () = assert!(
    std::mem::size_of::<FfiSongRow>()
        == 6 * std::mem::size_of::<*const c_char>() + 4 * std::mem::size_of::<std::ffi::c_int>(),
    "FfiSongRow layout mismatch — check C++ SongRowFfi"
);

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
    pub mood: String,
}
