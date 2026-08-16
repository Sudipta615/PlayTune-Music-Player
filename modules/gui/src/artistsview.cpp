#include "artistsview.h"
#include "apptheme.h"

#include <QHBoxLayout>
#include <QVBoxLayout>
#include <QListWidgetItem>
#include <QPixmap>
#include <QPainter>
#include <QPainterPath>
#include <QFile>
#include <QSettings>
#include <QStandardPaths>
#include <QFrame>
#include <QTimer>
#include <QShowEvent>

#include "songstable.h"
#include "custom_widgets.h"  // getDefaultAlbumArt
#include "coverloader.h"

ArtistsViewWidget::ArtistsViewWidget(QWidget* parent)
    : QWidget(parent) {
    setupUi();
}

void ArtistsViewWidget::setupUi() {
    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);

    m_stackedWidget = new QStackedWidget(this);

    // Page 0: Artist Grid inside ArtistsCard frame.
    auto* page0Card = new QFrame(m_stackedWidget);
    page0Card->setObjectName("ArtistsCard");

    auto* gridLayout = new QVBoxLayout(page0Card);
    gridLayout->setContentsMargins(16, 16, 16, 16);
    gridLayout->setSpacing(12);

    auto* titleLabel = new QLabel("Artists");
    titleLabel->setObjectName("ViewTitleLabel");
    gridLayout->addWidget(titleLabel);

    // Use the shared MediaGridWidget so the look & behaviour is identical
    // to the Home tab's grid view (Issue #1 — Inconsistent Grid Views).
    m_artistGrid = new MediaGridWidget;
    gridLayout->addWidget(m_artistGrid);

    connect(m_artistGrid, &MediaGridWidget::cardActivated, this, [this](int artist_id) {
        for (int i = 0; i < m_artistGrid->count(); ++i) {
            if (auto* item = m_artistGrid->item(i)) {
                if (item->data(Qt::UserRole).toInt() == artist_id) {
                    QString name = item->data(Qt::UserRole + 1).toString();
                    showArtistSongs(artist_id, name);
                    emit artistSelected(artist_id);
                    return;
                }
            }
        }
    });

    m_stackedWidget->addWidget(page0Card); // index 0

    // Page 1: Songs in selected Artist (direct SongsTableWidget with header back button)
    m_artistSongsTable = new SongsTableWidget(m_stackedWidget);
    connect(m_artistSongsTable, &SongsTableWidget::songSelected, this, &ArtistsViewWidget::songSelected);
    connect(m_artistSongsTable, &SongsTableWidget::backButtonClicked, this, [this]() {
        showArtistGrid();
        emit backToArtistsClicked();
    });

    m_stackedWidget->addWidget(m_artistSongsTable); // index 1
    m_stackedWidget->setCurrentIndex(0);

    mainLayout->addWidget(m_stackedWidget);
}

void ArtistsViewWidget::clearArtists() {
    if (m_artistGrid) m_artistGrid->clearGrid();
}

void ArtistsViewWidget::addArtistRow(int artist_id, const QString& name, int album_count, int track_count, const QString& cover_path) {
    if (!m_artistGrid) return;

    // Build the subtitle: "N albums · M tracks" (skipping empty parts).
    QString subtitle;
    if (album_count > 0) {
        subtitle = QString("%1 album%2").arg(album_count).arg(album_count == 1 ? "" : "s");
    }
    if (track_count > 0) {
        QString trStr = QString("%1 track%2").arg(track_count).arg(track_count == 1 ? "" : "s");
        subtitle = subtitle.isEmpty() ? trStr : (subtitle + " · " + trStr);
    }
    if (subtitle.isEmpty()) subtitle = "Artist";

    m_artistGrid->addCard(artist_id, name, subtitle, cover_path);
}

void ArtistsViewWidget::showArtistGrid() {
    if (m_artistSongsTable) {
        m_artistSongsTable->setBackButtonVisible(false);
    }
    if (m_stackedWidget) {
        m_stackedWidget->setCurrentIndex(0);
    }
    QTimer::singleShot(0, this, &ArtistsViewWidget::updateGridResponsive);
}

void ArtistsViewWidget::showArtistSongs(int /*artist_id*/, const QString& /*artist_name*/) {
    if (m_artistSongsTable) {
        m_artistSongsTable->setBackButtonVisible(true, "‹  Artists");
    }
    if (m_stackedWidget) {
        m_stackedWidget->setCurrentIndex(1);
    }
}

void ArtistsViewWidget::clearSongs() {
    if (m_artistSongsTable) m_artistSongsTable->clearSongs();
}

void ArtistsViewWidget::addSong(int index, int song_id, bool is_favorite, const QString& title,
                                const QString& artist, const QString& album,
                                const QString& duration, const QString& cover_path) {
    if (m_artistSongsTable) {
        m_artistSongsTable->addSong(index, song_id, is_favorite, title, artist, album, duration, cover_path);
    }
}

void ArtistsViewWidget::setPlayingState(bool playing) {
    if (m_artistSongsTable) m_artistSongsTable->setPlayingTrack(-2, playing);
}

void ArtistsViewWidget::setActiveIndex(int index, bool playing) {
    setPlayingSongId(index, playing);
}

void ArtistsViewWidget::setPlayingSongId(int songId, bool playing) {
    if (m_artistSongsTable) m_artistSongsTable->setPlayingSongId(songId, playing);
}

void ArtistsViewWidget::updateGridResponsive() {
    if (m_artistGrid) m_artistGrid->updateGridResponsive();
}

void ArtistsViewWidget::setOptimizedMode(bool enabled) {
    if (m_artistGrid) m_artistGrid->setOptimizedMode(enabled);
    if (m_artistSongsTable) m_artistSongsTable->setOptimizedMode(enabled);
}


void ArtistsViewWidget::resizeEvent(QResizeEvent* event) {
    QWidget::resizeEvent(event);
    QTimer::singleShot(0, this, &ArtistsViewWidget::updateGridResponsive);
}

void ArtistsViewWidget::showEvent(QShowEvent* event) {
    QWidget::showEvent(event);
    QTimer::singleShot(0, this, &ArtistsViewWidget::updateGridResponsive);
}
