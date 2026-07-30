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
//     spawns a one-shot QRunnable on a QThreadPool that loads + decodes
//     the pixmap off the GUI thread, then emits coverReady(path, size,
//     pixmap) on the GUI thread. The receiver can then call update() on
//     the relevant widget.
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

    /// Default album art (the PlayTune logo tile). Cached forever.
    const QPixmap& defaultCover();

    /// Convenience: if `path` is empty/missing, returns the default art
    /// scaled to `size`. Otherwise tries the cache; on miss, returns
    /// the default art and triggers an async load. Returns true if the
    /// fallback was used (caller may want to requestAsync to retry).
    bool resolveOrFallback(const QString& path, int size, QPixmap& outPix);

    /// Request an asynchronous load. If the pixmap is already cached,
    /// `coverReady` is emitted immediately on the GUI thread via
    /// QueuedConnection. Otherwise a worker thread loads + decodes the
    /// image, then emits `coverReady`. Multiple concurrent requests for
    /// the same (path, size) are coalesced into a single worker.
    void requestAsync(const QString& path, int size);

    /// Drop the entire pixmap cache. Used when the user clears the
    /// library (e.g. after deleting the on-disk cover cache).
    void clearCache();

    /// Total bytes used by the cache (delegates to QPixmapCache).
    int cacheLimitKb() const;
    void setCacheLimitKb(int kb);

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

    QSet<QString> m_pending;
    QPixmap m_defaultCover;
    bool m_defaultCoverInit = false;
};

#endif // COVERLOADER_H
