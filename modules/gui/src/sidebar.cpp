#include "sidebar.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QSpacerItem>
#include <QIcon>
#include <QSize>
#include <QEvent>
#include <QMouseEvent>
#include <QListWidgetItem>
#include <QMenu>
#include <QAction>
#include <QContextMenuEvent>

SidebarWidget::SidebarWidget(QWidget* parent) : QWidget(parent) {
    setObjectName("SidebarFrame");
    setAttribute(Qt::WA_StyledBackground, true);
    setupUi();
}

void SidebarWidget::setupUi() {
    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(15, 20, 15, 20);
    mainLayout->setSpacing(5);

    // 1. Logo Header (Icon + Name clickable for collapse/expand)
    m_logoContainer = new QWidget(this);
    m_logoContainer->setCursor(Qt::PointingHandCursor);
    m_logoContainer->setToolTip("Click to Toggle Sidebar Collapse/Expand");

    auto* logoLayout = new QHBoxLayout(m_logoContainer);
    logoLayout->setSpacing(8);
    logoLayout->setContentsMargins(5, 0, 5, 20);

    m_logoIcon = new QLabel(m_logoContainer);
    m_logoIcon->setPixmap(QIcon(":/resources/icons/playtune_logo.png").pixmap(24, 24));
    m_logoIcon->setCursor(Qt::PointingHandCursor);
    
    m_logoText = new QLabel("PlayTune", m_logoContainer);
    m_logoText->setObjectName("LogoLabel");
    m_logoText->setCursor(Qt::PointingHandCursor);

    logoLayout->addWidget(m_logoIcon);
    logoLayout->addWidget(m_logoText);
    logoLayout->addStretch();

    m_logoContainer->installEventFilter(this);
    m_logoIcon->installEventFilter(this);
    m_logoText->installEventFilter(this);

    mainLayout->addWidget(m_logoContainer);

    // 2. Navigation Section
    m_navGroup = new QButtonGroup(this);
    m_navGroup->setExclusive(true);

    QStringList navNames = {"Home", "Albums", "Artists", "Folders"};
    QStringList navIcons = {
        ":/resources/icons/home.png",
        ":/resources/icons/albums.png",
        ":/resources/icons/artists.png",
        ":/resources/icons/folders.png"
    };

    QStringList navTooltips = {
        "View Home Library & All Tracks",
        "Browse Library by Albums",
        "Browse Library by Artists",
        "Manage Imported Music Directories"
    };

    for (int i = 0; i < navNames.size(); ++i) {
        auto* btn = new QPushButton(navNames[i], this);
        btn->setObjectName("SidebarBtn");
        btn->setCheckable(true);
        btn->setIcon(QIcon(navIcons[i]));
        btn->setIconSize(QSize(18, 18));
        if (i < navTooltips.size()) btn->setToolTip(navTooltips[i]);
        
        m_navGroup->addButton(btn, i);
        mainLayout->addWidget(btn);
        m_allNavButtons.append(btn);
        m_allNavTexts.append(navNames[i]);

        // Highlight Home by default
        if (i == 0) {
            btn->setChecked(true);
        }
    }

    // Connect nav button clicks
    connect(m_navGroup, &QButtonGroup::idClicked, this, [this](int id) {
        switch (id) {
            case 0: emit homeClicked(); break;
            case 1: emit albumsClicked(); break;
            case 2: emit artistsClicked(); break;
            case 3: emit foldersClicked(); break;
        }
    });

    // 3. Playlists Header Section
    auto* playlistsHeaderLayout = new QHBoxLayout();
    playlistsHeaderLayout->setContentsMargins(5, 10, 5, 5);
    
    m_sectionLabel = new QLabel("Playlists", this);
    m_sectionLabel->setObjectName("SectionHeader");
    
    m_addPlaylistBtn = new QPushButton(this);
    m_addPlaylistBtn->setObjectName("IconButton");
    m_addPlaylistBtn->setIcon(QIcon(":/resources/icons/plus.png"));
    m_addPlaylistBtn->setIconSize(QSize(12, 12));
    m_addPlaylistBtn->setFixedSize(22, 22);
    m_addPlaylistBtn->setToolTip("Create New Custom Playlist");

    playlistsHeaderLayout->addWidget(m_sectionLabel);
    playlistsHeaderLayout->addStretch();
    playlistsHeaderLayout->addWidget(m_addPlaylistBtn);
    mainLayout->addLayout(playlistsHeaderLayout);

    connect(m_addPlaylistBtn, &QPushButton::clicked, this, &SidebarWidget::addPlaylistClicked);

    // Playlists Sub-items
    QStringList playlistNames = {"Favorites", "Recently Played", "Most Played"};
    QStringList playlistIcons = {
        ":/resources/icons/favorites.png",
        ":/resources/icons/recently_played.png",
        ":/resources/icons/most_played.png"
    };

    QStringList playlistTooltips = {
        "View Liked Songs & Favorites",
        "View Recently Played Tracks",
        "View Most Frequently Played Tracks"
    };

    for (int i = 0; i < playlistNames.size(); ++i) {
        auto* btn = new QPushButton(playlistNames[i], this);
        btn->setObjectName("SidebarBtn");
        btn->setIcon(QIcon(playlistIcons[i]));
        btn->setIconSize(QSize(18, 18));
        if (i < playlistTooltips.size()) btn->setToolTip(playlistTooltips[i]);
        btn->setCheckable(true);
        m_navGroup->addButton(btn, i + 4); // offset past the 4 main nav buttons
        mainLayout->addWidget(btn);
        m_allNavButtons.append(btn);
        m_allNavTexts.append(playlistNames[i]);

        connect(btn, &QPushButton::clicked, this, [this, i]() {
            if (i == 0) emit favoritesClicked();
            else if (i == 1) emit recentlyPlayedClicked();
            else if (i == 2) emit mostPlayedClicked();
        });
    }

    // Dynamic user-defined playlist list.
    m_playlistList = new QListWidget(this);
    m_playlistList->setObjectName("UserPlaylistsList");
    m_playlistList->setStyleSheet(
        "QListWidget { background-color: transparent; border: none; color: #C8C8D0; "
        "font-size: 13px; outline: none; }"
        "QListWidget::item { padding: 6px 8px 6px 28px; border-radius: 4px; "
        "background-image: url(:/resources/icons/list.png); "
        "background-position: 6px center; background-repeat: no-repeat; }"
        "QListWidget::item:hover { background-color: rgba(255, 255, 255, 0.05); }"
        "QListWidget::item:selected { background-color: rgba(255, 42, 122, 0.18); color: #FFFFFF; }");
    m_playlistList->setContextMenuPolicy(Qt::CustomContextMenu);
    m_playlistList->setMinimumHeight(0);
    m_playlistList->setMaximumHeight(220);
    m_playlistList->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    m_playlistList->setVerticalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    m_playlistList->setFocusPolicy(Qt::NoFocus);

    connect(m_playlistList, &QListWidget::itemClicked, this, [this](QListWidgetItem* item) {
        if (!item) return;
        int playlist_id = item->data(Qt::UserRole).toInt();
        emit playlistSelected(playlist_id);
    });
    connect(m_playlistList, &QListWidget::customContextMenuRequested, this,
            [this](const QPoint& pos) {
        auto* item = m_playlistList->itemAt(pos);
        if (!item) return;
        int playlist_id = item->data(Qt::UserRole).toInt();
        QString name = item->data(Qt::UserRole + 1).toString();
        QMenu menu(this);
        QAction* renameAction = menu.addAction("Rename…");
        QAction* deleteAction = menu.addAction("Delete");
        QAction* chosen = menu.exec(m_playlistList->viewport()->mapToGlobal(pos));
        if (chosen == renameAction) {
            emit playlistRenameRequested(playlist_id, name);
        } else if (chosen == deleteAction) {
            emit playlistDeleteRequested(playlist_id);
        }
    });
    mainLayout->addWidget(m_playlistList);

    // 4. Spacer pushing settings to bottom
    mainLayout->addStretch();

    // 5. Settings Button at Bottom
    auto* settingsBtn = new QPushButton("Settings", this);
    settingsBtn->setObjectName("SidebarBtn");
    settingsBtn->setIcon(QIcon(":/resources/icons/settings.png"));
    settingsBtn->setIconSize(QSize(18, 18));
    settingsBtn->setToolTip("Application Settings, Themes & Library Management");
    settingsBtn->setCheckable(true);
    m_navGroup->addButton(settingsBtn, 8); // offset past the 7 existing buttons
    mainLayout->addWidget(settingsBtn);
    m_allNavButtons.append(settingsBtn);
    m_allNavTexts.append("Settings");

    connect(settingsBtn, &QPushButton::clicked, this, [this]() {
        emit settingsClicked();
    });
}

void SidebarWidget::setCollapsed(bool collapsed) {
    m_isCollapsed = collapsed;
    setFixedWidth(m_isCollapsed ? 64 : 200);
    if (m_logoText) m_logoText->setVisible(!m_isCollapsed);
    if (m_sectionLabel) m_sectionLabel->setVisible(!m_isCollapsed);
    if (m_addPlaylistBtn) m_addPlaylistBtn->setVisible(!m_isCollapsed);
    if (m_playlistList) m_playlistList->setVisible(!m_isCollapsed);

    for (int i = 0; i < m_allNavButtons.size(); ++i) {
        if (m_isCollapsed) {
            m_allNavButtons[i]->setText("");
            m_allNavButtons[i]->setStyleSheet("QPushButton { text-align: center; padding: 10px 0px; }");
        } else {
            m_allNavButtons[i]->setText(m_allNavTexts[i]);
            m_allNavButtons[i]->setStyleSheet("");
        }
    }
    emit collapsedToggled(m_isCollapsed);
}

void SidebarWidget::clearPlaylists() {
    if (m_playlistList) m_playlistList->clear();
}

void SidebarWidget::addPlaylistRow(int playlist_id, const QString& name, int track_count, double duration_secs) {
    if (!m_playlistList) return;
    QString display = name;
    if (track_count > 0) {
        display += QString("  (%1)").arg(track_count);
    }
    auto* item = new QListWidgetItem(display);
    item->setData(Qt::UserRole, playlist_id);
    item->setData(Qt::UserRole + 1, name);
    item->setData(Qt::UserRole + 2, track_count);
    item->setData(Qt::UserRole + 3, duration_secs);
    item->setToolTip(QString("%1 — %2 track%3").arg(name).arg(track_count).arg(track_count == 1 ? "" : "s"));
    m_playlistList->addItem(item);
}

bool SidebarWidget::eventFilter(QObject* watched, QEvent* event) {
    if ((watched == m_logoContainer || watched == m_logoIcon || watched == m_logoText) &&
        event->type() == QEvent::MouseButtonPress) {
        auto* mouseEvent = static_cast<QMouseEvent*>(event);
        if (mouseEvent->button() == Qt::LeftButton) {
            setCollapsed(!m_isCollapsed);
            return true;
        }
    }
    return QWidget::eventFilter(watched, event);
}
