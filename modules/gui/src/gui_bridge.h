#ifndef GUI_BRIDGE_H
#define GUI_BRIDGE_H

#ifdef __cplusplus
extern "C" {
#endif

// Struct containing function pointers for actions initiated in the UI
typedef struct {
    void (*on_play_pause)(void);
    void (*on_prev)(void);
    void (*on_next)(void);
    void (*on_seek)(double seconds);
    void (*on_volume)(double volume);
    void (*on_eq_band)(int band_idx, double gain_db);
    void (*on_eq_enabled)(int enabled); // 0 or 1
    void (*on_select_song)(int song_idx);
    void (*on_preset_selected)(int preset_idx);
    void (*on_reset_eq)(void);
    void (*on_slider_param)(int param_idx, double value); // 0: Bass, 1: Treble, 2: Stereo Width, 3: Balance, 4: Preamp
    void (*on_clear_queue)(void);
    void (*on_import_files)(const char* const* paths, int count);
    void (*on_import_folder)(const char* folder_path);
    void (*on_delete_folder)(int folder_id);
    void (*on_toggle_favorite)(int song_id);
    void (*on_nav_tab)(int tab_id);
    void (*on_filter_folder)(int folder_id);
    void (*on_search)(const char* query);
    void (*on_eq_advanced_band)(int band_idx, double freq, double gain_db, double q, int filter_type); // filter_type: 0 LowShelf, 1 Peaking, 2 HighShelf
    void (*on_set_resampler_quality)(int quality); // 0 Low, 1 Medium, 2 High, 3 Ultra
    void (*on_set_output_backend)(int backend);
    void (*on_set_output_device)(const char* device_name);
    void (*on_gui_ready)(void);
    // ===== New callbacks for the essential feature set =====
    int (*on_create_playlist)(const char* name);
    int (*on_rename_playlist)(int playlist_id, const char* new_name);
    void (*on_delete_playlist)(int playlist_id);
    int (*on_add_track_to_playlist)(int playlist_id, int track_id);
    void (*on_remove_track_from_playlist)(int playlist_id, int track_id);
    void (*on_filter_playlist)(int playlist_id);
    void (*on_filter_album)(int album_id);
    void (*on_filter_artist)(int artist_id);
    void (*on_set_rating)(int track_id, int rating);
    void (*on_crossfade_toggled)(int enabled);
    void (*on_set_crossfade_duration)(int duration_ms);
    void (*on_notifications_toggled)(int enabled);
    void (*on_cursor_follows_playback)(int enabled);
    void (*on_set_speed)(double speed);
    void (*on_sleep_timer)(int minutes);
    int (*on_import_m3u)(const char* path, const char* playlist_name);
    int (*on_export_m3u)(int playlist_id, const char* path);
    void (*on_tray_toggled)(int enabled);
    void (*on_minimize_to_tray_toggled)(int enabled);
    void (*on_reorder_queue)(int from_idx, int to_idx);
    void (*on_remove_from_library)(int track_id);
} Callbacks;

typedef struct {
    int track_id;
    const char* title;
    const char* artist;
    const char* album;
    const char* album_artist;
    const char* genre;
    unsigned int year;
    unsigned int track_number;
    unsigned int disc_number;
    const char* cover_image_path;
} FfiTagEditRequest;

int playtune_update_track_tags(const FfiTagEditRequest* req);
int playtune_get_track_tags(
    int track_id,
    char* title_buf, int title_len,
    char* artist_buf, int artist_len,
    char* album_buf, int album_len,
    char* album_artist_buf, int album_artist_len,
    char* genre_buf, int genre_len,
    unsigned int* year,
    unsigned int* track_num,
    unsigned int* disc_num,
    char* cover_buf, int cover_len
);
void playtune_get_track_lyrics(int track_id);

// Starts the Qt application event loop
int run_qt_app(int argc, char** argv, Callbacks callbacks);

// Updates the playing state in the UI (1 for playing, 0 for paused)
void set_play_state(int playing);

// Updates the current playback position (elapsed & total time in seconds)
void set_playback_progress(double elapsed, double total);

// Updates metadata of a specific track row across all views after tag editing
void update_track_metadata(int track_id, const char* title, const char* artist, const char* album, const char* duration_str, const char* cover_path);

// Updates lyrics of a track row across all views / active playback
void update_track_lyrics(int track_id, const char* synced_lrc, const char* unsynced_lyrics);

// Updates metadata of the currently playing track
void set_track_info(const char* title, const char* artist, const char* album, const char* cover_path);

// Sets the active song row index in the table
void set_active_index(int index);

// Clears all entries in the main songs table
void clear_songs(void);

// Appends a track to the main songs table
void add_song(int index, int song_id, int is_favorite, const char* title, const char* artist, const char* album, const char* duration, const char* cover_path);

// ── Batch API (preferred for large libraries) ─────────────────────────
// Replaces the entire songs table in one transactional rebuild. Each
// entry in `rows` is laid out as 7 contiguous C strings + 3 ints:
//   struct SongRowFfi {
//     int display_index;
//     int song_id;
//     int is_favorite;     // 0 or 1
//     const char* title;
//     const char* artist;
//     const char* album;
//     const char* duration;
//     const char* cover_path;
//   };
// `count` is the number of entries. `rows` must point to `count` entries.
//
// This is the FFI equivalent of SongsTableWidget::setSongsBatch(): it
// converts N individual add_song FFI round-trips (each ~0.5 ms on the
// GUI thread) into a single ~10 ms rebuild for 1 000 tracks. For a
// 10 000-track library this cuts the refresh_ui time from ~5 s of UI
// freeze to a single ~100 ms transaction.
void set_songs_batch(const void* rows, int count);

// Clears all entries in the folders view
void clear_folders(void);

// Appends a folder to the folders view
void add_folder(int id, const char* path, const char* name, int track_count);

// Switches the active content view (0: Songs Table, 1: Settings Page, 2: Folders View)
void switch_view(int view_index);

// Clears all entries in the queue
void clear_queue(void);

// Appends a track to the right-side queue
void add_queue_song(int index, const char* title, const char* artist, const char* duration, const char* cover_path);

// Feeds real-time frequency band values to the waveform visualizer
void update_visualizer(const float* data, int size);

// Clears all audio device choices in settings UI
void clear_audio_devices(void);

// Appends an audio device choice to settings UI
void add_audio_device(const char* name, int is_current);

// Requests that the Qt application quit gracefully (thread-safe).
// Causes QApplication::quit to be invoked on the GUI thread via a
// QueuedConnection so all Drop impls and closeEvent handlers run.
void request_quit(void);

//
// CONTRACT: `hook` MUST be a plain C function pointer (extern "C") that is
// safe to call from the GUI thread. It must not block, allocate, or call
// back into Qt — it should just set a flag.
typedef struct {
    int track_id;
    float lufs;
    float peak;
    float rg_gain_db;
    float r128_gain_db;
} FfiLoudnessWriteItem;

void playtune_cancel_loudness_scan(void);
void playtune_start_loudness_scan(const int* track_ids, int count);
int playtune_write_loudness_results(const FfiLoudnessWriteItem* items, int count);

void loudness_scan_progress(int current, int total, const char* current_file);
void loudness_scan_track_result(int track_id, float lufs, float peak, float rg_gain_db, float r128_gain_db);
void loudness_scan_finished(int success, const char* error_msg);

// ===== New C ABI for the essential feature set =====
void clear_playlists(void);
void add_playlist(int playlist_id, const char* name, int track_count, double duration_secs);
void clear_albums(void);
void add_album(int album_id, const char* name, const char* artist, int track_count, double duration_secs, int year, const char* cover_path);
void clear_artists(void);
void add_artist(int artist_id, const char* name, int album_count, int track_count, const char* cover_path);
void clear_albums_in_artist(void);
void add_album_to_artist(int album_id, const char* name, const char* artist, int track_count, double duration_secs);
void set_speed_label(double speed);
void set_sleep_timer_remaining(int seconds_remaining);
void show_tray_message(const char* title, const char* body);
void scroll_songs_table_to_active(void);
void show_desktop_notification(const char* title, const char* body);
void set_rating_for_row(int track_id, int rating);

typedef void (*shutdown_hook_fn)(void);
void install_shutdown_hook(shutdown_hook_fn hook);

#ifdef __cplusplus
}
#endif

#endif // GUI_BRIDGE_H
