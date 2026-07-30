use crate::bridge::Callbacks;

pub mod eq;
pub mod library;
pub mod nav;
pub mod playback;

pub fn create_callbacks() -> Callbacks {
    Callbacks {
        on_play_pause: Some(playback::rust_play_pause),
        on_prev: Some(playback::rust_prev),
        on_next: Some(playback::rust_next),
        on_seek: Some(playback::rust_seek),
        on_volume: Some(playback::rust_volume),
        on_eq_band: Some(eq::rust_eq_band),
        on_eq_enabled: Some(eq::rust_eq_enabled),
        on_select_song: Some(playback::rust_select_song),
        on_preset_selected: Some(eq::rust_preset_selected),
        on_reset_eq: Some(eq::rust_reset_eq),
        on_slider_param: Some(eq::rust_slider_param),
        on_clear_queue: Some(library::rust_clear_queue),
        on_import_files: Some(library::rust_import_files),
        on_import_folder: Some(library::rust_import_folder),
        on_delete_folder: Some(library::rust_delete_folder),
        on_toggle_favorite: Some(library::rust_toggle_favorite),
        on_nav_tab: Some(nav::rust_nav_tab),
        on_filter_folder: Some(library::rust_filter_folder),
        on_search: Some(library::rust_search),
        on_eq_advanced_band: Some(eq::rust_eq_advanced_band),
        on_set_resampler_quality: Some(eq::rust_set_resampler_quality),
        on_set_output_backend: Some(eq::rust_set_output_backend),
        on_set_output_device: Some(eq::rust_set_output_device),
        on_gui_ready: Some(nav::rust_gui_ready),
        // New callbacks for the essential feature set
        on_create_playlist: Some(library::rust_create_playlist),
        on_rename_playlist: Some(library::rust_rename_playlist),
        on_delete_playlist: Some(library::rust_delete_playlist),
        on_add_track_to_playlist: Some(library::rust_add_track_to_playlist),
        on_remove_track_from_playlist: Some(library::rust_remove_track_from_playlist),
        on_filter_playlist: Some(library::rust_filter_playlist),
        on_filter_album: Some(library::rust_filter_album),
        on_filter_artist: Some(library::rust_filter_artist),
        on_set_rating: Some(library::rust_set_rating),
        on_crossfade_toggled: Some(library::rust_crossfade_toggled),
        on_set_crossfade_duration: Some(library::rust_set_crossfade_duration),
        on_notifications_toggled: Some(library::rust_notifications_toggled),
        on_cursor_follows_playback: Some(library::rust_cursor_follows_playback),
        on_set_speed: Some(library::rust_set_speed),
        on_sleep_timer: Some(library::rust_sleep_timer),
        on_import_m3u: Some(library::playtune_import_m3u),
        on_export_m3u: Some(library::playtune_export_m3u),
        on_tray_toggled: Some(library::rust_tray_toggled),
        on_minimize_to_tray_toggled: Some(library::rust_minimize_to_tray_toggled),
        on_reorder_queue: Some(library::rust_reorder_queue),
        on_remove_from_library: Some(library::rust_remove_from_library),
    }
}
