#include "mainwindow.h"
#include "gui_bridge_p.h"
#include "playlistcreatedialog.h"
#include "sleeptimerdialog.h"
#include <QSettings>
#include <QMenu>
#include <QMessageBox>
#include <QInputDialog>
#include <QFileDialog>
#include <QFileInfo>

void MainWindow::setupSystemTray() {
    QSettings s;
    m_trayEnabled = s.value("tray_enabled", false).toBool();
    m_minimizeToTray = s.value("minimize_to_tray", false).toBool();

    m_trayIcon = new QSystemTrayIcon(QIcon(":/resources/icons/playtune_logo.png"), this);
    m_trayIcon->setToolTip("PlayTune");

    m_trayMenu = new QMenu(this);
    m_trayPlayPauseAction = m_trayMenu->addAction("Play/Pause");
    m_trayMenu->addSeparator();
    m_trayPrevAction = m_trayMenu->addAction("Previous");
    m_trayNextAction = m_trayMenu->addAction("Next");
    m_trayMenu->addSeparator();
    m_trayShowHideAction = m_trayMenu->addAction("Show / Hide");
    m_trayMenu->addSeparator();
    m_trayQuitAction = m_trayMenu->addAction("Quit");

    m_trayIcon->setContextMenu(m_trayMenu);

    const auto& cb = GuiBridgeManager::instance().callbacks();
    connect(m_trayPlayPauseAction, &QAction::triggered, this, [cb]() {
        if (cb.on_play_pause) cb.on_play_pause();
    });
    connect(m_trayPrevAction, &QAction::triggered, this, [cb]() {
        if (cb.on_prev) cb.on_prev();
    });
    connect(m_trayNextAction, &QAction::triggered, this, [cb]() {
        if (cb.on_next) cb.on_next();
    });
    connect(m_trayShowHideAction, &QAction::triggered, this, [this]() {
        if (isHidden()) {
            if (m_wasMaximizedBeforeMinimize) {
                showMaximized();
            } else if (m_wasFullScreenBeforeMinimize) {
                showFullScreen();
            } else {
                show();
            }
            raise();
            activateWindow();
        } else {
            hide();
        }
    });
    connect(m_trayQuitAction, &QAction::triggered, this, [this]() {
        m_minimizeToTray = false;
        close();
    });
    connect(m_trayIcon, &QSystemTrayIcon::activated, this,
            [this](QSystemTrayIcon::ActivationReason reason) {
        if (reason == QSystemTrayIcon::Trigger) {
            if (isHidden()) {
                if (m_wasMaximizedBeforeMinimize) {
                    showMaximized();
                } else if (m_wasFullScreenBeforeMinimize) {
                    showFullScreen();
                } else {
                    show();
                }
                raise();
                activateWindow();
            } else {
                hide();
            }
        }
    });

    if (m_trayEnabled) {
        m_trayIcon->show();
        if (!m_trayBalloonShown) {
            m_trayBalloonShown = true;
            m_trayIcon->showMessage("PlayTune",
                                    "Playing in the background. Right-click the tray icon for controls.",
                                    QSystemTrayIcon::Information, 3000);
        }
    }
}

void MainWindow::closeEvent(QCloseEvent* event) {
    if (m_minimizeToTray && m_trayEnabled) {
        event->ignore();
        hide();
        if (m_trayIcon) {
            m_trayIcon->showMessage("PlayTune",
                                    "Still playing in the background. Click the tray icon to restore.",
                                    QSystemTrayIcon::Information, 2500);
        }
        return;
    }
    event->accept();
}

void MainWindow::onPlaylistCreateRequested() {
    PlaylistCreateDialog dlg(PlaylistCreateDialog::Mode::Create, QString(), this);
    connect(&dlg, &PlaylistCreateDialog::nameSubmitted, this, [](const QString& name) {
        const auto& cb = GuiBridgeManager::instance().callbacks();
        if (cb.on_create_playlist) {
            QByteArray ba = name.toUtf8();
            cb.on_create_playlist(ba.constData());
        }
    });
    dlg.exec();
}

void MainWindow::onPlaylistRenameRequested(int playlist_id, const QString& current_name) {
    PlaylistCreateDialog dlg(PlaylistCreateDialog::Mode::Rename, current_name, this);
    connect(&dlg, &PlaylistCreateDialog::nameSubmitted, this, [playlist_id](const QString& name) {
        const auto& cb = GuiBridgeManager::instance().callbacks();
        if (cb.on_rename_playlist) {
            QByteArray ba = name.toUtf8();
            cb.on_rename_playlist(playlist_id, ba.constData());
        }
    });
    dlg.exec();
}

void MainWindow::onPlaylistDeleteRequested(int playlist_id) {
    QMessageBox::StandardButton reply = QMessageBox::question(
        this, "Delete Playlist",
        "Are you sure you want to delete this playlist? The tracks themselves will not be removed.",
        QMessageBox::Yes | QMessageBox::No, QMessageBox::No);
    if (reply == QMessageBox::Yes) {
        const auto& cb = GuiBridgeManager::instance().callbacks();
        if (cb.on_delete_playlist) cb.on_delete_playlist(playlist_id);
    }
}

void MainWindow::onSleepTimerRequested() {
    SleepTimerDialog dlg(this);
    connect(&dlg, &SleepTimerDialog::durationSelected, this, [](int minutes) {
        const auto& cb = GuiBridgeManager::instance().callbacks();
        if (cb.on_sleep_timer) cb.on_sleep_timer(minutes);
    });
    dlg.exec();
}

void MainWindow::onImportM3URequested() {
    QString path = QFileDialog::getOpenFileName(
        this, "Import Playlist", "",
        "Playlist Files (*.m3u *.m3u8);;All Files (*.*)");
    if (path.isEmpty()) return;
    const auto& cb = GuiBridgeManager::instance().callbacks();
    if (cb.on_import_m3u) {
        QByteArray pathBa = path.toUtf8();
        int result = cb.on_import_m3u(pathBa.constData(), nullptr);
        if (result) {
            showToast(QString("Importing playlist from: %1").arg(QFileInfo(path).fileName()));
        } else {
            showToast("Failed to import playlist.");
        }
    }
}

void MainWindow::onExportM3URequested() {
    const auto& playlists = GuiBridgeManager::instance().playlists();
    if (playlists.isEmpty()) {
        showToast("No playlists to export. Create one first.");
        return;
    }
    QStringList items;
    for (const auto& pl : playlists) {
        items << pl.name;
    }
    bool ok = false;
    QString chosen = QInputDialog::getItem(this, "Export Playlist",
                                            "Select a playlist to export:",
                                            items, 0, false, &ok);
    if (!ok || chosen.isEmpty()) return;
    int playlist_id = -1;
    for (const auto& pl : playlists) {
        if (pl.name == chosen) {
            playlist_id = pl.id;
            break;
        }
    }
    if (playlist_id < 0) return;

    QString path = QFileDialog::getSaveFileName(
        this, "Export Playlist", chosen + ".m3u8",
        "M3U8 Playlist (*.m3u8);;M3U Playlist (*.m3u);;All Files (*.*)");
    if (path.isEmpty()) return;
    const auto& cb = GuiBridgeManager::instance().callbacks();
    if (cb.on_export_m3u) {
        QByteArray pathBa = path.toUtf8();
        int result = cb.on_export_m3u(playlist_id, pathBa.constData());
        if (result) {
            showToast(QString("Exported playlist to: %1").arg(QFileInfo(path).fileName()));
        } else {
            showToast("Failed to export playlist.");
        }
    }
}
