#![allow(clippy::too_many_arguments)]
use std::ffi::{c_char, c_double, c_float, c_int};

use super::types::{Callbacks, FfiSongRow};

/// Raw `extern "C"` declarations matching the symbols exported by the
/// C++ Qt library that Rust links against. All calls go through the safe
/// wrapper functions in `commands.rs`.
pub(super) mod raw {
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
