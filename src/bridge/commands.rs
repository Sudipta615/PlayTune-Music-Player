/// Safe Rust wrappers around every raw FFI call declared in `ffi.rs`.
/// All functions here are `pub` and form the public API of the bridge module.
use std::ffi::c_int;

use super::ffi::raw;
use super::types::{Callbacks, FfiSongRow, SongRowArg};
use crate::bridge::types::cstring_or_warn;

/// Start the Qt application. Blocks until GUI window closes.
pub fn start_gui(args: Vec<String>, callbacks: Callbacks) -> i32 {
    // Allocate each argument as an owned *mut c_char on the heap.
    let mut c_args: Vec<*mut std::ffi::c_char> = args
        .into_iter()
        .map(|arg| {
            let cstring = cstring_or_warn(&arg, "arg");
            std::ffi::CString::into_raw(cstring)
        })
        .collect();

    let exit_code =
        unsafe { raw::run_qt_app(c_args.len() as c_int, c_args.as_mut_ptr(), callbacks) as i32 };

    for ptr in c_args {
        if !ptr.is_null() {
            let _ = unsafe { std::ffi::CString::from_raw(ptr) };
        }
    }

    exit_code
}

pub fn set_play_state(playing: bool) {
    unsafe { raw::set_play_state(if playing { 1 } else { 0 }) }
}

pub fn set_playback_progress(elapsed: f64, total: f64) {
    unsafe { raw::set_playback_progress(elapsed, total) }
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
        raw::update_track_metadata(
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
        raw::update_track_lyrics(
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
        raw::set_track_info(
            c_title.as_ptr(),
            c_artist.as_ptr(),
            c_album.as_ptr(),
            c_cover.as_ptr(),
        );
    }
}

pub fn set_active_index(index: i32) {
    unsafe { raw::set_active_index(index) }
}

#[allow(dead_code)]
pub fn clear_songs() {
    unsafe { raw::clear_songs() }
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
        raw::add_song(
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
pub fn set_songs_batch(rows: &[SongRowArg]) {
    if rows.is_empty() {
        unsafe { raw::set_songs_batch(std::ptr::null(), 0) };
        return;
    }
    let mut cstrings: Vec<std::ffi::CString> = Vec::with_capacity(rows.len() * 6);
    let mut ffi_rows: Vec<FfiSongRow> = Vec::with_capacity(rows.len());

    for r in rows {
        let c_title = cstring_or_warn(&r.title, "title");
        let c_artist = cstring_or_warn(&r.artist, "artist");
        let c_album = cstring_or_warn(&r.album, "album");
        let c_duration = cstring_or_warn(&r.duration, "duration");
        let c_cover = cstring_or_warn(&r.cover_path, "cover_path");
        let c_mood = cstring_or_warn(&r.mood, "mood");

        let title_ptr = c_title.as_ptr();
        let artist_ptr = c_artist.as_ptr();
        let album_ptr = c_album.as_ptr();
        let duration_ptr = c_duration.as_ptr();
        let cover_ptr = c_cover.as_ptr();
        let mood_ptr = c_mood.as_ptr();

        cstrings.push(c_title);
        cstrings.push(c_artist);
        cstrings.push(c_album);
        cstrings.push(c_duration);
        cstrings.push(c_cover);
        cstrings.push(c_mood);

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
            mood: mood_ptr,
        });
    }

    unsafe { raw::set_songs_batch(ffi_rows.as_ptr(), ffi_rows.len() as c_int) }
}

pub fn clear_folders() {
    unsafe { raw::clear_folders() }
}

pub fn add_folder(id: i32, path: &str, name: &str, track_count: i32) {
    let c_path = cstring_or_warn(path, "path");
    let c_name = cstring_or_warn(name, "name");
    unsafe { raw::add_folder(id, c_path.as_ptr(), c_name.as_ptr(), track_count) }
}

pub fn switch_view(view_index: i32) {
    unsafe { raw::switch_view(view_index) }
}

pub fn clear_queue() {
    unsafe { raw::clear_queue() }
}

pub fn add_queue_song(index: i32, title: &str, artist: &str, duration: &str, cover_path: &str) {
    let c_title = cstring_or_warn(title, "title");
    let c_artist = cstring_or_warn(artist, "artist");
    let c_duration = cstring_or_warn(duration, "duration");
    let c_cover = cstring_or_warn(cover_path, "cover_path");

    unsafe {
        raw::add_queue_song(
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
        unsafe { raw::update_visualizer(truncated.as_ptr(), c_int::MAX) }
        return;
    }
    unsafe { raw::update_visualizer(data.as_ptr(), data.len() as c_int) }
}

/// Request that the Qt application quit gracefully. Safe to call from any thread.
pub fn request_quit() {
    unsafe { raw::request_quit() }
}

pub fn install_shutdown_hook(hook: extern "C" fn()) {
    unsafe { raw::install_shutdown_hook(hook) }
}

pub fn clear_audio_devices() {
    unsafe { raw::clear_audio_devices() }
}

pub fn add_audio_device(name: &str, is_current: bool) {
    let c_name = cstring_or_warn(name, "device_name");
    unsafe { raw::add_audio_device(c_name.as_ptr(), if is_current { 1 } else { 0 }) }
}

pub fn loudness_scan_progress(current: i32, total: i32, current_file: &str) {
    let c_file = cstring_or_warn(current_file, "current_file");
    unsafe { raw::loudness_scan_progress(current, total, c_file.as_ptr()) }
}

pub fn loudness_scan_track_result(
    track_id: i32,
    lufs: f32,
    peak: f32,
    rg_gain_db: f32,
    r128_gain_db: f32,
) {
    unsafe { raw::loudness_scan_track_result(track_id, lufs, peak, rg_gain_db, r128_gain_db) }
}

pub fn loudness_scan_finished(success: bool, error_msg: &str) {
    let c_msg = cstring_or_warn(error_msg, "error_msg");
    unsafe { raw::loudness_scan_finished(if success { 1 } else { 0 }, c_msg.as_ptr()) }
}

pub fn clear_playlists() {
    unsafe { raw::clear_playlists() }
}

pub fn add_playlist(playlist_id: i32, name: &str, track_count: i32, duration_secs: f64) {
    let c_name = cstring_or_warn(name, "playlist name");
    unsafe { raw::add_playlist(playlist_id, c_name.as_ptr(), track_count, duration_secs) }
}

pub fn clear_albums() {
    unsafe { raw::clear_albums() }
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
        raw::add_album(
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
    unsafe { raw::clear_artists() }
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
        raw::add_artist(artist_id, c_name.as_ptr(), album_count, track_count, c_cover.as_ptr())
    }
}

pub fn clear_albums_in_artist() {
    unsafe { raw::clear_albums_in_artist() }
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
        raw::add_album_to_artist(
            album_id,
            c_name.as_ptr(),
            c_artist.as_ptr(),
            track_count,
            duration_secs,
        )
    }
}

pub fn set_speed_label(speed: f64) {
    unsafe { raw::set_speed_label(speed) }
}

pub fn set_sleep_timer_remaining(seconds_remaining: i32) {
    unsafe { raw::set_sleep_timer_remaining(seconds_remaining) }
}

pub fn show_tray_message(title: &str, body: &str) {
    let c_title = cstring_or_warn(title, "tray title");
    let c_body = cstring_or_warn(body, "tray body");
    unsafe { raw::show_tray_message(c_title.as_ptr(), c_body.as_ptr()) }
}

pub fn scroll_songs_table_to_active() {
    unsafe { raw::scroll_songs_table_to_active() }
}

pub fn show_desktop_notification(title: &str, body: &str) {
    let c_title = cstring_or_warn(title, "notification title");
    let c_body = cstring_or_warn(body, "notification body");
    unsafe { raw::show_desktop_notification(c_title.as_ptr(), c_body.as_ptr()) }
}

pub fn set_rating_for_row(track_id: i32, rating: i32) {
    unsafe { raw::set_rating_for_row(track_id, rating) }
}
