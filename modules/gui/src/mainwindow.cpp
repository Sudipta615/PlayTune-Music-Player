#include "mainwindow.h"
#include "gui_bridge_p.h"
#include "apptheme.h"
#include <QHBoxLayout>
#include <QVBoxLayout>
#include <QApplication>
#include <QIcon>
#include <QPixmap>
#include <QTimer>
#include <QSettings>
#include <QPixmapCache>

MainWindow::MainWindow(QWidget* parent) : QMainWindow(parent) {
    setObjectName("MainWindow");
    setWindowTitle("PlayTune");
    resize(1400, 900);
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

    QTimer::singleShot(0, this, &MainWindow::forceAppIcon);
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

    const auto& cb = GuiBridgeManager::instance().callbacks();
    if (cb.on_volume) {
        cb.on_volume(m_currentVolume);
    }
    QTimer::singleShot(0, this, [cb]() {
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

    // Search bar layout
    auto* searchLayout = new QHBoxLayout();
    m_searchBar = new QLineEdit(this);
    m_searchBar->setObjectName("SearchBar");
    m_searchBar->setPlaceholderText("Search for songs, artists, albums...");
    m_searchBar->setToolTip("Search music library by song title, artist, or album (Ctrl+F / Return to search immediately)");
    m_searchAction = m_searchBar->addAction(
        ThemeManager::tintedIcon(":/resources/icons/search.png",
            ThemeManager::instance().currentTheme().iconColor),
        QLineEdit::LeadingPosition);
    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this](const ThemePalette& p) {
        if (m_searchAction) {
            m_searchAction->setIcon(ThemeManager::tintedIcon(":/resources/icons/search.png", p.iconColor));
        }
    });
    
    m_searchBar->setMaximumWidth(680);
    m_searchBar->setMinimumWidth(320);

    searchLayout->addStretch(1);
    searchLayout->addWidget(m_searchBar);
    searchLayout->addStretch(1);

    m_toggleRightTopBtn = new QPushButton("< Queue", this);
    m_toggleRightTopBtn->setFixedSize(76, 28);
    m_toggleRightTopBtn->setCursor(Qt::PointingHandCursor);
    m_toggleRightTopBtn->setToolTip("Expand Right Sidebar (Q)");
    {
        const auto& p = ThemeManager::instance().currentTheme();
        m_toggleRightTopBtn->setStyleSheet(QString(
            "QPushButton { background-color: %1; color: %2; border: 1px solid %3; border-radius: 6px; font-size: 12px; font-weight: bold; }"
            "QPushButton:hover { background-color: %4; border-color: %5; color: %6; }"
        ).arg(p.headerBg.name(), p.secondaryText.name(), p.cardBorder.name(),
              p.itemHoverBg.name(), p.primaryAccent.name(), p.primaryText.name()));
    }
    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this](const ThemePalette& p) {
        if (m_toggleRightTopBtn) {
            m_toggleRightTopBtn->setStyleSheet(QString(
                "QPushButton { background-color: %1; color: %2; border: 1px solid %3; border-radius: 6px; font-size: 12px; font-weight: bold; }"
                "QPushButton:hover { background-color: %4; border-color: %5; color: %6; }"
            ).arg(p.headerBg.name(), p.secondaryText.name(), p.cardBorder.name(),
                  p.itemHoverBg.name(), p.primaryAccent.name(), p.primaryText.name()));
        }
    });
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

    // Content Stack
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

    mainLayout->addWidget(centerPanel, 1);

    // Vertical Separator 2
    m_sep2 = new QFrame(this);
    m_sep2->setFrameShape(QFrame::VLine);
    m_sep2->setFrameShadow(QFrame::Plain);
    m_sep2->setStyleSheet("color: #242A3D; background-color: #242A3D; min-width: 1px; max-width: 1px; border: none;");
    mainLayout->addWidget(m_sep2);

    // 3. Right Sidebar (Queue)
    m_queueWidget = new QueueWidget(this);
    m_queueWidget->setFixedWidth(290);
    mainLayout->addWidget(m_queueWidget);

    // 4. Equalizer Window
    m_eqWindow = new EqualizerWindow(this);
}

void MainWindow::loadStyleSheet() {
    qApp->setStyleSheet(ThemeManager::instance().generateStylesheet());
}

SongsTableWidget* MainWindow::activeSongsTable() const {
    if (!m_contentStack) return m_songsTable;
    int idx = m_contentStack->currentIndex();
    switch (idx) {
        case 0:
            return m_songsTable;
        case 2:
            return m_foldersView ? m_foldersView->findChild<SongsTableWidget*>() : nullptr;
        case 3:
            return m_albumsView ? m_albumsView->findChild<SongsTableWidget*>() : nullptr;
        case 4:
            return m_artistsView ? m_artistsView->findChild<SongsTableWidget*>() : nullptr;
        default:
            return m_songsTable;
    }
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
