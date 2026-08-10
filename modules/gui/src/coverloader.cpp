#include "coverloader.h"
#include "custom_widgets.h"  // getDefaultAlbumArt
#include "apptheme.h"
#include <QPixmapCache>
#include <QThreadPool>
#include <QRunnable>
#include <QFile>
#include <QImage>
#include <QPainter>
#include <QPainterPath>
#include <QMetaObject>
#include <QImageReader>
#include <QSet>
#include <QPointer>

// ===========================================================================
// CoverLoader implementation
// ===========================================================================

namespace {

/// Build the cache key used inside QPixmapCache for a (path, size) tuple.
/// The "cov:" prefix namespaces our entries from other QPixmapCache users
/// (e.g. the QStyle engine).
inline QString cacheKey(const QString& path, int size) {
    return QStringLiteral("cov:%1:%2").arg(size).arg(path);
}

/// Build the cache key for the rounded variant of a cover.
inline QString roundedKey(const QString& path, int size, int radius) {
    return QStringLiteral("covr:%1:%2:%3").arg(size).arg(radius).arg(path);
}

/// Load + decode a pixmap off the GUI thread. The result is staged via
/// CoverLoader::stageDelivery() (also GUI-thread) so the flush timer can
/// batch-deliver all completed loads in one 16 ms tick instead of posting
/// individual QueuedConnection events for each.
///
/// Using a DEDICATED pool (m_pool, max 2 threads) rather than the global
/// QThreadPool prevents fast scrolling from queuing thousands of tasks that
/// saturate all CPU cores — two threads is enough to keep the visible rows
/// loaded without starving other work.
class CoverLoadTask : public QRunnable {
public:
    CoverLoadTask(const QString& path, int size, QPointer<CoverLoader> owner)
        : m_path(path), m_size(size), m_owner(std::move(owner)) {}

    void run() override {
        // The actual JPEG/PNG decode happens here, off the GUI thread.
        // We use QImageReader (not QPixmap::load) because QImageReader
        // is reentrant and can be called from any thread; QPixmap::load
        // requires GUI-thread affinity on some platforms (notably X11).
        QImage image;
        if (QFile::exists(m_path)) {
            QImageReader reader(m_path);
            // Downscale very large covers early so we never allocate a
            // 2000×2000 QImage just to immediately shrink it. This caps
            // peak RSS during bulk imports.
            reader.setScaledSize(QSize(m_size, m_size));
            image = reader.read();
        }

        // Stage delivery on the GUI thread. If the singleton has been
        // destroyed (app shutdown), the QPointer is null and we skip.
        if (!m_owner) return;
        QMetaObject::invokeMethod(
            m_owner.data(),
            [owner = m_owner, path = m_path, size = m_size, image]() {
                if (!owner) return;
                QPixmap pix;
                if (!image.isNull()) {
                    pix = QPixmap::fromImage(image);
                    if (!pix.isNull()) {
                        // Insert into QPixmapCache on the GUI thread
                        // (QPixmap has GUI-thread affinity on X11).
                        QPixmapCache::insert(cacheKey(path, size), pix);
                    }
                }
                QPixmap fallback = pix.isNull() ? owner->defaultCover(size) : pix;
                // Stage for batch delivery rather than emitting directly.
                // This coalesces all loads that complete between flush ticks
                // into a single paint update instead of N individual updates.
                owner->stageDelivery(path, size, fallback);
            },
            Qt::QueuedConnection);
    }

private:
    QString m_path;
    int m_size;
    QPointer<CoverLoader> m_owner;
};

} // namespace

CoverLoader& CoverLoader::instance() {
    static CoverLoader inst;
    return inst;
}

#include "apptheme.h"

CoverLoader::CoverLoader() : QObject(nullptr) {
    // Dedicated pool: 2 threads for cover decoding. Using the global pool
    // risks saturating all cores when the user scrolls quickly (hundreds
    // of tasks queued). Two threads keep visible rows loading without
    // blocking the CPU for other work.
    m_pool = new QThreadPool(this);
    int threads = qBound(2, QThread::idealThreadCount() / 2, 4);
    m_pool->setMaxThreadCount(threads);

    // 16 ms flush timer (targeting 60 fps). All coverReady signals are
    // emitted here in a single batch, which means at most one repaint per
    // frame instead of one repaint per completed cover load.
    m_flushTimer.setInterval(16);
    m_flushTimer.setSingleShot(false);
    connect(&m_flushTimer, &QTimer::timeout, this, &CoverLoader::flushDeliveries);
    m_flushTimer.start();

    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this](const ThemePalette&) {
        clearCache();
    });
}

bool CoverLoader::tryGet(const QString& path, int size, QPixmap& out) const {
    if (path.isEmpty()) return false;
    return QPixmapCache::find(cacheKey(path, size), &out);
}

bool CoverLoader::tryGetRounded(const QString& path, int size, int radius, QPixmap& out) {
    if (path.isEmpty()) return false;
    QString key = roundedKey(path, size, radius);
    if (QPixmapCache::find(key, &out)) return true;
    QPixmap raw;
    if (!tryGet(path, size, raw)) return false;
    // Build the rounded variant.
    QPixmap target(size, size);
    target.fill(Qt::transparent);
    QPainter painter(&target);
    painter.setRenderHint(QPainter::Antialiasing, true);
    painter.setRenderHint(QPainter::SmoothPixmapTransform, true);
    QPainterPath p;
    p.addRoundedRect(0, 0, size, size, radius, radius);
    painter.setClipPath(p);
    QPixmap scaled = raw.scaled(size, size, Qt::KeepAspectRatioByExpanding, Qt::SmoothTransformation);
    int ex = (size - scaled.width()) / 2;
    int ey = (size - scaled.height()) / 2;
    painter.drawPixmap(ex, ey, scaled);
    painter.end();
    QPixmapCache::insert(key, target);
    out = target;
    return true;
}

const QPixmap& CoverLoader::defaultCover(int displaySize) {
    // Load once at the requested display size. If the caller passes a
    // different size from a previous call we use the cached one rather
    // than re-scaling — in practice all callers use the same size.
    if (m_defaultCoverSize != displaySize || m_defaultCover.isNull()) {
        QPixmap full = getDefaultAlbumArt();
        if (full.isNull()) {
            // Fallback: return whatever we have (even if wrong size).
            return m_defaultCover;
        }
        // Scale to the actual display size (e.g. 130×130) so we don’t
        // permanently hold a 512×512 pixmap (~1 MB) when it is only ever
        // rendered at 130×130 (~67 KB).
        m_defaultCover = full.scaled(
            displaySize, displaySize,
            Qt::KeepAspectRatioByExpanding,
            Qt::SmoothTransformation);
        m_defaultCoverSize = displaySize;
    }
    return m_defaultCover;
}

bool CoverLoader::resolveOrFallback(const QString& path, int size, QPixmap& outPix) {
    if (path.isEmpty() || !QFile::exists(path)) {
        outPix = defaultCover(size);
        return true;
    }
    if (tryGet(path, size, outPix)) return false;
    outPix = defaultCover(size);
    return false;
}

void CoverLoader::requestAsync(const QString& path, int size) {
    if (path.isEmpty()) {
        // Empty path: stage the default cover for delivery on the next tick.
        stageDelivery(path, size, defaultCover(size));
        return;
    }
    QPixmap cached;
    if (tryGet(path, size, cached)) {
        // Cache hit: stage for batch delivery on the next flush tick.
        stageDelivery(path, size, cached);
        return;
    }
    enqueueLoad(path, size);
}

void CoverLoader::enqueueLoad(const QString& path, int size) {
    // Deduplicate concurrent loads for the same (path, size) tuple. The
    // cache insert in the worker is idempotent, so duplicate work would
    // be harmless, but dedup halves the CPU + I/O cost when many cards
    // scroll into view simultaneously.
    QString key = cacheKey(path, size);
    if (m_pending.contains(key)) return;  // Already in-flight.
    m_pending.insert(key);
    CoverLoadTask* task = new CoverLoadTask(path, size, this);
    task->setAutoDelete(true);
    // Use the dedicated 2-thread pool at low priority so cover decoding
    // doesn’t compete with audio processing or DB queries.
    m_pool->start(task, QThread::LowPriority);
}

void CoverLoader::stageDelivery(const QString& path, int size, const QPixmap& pix) {
    // Remove from the in-flight set so re-requests after LRU eviction work.
    m_pending.remove(cacheKey(path, size));
    // Accumulate for the next flush tick.
    m_pendingDelivery.append({path, size, pix});
}

void CoverLoader::flushDeliveries() {
    if (m_pendingDelivery.isEmpty()) return;
    // Swap out the list so new stageDelivery calls during signal emission
    // don’t modify the list we’re iterating.
    QVector<PendingItem> batch;
    batch.swap(m_pendingDelivery);
    for (const auto& item : batch) {
        emit coverReady(item.path, item.size, item.pix);
    }
}

void CoverLoader::clearCache() {
    QPixmapCache::clear();
    m_pending.clear();
    m_pendingDelivery.clear();
    // Reset default cover so it is reloaded at the next request.
    m_defaultCover = QPixmap();
    m_defaultCoverSize = 0;
}

int CoverLoader::cacheLimitKb() const { return QPixmapCache::cacheLimit(); }
void CoverLoader::setCacheLimitKb(int kb) { QPixmapCache::setCacheLimit(kb); }
