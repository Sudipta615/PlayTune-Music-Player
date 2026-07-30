#ifndef MEDIAGRIDVIEW_H
#define MEDIAGRIDVIEW_H

// ===========================================================================
// mediagridview.h — Unified grid component shared by Home, Albums and Artists
// ===========================================================================
//
// Problem (Issue #1 — Inconsistent Grid Views)
// --------------------------------------------
//   Prior to this refactor, three different QListWidget subclasses and three
//   different card QFrame subclasses existed:
//     * SongGridCard      (songstable.cpp)      — Home tab grid
//     * AlbumGridCard     (albumsview.cpp)      — Albums tab grid
//     * ArtistGridCard    (artistsview.cpp)     — Artists tab grid
//   Each implemented its own:
//     * card QSS (identical 14px-radius #0E121B frame)
//     * cover rounding helper (getRoundedPixmap, duplicated 4×)
//     * updateGridResponsive() (duplicated 3×)
//     * cover load + decode (run on the GUI thread, blocking scrolling)
//
// Solution
// --------
//   A single MediaGridCard + MediaGridWidget pair. All three tabs now share
//   the exact same appearance, sizing, lazy cover loading, virtualised
//   rendering (QListWidget with UniformItemSizes + Adjust), hover/selection
//   behaviour, and resize-responsive column recalculation.
//
// Performance notes
// -----------------
//   * Covers are NOT loaded synchronously in addItem(). The cover path is
//     stashed on the item; the card requests the cover lazily from the
//     shared CoverLoader (see coverloader.h) the first time the card becomes
//     visible. Off-screen cards never touch the disk.
//   * CoverLoader is process-wide and LRU-bounded, so the Home tab, Albums
//     tab and Artists tab share the same decoded pixmaps (saves ~50 MB on
//     a 1 000-track library where many covers repeat).
//   * UniformItemSizes = true lets QListWidget compute scroll geometry in
//     O(1) instead of O(n).
// ===========================================================================

#include <QListWidget>
#include <QListWidgetItem>
#include <QFrame>
#include <QLabel>
#include <QResizeEvent>
#include <QShowEvent>
#include <QString>
#include <QPixmap>
#include <QTimer>
#include <QSet>
#include <QPair>
#include <QVector>
#include <functional>

class MediaGridCard : public QFrame {
    Q_OBJECT
public:
    explicit MediaGridCard(QWidget* parent = nullptr);

    /// Update the card content. `cover_path` is resolved lazily via the
    /// shared CoverLoader; passing an empty path falls back to the default
    /// album art. `cover_size` overrides the resolved cover dimension.
    void setContent(const QString& title,
                    const QString& subtitle,
                    const QString& cover_path,
                    int coverSize = 130);

    /// Toggle the "playing" highlight (pink border + pink title text).
    void setPlaying(bool playing);

    /// Re-resolve the cover pixmap after the LRU cache evicted it, or after
    /// the card was scrolled back into view.
    void refreshCover();

    /// Current cover path stored on the card.
    const QString& coverPath() const { return m_coverPath; }
    int coverSize() const { return m_coverSize; }

    /// Update just the cover label's fixed size without re-triggering
    /// content loading. Used by MediaGridWidget::rebuildCardSizes() to
    /// avoid redundant setContent calls on every resize / tab switch.
    void updateCoverLabelSize(int size) { applyCoverSize(size); }

    /// Whether the cover has been loaded (or a load is in-flight).
    bool coverRequested() const { return m_coverRequested; }

    /// When set, refreshCover() will skip the actual cover load and just
    /// set the pending flag. Used by MediaGridWidget batch append.
    void setDeferRefresh(bool defer) { m_deferRefresh = defer; }

protected:
    void showEvent(QShowEvent* event) override;

private:
    void applyCoverSize(int size);

    QLabel* m_coverLabel = nullptr;
    QLabel* m_titleLabel = nullptr;
    QLabel* m_subtitleLabel = nullptr;
    QString m_coverPath;
    QString m_title;
    QString m_subtitle;
    int m_coverSize = 130;
    bool m_playing = false;
    bool m_coverRequested = false;
    bool m_deferRefresh = false;
};

/// A QListWidget configured for icon-mode grid layout with uniform item
/// sizes, lazy cover loading, and resize-responsive column recalculation.
class MediaGridWidget : public QListWidget {
    Q_OBJECT
public:
    explicit MediaGridWidget(QWidget* parent = nullptr);

    /// Append a card to the grid. The card widget is created on demand
    /// and bound to the item. Cover loading is deferred until the item
    /// becomes visible.
    ///
    /// `userData` is stashed on the item at Qt::UserRole and is typically
    /// the album / artist / track id used by the parent view's click
    /// handler.
    void addCard(int userData,
                 const QString& title,
                 const QString& subtitle,
                 const QString& cover_path,
                 bool deferCover = false);

    /// Recompute the card width / cover size from the current viewport
    /// width. Cheap if the size hasn't changed (early-return on size match).
    void updateGridResponsive();

    /// Mark the card at `index` as the playing track (or pass -1 to clear).
    void setPlayingIndex(int index);

    /// Clear all cards.
    void clearGrid();

    /// Number of cards currently in the grid.
    using QListWidget::count;

    /// Batch-append mode: call before adding many cards. Disables updates
    /// and signals so the grid is only laid out once in endBatchAppend().
    /// Typical usage:
    ///   grid->beginBatchAppend();
    ///   for (...) { grid->addCard(...); }
    ///   grid->endBatchAppend();
    void beginBatchAppend();
    void endBatchAppend();

signals:
    /// Emitted when the user clicks or double-clicks a card.
    /// `userData` is the value passed to addCard().
    void cardActivated(int userData);

protected:
    void resizeEvent(QResizeEvent* event) override;
    void showEvent(QShowEvent* event) override;

private slots:
    void onItemClicked(QListWidgetItem* item);
    void onItemDoubleClicked(QListWidgetItem* item);
    void processDeferredCoverLoads();

private:
    void rebuildCardSizes();
    void scheduleDeferredLoads();
    void enqueueDeferredCover(const QString& path, int size);

    int m_playingIndex = -1;
    int m_lastCols = 0;
    int m_lastCardWidth = 0;
    int m_lastCardHeight = 0;
    bool m_batchMode = false;

    QTimer* m_deferredLoadTimer = nullptr;
    QVector<QPair<QString, int>> m_deferredLoadQueue;
    QSet<QString> m_deferredLoadSeen;
};

#endif // MEDIAGRIDVIEW_H
