#include "mediagridview.h"
#include "coverloader.h"
#include "custom_widgets.h"  // getDefaultAlbumArt
#include "appsettings.h"
#include "apptheme.h"
#include <QFile>
#include <QVBoxLayout>
#include <QFontMetrics>
#include <QTimer>
#include <QScrollBar>
#include <QResizeEvent>
#include <QShowEvent>

// ===========================================================================
// Helpers
// ===========================================================================

namespace {

static const int kMaxConcurrentLoads = 20;
static const int kCoverLoadIntervalMs = 30;

} // namespace

// ===========================================================================
// MediaGridCard
// ===========================================================================

MediaGridCard::MediaGridCard(QWidget* parent) : QFrame(parent) {
    setObjectName("CardFrame");

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(4, 4, 4, 4);
    layout->setSpacing(1);

    m_coverLabel = new QLabel(this);
    m_coverLabel->setAlignment(Qt::AlignCenter);
    m_coverLabel->setStyleSheet("background: transparent; border: none;");

    m_titleLabel = new QLabel(this);
    m_titleLabel->setAlignment(Qt::AlignCenter);
    m_titleLabel->setWordWrap(false);
    m_titleLabel->setTextFormat(Qt::PlainText);
    m_titleLabel->setFixedHeight(16);

    m_subtitleLabel = new QLabel(this);
    m_subtitleLabel->setAlignment(Qt::AlignCenter);
    m_subtitleLabel->setWordWrap(false);
    m_subtitleLabel->setTextFormat(Qt::PlainText);
    m_subtitleLabel->setFixedHeight(14);

    layout->addWidget(m_coverLabel, 0, Qt::AlignCenter);
    layout->addSpacing(2);
    layout->addWidget(m_titleLabel);
    layout->addWidget(m_subtitleLabel);

    setPlaying(false);

    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this](const ThemePalette&) {
        setPlaying(m_playing);
        if (m_coverPath.isEmpty() || !QFile::exists(m_coverPath) || AppSettings::instance().isOptimizedMode()) {
            refreshCover();
        }
    });

    connect(&CoverLoader::instance(), &CoverLoader::coverReady,
            this, [this](const QString& path, int size, const QPixmap& pix) {
        if (path == m_coverPath && size == m_coverSize) {
            QPixmap rounded;
            if (path.isEmpty()) {
                rounded = pix.scaled(m_coverSize, m_coverSize,
                                     Qt::KeepAspectRatioByExpanding,
                                     Qt::SmoothTransformation);
            } else if (!CoverLoader::instance().tryGetRounded(
                           path, m_coverSize, 10, rounded)) {
                rounded = pix.scaled(m_coverSize, m_coverSize,
                                     Qt::KeepAspectRatioByExpanding,
                                     Qt::SmoothTransformation);
            }
            m_coverLabel->setPixmap(rounded);
            m_coverRequested = true;
        }
    });
}

void MediaGridCard::setContent(const QString& title,
                               const QString& subtitle,
                               const QString& cover_path,
                               int coverSize) {
    coverSize = qMax(60, coverSize);
    m_title = title;
    m_subtitle = subtitle;

    // Title + subtitle use elided text so long titles don't wrap.
    QFontMetrics tf(m_titleLabel->font());
    m_titleLabel->setText(tf.elidedText(title, Qt::ElideRight, coverSize + 8));
    QFontMetrics sf(m_subtitleLabel->font());
    m_subtitleLabel->setText(sf.elidedText(subtitle, Qt::ElideRight, coverSize + 8));

    // If the cover path or size changed, reset the pending-load flag and
    // trigger a fresh resolution.
    if (cover_path != m_coverPath || coverSize != m_coverSize) {
        m_coverPath = cover_path;
        m_coverSize = coverSize;
        m_coverRequested = false;
        applyCoverSize(coverSize);
        refreshCover();
    } else if (m_coverRequested) {
        // Same path/size — keep existing pixmap.
        applyCoverSize(coverSize);
    } else {
        applyCoverSize(coverSize);
        refreshCover();
    }
}

void MediaGridCard::setPlaying(bool playing) {
    m_playing = playing;
    const auto& p = ThemeManager::instance().currentTheme();
    if (m_playing) {
        m_titleLabel->setStyleSheet(QString(
            "font-weight: 600; font-size: 11px; color: %1; "
            "background: transparent; border: none;").arg(p.secondaryAccent.name()));
        m_subtitleLabel->setStyleSheet(QString(
            "font-size: 10px; color: %1; background: transparent; border: none;").arg(p.secondaryText.name()));
        setStyleSheet(QString(
            "QFrame#CardFrame {"
            "  background-color: %1;"
            "  border: 1px solid %2;"
            "  border-radius: 14px;"
            "}"
            "QFrame#CardFrame:hover {"
            "  background-color: %1;"
            "  border: 1px solid %2;"
            "}"
        ).arg(p.itemSelectedBg.name(), p.secondaryAccent.name()));
    } else {
        m_titleLabel->setStyleSheet(QString(
            "font-weight: 600; font-size: 11px; color: %1; "
            "background: transparent; border: none;").arg(p.secondaryText.name()));
        m_subtitleLabel->setStyleSheet(QString(
            "font-size: 10px; color: %1; background: transparent; border: none;").arg(p.mutedText.name()));
        setStyleSheet(QString(
            "QFrame#CardFrame {"
            "  background-color: %1;"
            "  border: 1px solid %2;"
            "  border-radius: 14px;"
            "}"
            "QFrame#CardFrame:hover {"
            "  background-color: %3;"
            "  border: 1px solid %4;"
            "}"
        ).arg(p.cardBg.name(), p.cardBorder.name(), p.itemHoverBg.name(), p.primaryAccent.name()));
    }
}

void MediaGridCard::refreshCover() {
    if (m_deferRefresh) {
        m_coverRequested = false;
        return;
    }
    // In Optimized Mode, skip real cover loads — show the default art instead.
    if (AppSettings::instance().isOptimizedMode()) {
        if (m_coverLabel) {
            QPixmap placeholder = getDefaultAlbumArt();
            if (!placeholder.isNull()) {
                m_coverLabel->setPixmap(
                    placeholder.scaled(m_coverSize, m_coverSize,
                                       Qt::KeepAspectRatioByExpanding,
                                       Qt::SmoothTransformation));
            } else {
                m_coverLabel->setPixmap(QPixmap());
            }
        }
        m_coverRequested = false;
        return;
    }

    if (m_coverPath.isEmpty()) {
        QPixmap def = CoverLoader::instance().defaultCover();
        m_coverLabel->setPixmap(def.scaled(m_coverSize, m_coverSize,
                                            Qt::KeepAspectRatioByExpanding,
                                            Qt::SmoothTransformation));
        m_coverRequested = true;
        return;
    }
    QPixmap rounded;
    if (CoverLoader::instance().tryGetRounded(m_coverPath, m_coverSize, 10, rounded)) {
        m_coverLabel->setPixmap(rounded);
        m_coverRequested = true;
        return;
    }
    // Cache miss: show the default cover immediately and trigger an async
    // load. The lambda connected in the constructor will fire when the
    // load completes and replace the pixmap.
    QPixmap fallback;
    CoverLoader::instance().resolveOrFallback(m_coverPath, m_coverSize, fallback);
    m_coverLabel->setPixmap(fallback);
    CoverLoader::instance().requestAsync(m_coverPath, m_coverSize);
    m_coverRequested = false;  // Will be set true when coverReady arrives.
}

void MediaGridCard::showEvent(QShowEvent* event) {
    QFrame::showEvent(event);
    // When the card first becomes visible, ensure the cover is requested.
    // (Cards created by QListWidget::setItemWidget are not immediately
    // shown if they're off-screen — refreshCover() in setContent() may
    // have been deferred.)
    if (!m_coverRequested) {
        refreshCover();
    }
}

void MediaGridCard::applyCoverSize(int size) {
    if (m_coverLabel) m_coverLabel->setFixedSize(size, size);
}

void MediaGridCard::setCoverOptimized(bool optimized) {
    if (optimized) {
        // Show the default album art placeholder instead of a blank dark square.
        // getDefaultAlbumArt() returns a pre-cached QPixmap — no disk I/O.
        QPixmap placeholder = getDefaultAlbumArt();
        if (m_coverLabel) {
            if (!placeholder.isNull()) {
                m_coverLabel->setPixmap(
                    placeholder.scaled(m_coverSize, m_coverSize,
                                       Qt::KeepAspectRatioByExpanding,
                                       Qt::SmoothTransformation));
            } else {
                m_coverLabel->setPixmap(QPixmap());
            }
        }
        // Mark as not-requested so refreshCover() re-fetches on Normal restore.
        m_coverRequested = false;
    } else {
        // Normal Mode restored: allow refreshCover to re-request the real cover.
        m_coverRequested = false;
    }
}

// ===========================================================================
// MediaGridWidget
// ===========================================================================

MediaGridWidget::MediaGridWidget(QWidget* parent) : QListWidget(parent) {
    setViewMode(QListWidget::IconMode);
    setResizeMode(QListWidget::Adjust);
    setMovement(QListWidget::Static);
    setUniformItemSizes(true);
    setSpacing(10);
    setWordWrap(false);
    setFocusPolicy(Qt::NoFocus);
    setStyleSheet(
        "QListWidget { background-color: transparent; border: none; }"
        "QListWidget::item { background-color: transparent; border: none; padding: 4px; }"
        "QListWidget::item:hover, QListWidget::item:selected {"
        "    background-color: transparent; border: none;"
        "}");
    setVerticalScrollMode(QAbstractItemView::ScrollPerPixel);
    setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);

    connect(this, &QListWidget::itemClicked, this, &MediaGridWidget::onItemClicked);
    connect(this, &QListWidget::itemDoubleClicked, this, &MediaGridWidget::onItemDoubleClicked);

    m_deferredLoadTimer = new QTimer(this);
    m_deferredLoadTimer->setSingleShot(false);
    m_deferredLoadTimer->setInterval(kCoverLoadIntervalMs);
    connect(m_deferredLoadTimer, &QTimer::timeout, this, &MediaGridWidget::processDeferredCoverLoads);
}

void MediaGridWidget::addCard(int userData,
                              const QString& title,
                              const QString& subtitle,
                              const QString& cover_path,
                              bool deferCover) {
    auto* item = new QListWidgetItem(this);
    item->setData(Qt::UserRole, userData);
    item->setData(Qt::UserRole + 1, title);
    item->setData(Qt::UserRole + 2, subtitle);
    item->setData(Qt::UserRole + 3, cover_path);

    item->setSizeHint(QSize(140, 185));

    auto* card = new MediaGridCard(this);
    if (deferCover || m_batchMode) {
        card->setDeferRefresh(true);
    }
    card->setContent(title, subtitle, cover_path, 110);
    addItem(item);
    setItemWidget(item, card);
}

void MediaGridWidget::clearGrid() {
    if (m_deferredLoadTimer) m_deferredLoadTimer->stop();
    m_deferredLoadQueue.clear();
    m_deferredLoadSeen.clear();
    clear();
    m_playingIndex = -1;
}

void MediaGridWidget::beginBatchAppend() {
    m_batchMode = true;
    setUpdatesEnabled(false);
    blockSignals(true);
}

void MediaGridWidget::endBatchAppend() {
    m_batchMode = false;
    // Clear defer-refresh on all cards so they can load covers normally.
    for (int i = 0; i < count(); ++i) {
        if (auto* it = item(i)) {
            if (auto* card = qobject_cast<MediaGridCard*>(itemWidget(it))) {
                card->setDeferRefresh(false);
            }
        }
    }
    blockSignals(false);
    setUpdatesEnabled(true);
    QTimer::singleShot(0, this, &MediaGridWidget::updateGridResponsive);
}

void MediaGridWidget::setPlayingIndex(int index) {
    if (m_playingIndex == index) return;
    // Update only the previously-playing and newly-playing cards. O(1)
    // instead of the O(n) full-rescan that the old code did.
    if (m_playingIndex >= 0 && m_playingIndex < count()) {
        if (auto* it = item(m_playingIndex)) {
            if (auto* card = qobject_cast<MediaGridCard*>(itemWidget(it))) {
                card->setPlaying(false);
            }
        }
    }
    m_playingIndex = index;
    if (index >= 0 && index < count()) {
        if (auto* it = item(index)) {
            if (auto* card = qobject_cast<MediaGridCard*>(itemWidget(it))) {
                card->setPlaying(true);
            }
        }
    }
}

void MediaGridWidget::updateGridResponsive() {
    if (!viewport()) return;
    int w = viewport()->width();
    if (w <= 0) return;
    if (verticalScrollBar() && verticalScrollBar()->isVisible()) {
        w -= verticalScrollBar()->sizeHint().width();
    }

    const int spacing = 10;
    const int targetMin = 120;
    int cols = qMax(1, (w + spacing) / (targetMin + spacing));
    int cardWidth = (w - (cols - 1) * spacing) / cols;
    int coverSize = qBound(50, cardWidth - 24, 130);
    int cardHeight = coverSize + 56;

    if (cols == m_lastCols && cardWidth == m_lastCardWidth &&
        cardHeight == m_lastCardHeight) {
        return;
    }
    m_lastCols = cols;
    m_lastCardWidth = cardWidth;
    m_lastCardHeight = cardHeight;

    setUpdatesEnabled(false);
    setGridSize(QSize(cardWidth, cardHeight));
    setIconSize(QSize(coverSize, coverSize));
    rebuildCardSizes();
    setUpdatesEnabled(true);
}

void MediaGridWidget::rebuildCardSizes() {
    if (!viewport()) return;
    int w = viewport()->width();
    if (w <= 0) return;
    const int spacing = 10;
    const int targetMin = 120;
    int cols = qMax(1, (w + spacing) / (targetMin + spacing));
    int cardWidth = (w - (cols - 1) * spacing) / cols;
    int coverSize = qBound(50, cardWidth - 24, 130);
    int cardHeight = coverSize + 56;

    for (int i = 0; i < count(); ++i) {
        if (auto* it = item(i)) {
            it->setSizeHint(QSize(cardWidth, cardHeight));
            if (auto* card = qobject_cast<MediaGridCard*>(itemWidget(it))) {
                // Only update the cover label size — don't re-trigger
                // setContent (which may re-request covers). The actual
                // cover pixmap will be refreshed lazily by the
                // deferred-load timer when the card scrolls into view.
                card->updateCoverLabelSize(coverSize);
            }
        }
    }

    // Kick off deferred cover loading for visible cards.
    scheduleDeferredLoads();
}

void MediaGridWidget::scheduleDeferredLoads() {
    if (!viewport()) return;
    QRect visible = viewport()->rect();
    int top = indexAt(QPoint(0, visible.top())).row();
    int bot = indexAt(QPoint(0, visible.bottom())).row();
    if (top < 0) top = 0;
    if (bot < 0) bot = count() - 1;

    for (int i = top; i <= bot && i < count(); ++i) {
        if (auto* it = item(i)) {
            QString coverPath = it->data(Qt::UserRole + 3).toString();
            if (coverPath.isEmpty()) continue;
            if (auto* card = qobject_cast<MediaGridCard*>(itemWidget(it))) {
                if (card->coverRequested()) continue;
                enqueueDeferredCover(coverPath, card->coverSize());
            }
        }
    }

    if (!m_deferredLoadQueue.isEmpty() && !m_deferredLoadTimer->isActive()) {
        m_deferredLoadTimer->start();
    }
}

void MediaGridWidget::enqueueDeferredCover(const QString& path, int size) {
    QString key = QStringLiteral("%1:%2").arg(size).arg(path);
    if (m_deferredLoadSeen.contains(key)) return;
    m_deferredLoadSeen.insert(key);
    m_deferredLoadQueue.append({path, size});
}

void MediaGridWidget::processDeferredCoverLoads() {
    // In Optimized Mode do not load any covers for grid cards.
    if (AppSettings::instance().isOptimizedMode()) {
        m_deferredLoadQueue.clear();
        m_deferredLoadTimer->stop();
        return;
    }
    int batch = qMin(kMaxConcurrentLoads, m_deferredLoadQueue.size());
    if (batch == 0) {
        m_deferredLoadTimer->stop();
        return;
    }

    for (int i = 0; i < batch; ++i) {
        if (m_deferredLoadQueue.isEmpty()) break;
        auto [path, size] = m_deferredLoadQueue.takeFirst();
        CoverLoader::instance().requestAsync(path, size);
    }

    if (m_deferredLoadQueue.isEmpty()) {
        m_deferredLoadTimer->stop();
    }
}

void MediaGridWidget::onItemClicked(QListWidgetItem* item) {
    if (!item) return;
    emit cardActivated(item->data(Qt::UserRole).toInt());
}

void MediaGridWidget::onItemDoubleClicked(QListWidgetItem* item) {
    if (!item) return;
    emit cardActivated(item->data(Qt::UserRole).toInt());
}

void MediaGridWidget::resizeEvent(QResizeEvent* event) {
    QListWidget::resizeEvent(event);
    // Defer the grid update to the next event-loop iteration so we don't
    // block the resize loop itself (which can fire 60+ times per second
    // during a window drag).
    QTimer::singleShot(0, this, &MediaGridWidget::updateGridResponsive);
}

void MediaGridWidget::showEvent(QShowEvent* event) {
    QListWidget::showEvent(event);
    QTimer::singleShot(0, this, &MediaGridWidget::updateGridResponsive);
}

void MediaGridWidget::setOptimizedMode(bool enabled) {
    if (enabled) {
        // Stop and flush the deferred cover-load queue immediately.
        if (m_deferredLoadTimer) m_deferredLoadTimer->stop();
        m_deferredLoadQueue.clear();
        m_deferredLoadSeen.clear();
    }
    // Show or hide the cover label on all existing cards.
    for (int i = 0; i < count(); ++i) {
        if (auto* it = item(i)) {
            if (auto* card = qobject_cast<MediaGridCard*>(itemWidget(it))) {
                card->setCoverOptimized(enabled);
            }
        }
    }
    // Kick off lazy cover reloads if switching back to Normal Mode.
    if (!enabled) {
        scheduleDeferredLoads();
    }
}
