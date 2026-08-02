#include "albumsview.h"

#include <QHBoxLayout>
#include <QVBoxLayout>
#include <QHeaderView>
#include <QListWidgetItem>
#include <QPixmap>
#include <QPainter>
#include <QPainterPath>
#include <QIcon>
#include <QFile>
#include <QFileInfo>
#include <QSettings>
#include <QStandardPaths>
#include <QDir>
#include <QTimer>
#include <QShowEvent>

#include "songstable.h"
#include "custom_widgets.h"  // getDefaultAlbumArt

AlbumsViewWidget::AlbumsViewWidget(QWidget* parent)
    : QWidget(parent) {
    setupUi();
}

void AlbumsViewWidget::setupUi() {
    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);

    m_stackedWidget = new QStackedWidget(this);

    // Page 0: album grid inside AlbumsCard frame.
    auto* page0Card = new QFrame(m_stackedWidget);
    page0Card->setObjectName("AlbumsCard");
    page0Card->setStyleSheet(
        "QFrame#AlbumsCard {"
        "   background-color: #0F121D;"
        "   border: 1px solid #1E2538;"
        "   border-radius: 16px;"
        "}"
    );

    auto* gridLayout = new QVBoxLayout(page0Card);
    gridLayout->setContentsMargins(16, 16, 16, 16);
    gridLayout->setSpacing(12);

    auto* titleLabel = new QLabel("Albums");
    titleLabel->setStyleSheet(
        "font-size: 22px; font-weight: 600; color: #F0F0F5; padding-bottom: 8px;");
    gridLayout->addWidget(titleLabel);

    // Use the shared MediaGridWidget so the look & behaviour is identical
    // to the Home tab's grid view (Issue #1 — Inconsistent Grid Views).
    m_albumGrid = new MediaGridWidget;
    gridLayout->addWidget(m_albumGrid);

    connect(m_albumGrid, &MediaGridWidget::cardActivated, this, [this](int album_id) {
        // Look up the album name from the item's user data.
        for (int i = 0; i < m_albumGrid->count(); ++i) {
            if (auto* item = m_albumGrid->item(i)) {
                if (item->data(Qt::UserRole).toInt() == album_id) {
                    QString name = item->data(Qt::UserRole + 1).toString();
                    showAlbumSongs(album_id, name);
                    emit albumSelected(album_id);
                    return;
                }
            }
        }
    });

    m_stackedWidget->addWidget(page0Card); // index 0

    // Page 1: songs in selected album (direct SongsTableWidget with header back button)
    m_albumSongsTable = new SongsTableWidget(m_stackedWidget);
    connect(m_albumSongsTable, &SongsTableWidget::songSelected, this, &AlbumsViewWidget::songSelected);
    connect(m_albumSongsTable, &SongsTableWidget::backButtonClicked, this, [this]() {
        showAlbumGrid();
        emit backToAlbumsClicked();
    });

    m_stackedWidget->addWidget(m_albumSongsTable); // index 1
    m_stackedWidget->setCurrentIndex(0);

    mainLayout->addWidget(m_stackedWidget);
}

void AlbumsViewWidget::clearAlbums() {
    m_albumGrid->clearGrid();
}

void AlbumsViewWidget::addAlbumRow(int album_id, const QString& name,
                                   const QString& artist, int track_count,
                                   double duration_secs, int year,
                                   const QString& cover_path) {
    Q_UNUSED(duration_secs);
    QString actualCoverPath = cover_path;
    if (actualCoverPath.isEmpty() || !QFile::exists(actualCoverPath)) {
        QString cover_dir = QStandardPaths::writableLocation(QStandardPaths::CacheLocation) + "/playtune/covers";
        QString fallback = QString("%1/%2.png").arg(cover_dir).arg(album_id);
        if (QFile::exists(fallback)) {
            actualCoverPath = fallback;
        }
    }

    // Build the subtitle: "Artist · Year · N tracks" (with empty parts
    // skipped). The MediaGridCard handles eliding and rendering.
    QString subtitle;
    if (!artist.isEmpty()) subtitle = artist;
    if (year > 0) {
        subtitle = subtitle.isEmpty() ? QString::number(year) : (subtitle + " · " + QString::number(year));
    }
    if (track_count > 0) {
        QString trStr = QString("%1 track%2").arg(track_count).arg(track_count == 1 ? "" : "s");
        subtitle = subtitle.isEmpty() ? trStr : (subtitle + " · " + trStr);
    }

    m_albumGrid->addCard(album_id, name, subtitle, actualCoverPath);
}

void AlbumsViewWidget::showAlbumGrid() {
    if (m_albumSongsTable) {
        m_albumSongsTable->setBackButtonVisible(false);
    }
    if (m_stackedWidget) {
        m_stackedWidget->setCurrentIndex(0);
    }
    QTimer::singleShot(0, this, &AlbumsViewWidget::updateGridResponsive);
}

void AlbumsViewWidget::showAlbumSongs(int /*album_id*/, const QString& /*album_name*/) {
    if (m_albumSongsTable) {
        m_albumSongsTable->setBackButtonVisible(true, "‹  Albums");
    }
    if (m_stackedWidget) {
        m_stackedWidget->setCurrentIndex(1);
    }
}

void AlbumsViewWidget::clearSongs() {
    if (m_albumSongsTable) m_albumSongsTable->clearSongs();
}

void AlbumsViewWidget::addSong(int index, int song_id, bool is_favorite, const QString& title,
                               const QString& artist, const QString& album,
                               const QString& duration, const QString& cover_path) {
    if (m_albumSongsTable) {
        m_albumSongsTable->addSong(index, song_id, is_favorite, title, artist, album, duration, cover_path);
    }
}

void AlbumsViewWidget::setPlayingState(bool playing) {
    if (m_albumSongsTable) m_albumSongsTable->setPlayingTrack(-2, playing);
}

void AlbumsViewWidget::setActiveIndex(int index, bool playing) {
    setPlayingSongId(index, playing);
}

void AlbumsViewWidget::setPlayingSongId(int songId, bool playing) {
    if (m_albumSongsTable) m_albumSongsTable->setPlayingSongId(songId, playing);
}

void AlbumsViewWidget::updateGridResponsive() {
    if (m_albumGrid) m_albumGrid->updateGridResponsive();
}

void AlbumsViewWidget::setOptimizedMode(bool enabled) {
    if (m_albumGrid) m_albumGrid->setOptimizedMode(enabled);
    if (m_albumSongsTable) m_albumSongsTable->setOptimizedMode(enabled);
}

void AlbumsViewWidget::resizeEvent(QResizeEvent* event) {
    QWidget::resizeEvent(event);
    QTimer::singleShot(0, this, &AlbumsViewWidget::updateGridResponsive);
}

void AlbumsViewWidget::showEvent(QShowEvent* event) {
    QWidget::showEvent(event);
    QTimer::singleShot(0, this, &AlbumsViewWidget::updateGridResponsive);
}
