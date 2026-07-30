#include "coverloader.h"
#include "custom_widgets.h"  // getDefaultAlbumArt
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

/// Load + decode a pixmap off the GUI thread. The result is delivered to
/// the singleton via QMetaObject::invokeMethod on the GUI thread, which
/// then emits coverReady so every interested widget receives it.
///
/// Note: we decode to QImage (not QPixmap) on the worker thread because
/// QPixmap has GUI-thread affinity on X11. The QImage→QPixmap conversion
/// + QPixmapCache insert happen on the GUI thread in the delivery lambda.
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

        // Deliver on the GUI thread. If the singleton has been destroyed
        // (app shutdown), the QPointer is null and we skip emission.
        if (!m_owner) return;
        // Capture `image` by copy (QImage uses implicit sharing, so the
        // copy is just a refcount increment — O(1)). We must NOT use
        // std::move here because QMetaObject::invokeMethod with
        // Qt::QueuedConnection requires the lambda to be copyable (it
        // copies the lambda into a QFunctorSlotObject).
        QMetaObject::invokeMethod(
            m_owner.data(),
            [owner = m_owner, path = m_path, size = m_size, image]() {
                if (!owner) return;
                QPixmap pix;
                if (!image.isNull()) {
                    pix = QPixmap::fromImage(image);
                    if (!pix.isNull()) {
                        // Insert into the QPixmapCache so future lookups
                        // (sync or async) hit the cache instead of re-
                        // decoding. This must happen on the GUI thread
                        // because QPixmap has GUI-thread affinity on X11.
                        QPixmapCache::insert(cacheKey(path, size), pix);
                    }
                }
                QPixmap fallback = pix.isNull() ? owner->defaultCover() : pix;
                emit owner->coverReady(path, size, fallback);
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

CoverLoader::CoverLoader() : QObject(nullptr) {
    // Remove the pending-load entry when a load completes (either from a
    // cache hit delivered via QueuedConnection, or from a worker thread).
    // This allows a future requestAsync() call for the same (path, size)
    // to spawn a fresh worker if the pixmap was evicted from the LRU.
    connect(this, &CoverLoader::coverReady,
            this, [this](const QString& path, int size, const QPixmap&) {
        m_pending.remove(cacheKey(path, size));
    }, Qt::QueuedConnection);
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

const QPixmap& CoverLoader::defaultCover() {
    if (!m_defaultCoverInit) {
        m_defaultCover = getDefaultAlbumArt();
        m_defaultCoverInit = true;
    }
    return m_defaultCover;
}

bool CoverLoader::resolveOrFallback(const QString& path, int size, QPixmap& outPix) {
    if (path.isEmpty() || !QFile::exists(path)) {
        outPix = defaultCover().scaled(size, size, Qt::KeepAspectRatioByExpanding, Qt::SmoothTransformation);
        return true;
    }
    if (tryGet(path, size, outPix)) return false;
    outPix = defaultCover().scaled(size, size, Qt::KeepAspectRatioByExpanding, Qt::SmoothTransformation);
    return false;
}

void CoverLoader::requestAsync(const QString& path, int size) {
    if (path.isEmpty()) {
        emit coverReady(path, size, defaultCover());
        return;
    }
    QPixmap cached;
    if (tryGet(path, size, cached)) {
        // Cache hit: deliver on the GUI thread immediately (queued
        // connection, so we don't recurse into the caller's stack).
        QMetaObject::invokeMethod(
            this,
            [this, path, size, cached]() { emit coverReady(path, size, cached); },
            Qt::QueuedConnection);
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
    QThreadPool::globalInstance()->start(task, QThread::LowPriority);
}

void CoverLoader::clearCache() {
    QPixmapCache::clear();
    m_pending.clear();
}

int CoverLoader::cacheLimitKb() const { return QPixmapCache::cacheLimit(); }
void CoverLoader::setCacheLimitKb(int kb) { QPixmapCache::setCacheLimit(kb); }
