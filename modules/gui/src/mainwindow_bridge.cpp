#include "mainwindow.h"
#include "gui_bridge_p.h"
#include "apptheme.h"
#include <QFileDialog>
#include <QFileInfo>
#include <QPixmapCache>
#include <QTimer>

void MainWindow::connectBridge() {
    const auto& cb = GuiBridgeManager::instance().callbacks();

    // CONNECT UI TRIGGERS TO RUST CALLBACKS
    // ------------------------------------
    
    // Now Playing Media Controls
    connect(m_nowPlayingCard, &NowPlayingCard::playPauseClicked, this, [cb]() {
        if (cb.on_play_pause) cb.on_play_pause();
    });
    connect(m_nowPlayingCard, &NowPlayingCard::prevClicked, this, [cb]() {
        if (cb.on_prev) cb.on_prev();
    });
    connect(m_nowPlayingCard, &NowPlayingCard::nextClicked, this, [cb]() {
        if (cb.on_next) cb.on_next();
    });
    connect(m_nowPlayingCard, &NowPlayingCard::seekRequested, this, [cb](double secs) {
        if (cb.on_seek) cb.on_seek(secs);
    });
    connect(m_nowPlayingCard, &NowPlayingCard::repeatClicked, this, [cb](bool checked) {
        if (cb.on_slider_param) cb.on_slider_param(5, checked ? 1.0 : 0.0); // param 5 = repeat toggle
    });
    connect(m_nowPlayingCard, &NowPlayingCard::shuffleClicked, this, [cb](bool checked) {
        if (cb.on_slider_param) cb.on_slider_param(6, checked ? 1.0 : 0.0); // param 6 = shuffle toggle
    });

    connect(m_nowPlayingCard, &NowPlayingCard::eqClicked, this, [this]() {
        if (m_eqWindow->isVisible()) {
            m_eqWindow->hide();
        } else {
            if ((m_eqWindow->pos().x() <= 0 && m_eqWindow->pos().y() <= 0) ||
                m_eqWindow->pos().x() + 100 > width() || m_eqWindow->pos().y() + 100 > height()) {
                int cx = qMax(0, (width() - m_eqWindow->width()) / 2);
                int cy = qMax(0, (height() - m_eqWindow->height()) / 2);
                m_eqWindow->move(cx, cy);
            }
            m_eqWindow->show();
            m_eqWindow->raise();
            m_eqWindow->activateWindow();
        }
    });

    // Speed slider + sleep timer button.
    connect(m_nowPlayingCard, &NowPlayingCard::speedChanged, this, [cb](double speed) {
        if (cb.on_set_speed) cb.on_set_speed(speed);
    });
    connect(m_nowPlayingCard, &NowPlayingCard::sleepTimerClicked, this, [this]() {
        onSleepTimerRequested();
    });

    connect(m_nowPlayingCard, &NowPlayingCard::editTagsClicked, this, [this]() {
        if (m_songsTable) {
            int playingIdx = m_songsTable->playingTrackIdx();
            if (playingIdx >= 0) {
                m_songsTable->openTagEditorDialog(playingIdx);
            } else {
                showToast(tr("No track is currently playing from the library."));
            }
        }
    });

    // EQ Panel internal changes -> notify Rust
    connect(m_eqWindow, &EqualizerWindow::eqToggled, this, [cb](bool enabled) {
        if (cb.on_eq_enabled) cb.on_eq_enabled(enabled ? 1 : 0);
    });
    connect(m_eqWindow, &EqualizerWindow::bandChanged, this, [cb](int idx, double db) {
        if (cb.on_eq_band) cb.on_eq_band(idx, db);
    });
    connect(m_eqWindow, &EqualizerWindow::presetSelected, this, [cb](int presetIdx) {
        if (cb.on_preset_selected) cb.on_preset_selected(presetIdx);
    });
    connect(m_eqWindow, &EqualizerWindow::resetEqClicked, this, [cb]() {
        if (cb.on_reset_eq) cb.on_reset_eq();
    });
    connect(m_eqWindow, &EqualizerWindow::sliderParamChanged, this, [cb](int paramIdx, double val) {
        if (cb.on_slider_param) cb.on_slider_param(paramIdx, val);
    });
    connect(m_eqWindow, &EqualizerWindow::advancedBandChanged, this, [cb](int idx, double freq, double gainDb, double q, int ftype) {
        if (cb.on_eq_advanced_band) cb.on_eq_advanced_band(idx, freq, gainDb, q, ftype);
    });
    connect(m_eqWindow, &EqualizerWindow::resamplerQualityChanged, this, [cb](int quality) {
        if (cb.on_set_resampler_quality) cb.on_set_resampler_quality(quality);
    });
    m_eqWindow->emitInitialState();

    // Songs Table Item Selection
    connect(m_songsTable, &SongsTableWidget::songSelected, this, [cb](int index) {
        if (cb.on_select_song) cb.on_select_song(index);
    });

    // Queue Panel triggers
    connect(m_queueWidget, &QueueWidget::clearQueueClicked, this, [cb]() {
        if (cb.on_clear_queue) cb.on_clear_queue();
    });
    connect(m_queueWidget, &QueueWidget::volumeChanged, this, [this, cb](double vol) {
        m_currentVolume = vol;  // keep keyboard shortcut state in sync
        m_isMuted = (vol == 0.0);
        if (vol > 0.0) {
            m_volumeBeforeMute = vol;
        }
        if (cb.on_volume) cb.on_volume(vol);
    });
    connect(m_queueWidget, &QueueWidget::queueSongSelected, this, [cb](int index) {
        // Map queue song selection directly to player selection
        if (cb.on_select_song) cb.on_select_song(index);
    });
    connect(m_queueWidget, &QueueWidget::seekRequested, this, [cb](double secs) {
        if (cb.on_seek) cb.on_seek(secs);
    });
    connect(m_queueWidget, &QueueWidget::toggleRightSidebarRequested, this, [this]() {
        bool visible = !m_queueWidget->isVisible();
        m_queueHiddenByUser = !visible;
        if (m_contentStack && m_contentStack->currentIndex() != 1) {
            m_queueWidget->setVisible(visible);
            if (m_sep2) m_sep2->setVisible(visible);
            if (m_toggleRightTopBtn) m_toggleRightTopBtn->setVisible(!visible);
        }
    });

    // Navigation Clicks
    connect(m_sidebar, &SidebarWidget::homeClicked, this, [this, cb]() {
        m_contentStack->setCurrentIndex(0);
        if (cb.on_nav_tab) cb.on_nav_tab(0);
    });
    connect(m_sidebar, &SidebarWidget::albumsClicked, this, [this, cb]() {
        m_contentStack->setCurrentIndex(3);
        if (cb.on_nav_tab) cb.on_nav_tab(1);
    });
    connect(m_sidebar, &SidebarWidget::artistsClicked, this, [this, cb]() {
        m_contentStack->setCurrentIndex(4);
        if (cb.on_nav_tab) cb.on_nav_tab(2);
    });
    connect(m_sidebar, &SidebarWidget::foldersClicked, this, [this, cb]() {
        m_contentStack->setCurrentIndex(2);
        if (cb.on_nav_tab) cb.on_nav_tab(3);
    });
    connect(m_sidebar, &SidebarWidget::settingsClicked, this, [this, cb]() {
        m_contentStack->setCurrentIndex(1);
        if (cb.on_nav_tab) cb.on_nav_tab(4);
    });
    connect(m_sidebar, &SidebarWidget::favoritesClicked, this, [this, cb]() {
        m_contentStack->setCurrentIndex(0);
        if (cb.on_nav_tab) cb.on_nav_tab(5);
    });
    connect(m_sidebar, &SidebarWidget::recentlyPlayedClicked, this, [this, cb]() {
        m_contentStack->setCurrentIndex(0);
        if (cb.on_nav_tab) cb.on_nav_tab(6);
    });
    connect(m_sidebar, &SidebarWidget::mostPlayedClicked, this, [this, cb]() {
        m_contentStack->setCurrentIndex(0);
        if (cb.on_nav_tab) cb.on_nav_tab(7);
    });

    // Settings Page Actions
    connect(m_settingsPage, &SettingsPageWidget::addSongsRequested, this, [this, cb]() {
        QStringList files = QFileDialog::getOpenFileNames(
            this, "Select Audio Tracks", "", "Audio Files (*.mp3 *.wav *.flac *.ogg *.m4a *.aac)"
        );
        if (!files.isEmpty() && cb.on_import_files) {
            int count = files.size();
            QVector<QByteArray> utf8Paths;
            QVector<const char*> cPaths;
            for (const QString& f : files) {
                utf8Paths.append(f.toUtf8());
            }
            for (const QByteArray& ba : utf8Paths) {
                cPaths.append(ba.constData());
            }
            cb.on_import_files(cPaths.constData(), cPaths.size());
            showToast(QString("\u2705 Importing %1 track%2 into library...").arg(count).arg(count == 1 ? "" : "s"));
        }
    });
    connect(m_settingsPage, &SettingsPageWidget::addFoldersRequested, this, [this, cb]() {
        QString dir = QFileDialog::getExistingDirectory(this, "Select Music Folder");
        if (!dir.isEmpty() && cb.on_import_folder) {
            QByteArray ba = dir.toUtf8();
            cb.on_import_folder(ba.constData());
            showToast(QString("\u2705 Scanning folder: ") + QFileInfo(dir).fileName());
        }
    });



    connect(m_settingsPage, &SettingsPageWidget::tooltipsToggled, this, [this](bool enabled) {
        if (m_toolTipController) {
            m_toolTipController->setEnabled(enabled);
        }
    });
    connect(m_settingsPage, &SettingsPageWidget::moodColumnToggled,
            m_songsTable, &SongsTableWidget::setMoodColumnVisible);
    m_songsTable->setMoodColumnVisible(m_settingsPage->isMoodColumnEnabled());
    connect(m_settingsPage, &SettingsPageWidget::gaplessToggled, this, [cb](bool enabled) {
        if (cb.on_crossfade_toggled) cb.on_crossfade_toggled(enabled ? 0 : 1);
    });
    connect(m_settingsPage, &SettingsPageWidget::crossfadeToggled, this, [cb](bool enabled) {
        if (cb.on_crossfade_toggled) cb.on_crossfade_toggled(enabled ? 1 : 0);
    });
    connect(m_settingsPage, &SettingsPageWidget::crossfadeDurationChanged, this, [cb](int ms) {
        if (cb.on_set_crossfade_duration) cb.on_set_crossfade_duration(ms);
    });
    connect(m_settingsPage, &SettingsPageWidget::cursorFollowsToggled, this, [cb](bool enabled) {
        if (cb.on_cursor_follows_playback) cb.on_cursor_follows_playback(enabled ? 1 : 0);
    });
    connect(m_settingsPage, &SettingsPageWidget::notificationsToggled, this, [cb](bool enabled) {
        if (cb.on_notifications_toggled) cb.on_notifications_toggled(enabled ? 1 : 0);
    });
    connect(m_settingsPage, &SettingsPageWidget::trayToggled, this, [this, cb](bool enabled) {
        if (cb.on_tray_toggled) cb.on_tray_toggled(enabled ? 1 : 0);
        m_trayEnabled = enabled;
        if (m_trayIcon) {
            if (enabled) {
                m_trayIcon->show();
                if (!m_trayBalloonShown) {
                    m_trayBalloonShown = true;
                    m_trayIcon->showMessage("PlayTune",
                                            "Now playing in the background. Right-click for controls.",
                                            QSystemTrayIcon::Information, 3000);
                }
            } else {
                m_trayIcon->hide();
            }
        }
    });
    connect(m_settingsPage, &SettingsPageWidget::minimizeToTrayToggled, this, [this, cb](bool enabled) {
        if (cb.on_minimize_to_tray_toggled) cb.on_minimize_to_tray_toggled(enabled ? 1 : 0);
        m_minimizeToTray = enabled;
    });
    connect(m_settingsPage, &SettingsPageWidget::importM3URequested, this, [this]() {
        onImportM3URequested();
    });
    connect(m_settingsPage, &SettingsPageWidget::exportM3URequested, this, [this]() {
        onExportM3URequested();
    });
    connect(m_settingsPage, &SettingsPageWidget::outputBackendChanged, this, [cb](int backend) {
        if (cb.on_set_output_backend) cb.on_set_output_backend(backend);
    });
    if (m_toolTipController && m_settingsPage) {
        m_toolTipController->setEnabled(m_settingsPage->isTooltipsEnabled());
    }

    // Live signal wiring for Optimized Mode
    connect(m_settingsPage, &SettingsPageWidget::optimizedModeToggled,
            m_nowPlayingCard,  &NowPlayingCard::setOptimizedMode);
    connect(m_settingsPage, &SettingsPageWidget::optimizedModeToggled,
            m_songsTable,      &SongsTableWidget::setOptimizedMode);
    connect(m_settingsPage, &SettingsPageWidget::optimizedModeToggled,
            m_queueWidget,     &QueueWidget::setOptimizedMode);
    connect(m_settingsPage, &SettingsPageWidget::optimizedModeToggled,
            m_albumsView,      &AlbumsViewWidget::setOptimizedMode);
    connect(m_settingsPage, &SettingsPageWidget::optimizedModeToggled,
            m_artistsView,     &ArtistsViewWidget::setOptimizedMode);
    connect(m_settingsPage, &SettingsPageWidget::optimizedModeToggled,
            this, [cb](bool on) {
        if (on && cb.on_eq_enabled) cb.on_eq_enabled(0);
    });

    if (m_settingsPage->isOptimizedMode()) {
        QTimer::singleShot(0, this, [this, cb]() {
            m_nowPlayingCard->setOptimizedMode(true);
            m_songsTable->setOptimizedMode(true);
            m_queueWidget->setOptimizedMode(true);
            m_albumsView->setOptimizedMode(true);
            m_artistsView->setOptimizedMode(true);
            if (cb.on_eq_enabled) cb.on_eq_enabled(0);
            QPixmapCache::setCacheLimit(2 * 1024);
        });
    }

    // Folders View Actions
    connect(m_foldersView, &FoldersViewWidget::folderSelected, this, [cb](int folderId) {
        if (cb.on_filter_folder) cb.on_filter_folder(folderId);
    });
    connect(m_foldersView, &FoldersViewWidget::backToFoldersClicked, this, [cb]() {
        if (cb.on_nav_tab) cb.on_nav_tab(3);
    });
    connect(m_foldersView, &FoldersViewWidget::songSelected, this, [cb](int index) {
        if (cb.on_select_song) cb.on_select_song(index);
    });
    connect(m_foldersView, &FoldersViewWidget::deleteFolderRequested, this, [cb](int folderId) {
        if (cb.on_delete_folder) cb.on_delete_folder(folderId);
    });
    connect(m_settingsPage, &SettingsPageWidget::deleteFolderRequested, this, [cb](int folderId) {
        if (cb.on_delete_folder) cb.on_delete_folder(folderId);
    });

    m_searchTimer = new QTimer(this);
    m_searchTimer->setSingleShot(true);
    m_searchTimer->setInterval(250);
    connect(m_searchTimer, &QTimer::timeout, this, [this, cb]() {
        if (cb.on_search && m_searchBar) {
            QByteArray ba = m_searchBar->text().toUtf8();
            cb.on_search(ba.constData());
        }
    });
    connect(m_searchBar, &QLineEdit::textChanged, this, [this](const QString&) {
        if (m_searchTimer) {
            m_searchTimer->start();
        }
    });
    connect(m_searchBar, &QLineEdit::returnPressed, this, [this, cb]() {
        if (m_searchTimer) {
            m_searchTimer->stop();
        }
        if (cb.on_search && m_searchBar) {
            QByteArray ba = m_searchBar->text().toUtf8();
            cb.on_search(ba.constData());
        }
    });

    // Manager Signals
    auto& manager = GuiBridgeManager::instance();

    connect(&manager, &GuiBridgeManager::playStateChanged, this, [this](bool playing) {
        m_nowPlayingCard->setPlayState(playing);
        int songId = m_songsTable ? m_songsTable->playingSongId() : -1;
        if (m_songsTable) m_songsTable->setPlayingSongId(songId, playing);
        if (m_foldersView) m_foldersView->setPlayingSongId(songId, playing);
        if (m_albumsView) m_albumsView->setPlayingSongId(songId, playing);
        if (m_artistsView) m_artistsView->setPlayingSongId(songId, playing);
    }, Qt::QueuedConnection);

    connect(&manager, &GuiBridgeManager::activeIndexChanged, this, [this](int songId) {
        bool playing = m_nowPlayingCard ? m_nowPlayingCard->isPlaying() : false;
        if (m_songsTable) m_songsTable->setPlayingSongId(songId, playing);
        if (m_foldersView) m_foldersView->setPlayingSongId(songId, playing);
        if (m_albumsView) m_albumsView->setPlayingSongId(songId, playing);
        if (m_artistsView) m_artistsView->setPlayingSongId(songId, playing);
    }, Qt::QueuedConnection);

    connect(&manager, &GuiBridgeManager::progressChanged, this, [this](double elapsed, double total) {
        m_nowPlayingCard->setPlaybackProgress(elapsed, total);
        if (m_queueWidget) m_queueWidget->updatePlaybackProgress(elapsed);
    }, Qt::QueuedConnection);

    connect(&manager, &GuiBridgeManager::trackChanged, this, [this](const QString& title, const QString& artist, const QString& album, const QString& coverPath) {
        m_nowPlayingCard->setTrackInfo(title, artist, album, coverPath);
        m_queueWidget->setTrackInfo(title, artist, album, coverPath);
    }, Qt::QueuedConnection);

    connect(&manager, &GuiBridgeManager::trackMetadataUpdated, this, &MainWindow::onTrackMetadataUpdated, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::trackLyricsUpdated, m_queueWidget, &QueueWidget::setTrackLyrics, Qt::QueuedConnection);

    connect(&manager, &GuiBridgeManager::songsCleared, this, [this]() {
        SongsTableWidget* t = activeSongsTable();
        if (t) t->clearSongs();
    }, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::songAdded, this, [this](
        int index, int song_id, bool is_favorite,
        const QString& title, const QString& artist,
        const QString& album, const QString& duration,
        const QString& cover_path) {
        SongsTableWidget* t = activeSongsTable();
        if (t) t->addSong(index, song_id, is_favorite, title, artist, album, duration, cover_path);
    }, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::songsBatchReplaced, this, [this](QVector<SongRow> rows) {
        SongsTableWidget* t = activeSongsTable();
        if (t) t->setSongsBatch(std::move(rows));
    }, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::trackRatingUpdated, this, [this](int track_id, int rating) {
        if (m_songsTable) m_songsTable->setRatingForRow(track_id, rating);
    }, Qt::QueuedConnection);

    connect(&manager, &GuiBridgeManager::foldersCleared, m_foldersView, &FoldersViewWidget::clearFolders, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::folderAdded, m_foldersView, &FoldersViewWidget::addFolderRow, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::viewSwitched, this, [this](int idx) {
        if (idx >= 0 && idx < m_contentStack->count()) {
            m_contentStack->setCurrentIndex(idx);
        }
    }, Qt::QueuedConnection);

    connect(&manager, &GuiBridgeManager::queueCleared, m_queueWidget, &QueueWidget::clearQueue, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::queueUpdateBegan, m_queueWidget, &QueueWidget::beginQueueUpdate, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::queueUpdateEnded, m_queueWidget, &QueueWidget::endQueueUpdate, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::queueSongAdded, m_queueWidget, &QueueWidget::addQueueSong, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::visualizerUpdated, m_nowPlayingCard, &NowPlayingCard::updateVisualizer, Qt::QueuedConnection);

    connect(&manager, &GuiBridgeManager::foldersCleared, m_settingsPage, &SettingsPageWidget::clearFolderList, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::folderAdded, m_settingsPage, &SettingsPageWidget::addFolderToList, Qt::QueuedConnection);

    connect(m_sidebar, &SidebarWidget::collapsedToggled, this, [this](bool collapsed) {
        if (!m_inResizeEvent) {
            m_sidebarCollapsedByUser = collapsed;
        }
    });

    connect(m_sidebar, &SidebarWidget::addPlaylistClicked, this, [this]() {
        onPlaylistCreateRequested();
    });

    connect(m_sidebar, &SidebarWidget::playlistSelected, this, [this, cb](int playlist_id) {
        m_contentStack->setCurrentIndex(0);
        if (cb.on_filter_playlist) cb.on_filter_playlist(playlist_id);
    });

    connect(m_sidebar, &SidebarWidget::playlistRenameRequested, this,
            [this](int playlist_id, const QString& current_name) {
        onPlaylistRenameRequested(playlist_id, current_name);
    });
    connect(m_sidebar, &SidebarWidget::playlistDeleteRequested, this, [this, cb](int playlist_id) {
        onPlaylistDeleteRequested(playlist_id);
        Q_UNUSED(cb);
    });

    connect(m_albumsView, &AlbumsViewWidget::albumSelected, this, [cb](int album_id) {
        if (cb.on_filter_album) cb.on_filter_album(album_id);
    });
    connect(m_albumsView, &AlbumsViewWidget::backToAlbumsClicked, this, [this, cb]() {
        m_contentStack->setCurrentIndex(3);
        if (cb.on_nav_tab) cb.on_nav_tab(1);
    });
    connect(m_albumsView, &AlbumsViewWidget::songSelected, this, [cb](int index) {
        if (cb.on_select_song) cb.on_select_song(index);
    });

    connect(m_artistsView, &ArtistsViewWidget::artistSelected, this, [cb](int artist_id) {
        if (cb.on_filter_artist) cb.on_filter_artist(artist_id);
    });
    connect(m_artistsView, &ArtistsViewWidget::backToArtistsClicked, this, [this, cb]() {
        m_contentStack->setCurrentIndex(4);
        if (cb.on_nav_tab) cb.on_nav_tab(2);
    });
    connect(m_artistsView, &ArtistsViewWidget::songSelected, this, [cb](int index) {
        if (cb.on_select_song) cb.on_select_song(index);
    });

    connect(&manager, &GuiBridgeManager::playlistsCleared, m_sidebar,
            &SidebarWidget::clearPlaylists, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::playlistAdded, m_sidebar,
            &SidebarWidget::addPlaylistRow, Qt::QueuedConnection);

    connect(&manager, &GuiBridgeManager::albumsCleared, m_albumsView,
            &AlbumsViewWidget::clearAlbums, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::albumAdded, m_albumsView,
            &AlbumsViewWidget::addAlbumRow, Qt::QueuedConnection);

    connect(&manager, &GuiBridgeManager::artistsCleared, m_artistsView,
            &ArtistsViewWidget::clearArtists, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::artistAdded, m_artistsView,
            &ArtistsViewWidget::addArtistRow, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::albumsInArtistCleared, m_artistsView,
            &ArtistsViewWidget::clearAlbumsInArtist, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::albumInArtistAdded, m_artistsView,
            &ArtistsViewWidget::addAlbumInArtist, Qt::QueuedConnection);

    connect(&manager, &GuiBridgeManager::speedLabelChanged, m_nowPlayingCard,
            &NowPlayingCard::setSpeedLabel, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::sleepTimerRemainingChanged, m_nowPlayingCard,
            &NowPlayingCard::setSleepTimerRemaining, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::scrollSongsTableToActiveRequested, m_songsTable,
            &SongsTableWidget::scrollToActive, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::desktopNotificationRequested, this,
            [this](const QString& title, const QString& body) {
        showToast(title + (body.isEmpty() ? QString() : (QString(" — ") + body)));
    });
    connect(&manager, &GuiBridgeManager::trayMessageRequested, this,
            [this](const QString& title, const QString& body) {
        if (m_trayIcon && m_trayEnabled) {
            m_trayIcon->showMessage(title, body,
                                    QSystemTrayIcon::Information, 4000);
        }
    });

    connect(m_contentStack, &QStackedWidget::currentChanged, this, [this](int idx) {
        updateLayoutForCurrentTab(idx);
        int songId = m_songsTable ? m_songsTable->playingSongId() : -1;
        bool playing = m_nowPlayingCard ? m_nowPlayingCard->isPlaying() : false;
        if (songId > 0) {
            if (m_songsTable) m_songsTable->setPlayingSongId(songId, playing);
            if (m_foldersView) m_foldersView->setPlayingSongId(songId, playing);
            if (m_albumsView) m_albumsView->setPlayingSongId(songId, playing);
            if (m_artistsView) m_artistsView->setPlayingSongId(songId, playing);
        }
    });

    setupSystemTray();
}

void MainWindow::onTrackMetadataUpdated(int track_id, const QString& title, const QString& artist, const QString& album, const QString& duration_str, const QString& cover_path) {
    if (m_songsTable) {
        m_songsTable->updateTrackRow(track_id, title, artist, album, duration_str, cover_path);
        if (m_songsTable->playingSongId() == track_id) {
            m_nowPlayingCard->setTrackInfo(title, artist, album, cover_path);
            m_queueWidget->setTrackInfo(title, artist, album, cover_path);
        }
    }
    if (m_foldersView) {
        m_foldersView->updateTrackRow(track_id, title, artist, album, duration_str, cover_path);
    }
    showToast(tr("Metadata tags updated for \"%1\"").arg(title));
}
