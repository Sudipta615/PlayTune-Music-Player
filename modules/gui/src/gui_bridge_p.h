#ifndef GUI_BRIDGE_P_H
#define GUI_BRIDGE_P_H

#include <QObject>
#include <QString>
#include <QVector>
#include <QPair>
#include "gui_bridge.h"
#include "songstable.h"  // SongRow

/// A user-defined playlist row pushed by the Rust backend via `add_playlist`.
struct PlaylistRow {
    int id = 0;
    QString name;
    int track_count = 0;
    double duration_secs = 0.0;
};

class GuiBridgeManager : public QObject {
    Q_OBJECT
public:
    static GuiBridgeManager& instance() {
        static GuiBridgeManager inst;
        return inst;
    }

    void setCallbacks(Callbacks cb) {
        m_callbacks = cb;
    }

    const Callbacks& callbacks() const {
        return m_callbacks;
    }

    /// Read-only access to the cached playlist list. Populated by
    /// `add_playlist()` calls and cleared by `clear_playlists()`. Used by
    /// the songs table's "Add to Playlist" submenu and by the sidebar.
    const QVector<PlaylistRow>& playlists() const { return m_playlists; }
    /// Replace the cached playlist list (called by clear_playlists +
    /// add_playlist implementations in gui_bridge.cpp).
    void setPlaylists(const QVector<PlaylistRow>& list) { m_playlists = list; }
    void appendPlaylist(const PlaylistRow& row) { m_playlists.append(row); }

signals:
    void playStateChanged(bool playing);
    void progressChanged(double elapsed, double total);
    void trackChanged(const QString& title, const QString& artist, const QString& album, const QString& cover_path);
    void trackMetadataUpdated(int track_id, const QString& title, const QString& artist, const QString& album, const QString& duration_str, const QString& cover_path);
    void trackLyricsUpdated(int track_id, const QString& synced_lrc, const QString& unsynced_lyrics);
    void activeIndexChanged(int index);
    void songsCleared();
    void songAdded(int index, int song_id, bool is_favorite, const QString& title, const QString& artist, const QString& album, const QString& duration, const QString& cover_path);

    /// Batched song-replace signal. Emitted by set_songs_batch() (FFI).
    /// The receiver is expected to call SongsTableWidget::setSongsBatch
    /// with the moved QVector<SongRow>. This collapses O(n) per-track
    /// FFI round-trips into one signal emission per refresh.
    void songsBatchReplaced(QVector<SongRow> rows);
    void queueCleared();
    void queueUpdateBegan();
    void queueUpdateEnded();
    void queueSongAdded(int index, const QString& title, const QString& artist, const QString& duration, const QString& cover_path);
    void foldersCleared();
    void folderAdded(int id, const QString& path, const QString& name, int track_count);
    void viewSwitched(int view_index);
    void visualizerUpdated(const QVector<float>& data);
    void audioDevicesCleared();
    void audioDeviceAdded(const QString& name, bool is_current);
    void loudnessScanProgress(int current, int total, const QString& current_file);
    void loudnessScanTrackResult(int track_id, float lufs, float peak, float rg_gain_db, float r128_gain_db);
    void loudnessScanFinished(bool success, const QString& error_msg);
    // New signals for the essential feature set
    void playlistsCleared();
    void playlistAdded(int playlist_id, const QString& name, int track_count, double duration_secs);
    void albumsCleared();
    void albumAdded(int album_id, const QString& name, const QString& artist, int track_count, double duration_secs, int year, const QString& cover_path);
    void artistsCleared();
    void artistAdded(int artist_id, const QString& name, int album_count, int track_count, const QString& cover_path);
    void albumsInArtistCleared();
    void albumInArtistAdded(int album_id, const QString& name, const QString& artist, int track_count, double duration_secs);
    void speedLabelChanged(double speed);
    void sleepTimerRemainingChanged(int seconds_remaining);
    void trayMessageRequested(const QString& title, const QString& body);
    void scrollSongsTableToActiveRequested();
    void desktopNotificationRequested(const QString& title, const QString& body);
    void trackRatingUpdated(int track_id, int rating);

private:
    GuiBridgeManager() = default;
    Callbacks m_callbacks = {};
    QVector<PlaylistRow> m_playlists;
};

#endif // GUI_BRIDGE_P_H
