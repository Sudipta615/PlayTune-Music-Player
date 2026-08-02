#ifndef SONGSTABLE_H
#define SONGSTABLE_H

#include <QWidget>
#include <QTableWidget>
#include <QHeaderView>
#include <QVector>
#include <QStackedWidget>
#include <QListWidget>
#include <QLabel>
#include <QPushButton>
#include <QMenu>
#include <QHash>
#include <QMetaType>

#include "mediagridview.h"

// The 3-bar moving equalizer visualizer next to the playing song title
class PlayingEqualizerIcon : public QWidget {
    Q_OBJECT
public:
    explicit PlayingEqualizerIcon(QWidget* parent = nullptr);
    ~PlayingEqualizerIcon() override = default;

    void setPlaying(bool playing);

protected:
    void paintEvent(QPaintEvent* event) override;
    void timerEvent(QTimerEvent* event) override;

private:
    bool m_isPlaying = true;
    int m_timerId = -1;
    QVector<float> m_heights = {0.3f, 0.7f, 0.5f};
    float m_targetHeights[3] = {0.3f, 0.7f, 0.5f};
};

/// Lightweight row-data record held in C++ for the songs table. The Rust
/// backend pushes these in batches via `addSongsBatch` to avoid 1 FFI
/// round-trip per track (which was the single biggest UI-thread bottleneck
/// for large libraries — see refresh_ui in ui_sync.rs).
struct SongRow {
    int displayIndex = 0;
    int songId = 0;
    bool isFavorite = false;
    QString title;
    QString artist;
    QString album;
    QString duration;
    QString coverPath;
};

// Required so QVector<SongRow> can cross the Qt::QueuedConnection
// boundary (the signal songsBatchReplaced is emitted from a worker
// thread and delivered on the GUI thread). Without this, Qt prints
// "QObject::connect: Cannot queue arguments of type 'QVector<SongRow>'"
// and silently drops the signal.
Q_DECLARE_METATYPE(QVector<SongRow>)

// Main Songs Table Widget
class SongsTableWidget : public QWidget {
    Q_OBJECT
public:
    explicit SongsTableWidget(QWidget* parent = nullptr);
    ~SongsTableWidget() override = default;

    // ── Single-track API (kept for back-compat with the bridge) ──────────
    void clearSongs();
    void addSong(int index, int songId, bool isFavorite, const QString& title,
                 const QString& artist, const QString& album, const QString& duration,
                 const QString& coverPath);

    // ── Batch API (preferred for >50 tracks) ─────────────────────────────
    /// Replace the entire table content in one transactional rebuild.
    /// `rows` is moved into the table; the caller does not need to keep
    /// it alive. Internally:
    ///   * signals are blocked
    ///   * updates are disabled
    ///   * the table is repopulated with setRowCount + bulk item assignment
    ///   * covers are NOT loaded — they're resolved lazily by the row
    ///     delegate when each row becomes visible (via the shared
    ///     CoverLoader cache).
    /// This reduces a 10 000-track refresh from ~10 000 × cover-load +
    /// widget-create calls on the GUI thread to a single transactional
    /// rebuild that touches only the visible rows.
    void setSongsBatch(QVector<SongRow> rows);

    void setPlayingTrack(int trackIdx, bool playing);
    void setPlayingSongId(int songId, bool playing);
    void setResponsiveWidth(int width);
    void updateTrackRow(int songId, const QString& title, const QString& artist, const QString& album, const QString& duration, const QString& coverPath);
    void openTagEditorDialog(int row);
    void openLoudnessScannerDialog(const QVector<int>& trackIds = QVector<int>());

    /// Update the rating cell for the row containing `songId`.
    void setRatingForRow(int songId, int rating);

    /// Force the table to scroll to the playing row. Called by the bridge
    /// when the user toggles "cursor follows playback" on.
    void scrollToActive();

    /// Apply or remove Optimized Mode live (hides/shows thumbnails, gates cover loads).
    void setOptimizedMode(bool enabled);

    /// Show or hide the header back button (e.g. for drill-down views in Folders, Albums, Artists)
    void setBackButtonVisible(bool visible, const QString& text = "‹  Back");

    int playingTrackIdx() const { return m_playingTrackIdx; }
    int playingSongId() const { return m_playingSongId; }
    int hoveredRow() const { return m_hoveredRow; }
    QTableWidget* tableWidget() const { return m_table; }

    /// Trigger a re-layout of the grid page (only called when the user
    /// switches to grid view; the grid is NOT populated while the table
    /// view is active, which saves ~676 MB of pixmap memory on a 10 000-
    /// track library).
    void updateGridResponsive();
    void flushSongCount();

signals:
    void songSelected(int index);
    void backButtonClicked();

protected:
    bool eventFilter(QObject* watched, QEvent* event) override;
    void resizeEvent(QResizeEvent* event) override;
    void showEvent(QShowEvent* event) override;

private slots:
    void onCellEntered(int row, int col);
    /// O(1) row-style update: only repaints the previously-hovered and
    /// newly-hovered rows (plus the playing row if it changed). The
    /// previous implementation iterated all rows on every hover event,
    /// which made scrolling a 10 000-track table visibly janky.
    void updateRowStyles();

private:
    void setupUi();
    void populateGridFromTable();  // Lazily build grid cards from m_rows.
    QPixmap getThumbnail(const QString& title);
    void populateAddToPlaylistMenu(QMenu* menu, int songId);
    void refreshSingleRowStyle(int row);

    QTableWidget* m_table = nullptr;
    QStackedWidget* m_stackedWidget = nullptr;
    MediaGridWidget* m_gridWidget = nullptr;
    QLabel* m_songCountLabel = nullptr;
    QPushButton* m_backBtn = nullptr;
    int m_playingTrackIdx = -1;
    int m_playingSongId = -1;
    int m_hoveredRow = -1;
    int m_previousHoveredRow = -1;
    bool m_isPlaying = false;
    QVector<PlayingEqualizerIcon*> m_eqIcons;
    /// Cache of the last batch pushed via setSongsBatch. The grid view is
    /// populated lazily from this on the first switch to grid mode, so
    /// we don't pay the grid-card widget creation cost when the user
    /// never leaves the table view.
    QVector<SongRow> m_rows;
    /// Lazily-populated flag: true once the grid has been built from
    /// m_rows. Reset to false whenever clearSongs() or setSongsBatch()
    /// is called.
    bool m_gridPopulated = false;
    QHash<int, int> m_songIdToRow;
    bool m_songCountDirty = false;
};

#endif // SONGSTABLE_H
