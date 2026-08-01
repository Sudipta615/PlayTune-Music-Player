#ifndef COVERLOADER_H
#define COVERLOADER_H

// ===========================================================================
// coverloader.h — Process-wide album-cover pixmap cache with async loading
// ===========================================================================
//
// Problem (Issues #2 + #3 — performance + RAM)
// --------------------------------------------
//   Before this refactor, every grid card, every songs-table row, the
//   Now-Playing card and the Queue widget called `QPixmap::load(path)`
//   inline on the GUI thread. For a 1 000-track library that meant:
//     * 1 000 JPEG decodes on the GUI thread during refresh_ui (UI freeze).
//     * 4 copies of each decoded pixmap (Home + Albums + Artists + Queue
//       each kept their own copy). With 1 000 covers at 200×200×4 bytes
//       that's ~1.6 MB × 4 = 6.4 MB just for cover storage, and each
//       `addSong` FFI round-trip paid the full decode cost again.
//
// Solution
// --------
//   * CoverLoader is a thread-safe singleton living on the GUI thread.
//     It wraps Qt's built-in QPixmapCache with a process-wide key
//     namespace ("cov:<size>:<path>"), so the Home, Albums, Artists and
//     Queue tabs all share the same decoded pixmaps.
//   * For synchronous lookups (e.g. inside a delegate's paint()), the
//     loader returns the cached pixmap if present, or the default album
//     art otherwise — it NEVER does I/O on the calling thread.
//   * For async loads (e.g. when a card scrolls into view), the loader
//     spawns a one-shot QRunnable on a DEDICATED 2-thread pool that loads
//     + decodes the pixmap off the GUI thread, then emits coverReady on
//     the GUI thread. Using only 2 threads prevents fast scrolling from
//     saturating all CPU cores with thousands of queued decode tasks.
//   * coverReady emissions are BATCHED via a 16 ms flush timer: completed
//     loads accumulate in m_pendingDelivery and are flushed together. This
//     reduces the number of QueuedConnection events posted to the GUI
//     thread from O(visible rows * scroll speed) to O(60 fps).
//   * The default cover is loaded and scaled to the actual display size
//     (e.g. 130×130) on first request, so we don't hold a full-res
//     512×512 pixmap (~1 MB) in RAM when it is only rendered at 130×130
//     (~67 KB).
//   * The cache is bounded by QPixmapCache::setCacheLimit() (set once in
//     MainWindow to 20 MB). Once full, the LRU eviction built into
//     QPixmapCache kicks in, capping total cover memory regardless of
//     library size.
// ===========================================================================

#include <QObject>
#include <QPixmap>
#include <QString>
#include <QHash>
#include <QSet>
#include <QTimer>
#include <QVector>

class QThreadPool;

class CoverLoader : public QObject {
    Q_OBJECT
public:
    static CoverLoader& instance();

    /// Synchronous cache lookup. Returns false on miss.
    /// NEVER does disk I/O — safe to call from paint() on the GUI thread.
    bool tryGet(const QString& path, int size, QPixmap& out) const;

    /// Synchronous cache lookup with rounded-corner post-processing.
    /// The rounded variant is cached under a separate key so subsequent
    /// lookups are free.
    bool tryGetRounded(const QString& path, int size, int radius, QPixmap& out);

    /// Default album art (the PlayTune logo tile). Loaded and scaled to
    /// `displaySize` on first call; subsequent calls return the cached
    /// pixmap without re-reading the resource.
    const QPixmap& defaultCover(int displaySize = 130);

    /// Convenience: if `path` is empty/missing, returns the default art
    /// scaled to `size`. Otherwise tries the cache; on miss, returns
    /// the default art and triggers an async load. Returns true if the
    /// fallback was used (caller may want to requestAsync to retry).
    bool resolveOrFallback(const QString& path, int size, QPixmap& outPix);

    /// Request an asynchronous load. If the pixmap is already cached,
    /// it is staged for delivery on the next flush tick (≤16 ms) instead
    /// of emitting immediately, to coalesce rapid scroll events. Otherwise
    /// a worker thread loads + decodes the image; the result is also
    /// batched through the flush timer. Multiple concurrent requests for
    /// the same (path, size) are coalesced into a single worker.
    void requestAsync(const QString& path, int size);

    /// Drop the entire pixmap cache. Used when the user clears the
    /// library (e.g. after deleting the on-disk cover cache).
    void clearCache();

    /// Total bytes used by the cache (delegates to QPixmapCache).
    int cacheLimitKb() const;
    void setCacheLimitKb(int kb);

public slots:
    /// Called (on the GUI thread, via QMetaObject::invokeMethod) by worker
    /// threads to stage a decoded image for batch delivery.
    void stageDelivery(const QString& path, int size, const QPixmap& pix);

    /// Flush all staged deliveries and emit coverReady for each.
    void flushDeliveries();

signals:
    /// Emitted on the GUI thread whenever a cover becomes available
    /// (either from a cache hit during requestAsync, or from a worker
    /// finishing an off-GUI-thread load).
    /// `path` and `size` identify the request; `pix` is the decoded
    /// pixmap (or the default cover if the load failed).
    void coverReady(const QString& path, int size, const QPixmap& pix);

private:
    CoverLoader();
    ~CoverLoader() override = default;
    CoverLoader(const CoverLoader&) = delete;
    CoverLoader& operator=(const CoverLoader&) = delete;

    void enqueueLoad(const QString& path, int size);

    // Dedicated 2-thread pool for cover decoding. Separate from the
    // global QThreadPool so cover loads don't starve other workers.
    QThreadPool* m_pool = nullptr;

    // Pending-load deduplication set (keyed by cacheKey(path, size)).
    QSet<QString> m_pending;

    // Pending deliveries accumulated between flush ticks.
    struct PendingItem {
        QString path;
        int     size;
        QPixmap pix;
    };
    QVector<PendingItem> m_pendingDelivery;

    // 16 ms batch-flush timer (targets 60 fps).
    QTimer m_flushTimer;

    // Default cover, scaled to display size on first use.
    QPixmap m_defaultCover;
    int     m_defaultCoverSize = 0; // 0 means not yet loaded
};

#endif // COVERLOADER_H
