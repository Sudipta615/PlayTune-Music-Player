#include "gui_bridge.h"
#include "gui_bridge_p.h"
#include "mainwindow.h"
#include <QApplication>
#include <QCoreApplication>
#include <QIcon>
#include <QVector>
#include <cstring>
#include <atomic>

#include <QPixmap>
#include <QDir>
#include <QStandardPaths>
#include <QFile>

#include "appsettings.h"

// C-ABI struct used by set_songs_batch(). Must match the Rust-side

// definition in src/bridge.rs (FfiSongRow). The struct is packed the
// same way on x86-64 / aarch64 (8-byte aligned pointers, 4-byte ints
// with 4-byte padding before each pointer that follows an int).
struct SongRowFfi {
    int display_index;
    int song_id;
    int is_favorite;
    int _pad;  // explicit padding so the next field is 8-byte aligned
    const char* title;
    const char* artist;
    const char* album;
    const char* duration;
    const char* cover_path;
};
static_assert(sizeof(SongRowFfi) == 5 * sizeof(void*) + 4 * sizeof(int),
              "SongRowFfi layout mismatch — check Rust side");

#if defined(_WIN32) || defined(WIN32)
#include <windows.h>
#include <shlobj.h>
#endif

namespace {
    std::atomic<shutdown_hook_fn> g_shutdown_hook{nullptr};
    std::atomic<bool> g_gui_initialized{false};

    void qt_message_filter(QtMsgType type, const QMessageLogContext& context, const QString& msg) {
        if (msg.contains("fromIccProfile") || msg.contains("iCCP") || msg.contains("known incorrect sRGB profile") || msg.contains("OpenType support missing")) {
            return;
        }
        if (type == QtWarningMsg || type == QtCriticalMsg || type == QtFatalMsg) {
            QByteArray localMsg = msg.toLocal8Bit();
            fprintf(stderr, "%s\n", localMsg.constData());
        }
    }

    QIcon createMultiSizeAppIcon() {
        QIcon icon;
        // Load the source pixmap from resources.
        QPixmap base(":/resources/icons/playtune_logo.png");
        if (base.isNull()) {
            // Fallback: try loading directly as icon.
            return QIcon(":/resources/icons/playtune_logo.png");
        }
        // Add the native-size pixmap first (usually 512x512).
        icon.addPixmap(base);
        // Also add common taskbar sizes so the WM never needs to scale.
        static const int sizes[] = {16, 22, 24, 32, 48, 64, 128, 256};
        for (int s : sizes) {
            if (s != base.width()) {
                icon.addPixmap(base.scaled(s, s, Qt::KeepAspectRatio, Qt::SmoothTransformation));
            }
        }
        return icon;
    }

#if defined(__linux__) || defined(__unix__) || defined(__FreeBSD__)
    void ensure_linux_desktop_icon() {
        QString iconDir = QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation) + "/icons/hicolor/256x256/apps";
        QDir().mkpath(iconDir);
        QString iconPath = iconDir + "/playtune.png";
        if (!QFile::exists(iconPath)) {
            QPixmap pm(":/resources/icons/playtune_logo.png");
            if (!pm.isNull()) {
                pm.scaled(256, 256, Qt::KeepAspectRatio, Qt::SmoothTransformation).save(iconPath, "PNG");
            }
        }

        QString appDir = QStandardPaths::writableLocation(QStandardPaths::ApplicationsLocation);
        QDir().mkpath(appDir);
        QString desktopPath = appDir + "/playtune.desktop";
        if (!QFile::exists(desktopPath)) {
            QFile f(desktopPath);
            if (f.open(QIODevice::WriteOnly | QIODevice::Text)) {
                QString content =
                    "[Desktop Entry]\n"
                    "Name=PlayTune\n"
                    "Comment=Audiophile Fidelity Music Player\n"
                    "Exec=" + QCoreApplication::applicationFilePath() + "\n"
                    "Icon=playtune\n"
                    "Terminal=false\n"
                    "Type=Application\n"
                    "Categories=AudioVideo;Audio;Player;Qt;\n"
                    "StartupWMClass=playtune\n";
                f.write(content.toUtf8());
                f.close();
            }
        }
    }
#endif
}

void install_shutdown_hook(shutdown_hook_fn hook) {
    g_shutdown_hook.store(hook, std::memory_order_release);
}

int run_qt_app(int argc, char** argv, Callbacks callbacks) {
#if defined(_WIN32) || defined(WIN32)
    SetCurrentProcessExplicitAppUserModelID(L"PlayTune.AudioPlayer.1");
#endif

    qInstallMessageHandler(qt_message_filter);

    // Ensure GUI resources are initialized
    // (This is standard when linking Qt code as a static library)
    Q_INIT_RESOURCE(resources);

    QCoreApplication* existing_core = QCoreApplication::instance();
    if (existing_core && qobject_cast<QApplication*>(existing_core) == nullptr) {
        // A non-QApplication QCoreApplication already exists. Constructing
        // a QApplication would assert. Log and return a non-zero exit code
        // so the Rust host can surface a diagnostic.
        qCritical("run_qt_app: a non-QApplication QCoreApplication already exists; "
                  "cannot construct QApplication. Aborting GUI initialization.");
        return 1;
    }

    QApplication* app = qobject_cast<QApplication*>(existing_core);
    bool created = false;
    if (!app) {
        app = new QApplication(argc, argv);
        created = true;
    }
    app->setOrganizationName("PlayTune");
    app->setOrganizationDomain("playtune.audio");
    app->setApplicationName("PlayTune");
    app->setApplicationDisplayName("PlayTune");
    app->setDesktopFileName(QStringLiteral("playtune.desktop"));

#if defined(__linux__) || defined(__unix__) || defined(__FreeBSD__)
    ensure_linux_desktop_icon();
#endif

    QIcon appIcon = createMultiSizeAppIcon();
    app->setWindowIcon(appIcon);

    qRegisterMetaType<QVector<float>>("QVector<float>");
    // Register QVector<SongRow> so the songsBatchReplaced signal can be
    // delivered across threads (Qt::QueuedConnection). See songstable.h.
    qRegisterMetaType<QVector<SongRow>>("QVector<SongRow>");

    QObject::connect(app, &QCoreApplication::aboutToQuit, app, []() {
        shutdown_hook_fn hook = g_shutdown_hook.load(std::memory_order_acquire);
        if (hook) {
            hook();
        }
    });

    // Eagerly initialize the singleton on the main GUI thread to ensure correct thread ownership/affinity
    GuiBridgeManager::instance();
    g_gui_initialized.store(true, std::memory_order_release);

    // Save the callbacks in the thread-safe bridge manager and ensure GUI thread affinity
    GuiBridgeManager::instance().moveToThread(app->thread());
    GuiBridgeManager::instance().setCallbacks(callbacks);

    int ret;
    {
        MainWindow w;
        w.show();
        // Start the event loop
        ret = app->exec();
    } // w and all child widgets/timers/animations destroyed HERE,
      // while app is still alive.

    g_gui_initialized.store(false, std::memory_order_release);

    if (created) {
        delete app;
    }
    return ret;
}

void set_play_state(int playing) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().playStateChanged(playing != 0);
}

void set_playback_progress(double elapsed, double total) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().progressChanged(elapsed, total);
}

void update_track_metadata(int track_id, const char* title, const char* artist, const char* album, const char* duration_str, const char* cover_path) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().trackMetadataUpdated(
        track_id,
        QString::fromUtf8(title ? title : ""),
        QString::fromUtf8(artist ? artist : ""),
        QString::fromUtf8(album ? album : ""),
        QString::fromUtf8(duration_str ? duration_str : ""),
        QString::fromUtf8(cover_path ? cover_path : "")
    );
}

void update_track_lyrics(int track_id, const char* synced_lrc, const char* unsynced_lyrics) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().trackLyricsUpdated(
        track_id,
        QString::fromUtf8(synced_lrc ? synced_lrc : ""),
        QString::fromUtf8(unsynced_lyrics ? unsynced_lyrics : "")
    );
}

void set_track_info(const char* title, const char* artist, const char* album, const char* cover_path) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().trackChanged(
        QString::fromUtf8(title ? title : ""),
        QString::fromUtf8(artist ? artist : ""),
        QString::fromUtf8(album ? album : ""),
        QString::fromUtf8(cover_path ? cover_path : "")
    );
}

void set_active_index(int index) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().activeIndexChanged(index);
}

void clear_songs(void) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().songsCleared();
}

void add_song(int index, int song_id, int is_favorite, const char* title, const char* artist, const char* album, const char* duration, const char* cover_path) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().songAdded(
        index,
        song_id,
        is_favorite != 0,
        QString::fromUtf8(title ? title : ""),
        QString::fromUtf8(artist ? artist : ""),
        QString::fromUtf8(album ? album : ""),
        QString::fromUtf8(duration ? duration : ""),
        QString::fromUtf8(cover_path ? cover_path : "")
    );
}

void set_songs_batch(const void* rows, int count) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    if (!rows || count <= 0) {
        // Empty batch is equivalent to clear.
        emit GuiBridgeManager::instance().songsCleared();
        return;
    }
    const SongRowFfi* ffi_rows = static_cast<const SongRowFfi*>(rows);
    QVector<SongRow> out;
    out.reserve(count);
    for (int i = 0; i < count; ++i) {
        const SongRowFfi& r = ffi_rows[i];
        SongRow row;
        row.displayIndex = r.display_index;
        row.songId = r.song_id;
        row.isFavorite = (r.is_favorite != 0);
        row.title   = QString::fromUtf8(r.title   ? r.title   : "");
        row.artist  = QString::fromUtf8(r.artist  ? r.artist  : "");
        row.album   = QString::fromUtf8(r.album   ? r.album   : "");
        row.duration= QString::fromUtf8(r.duration? r.duration: "");
        row.coverPath = QString::fromUtf8(r.cover_path ? r.cover_path : "");
        out.append(std::move(row));
    }
    emit GuiBridgeManager::instance().songsBatchReplaced(std::move(out));
}

void clear_queue(void) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().queueCleared();
}

void add_queue_song(int index, const char* title, const char* artist, const char* duration, const char* cover_path) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().queueSongAdded(
        index,
        QString::fromUtf8(title ? title : ""),
        QString::fromUtf8(artist ? artist : ""),
        QString::fromUtf8(duration ? duration : ""),
        QString::fromUtf8(cover_path ? cover_path : "")
    );
}

void clear_folders(void) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().foldersCleared();
}

void add_folder(int id, const char* path, const char* name, int track_count) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().folderAdded(
        id,
        QString::fromUtf8(path ? path : ""),
        QString::fromUtf8(name ? name : ""),
        track_count
    );
}

void switch_view(int view_index) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().viewSwitched(view_index);
}

void update_visualizer(const float* data, int size) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    if (AppSettings::instance().isOptimizedMode()) return; // 0 CPU cost when optimized
    if (!data || size <= 0) return;

    static thread_local QVector<float> tlsBuf;
    tlsBuf.resize(size);
    std::memcpy(tlsBuf.data(), data, static_cast<std::size_t>(size) * sizeof(float));
    QVector<float> bufCopy = tlsBuf;
    // Post the emission to the GUI thread via invokeMethod so this is safe
    // to call from any thread.
    QMetaObject::invokeMethod(
        &GuiBridgeManager::instance(),
        [bufCopy]() { emit GuiBridgeManager::instance().visualizerUpdated(bufCopy); },
        Qt::QueuedConnection);
}

void clear_audio_devices(void) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().audioDevicesCleared();
}

void add_audio_device(const char* name, int is_current) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().audioDeviceAdded(
        QString::fromUtf8(name ? name : ""),
        is_current != 0
    );
}

void loudness_scan_progress(int current, int total, const char* current_file) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    QString fileStr = QString::fromUtf8(current_file ? current_file : "");
    QMetaObject::invokeMethod(
        &GuiBridgeManager::instance(),
        [current, total, fileStr]() {
            emit GuiBridgeManager::instance().loudnessScanProgress(current, total, fileStr);
        },
        Qt::QueuedConnection);
}

void loudness_scan_track_result(int track_id, float lufs, float peak, float rg_gain_db, float r128_gain_db) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    QMetaObject::invokeMethod(
        &GuiBridgeManager::instance(),
        [track_id, lufs, peak, rg_gain_db, r128_gain_db]() {
            emit GuiBridgeManager::instance().loudnessScanTrackResult(track_id, lufs, peak, rg_gain_db, r128_gain_db);
        },
        Qt::QueuedConnection);
}

void loudness_scan_finished(int success, const char* error_msg) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    bool succ = (success != 0);
    QString msgStr = QString::fromUtf8(error_msg ? error_msg : "");
    QMetaObject::invokeMethod(
        &GuiBridgeManager::instance(),
        [succ, msgStr]() {
            emit GuiBridgeManager::instance().loudnessScanFinished(succ, msgStr);
        },
        Qt::QueuedConnection);
}

void request_quit() {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    QMetaObject::invokeMethod(
        qApp,
        []() { QApplication::quit(); },
        Qt::QueuedConnection);
}

// ========================================================================
// New C ABI for the essential feature set
// ========================================================================

void clear_playlists(void) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    GuiBridgeManager::instance().setPlaylists({});
    emit GuiBridgeManager::instance().playlistsCleared();
}

void add_playlist(int playlist_id, const char* name, int track_count, double duration_secs) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    GuiBridgeManager::instance().appendPlaylist({
        playlist_id,
        QString::fromUtf8(name ? name : ""),
        track_count,
        duration_secs
    });
    emit GuiBridgeManager::instance().playlistAdded(
        playlist_id,
        QString::fromUtf8(name ? name : ""),
        track_count,
        duration_secs
    );
}

void clear_albums(void) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().albumsCleared();
}

void add_album(int album_id, const char* name, const char* artist, int track_count, double duration_secs, int year, const char* cover_path) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().albumAdded(
        album_id,
        QString::fromUtf8(name ? name : ""),
        QString::fromUtf8(artist ? artist : ""),
        track_count,
        duration_secs,
        year,
        QString::fromUtf8(cover_path ? cover_path : "")
    );
}

void clear_artists(void) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().artistsCleared();
}

void add_artist(int artist_id, const char* name, int album_count, int track_count, const char* cover_path) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().artistAdded(
        artist_id,
        QString::fromUtf8(name ? name : ""),
        album_count,
        track_count,
        QString::fromUtf8(cover_path ? cover_path : "")
    );
}

void clear_albums_in_artist(void) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().albumsInArtistCleared();
}

void add_album_to_artist(int album_id, const char* name, const char* artist, int track_count, double duration_secs) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().albumInArtistAdded(
        album_id,
        QString::fromUtf8(name ? name : ""),
        QString::fromUtf8(artist ? artist : ""),
        track_count,
        duration_secs
    );
}

void set_speed_label(double speed) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().speedLabelChanged(speed);
}

void set_sleep_timer_remaining(int seconds_remaining) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().sleepTimerRemainingChanged(seconds_remaining);
}

void show_tray_message(const char* title, const char* body) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    QString t = QString::fromUtf8(title ? title : "");
    QString b = QString::fromUtf8(body ? body : "");
    QMetaObject::invokeMethod(
        &GuiBridgeManager::instance(),
        [t, b]() { emit GuiBridgeManager::instance().trayMessageRequested(t, b); },
        Qt::QueuedConnection);
}

void scroll_songs_table_to_active(void) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    emit GuiBridgeManager::instance().scrollSongsTableToActiveRequested();
}

void show_desktop_notification(const char* title, const char* body) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    QString t = QString::fromUtf8(title ? title : "");
    QString b = QString::fromUtf8(body ? body : "");
    QMetaObject::invokeMethod(
        &GuiBridgeManager::instance(),
        [t, b]() { emit GuiBridgeManager::instance().desktopNotificationRequested(t, b); },
        Qt::QueuedConnection);
}

void set_rating_for_row(int track_id, int rating) {
    if (!g_gui_initialized.load(std::memory_order_acquire)) return;
    QMetaObject::invokeMethod(
        &GuiBridgeManager::instance(),
        [track_id, rating]() { emit GuiBridgeManager::instance().trackRatingUpdated(track_id, rating); },
        Qt::QueuedConnection);
}

