#include "gui_bridge.h"
#include <iostream>
#include <QTimer>
#include <QVector>
#include <cmath>
#include <QApplication>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

// Global mock state
static bool demo_playing = true;
static int demo_active_idx = 0;
static double demo_elapsed = 84.0; // starts at 1:24
static double demo_total = 238.0;   // 3:58

// Mock song details
const char* titles[] = {"Midnight Dreams", "Echoes", "Starlight", "Endless Road", "Breathe Again", "Sailing Home", "Letting Go", "Golden Hours", "Night Drive"};
const char* artists[] = {"Horizon Lines", "Aurora Skies", "Nova Bloom", "The Wanderers", "Luna Track", "Ocean Avenue", "Paper Kites", "Sunset Kids", "Coastal Run"};
const char* albums[] = {"Lost in Reverie", "Waves of Time", "Starlight EP", "Paths", "Moments", "Tides", "On the Corner", "Chapter One", "Roadside"};
const double durations_secs[] = {238.0, 261.0, 227.0, 302.0, 250.0, 275.0, 213.0, 258.0, 239.0};
const char* durations[] = {"3:58", "4:21", "3:47", "5:02", "4:10", "4:35", "3:33", "4:18", "3:59"};

static void update_track_ui() {
    std::cout << "[C++ Demo] Track Changed: " << titles[demo_active_idx] << std::endl;
    demo_total = durations_secs[demo_active_idx];
    
    // Push track info to UI
    set_track_info(titles[demo_active_idx], artists[demo_active_idx], albums[demo_active_idx], "");
    set_active_index(demo_active_idx);
    set_playback_progress(demo_elapsed, demo_total);
}

// Dummy console callbacks for C++ testing
extern "C" {
    void dummy_play_pause() {
        demo_playing = !demo_playing;
        std::cout << "[C++ Callback] Play/Pause clicked. Playing: " << (demo_playing ? "YES" : "NO") << std::endl;
        set_play_state(demo_playing ? 1 : 0);
    }
    
    void dummy_prev() {
        demo_active_idx = (demo_active_idx - 1 + 9) % 9;
        demo_elapsed = 0.0;
        update_track_ui();
        set_play_state(demo_playing ? 1 : 0);
    }
    
    void dummy_next() {
        demo_active_idx = (demo_active_idx + 1) % 9;
        demo_elapsed = 0.0;
        update_track_ui();
        set_play_state(demo_playing ? 1 : 0);
    }
    
    void dummy_seek(double sec) {
        demo_elapsed = sec;
        std::cout << "[C++ Callback] Seek to: " << sec << " seconds" << std::endl;
        set_playback_progress(demo_elapsed, demo_total);
    }
    
    void dummy_volume(double vol) {
        std::cout << "[C++ Callback] Volume set to: " << (vol * 100.0) << "%" << std::endl;
    }
    
    void dummy_eq_band(int band, double gain) {
        std::cout << "[C++ Callback] EQ Band " << band << " adjusted to: " << gain << " dB" << std::endl;
    }
    
    void dummy_eq_enabled(int enabled) {
        std::cout << "[C++ Callback] EQ Enabled state: " << enabled << std::endl;
    }
    
    void dummy_select_song(int idx) {
        if (idx >= 0 && idx < 9) {
            demo_active_idx = idx;
            demo_elapsed = 0.0;
            update_track_ui();
            demo_playing = true;
            set_play_state(1);
        }
    }
    
    void dummy_preset(int preset) {
        std::cout << "[C++ Callback] EQ Preset selected: " << preset << std::endl;
    }
    
    void dummy_reset() {
        std::cout << "[C++ Callback] EQ Reset clicked" << std::endl;
    }
    
    void dummy_param(int param, double val) {
        std::cout << "[C++ Callback] Param " << param << " set to: " << val << std::endl;
    }
    
    void dummy_clear() {
        std::cout << "[C++ Callback] Clear Queue clicked" << std::endl;
        clear_queue();
    }
    
    int playtune_update_track_tags(const FfiTagEditRequest* req) {
        std::cout << "[C++ Demo] playtune_update_track_tags called for track ID: " << (req ? req->track_id : -1) << std::endl;
        if (req && req->title) {
            std::cout << "  Title: " << req->title << "\n";
        }
        return 1;
    }

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
    ) {
        std::cout << "[C++ Demo] playtune_get_track_tags called for track ID: " << track_id << std::endl;
        if (title_buf && title_len > 0 && track_id >= 0 && track_id < 9) {
            std::string t = titles[track_id];
            std::copy(t.begin(), t.end(), title_buf);
            title_buf[std::min((int)t.size(), title_len - 1)] = 0;
        }
        return 1;
    }

    void playtune_cancel_loudness_scan(void) {
        std::cout << "[C++ Demo] playtune_cancel_loudness_scan called\n";
    }

    void playtune_start_loudness_scan(const int* track_ids, int count) {
        std::cout << "[C++ Demo] playtune_start_loudness_scan called for " << count << " tracks\n";
        QTimer::singleShot(200, []() {
            loudness_scan_progress(1, 3, "Echoes");
            loudness_scan_track_result(1, -17.8f, 0.94f, -0.2f, -5.2f);
        });
        QTimer::singleShot(400, []() {
            loudness_scan_progress(2, 3, "Starlight");
            loudness_scan_track_result(2, -14.5f, 0.99f, -3.5f, -8.5f);
        });
        QTimer::singleShot(600, []() {
            loudness_scan_progress(3, 3, "Endless Road");
            loudness_scan_track_result(3, -20.2f, 0.85f, 2.2f, -2.8f);
            loudness_scan_finished(1, "");
        });
    }

    int playtune_write_loudness_results(const FfiLoudnessWriteItem* items, int count) {
        std::cout << "[C++ Demo] playtune_write_loudness_results called for " << count << " items\n";
        return 1;
    }
}

int main(int argc, char** argv) {
    QApplication app(argc, argv);
    std::cout << "[C++ Demo] Starting PlayTune Standalone GUI..." << std::endl;

    Callbacks cb = {
        dummy_play_pause,
        dummy_prev,
        dummy_next,
        dummy_seek,
        dummy_volume,
        dummy_eq_band,
        dummy_eq_enabled,
        dummy_select_song,
        dummy_preset,
        dummy_reset,
        dummy_param,
        dummy_clear,
        [](const char* const*, int) { std::cout << "[Demo] on_import_files\n"; },
        [](const char*) { std::cout << "[Demo] on_import_folder\n"; },
        [](int id) { std::cout << "[Demo] on_delete_folder " << id << "\n"; },
        [](int id) { std::cout << "[Demo] on_toggle_favorite " << id << "\n"; },
        [](int id) { std::cout << "[Demo] on_nav_tab " << id << "\n"; },
        [](int id) { std::cout << "[Demo] on_filter_folder " << id << "\n"; },
        [](const char* q) { std::cout << "[Demo] on_search: " << (q ? q : "") << "\n"; },
        [](int band, double freq, double gain, double q, int ftype) {
            std::cout << "[Demo] on_eq_advanced_band " << band << " freq=" << freq << " gain=" << gain << " q=" << q << " ftype=" << ftype << "\n";
        },
        [](int quality) { std::cout << "[Demo] on_set_resampler_quality " << quality << "\n"; }
    };

    std::cout << "[C++ Demo] GUI initialized. Controls active." << std::endl;

    // Setup active playback ticker timer (100ms ticks)
    QTimer ticker;
    float visualizer_phase = 0.0f;
    QVector<float> spectrum(65, 0.1f);

    QObject::connect(&ticker, &QTimer::timeout, [&visualizer_phase, &spectrum]() {
        if (demo_playing) {
            demo_elapsed += 0.1;
            
            // Advance to next song when finished
            if (demo_elapsed >= demo_total) {
                demo_elapsed = 0.0;
                demo_active_idx = (demo_active_idx + 1) % 9;
                update_track_ui();
            } else {
                set_playback_progress(demo_elapsed, demo_total);
            }

            // Sync visualizer data wiggles
            visualizer_phase += 0.25f;
            for (int i = 0; i < 65; ++i) {
                float frac = (float)i / 65.0f;
                float val = 0.15f 
                    + 0.40f * std::sin(frac * M_PI)
                    + 0.25f * std::sin(frac * 4.0f * M_PI + visualizer_phase) * std::cos(frac * 2.5f * M_PI)
                    + 0.10f * std::cos(frac * 12.0f * M_PI + visualizer_phase * 2.0f);
                spectrum[i] = std::abs(val) > 0.95f ? 0.95f : (std::abs(val) < 0.1f ? 0.1f : std::abs(val));
            }
            update_visualizer(spectrum.data(), spectrum.size());
        }
    });
    ticker.start(100);

    return run_qt_app(argc, argv, cb);
}
