#include "songstable.h"
#include "gui_bridge_p.h"
#include "coverloader.h"
#include "tageditordialog.h"
#include "loudnessscannerdialog.h"
#include <QSettings>
#include <QEvent>
#include <QCursor>
#include <QMenu>
#include <QAction>
#include <QTimer>
#include <QDebug>

void SongsTableWidget::setPlayingSongId(int songId, bool playing) {
    m_playingSongId = songId;
    m_isPlaying = playing;

    int newPlayingRow = m_songIdToRow.value(songId, -1);

    int oldRow = m_playingTrackIdx;
    m_playingTrackIdx = newPlayingRow;

    if (oldRow >= 0 && oldRow < m_table->rowCount()) {
        if (oldRow < m_eqIcons.size()) {
            m_eqIcons[oldRow]->setVisible(false);
            m_eqIcons[oldRow]->setPlaying(false);
        }
        if (auto* titleCont = m_table->cellWidget(oldRow, 1)) {
            if (auto* titleLabel = titleCont->findChild<QLabel*>("SongTitleLabel")) {
                titleLabel->setStyleSheet("font-weight: 500; font-size: 13px;");
            }
        }
        refreshSingleRowStyle(oldRow);
    }

    if (m_playingTrackIdx >= 0 && m_playingTrackIdx < m_table->rowCount()) {
        if (m_playingTrackIdx < m_eqIcons.size()) {
            m_eqIcons[m_playingTrackIdx]->setVisible(true);
            m_eqIcons[m_playingTrackIdx]->setPlaying(m_isPlaying);
        }
        if (auto* titleCont = m_table->cellWidget(m_playingTrackIdx, 1)) {
            if (auto* titleLabel = titleCont->findChild<QLabel*>("SongTitleLabel")) {
                const auto& p = ThemeManager::instance().currentTheme();
                titleLabel->setStyleSheet(QString("font-weight: bold; font-size: 13px; color: %1;").arg(p.secondaryAccent.name()));
            }
        }
        refreshSingleRowStyle(m_playingTrackIdx);
    }

    if (m_gridPopulated) {
        m_gridWidget->setPlayingIndex(m_playingTrackIdx);
    }

    if (m_playingTrackIdx >= 0 && m_playingTrackIdx < m_table->rowCount()) {
        QSettings s;
        bool cursorFollows = s.value("cursor_follows_playback", false).toBool();
        if (cursorFollows) {
            if (QTableWidgetItem* it = m_table->item(m_playingTrackIdx, 0)) {
                m_table->scrollToItem(it, QAbstractItemView::EnsureVisible);
            }
        }
    }
}

void SongsTableWidget::setPlayingTrack(int trackIdx, bool playing) {
    if (trackIdx == -2) {
        setPlayingSongId(m_playingSongId, playing);
        return;
    }
    if (trackIdx >= 0 && trackIdx < m_table->rowCount()) {
        if (QTableWidgetItem* first = m_table->item(trackIdx, 0)) {
            int songId = first->data(Qt::UserRole).toInt();
            setPlayingSongId(songId, playing);
            return;
        }
    }
    setPlayingSongId(-1, playing);
}

bool SongsTableWidget::eventFilter(QObject* watched, QEvent* event) {
    if (watched == m_table->viewport()) {
        if (event->type() == QEvent::Leave) {
            QPoint localPos = m_table->mapFromGlobal(QCursor::pos());
            if (!m_table->rect().contains(localPos)) {
                int prev = m_hoveredRow;
                m_hoveredRow = -1;
                if (prev >= 0) refreshSingleRowStyle(prev);
            }
        }
    } else if (watched) {
        QVariant rowProp = watched->property("rowIdx");
        if (rowProp.isValid()) {
            int r = rowProp.toInt();
            int c = watched->property("colIdx").toInt();
            if (event->type() == QEvent::Enter) {
                m_table->clearSelection();
                onCellEntered(r, c);
            } else if (event->type() == QEvent::MouseButtonPress || event->type() == QEvent::MouseButtonRelease) {
                m_table->clearSelection();
            }
        }
    }
    return QWidget::eventFilter(watched, event);
}

void SongsTableWidget::setMoodColumnVisible(bool visible) {
    if (m_table) {
        m_table->setColumnHidden(2, !visible);
    }
}

void SongsTableWidget::setResponsiveWidth(int width) {
    if (m_table) {
        if (width < 500) {
            m_table->setColumnHidden(4, true);  // Album
            m_table->setColumnHidden(5, true);  // Duration
            m_table->setColumnHidden(7, true);  // 3 dots
        } else if (width < 680) {
            m_table->setColumnHidden(4, true);  // Album
            m_table->setColumnHidden(5, false); // Duration
            m_table->setColumnHidden(7, false); // 3 dots
        } else {
            m_table->setColumnHidden(4, false); // Album
            m_table->setColumnHidden(5, false); // Duration
            m_table->setColumnHidden(7, false); // 3 dots
        }
    }
}

void SongsTableWidget::openTagEditorDialog(int row) {
    if (row < 0 || row >= m_table->rowCount()) return;
    QTableWidgetItem* firstItem = m_table->item(row, 0);
    if (!firstItem) return;

    int songId = firstItem->data(Qt::UserRole).toInt();
    QString coverPath = firstItem->data(Qt::UserRole + 1).toString();

    QString title;
    if (QWidget* titleCont = m_table->cellWidget(row, 1)) {
        if (auto* lbl = titleCont->findChild<QLabel*>("SongTitleLabel")) {
            title = lbl->text();
        }
    }
    if (title.isEmpty() && m_table->item(row, 1)) {
        title = m_table->item(row, 1)->text();
    }

    QString artist = m_table->item(row, 3) ? m_table->item(row, 3)->text() : "";
    QString album = m_table->item(row, 4) ? m_table->item(row, 4)->text() : "";

    char titleBuf[512] = {0};
    char artistBuf[512] = {0};
    char albumBuf[512] = {0};
    char albumArtistBuf[512] = {0};
    char genreBuf[512] = {0};
    char coverBuf[1024] = {0};
    unsigned int year = 0, trackNum = 0, discNum = 0;

    int res = playtune_get_track_tags(
        songId,
        titleBuf, sizeof(titleBuf),
        artistBuf, sizeof(artistBuf),
        albumBuf, sizeof(albumBuf),
        albumArtistBuf, sizeof(albumArtistBuf),
        genreBuf, sizeof(genreBuf),
        &year, &trackNum, &discNum,
        coverBuf, sizeof(coverBuf)
    );

    TagEditorTrackData data;
    data.track_id = songId;
    if (res == 1) {
        data.title = titleBuf[0] ? QString::fromUtf8(titleBuf) : title;
        data.artist = artistBuf[0] ? QString::fromUtf8(artistBuf) : artist;
        data.album = albumBuf[0] ? QString::fromUtf8(albumBuf) : album;
        data.album_artist = QString::fromUtf8(albumArtistBuf);
        data.genre = QString::fromUtf8(genreBuf);
        data.year = year;
        data.track_number = trackNum;
        data.disc_number = discNum;
        data.cover_path = coverBuf[0] ? QString::fromUtf8(coverBuf) : coverPath;
    } else {
        data.title = title;
        data.artist = artist;
        data.album = album;
        data.album_artist = "";
        data.genre = "";
        data.year = 0;
        data.track_number = 0;
        data.disc_number = 0;
        data.cover_path = coverPath;
    }

    TagEditorDialog dlg(data, this);
    dlg.exec();
}

void SongsTableWidget::openLoudnessScannerDialog(const QVector<int>& trackIds) {
    LoudnessScannerDialog dlg(trackIds, this);
    dlg.exec();
}

void SongsTableWidget::updateTrackRow(int songId, const QString& title, const QString& artist, const QString& album, const QString& duration, const QString& coverPath) {
    for (int row = 0; row < m_table->rowCount(); ++row) {
        QTableWidgetItem* firstItem = m_table->item(row, 0);
        if (firstItem && firstItem->data(Qt::UserRole).toInt() == songId) {
            firstItem->setData(Qt::UserRole + 1, coverPath);

            if (QWidget* titleCont = m_table->cellWidget(row, 1)) {
                if (auto* lbl = titleCont->findChild<QLabel*>("SongTitleLabel")) {
                    lbl->setText(title);
                }
                const auto labels = titleCont->findChildren<QLabel*>();
                for (QLabel* l : labels) {
                    if (l->objectName() != "SongTitleLabel") {
                        l->setPixmap(getThumbnail(coverPath));
                        break;
                    }
                }
            }

            if (QTableWidgetItem* artItem = m_table->item(row, 3)) artItem->setText(artist);
            if (QTableWidgetItem* albItem = m_table->item(row, 4)) albItem->setText(album);
            if (!duration.isEmpty()) {
                if (QTableWidgetItem* durItem = m_table->item(row, 5)) durItem->setText(duration);
            }
            break;
        }
    }

    for (int i = 0; i < m_rows.size(); ++i) {
        if (m_rows[i].songId == songId) {
            m_rows[i].title = title;
            m_rows[i].artist = artist;
            m_rows[i].album = album;
            m_rows[i].duration = duration;
            m_rows[i].coverPath = coverPath;
            break;
        }
    }

    if (m_gridPopulated) {
        m_gridWidget->clearGrid();
        populateGridFromTable();
    }
}

void SongsTableWidget::populateAddToPlaylistMenu(QMenu* menu, int songId) {
    const auto& playlists = GuiBridgeManager::instance().playlists();
    if (playlists.isEmpty()) {
        auto* noPlaylistsAction = menu->addAction(tr("(No playlists yet)"));
        noPlaylistsAction->setEnabled(false);
        return;
    }
    for (const auto& pl : playlists) {
        QString label = pl.name;
        if (pl.track_count > 0) {
            label += QString("  (%1)").arg(pl.track_count);
        }
        auto* act = menu->addAction(label);
        connect(act, &QAction::triggered, this, [songId, pl]() {
            const auto& cb = GuiBridgeManager::instance().callbacks();
            if (cb.on_add_track_to_playlist) {
                cb.on_add_track_to_playlist(pl.id, songId);
            }
        });
    }
}

void SongsTableWidget::setRatingForRow(int songId, int rating) {
    Q_UNUSED(songId);
    Q_UNUSED(rating);
}

void SongsTableWidget::scrollToActive() {
    if (m_playingTrackIdx >= 0 && m_playingTrackIdx < m_table->rowCount()) {
        if (QTableWidgetItem* it = m_table->item(m_playingTrackIdx, 0)) {
            m_table->scrollToItem(it, QAbstractItemView::EnsureVisible);
        }
    }
}

void SongsTableWidget::setBackButtonVisible(bool visible, const QString& text) {
    if (m_backBtn) {
        if (!text.isEmpty()) {
            m_backBtn->setText(text);
        }
        m_backBtn->setVisible(visible);
    }
}

void SongsTableWidget::resizeEvent(QResizeEvent* event) {
    QWidget::resizeEvent(event);
    QTimer::singleShot(0, this, [this]() {
        if (m_gridWidget) m_gridWidget->updateGridResponsive();
    });
}

void SongsTableWidget::showEvent(QShowEvent* event) {
    QWidget::showEvent(event);
    QTimer::singleShot(0, this, [this]() {
        if (m_gridWidget) m_gridWidget->updateGridResponsive();
        loadVisibleThumbnails();
        if (m_playingSongId != -1) {
            setPlayingSongId(m_playingSongId, m_isPlaying);
        }
    });
}
