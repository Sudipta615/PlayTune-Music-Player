#include "mainwindow.h"
#include "gui_bridge_p.h"
#include "apptheme.h"
#include <QHBoxLayout>
#include <QVBoxLayout>
#include <QFile>
#include <QIcon>
#include <QImage>
#include <QSize>
#include <QApplication>
#include <QScreen>
#include <QCoreApplication>
#include <QTimer>
#include <QFileDialog>
#include <QShortcut>
#include <QSlider>
#include <QDebug>
#include <QEvent>
#include <QToolTip>
#include <QDialog>
#include <QSettings>
#include <QPropertyAnimation>
#include <QGraphicsOpacityEffect>
#include <QFileInfo>
#include <QResizeEvent>
#include <QInputDialog>
#include <QMessageBox>
#include <QCloseEvent>
#include <QPixmapCache>
#include "playlistcreatedialog.h"
#include "sleeptimerdialog.h"

#if defined(_WIN32) || defined(WIN32)
#include <windows.h>
#endif

class ToolTipController : public QObject {
public:
    explicit ToolTipController(QObject* parent = nullptr) : QObject(parent) {}

    void setEnabled(bool enabled) {
        if (m_enabled == enabled) return;
        m_enabled = enabled;
        if (!m_enabled) {
            QToolTip::hideText();
        }
    }

    bool isEnabled() const { return m_enabled; }

protected:
    bool eventFilter(QObject* obj, QEvent* event) override {
        // If tooltips are turned off, block all tooltip requests and ensure any visible tooltip is hidden.
        if (!m_enabled) {
            if (event->type() == QEvent::ToolTip) {
                QToolTip::hideText();
                return true; // Block event
            }
            if (event->type() == QEvent::Enter || event->type() == QEvent::MouseMove || event->type() == QEvent::Leave) {
                if (QToolTip::isVisible()) {
                    QToolTip::hideText();
                }
            }
            return QObject::eventFilter(obj, event);
        }

        // When tooltips are ON:
        if (!obj->isWidgetType()) {
            return QObject::eventFilter(obj, event);
        }

        QWidget* w = qobject_cast<QWidget*>(obj);
        if (!w) {
            return QObject::eventFilter(obj, event);
        }

        // Walk up to find if w or any parent (up to the window level) has a tooltip
        QWidget* target = w;
        while (target && target->toolTip().isEmpty() && target->parentWidget() && !target->parentWidget()->isWindow()) {
            target = target->parentWidget();
        }

        const bool hasToolTip = target && !target->toolTip().isEmpty() && !target->isWindow();

        switch (event->type()) {
        case QEvent::Enter: {
            if (hasToolTip) {
                showInstantToolTip(target);
            } else if (QToolTip::isVisible()) {
                QToolTip::hideText();
            }
            break;
        }
        case QEvent::Leave:
        case QEvent::MouseButtonPress:
        case QEvent::WindowDeactivate:
        case QEvent::Hide: {
            if (QToolTip::isVisible()) {
                QToolTip::hideText();
            }
            break;
        }
        case QEvent::ToolTip: {
            if (hasToolTip) {
                // Ensure tooltip is shown instantly near the component rather than using Qt's delayed globalPos
                showInstantToolTip(target);
                return true; // Intercept to prevent Qt's default tooltip handler from moving/delaying it
            } else {
                QToolTip::hideText();
                return true;
            }
        }
        default:
            break;
        }

        return QObject::eventFilter(obj, event);
    }

private:
    void showInstantToolTip(QWidget* target) {
        if (!target || target->toolTip().isEmpty()) return;

        QPoint globalTopLeft = target->mapToGlobal(QPoint(0, 0));
        int th = target->height();

        // Position near the component (right below the component with a clean 4px vertical margin)
        QPoint pos = globalTopLeft + QPoint(8, th + 4);

        QScreen* screen = target->screen();
        if (!screen) screen = QGuiApplication::primaryScreen();
        if (screen) {
            QRect geom = screen->availableGeometry();
            // If placing below overflows screen bottom, place directly above the component
            if (pos.y() + 40 > geom.bottom()) {
                pos = globalTopLeft + QPoint(8, -32);
            }
            // Ensure horizontal positioning stays within screen bounds
            if (pos.x() + 260 > geom.right()) {
                pos.setX(qMax(geom.left() + 4, geom.right() - 260));
            }
        }

        QToolTip::showText(pos, target->toolTip(), target, target->rect());
    }

    bool m_enabled = true;
};

MainWindow::MainWindow(QWidget* parent) : QMainWindow(parent) {
    setObjectName("MainWindow");
    setWindowTitle("PlayTune");
    resize(1200, 800);
    setMinimumSize(900, 600);

    if (!qApp->windowIcon().isNull()) {
        setWindowIcon(qApp->windowIcon());
    } else {
        QPixmap basePixmap(":/resources/icons/playtune_logo.png");
        QIcon appIcon;
        static const int sizes[] = {16, 22, 24, 32, 48, 64, 128, 256, 512};
        for (int s : sizes) {
            appIcon.addPixmap(basePixmap.scaled(s, s, Qt::KeepAspectRatio, Qt::SmoothTransformation));
        }
        appIcon.addPixmap(basePixmap);
        setWindowIcon(appIcon);
    }

    // Force the icon at the native window level once the window handle exists.
    // On Windows this uses WM_SETICON to override the default EXE icon in the
    // taskbar; on macOS it ensures the dock icon is set.
    QTimer::singleShot(0, this, &MainWindow::forceAppIcon);

    // Pixmap cache: 15 MB is plenty for ~750 unique covers at 200×200×4
    // bytes each (≈ 160 KB per cover). The CoverLoader singleton (see
    // coverloader.h) layers on top of this cache and adds async load +
    // LRU eviction, so total process cover memory stays bounded
    // regardless of library size.
    QPixmapCache::setCacheLimit(15 * 1024);

    setupUi();

    m_toolTipController = new ToolTipController(this);
    qApp->installEventFilter(m_toolTipController);

    loadStyleSheet();
    connectBridge();
    setupKeyboardShortcuts();

    {
        QSettings s("PlayTune", "Settings");
        m_currentVolume = s.value("volume", 75).toInt() / 100.0;
        if (m_currentVolume < 0.0) m_currentVolume = 0.0;
        if (m_currentVolume > 1.0) m_currentVolume = 1.0;
        m_volumeBeforeMute = m_currentVolume;
    }

    // Notify the Rust backend of initial volume and defer GUI ready signal until event loop runs
    const auto& cb = GuiBridgeManager::instance().callbacks();
    if (cb.on_volume) {
        cb.on_volume(m_currentVolume);
    }
    QTimer::singleShot(50, this, [cb]() {
        if (cb.on_gui_ready) {
            cb.on_gui_ready();
        }
    });
}

void MainWindow::setupUi() {
    auto* centralWidget = new QWidget(this);
    centralWidget->setObjectName("CentralPanel");
    setCentralWidget(centralWidget);

    auto* mainLayout = new QHBoxLayout(centralWidget);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);

    // 1. Sidebar (Left)
    m_sidebar = new SidebarWidget(this);
    m_sidebar->setFixedWidth(200);
    mainLayout->addWidget(m_sidebar);

    // Vertical Separator 1
    m_sep1 = new QFrame(this);
    m_sep1->setFrameShape(QFrame::VLine);
    m_sep1->setFrameShadow(QFrame::Plain);
    m_sep1->setStyleSheet("color: #242A3D; background-color: #242A3D; min-width: 1px; max-width: 1px; border: none;");
    mainLayout->addWidget(m_sep1);

    // 2. Center Panel (Search + Now Playing + Songs Table)
    auto* centerPanel = new QWidget(this);
    centerPanel->setObjectName("CenterPanel");
    centerPanel->setAttribute(Qt::WA_StyledBackground, true);
    auto* centerLayout = new QVBoxLayout(centerPanel);
    centerLayout->setContentsMargins(20, 20, 20, 20);
    centerLayout->setSpacing(20);

    // Search bar layout (centered relative to player card)
    auto* searchLayout = new QHBoxLayout();
    m_searchBar = new QLineEdit(this);
    m_searchBar->setObjectName("SearchBar");
    m_searchBar->setPlaceholderText("Search for songs, artists, albums...");
    m_searchBar->setToolTip("Search music library by song title, artist, or album (Ctrl+F / Return to search immediately)");
    m_searchBar->addAction(QIcon(":/resources/icons/search.png"), QLineEdit::LeadingPosition);
    
    m_searchBar->setMaximumWidth(680);
    m_searchBar->setMinimumWidth(320);

    searchLayout->addStretch(1);
    searchLayout->addWidget(m_searchBar);
    searchLayout->addStretch(1);

    m_toggleRightTopBtn = new QPushButton("< Queue", this);
    m_toggleRightTopBtn->setFixedSize(76, 28);
    m_toggleRightTopBtn->setCursor(Qt::PointingHandCursor);
    m_toggleRightTopBtn->setToolTip("Expand Right Sidebar (Q)");
    m_toggleRightTopBtn->setStyleSheet("QPushButton { background-color: #1A2030; color: #E1E4EB; border: 1px solid #28324A; border-radius: 6px; font-size: 12px; font-weight: bold; } QPushButton:hover { background-color: #1B1130; border-color: #7B1FA2; color: #FFFFFF; }");
    m_toggleRightTopBtn->setVisible(false);
    connect(m_toggleRightTopBtn, &QPushButton::clicked, this, [this]() {
        m_queueHiddenByUser = false;
        if (m_contentStack && m_contentStack->currentIndex() != 1) {
            if (m_queueWidget) m_queueWidget->setVisible(true);
            if (m_sep2) m_sep2->setVisible(true);
            m_toggleRightTopBtn->setVisible(false);
        }
    });
    searchLayout->addWidget(m_toggleRightTopBtn);

    centerLayout->addLayout(searchLayout);

    // Now Playing Card
    m_nowPlayingCard = new NowPlayingCard(this);
    centerLayout->addWidget(m_nowPlayingCard);

    // Content Stack (0: Songs Table, 1: Settings Page, 2: Folders View,
    //                3: Albums View, 4: Artists View)
    m_contentStack = new QStackedWidget(this);

    m_songsTable = new SongsTableWidget(this);
    m_contentStack->addWidget(m_songsTable); // index 0

    m_settingsPage = new SettingsPageWidget(this);
    m_contentStack->addWidget(m_settingsPage); // index 1

    m_foldersView = new FoldersViewWidget(this);
    m_contentStack->addWidget(m_foldersView); // index 2

    m_albumsView = new AlbumsViewWidget(this);
    m_contentStack->addWidget(m_albumsView); // index 3

    m_artistsView = new ArtistsViewWidget(this);
    m_contentStack->addWidget(m_artistsView); // index 4

    connect(m_contentStack, &QStackedWidget::currentChanged, this, &MainWindow::updateLayoutForCurrentTab);

    centerLayout->addWidget(m_contentStack, 1);

    mainLayout->addWidget(centerPanel, 1); // center panel stretches horizontally

    // Vertical Separator 2
    m_sep2 = new QFrame(this);
    m_sep2->setFrameShape(QFrame::VLine);
    m_sep2->setFrameShadow(QFrame::Plain);
    m_sep2->setStyleSheet("color: #242A3D; background-color: #242A3D; min-width: 1px; max-width: 1px; border: none;");
    mainLayout->addWidget(m_sep2);

    // 3. Right Sidebar (Queue)
    m_queueWidget = new QueueWidget(this);
    m_queueWidget->setFixedWidth(290); // Widened right bar
    mainLayout->addWidget(m_queueWidget);

    // 4. Create separate Equalizer Window
    m_eqWindow = new EqualizerWindow(this);
}

void MainWindow::loadStyleSheet() {
    qApp->setStyleSheet(ThemeManager::instance().generateStylesheet());
}

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
    //
    // We now apply a QSS overlay that overrides the accent color used by
    // buttons, sliders, and highlights. The base QSS is preserved (loaded
    // once in loadStyleSheet()); we append the per-theme overrides here.
    // The overlay is applied immediately for the currently active (saved)
    // theme and re-applied on every subsequent theme change. Without the
    // immediate call, widgets whose styles were hard-coded in setupUi (e.g.
    // the separators) kept the dark-theme colors when the app started with
    // a light theme saved.
    auto applyThemeOverlay = [this](const ThemePalette& palette) {
        if (m_sep1) {
            m_sep1->setStyleSheet(QString("color: %1; background-color: %1; min-width: 1px; max-width: 1px; border: none;").arg(palette.separatorColor.name()));
        }
        if (m_sep2) {
            m_sep2->setStyleSheet(QString("color: %1; background-color: %1; min-width: 1px; max-width: 1px; border: none;").arg(palette.separatorColor.name()));
        }
        if (m_toggleRightTopBtn) {
            m_toggleRightTopBtn->setStyleSheet(
                QString("QPushButton { background-color: %1; color: %2; border: 1px solid %3; border-radius: 6px; font-size: 12px; font-weight: bold; }"
                        "QPushButton:hover { background-color: %4; border-color: %5; color: %6; }")
                .arg(palette.headerBg.name())
                .arg(palette.secondaryText.name())
                .arg(palette.cardBorder.name())
                .arg(palette.itemHoverBg.name())
                .arg(palette.primaryAccent.name())
                .arg(palette.primaryText.name())
            );
        }
        if (m_searchBar) {
            m_searchBar->setStyleSheet(
                QString("QLineEdit#SearchBar { background-color: %1; border: 1px solid %2; border-radius: 10px; color: %3; padding: 8px 12px; font-size: 13px; }"
                        "QLineEdit#SearchBar:hover { border: 1px solid %4; }"
                        "QLineEdit#SearchBar:focus { border: 1px solid %5; }")
                .arg(palette.headerBg.name())
                .arg(palette.cardBorder.name())
                .arg(palette.primaryText.name())
                .arg(palette.primaryAccent.name())
                .arg(palette.secondaryAccent.name())
            );
        }
    };
    applyThemeOverlay(ThemeManager::instance().currentTheme());
    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [applyThemeOverlay](const ThemePalette& palette) {
        applyThemeOverlay(palette);
    });

    connect(m_settingsPage, &SettingsPageWidget::tooltipsToggled, this, [this](bool enabled) {
        if (m_toolTipController) {
            m_toolTipController->setEnabled(enabled);
        }
    });
    // Wire the gapless/crossfade toggles. Gapless is the inverse of crossfade:
    // when gapless is ON, crossfade is OFF and vice versa. We expose this to
    // the backend as a single crossfade_enabled flag.
    connect(m_settingsPage, &SettingsPageWidget::gaplessToggled, this, [cb](bool enabled) {
        // Gapless ON → crossfade OFF; Gapless OFF → crossfade ON.
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
    connect(m_settingsPage, &SettingsPageWidget::outputDeviceChanged, this, [cb](const QString& deviceName) {
        if (cb.on_set_output_device) {
            QByteArray ba = deviceName.toUtf8();
            cb.on_set_output_device(ba.constData());
        }
    });
    if (m_toolTipController && m_settingsPage) {
        m_toolTipController->setEnabled(m_settingsPage->isTooltipsEnabled());
    }

    // ── Optimized Mode: live signal wiring ────────────────────────────────
    // Each signal connection propagates the toggle to a specific widget
    // with no app restart. The lambda for CoverLoader updates the global
    // QPixmapCache limit and flushes covers immediately when enabling.
    connect(m_settingsPage, &SettingsPageWidget::optimizedModeToggled,
            m_nowPlayingCard,  &NowPlayingCard::setOptimizedMode);
    connect(m_settingsPage, &SettingsPageWidget::optimizedModeToggled,
            m_songsTable,      &SongsTableWidget::setOptimizedMode);
    connect(m_settingsPage, &SettingsPageWidget::optimizedModeToggled,
            m_queueWidget,     &QueueWidget::setOptimizedMode);
    // AlbumsView and ArtistsView expose their inner MediaGridWidget through
    // setOptimizedMode forwarded at the view level via the albums/artists slot.
    connect(m_settingsPage, &SettingsPageWidget::optimizedModeToggled,
            m_albumsView,      &AlbumsViewWidget::setOptimizedMode);
    connect(m_settingsPage, &SettingsPageWidget::optimizedModeToggled,
            m_artistsView,     &ArtistsViewWidget::setOptimizedMode);
    // EQ: when Optimized Mode is on, disable EQ DSP if it was already enabled
    // (flat EQ bypasses for free via the early-return in ParametricEq::process,
    // but an explicit disable avoids the filter loop entirely).
    connect(m_settingsPage, &SettingsPageWidget::optimizedModeToggled,
            this, [cb](bool on) {
        if (on && cb.on_eq_enabled) cb.on_eq_enabled(0);
    });

    // Apply Optimized Mode on startup if the setting was saved as true.
    if (m_settingsPage->isOptimizedMode()) {
        // Use singleShot so all widgets are fully constructed before we apply.
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
    // ─────────────────────────────────────────────────────────────────────

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
        // Restart the debounce timer on every keystroke.
        if (m_searchTimer) {
            m_searchTimer->start();
        }
    });
    connect(m_searchBar, &QLineEdit::returnPressed, this, [this, cb]() {
        // Stop the debounce and fire immediately on Return.
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

    // ── Songs-table event dispatching ──────────────────────────────────
    // Old code routed songsCleared / songAdded to ALL FOUR views (Home,
    // Folders inner, Albums inner, Artists inner). For a 10 000-track
    // refresh that meant 4× the widget creation, 4× the cover loads,
    // and ~4× the RAM. Now we route to a single "active" songs table
    // determined by which tab the user is currently looking at.
    //
    // The batch signal (songsBatchReplaced) is the preferred fast path:
    // it does a single transactional rebuild of the active table.
    // The per-track signals (songsCleared / songAdded) are kept for
    // back-compat with the incremental update paths (rare; e.g. when
    // a single track's tags are edited and the backend pushes just
    // that one update).
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
    connect(&manager, &GuiBridgeManager::queueSongAdded, m_queueWidget, &QueueWidget::addQueueSong, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::visualizerUpdated, m_nowPlayingCard, &NowPlayingCard::updateVisualizer, Qt::QueuedConnection);

    // Mirror folder list into the Settings page
    connect(&manager, &GuiBridgeManager::foldersCleared, m_settingsPage, &SettingsPageWidget::clearFolderList, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::folderAdded, m_settingsPage, &SettingsPageWidget::addFolderToList, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::audioDevicesCleared, m_settingsPage, &SettingsPageWidget::clearAudioDeviceList, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::audioDeviceAdded, m_settingsPage, &SettingsPageWidget::addAudioDeviceToList, Qt::QueuedConnection);

    // Sidebar collapse tracking connection
    connect(m_sidebar, &SidebarWidget::collapsedToggled, this, [this](bool collapsed) {
        if (!m_inResizeEvent) {
            m_sidebarCollapsedByUser = collapsed;
        }
    });

    // Sidebar: Add Playlist button → create-playlist dialog
    connect(m_sidebar, &SidebarWidget::addPlaylistClicked, this, [this]() {
        onPlaylistCreateRequested();
    });

    // Sidebar: user clicks a playlist row → filter songs table by playlist.
    connect(m_sidebar, &SidebarWidget::playlistSelected, this, [this, cb](int playlist_id) {
        m_contentStack->setCurrentIndex(0);  // Show songs table page
        if (cb.on_filter_playlist) cb.on_filter_playlist(playlist_id);
    });

    // Sidebar: user right-clicks → Rename / Delete
    connect(m_sidebar, &SidebarWidget::playlistRenameRequested, this,
            [this](int playlist_id, const QString& current_name) {
        onPlaylistRenameRequested(playlist_id, current_name);
    });
    connect(m_sidebar, &SidebarWidget::playlistDeleteRequested, this, [this, cb](int playlist_id) {
        onPlaylistDeleteRequested(playlist_id);
        Q_UNUSED(cb);
    });

    // Albums view: clicking an album filters tracks for the albums view inner table
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

    // Artists view: clicking an artist filters songs for the artist view inner table
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

    // Manager signals for the new views.
    connect(&manager, &GuiBridgeManager::playlistsCleared, m_sidebar,
            &SidebarWidget::clearPlaylists, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::playlistAdded, m_sidebar,
            &SidebarWidget::addPlaylistRow, Qt::QueuedConnection);

    connect(&manager, &GuiBridgeManager::albumsCleared, m_albumsView,
            &AlbumsViewWidget::clearAlbums, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::albumAdded, m_albumsView,
            &AlbumsViewWidget::addAlbumRow, Qt::QueuedConnection);
    // NOTE: songsCleared / songAdded / songsBatchReplaced for the Albums
    // view's inner table are now routed via the activeSongsTable()
    // dispatcher above. The old code connected these signals to all four
    // views simultaneously, which caused 4× widget creation and 4× cover
    // loads on every refresh_ui call (the single biggest source of UI
    // freezes for libraries > 1 000 tracks).

    connect(&manager, &GuiBridgeManager::artistsCleared, m_artistsView,
            &ArtistsViewWidget::clearArtists, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::artistAdded, m_artistsView,
            &ArtistsViewWidget::addArtistRow, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::albumsInArtistCleared, m_artistsView,
            &ArtistsViewWidget::clearAlbumsInArtist, Qt::QueuedConnection);
    connect(&manager, &GuiBridgeManager::albumInArtistAdded, m_artistsView,
            &ArtistsViewWidget::addAlbumInArtist, Qt::QueuedConnection);
    // NOTE: songsCleared / songAdded for the Artists view's inner table
    // are also routed via the activeSongsTable() dispatcher above.

    // Speed label & sleep timer signals.
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

    // Initialize the system tray now that the bridge is wired.
    setupSystemTray();

    // Auto-populate mock tracks if running in standalone C++ demo mode
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

// Returns the SongsTableWidget currently visible to the user, or
// m_songsTable as a fallback. Used by the songs-event dispatcher so
// songsCleared / songAdded / songsBatchReplaced are routed to ONE view
// instead of four. See header comment for details.
SongsTableWidget* MainWindow::activeSongsTable() const {
    if (!m_contentStack) return m_songsTable;
    int idx = m_contentStack->currentIndex();
    switch (idx) {
        case 0:  // Home / Songs / Favorites / Recently / Most / Playlist
            return m_songsTable;
        case 2:  // Folders tab
            return m_foldersView ? m_foldersView->findChild<SongsTableWidget*>() : nullptr;
        case 3:  // Albums tab
            // The inner table is on page 1 of the Albums view's stack.
            // If we're on page 0 (grid), no songs table is visible.
            return m_albumsView ? m_albumsView->findChild<SongsTableWidget*>() : nullptr;
        case 4:  // Artists tab
            return m_artistsView ? m_artistsView->findChild<SongsTableWidget*>() : nullptr;
        default:
            return m_songsTable;
    }
}

// ---------------------------------------------------------------------------
// Keyboard Shortcut Setup
// ---------------------------------------------------------------------------
void MainWindow::setupKeyboardShortcuts() {
    // Ctrl+F  →  focus the search bar
    auto* searchShortcut = new QShortcut(QKeySequence(Qt::CTRL | Qt::Key_F), this);
    connect(searchShortcut, &QShortcut::activated, this, [this]() {
        m_searchBar->setFocus();
        m_searchBar->selectAll();
    });

    setFocusPolicy(Qt::StrongFocus);
}

// ---------------------------------------------------------------------------
// Toast Notification
// ---------------------------------------------------------------------------
void MainWindow::showToast(const QString& message) {
    // Create a floating label anchored to the bottom-right of the main window
    auto* toast = new QLabel(message, this);
    toast->setObjectName("ToastNotification");
    toast->setAlignment(Qt::AlignCenter);
    toast->setWordWrap(false);
    toast->setStyleSheet(
        "QLabel#ToastNotification {"
        "  background-color: #1E293B;"
        "  color: #E2E8F0;"
        "  border: 1px solid #334155;"
        "  border-radius: 10px;"
        "  padding: 10px 20px;"
        "  font-size: 13px;"
        "  font-weight: 600;"
        "}"
    );
    toast->adjustSize();
    toast->setFixedWidth(qMax(280, toast->sizeHint().width() + 40));

    // Position bottom-right with 20px margin
    int x = width() - toast->width() - 20;
    int y = height() - toast->height() - 24;
    toast->move(x, y);
    toast->raise();
    toast->show();

    // Fade-in
    auto* effect = new QGraphicsOpacityEffect(toast);
    toast->setGraphicsEffect(effect);
    auto* fadeIn = new QPropertyAnimation(effect, "opacity", toast);
    fadeIn->setDuration(250);
    fadeIn->setStartValue(0.0);
    fadeIn->setEndValue(1.0);
    fadeIn->start(QAbstractAnimation::DeleteWhenStopped);

    // Auto-dismiss after 3.5s with fade-out
    QTimer::singleShot(3500, toast, [toast, effect]() {
        auto* fadeOut = new QPropertyAnimation(effect, "opacity", toast);
        fadeOut->setDuration(350);
        fadeOut->setStartValue(1.0);
        fadeOut->setEndValue(0.0);
        connect(fadeOut, &QPropertyAnimation::finished, toast, &QLabel::deleteLater);
        fadeOut->start(QAbstractAnimation::DeleteWhenStopped);
    });
}

// ---------------------------------------------------------------------------
// keyPressEvent  –  global keyboard shortcuts
// ---------------------------------------------------------------------------
void MainWindow::keyPressEvent(QKeyEvent* event) {
    // Don't steal keys while the user is typing in the search bar
    if (m_searchBar->hasFocus()) {
        QMainWindow::keyPressEvent(event);
        return;
    }

    const auto& cb = GuiBridgeManager::instance().callbacks();
    const bool  shift = event->modifiers() & Qt::ShiftModifier;

    switch (event->key()) {

    // ── Play / Pause ──────────────────────────────────────────────────────
    case Qt::Key_Space:
        if (cb.on_play_pause) cb.on_play_pause();
        event->accept();
        return;

    // ── Next / Prev track ─────────────────────────────────────────────────
    case Qt::Key_Right:
        if (shift) {
            // Shift+→  seek forward 5 seconds
            double newPos = m_nowPlayingCard->elapsedSeconds() + 5.0;
            double total = m_nowPlayingCard->totalSeconds();
            if (total > 0.0) {
                newPos = qMin(newPos, total);
            }
            emit m_nowPlayingCard->seekRequested(newPos);
        } else {
            // → or Ctrl+→  next track
            if (cb.on_next) cb.on_next();
        }
        event->accept();
        return;

    case Qt::Key_Left:
        if (shift) {
            // Shift+←  seek backward 5 seconds
            double newPos = m_nowPlayingCard->elapsedSeconds() - 5.0;
            emit m_nowPlayingCard->seekRequested(qMax(0.0, newPos));
        } else {
            // ← or Ctrl+←  previous track
            if (cb.on_prev) cb.on_prev();
        }
        event->accept();
        return;

    // ── Volume ────────────────────────────────────────────────────────────
    case Qt::Key_Up: {
        // ↑ or Ctrl+↑  volume up 5 %
        m_currentVolume = qMin(1.0, m_currentVolume + 0.05);
        m_isMuted = false;
        m_volumeBeforeMute = m_currentVolume;
        if (auto* slider = m_queueWidget->findChild<QSlider*>("VolumeSlider")) {
            slider->setValue(static_cast<int>(m_currentVolume * 100));
        } else if (cb.on_volume) {
            cb.on_volume(m_currentVolume);
        }
        event->accept();
        return;
    }
    case Qt::Key_Down: {
        // ↓ or Ctrl+↓  volume down 5 %
        m_currentVolume = qMax(0.0, m_currentVolume - 0.05);
        if (m_currentVolume == 0.0) m_isMuted = true;
        if (m_currentVolume > 0.0) m_volumeBeforeMute = m_currentVolume;
        if (auto* slider = m_queueWidget->findChild<QSlider*>("VolumeSlider")) {
            slider->setValue(static_cast<int>(m_currentVolume * 100));
        } else if (cb.on_volume) {
            cb.on_volume(m_currentVolume);
        }
        event->accept();
        return;
    }

    // ── Mute toggle ───────────────────────────────────────────────────────
    case Qt::Key_M: {
        if (m_isMuted) {
            // Unmute: restore previous volume
            m_isMuted = false;
            m_currentVolume = m_volumeBeforeMute;
        } else {
            m_volumeBeforeMute = m_currentVolume;
            m_isMuted = true;
            m_currentVolume = 0.0;
        }
        if (auto* slider = m_queueWidget->findChild<QSlider*>("VolumeSlider")) {
            slider->setValue(static_cast<int>(m_currentVolume * 100));
        } else if (cb.on_volume) {
            cb.on_volume(m_currentVolume);
        }
        event->accept();
        return;
    }

    // ── Repeat toggle  (R) ────────────────────────────────────────────────
    case Qt::Key_R: {
        for (auto* btn : m_nowPlayingCard->findChildren<QPushButton*>("MediaControlBtn")) {
            if (btn->toolTip().startsWith("Repeat")) {
                btn->click();
                break;
            }
        }
        event->accept();
        return;
    }

    // ── Shuffle toggle  (S) ───────────────────────────────────────────────
    case Qt::Key_S: {
        for (auto* btn : m_nowPlayingCard->findChildren<QPushButton*>("MediaControlBtn")) {
            if (btn->toolTip().startsWith("Shuffle")) {
                btn->click();
                break;
            }
        }
        event->accept();
        return;
    }

    // ── Equalizer toggle  (E) ─────────────────────────────────────────────
    case Qt::Key_E: {
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
        event->accept();
        return;
    }

    // ── Toggle Queue Sidebar  (Q) ─────────────────────────────────────────
    case Qt::Key_Q: {
        if (m_queueWidget) {
            bool visible = !m_queueWidget->isVisible();
            m_queueHiddenByUser = !visible;
            if (m_contentStack && m_contentStack->currentIndex() != 1) {
                m_queueWidget->setVisible(visible);
                if (m_sep2) m_sep2->setVisible(visible);
                if (m_toggleRightTopBtn) m_toggleRightTopBtn->setVisible(!visible);
            }
        }
        event->accept();
        return;
    }

    default:
        break;
    }

    QMainWindow::keyPressEvent(event);
}

void MainWindow::resizeEvent(QResizeEvent* event) {
    QMainWindow::resizeEvent(event);
    int w = width();

    m_inResizeEvent = true;

    // Left Sidebar staged collapse
    if (w < 1050) {
        // Auto-collapse on smaller screens
        if (!m_sidebar->isCollapsed()) {
            m_sidebar->setCollapsed(true);
        }
    } else {
        // Restore if user didn't explicitly collapse it
        if (!m_sidebarCollapsedByUser && m_sidebar->isCollapsed()) {
            m_sidebar->setCollapsed(false);
        }
    }

    // Right Sidebar (Queue) staged collapse
    if (w < 850) {
        // Auto-hide on very small screens
        if (m_queueWidget->isVisible()) {
            m_queueWidget->setVisible(false);
            if (m_sep2) m_sep2->setVisible(false);
            if (m_toggleRightTopBtn && m_contentStack && m_contentStack->currentIndex() != 1) {
                m_toggleRightTopBtn->setVisible(true);
            }
        }
    } else {
        // Restore if user didn't explicitly hide it
        if (!m_queueHiddenByUser && !m_queueWidget->isVisible()) {
            if (m_contentStack && m_contentStack->currentIndex() != 1) {
                m_queueWidget->setVisible(true);
                if (m_sep2) m_sep2->setVisible(true);
                if (m_toggleRightTopBtn) m_toggleRightTopBtn->setVisible(false);
            }
        }
    }

    // Pass the center panel width to songs table to hide columns
    int centerWidth = w - (m_sidebar->isVisible() ? m_sidebar->width() : 0) 
                        - (m_queueWidget->isVisible() ? m_queueWidget->width() : 0);
    if (m_songsTable) {
        m_songsTable->setResponsiveWidth(centerWidth);
    }

    // Center panel layout margin adjustments
    if (auto* centralWidget = this->centralWidget()) {
        if (auto* centerPanel = centralWidget->findChild<QWidget*>("CenterPanel")) {
            if (auto* centerLayout = qobject_cast<QVBoxLayout*>(centerPanel->layout())) {
                int sideMargin = (w < 700) ? 10 : 20;
                centerLayout->setContentsMargins(sideMargin, sideMargin, sideMargin, sideMargin);
                centerLayout->setSpacing(sideMargin);
            }
        }
    }

    // Keep EQ window within main window bounds on resize
    if (m_eqWindow && m_eqWindow->isVisible()) {
        QPoint eqPos = m_eqWindow->pos();
        int maxX = width() - m_eqWindow->width();
        int maxY = height() - m_eqWindow->height();
        eqPos.setX(qBound(0, eqPos.x(), qMax(0, maxX)));
        eqPos.setY(qBound(0, eqPos.y(), qMax(0, maxY)));
        m_eqWindow->move(eqPos);
    }

    m_inResizeEvent = false;
}

void MainWindow::updateLayoutForCurrentTab(int index) {
    bool isSettings = (index == 1);

    if (m_searchBar) m_searchBar->setVisible(!isSettings);
    if (m_nowPlayingCard) m_nowPlayingCard->setVisible(!isSettings);

    if (isSettings) {
        if (m_queueWidget) m_queueWidget->setVisible(false);
        if (m_sep2) m_sep2->setVisible(false);
        if (m_toggleRightTopBtn) m_toggleRightTopBtn->setVisible(false);
    } else {
        if (m_queueWidget) m_queueWidget->setVisible(!m_queueHiddenByUser);
        if (m_sep2) m_sep2->setVisible(!m_queueHiddenByUser);
        if (m_toggleRightTopBtn) m_toggleRightTopBtn->setVisible(m_queueHiddenByUser);
    }
}

// ========================================================================
// New feature implementations: system tray, playlist dialogs, M3U IO
// ========================================================================

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
            show();
            raise();
            activateWindow();
        } else {
            hide();
        }
    });
    connect(m_trayQuitAction, &QAction::triggered, this, [this]() {
        // Force quit even if minimize-to-tray is on.
        m_minimizeToTray = false;
        close();
    });
    connect(m_trayIcon, &QSystemTrayIcon::activated, this,
            [this](QSystemTrayIcon::ActivationReason reason) {
        if (reason == QSystemTrayIcon::Trigger) {
            if (isHidden()) {
                show();
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
        // Hide to tray instead of quitting.
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
    // Ask the user to pick a playlist ID — for now, use the active playlist
    // if there is one; otherwise prompt with a list.
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

void MainWindow::forceAppIcon() {
    QIcon icon = windowIcon();
    if (icon.isNull()) return;

    // Re-apply the icon now that the native window handle exists.
    // This ensures Qt sets platform properties like _NET_WM_ICON (Linux)
    // and the title-bar icon correctly.
    setWindowIcon(icon);

#if defined(_WIN32) || defined(WIN32)
    HWND hwnd = reinterpret_cast<HWND>(this->winId());
    if (!hwnd) return;

    static const int sizes[] = {16, 32, 48, 64, 128, 256};
    for (int size : sizes) {
        QPixmap px = icon.pixmap(size, size);
        if (px.isNull()) continue;
        QImage img = px.toImage().convertToFormat(QImage::Format_ARGB32);
        if (img.isNull()) continue;
        HICON hIcon = img.toHICON();
        if (hIcon) {
            if (size <= 32) {
                SendMessage(hwnd, WM_SETICON, ICON_SMALL, reinterpret_cast<LPARAM>(hIcon));
            }
            SendMessage(hwnd, WM_SETICON, ICON_BIG, reinterpret_cast<LPARAM>(hIcon));
            DestroyIcon(hIcon);
        }
    }
#endif
}
