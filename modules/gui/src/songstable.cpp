#include "songstable.h"
#include "gui_bridge_p.h"
#include "coverloader.h"
#include "appsettings.h"
#include "tageditordialog.h"
#include "loudnessscannerdialog.h"
#include "apptheme.h"
#include "custom_widgets.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QComboBox>
#include <QPushButton>
#include <QIcon>
#include <QPainter>
#include <QTableWidgetItem>
#include <QButtonGroup>
#include <QMenu>
#include <QAction>
#include <QTimer>
#include <QDebug>
#include <QPainterPath>
#include <QScrollBar>

namespace {

static QPixmap loadThumbnail(const QString& coverPath, bool requestAsync = false) {
    if (AppSettings::instance().isOptimizedMode()) {
        QPixmap def = getDefaultAlbumArt();
        QPixmap target(44, 44);
        target.fill(Qt::transparent);
        QPainter painter(&target);
        painter.setRenderHint(QPainter::Antialiasing, true);
        painter.setRenderHint(QPainter::SmoothPixmapTransform, true);
        QPainterPath path;
        path.addRoundedRect(0, 0, 44, 44, 8, 8);
        painter.setClipPath(path);
        QPixmap scaled = def.scaled(44, 44, Qt::KeepAspectRatioByExpanding, Qt::SmoothTransformation);
        painter.drawPixmap((44 - scaled.width()) / 2, (44 - scaled.height()) / 2, scaled);
        return target;
    }
    QPixmap rounded;
    if (CoverLoader::instance().tryGetRounded(coverPath, 44, 8, rounded)) {
        return rounded;
    }

    QPixmap fallback;
    CoverLoader::instance().resolveOrFallback(coverPath, 44, fallback);
    if (requestAsync && !coverPath.isEmpty()) {
        CoverLoader::instance().requestAsync(coverPath, 44);
    }
    QPixmap target(44, 44);
    target.fill(Qt::transparent);
    QPainter painter(&target);
    painter.setRenderHint(QPainter::Antialiasing, true);
    painter.setRenderHint(QPainter::SmoothPixmapTransform, true);
    QPainterPath path;
    path.addRoundedRect(0, 0, 44, 44, 8, 8);
    painter.setClipPath(path);
    QPixmap scaled = fallback.scaled(44, 44, Qt::KeepAspectRatioByExpanding, Qt::SmoothTransformation);
    painter.drawPixmap((44 - scaled.width()) / 2, (44 - scaled.height()) / 2, scaled);
    return target;
}

} // namespace

SongsTableWidget::SongsTableWidget(QWidget* parent) : QWidget(parent) {
    setupUi();
}

void SongsTableWidget::setupUi() {
    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);

    auto* cardFrame = new QFrame(this);
    cardFrame->setObjectName("SongsCard");

    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this](const ThemePalette& p) {
        QIcon actionIcon = ThemeManager::tintedIcon(":/resources/icons/more.png", p.iconColor);
        for (QPushButton* btn : m_actionButtons) {
            if (btn) btn->setIcon(actionIcon);
        }
        if (auto* h = m_table->horizontalHeaderItem(5)) {
            h->setIcon(ThemeManager::tintedIcon(":/resources/icons/recently_played.png", p.iconColor));
        }
        if (auto* h = m_table->horizontalHeaderItem(6)) {
            h->setIcon(ThemeManager::tintedIcon(":/resources/icons/favorite.png", p.iconColor));
        }
        if (m_table && m_table->viewport()) {
            m_table->viewport()->update();
        }
    });

    auto* cardLayout = new QVBoxLayout(cardFrame);
    cardLayout->setContentsMargins(16, 16, 16, 16);
    cardLayout->setSpacing(12);

    auto* headerLayout = new QHBoxLayout();
    headerLayout->setContentsMargins(0, 0, 0, 5);

    m_backBtn = new QPushButton("‹  Back", this);
    m_backBtn->setCursor(Qt::PointingHandCursor);
    m_backBtn->setVisible(false);
    connect(m_backBtn, &QPushButton::clicked, this, &SongsTableWidget::backButtonClicked);

    auto updateBackBtnStyle = [this](const ThemePalette& p) {
        if (m_backBtn) m_backBtn->setStyleSheet(QString(
            "QPushButton { background: transparent; border: none; color: %1; font-size: 15px; font-weight: 600; padding: 0px 8px 0px 0px; margin-right: 4px; }"
            "QPushButton:hover { color: %2; }"
        ).arg(p.secondaryAccent.name(), p.primaryAccent.name()));
    };
    updateBackBtnStyle(ThemeManager::instance().currentTheme());
    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [updateBackBtnStyle](const ThemePalette& p) {
        updateBackBtnStyle(p);
    });

    auto* songsLabel = new QLabel("Songs", this);
    songsLabel->setObjectName("ContentHeader");

    auto* songCountLabel = new QLabel("0 songs", this);
    songCountLabel->setObjectName("ContentSubHeader");
    m_songCountLabel = songCountLabel;

    headerLayout->addWidget(m_backBtn);
    headerLayout->addWidget(songsLabel);
    headerLayout->addWidget(songCountLabel);
    headerLayout->addStretch();

    auto* sortCombo = new QComboBox(this);
    ThemeManager::setupComboBox(sortCombo);
    sortCombo->addItem("Sort by: Title");
    sortCombo->addItem("Sort by: Artist");
    sortCombo->addItem("Sort by: Mood");
    sortCombo->addItem("Sort by: Date");
    sortCombo->setToolTip("Sort Library Songs by Title, Artist, Mood, or Date Added");
    connect(sortCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this, [this](int idx) {
        if (m_rows.isEmpty()) return;
        if (idx == 0) {
            std::sort(m_rows.begin(), m_rows.end(), [](const SongRow& a, const SongRow& b) {
                return a.title.localeAwareCompare(b.title) < 0;
            });
        } else if (idx == 1) {
            std::sort(m_rows.begin(), m_rows.end(), [](const SongRow& a, const SongRow& b) {
                return a.artist.localeAwareCompare(b.artist) < 0;
            });
        } else if (idx == 2) {
            std::sort(m_rows.begin(), m_rows.end(), [](const SongRow& a, const SongRow& b) {
                if (a.mood.isEmpty() != b.mood.isEmpty()) {
                    return !a.mood.isEmpty();
                }
                return a.mood.localeAwareCompare(b.mood) < 0;
            });
        } else if (idx == 3) {
            std::sort(m_rows.begin(), m_rows.end(), [](const SongRow& a, const SongRow& b) {
                return a.songId < b.songId;
            });
        }
        for (int i = 0; i < m_rows.size(); ++i) {
            m_rows[i].displayIndex = i + 1;
        }
        setSongsBatch(m_rows);
    });
    headerLayout->addWidget(sortCombo);

    auto* listBtn = new QPushButton(this);
    listBtn->setObjectName("IconButton");
    listBtn->setCheckable(true);
    listBtn->setChecked(true);
    listBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/list.png",
        ThemeManager::instance().currentTheme().iconColor));
    listBtn->setIconSize(QSize(16, 16));
    listBtn->setFixedSize(30, 30);
    listBtn->setToolTip("Switch to Table List View");
    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [listBtn](const ThemePalette& p) {
        listBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/list.png", p.iconColor));
    });

    auto* gridBtn = new QPushButton(this);
    gridBtn->setObjectName("IconButton");
    gridBtn->setCheckable(true);
    gridBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/grid.png",
        ThemeManager::instance().currentTheme().iconColor));
    gridBtn->setIconSize(QSize(16, 16));
    gridBtn->setFixedSize(30, 30);
    gridBtn->setToolTip("Switch to Album Card Grid View");
    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [gridBtn](const ThemePalette& p) {
        gridBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/grid.png", p.iconColor));
    });

    auto* viewGroup = new QButtonGroup(this);
    viewGroup->setExclusive(true);
    viewGroup->addButton(listBtn);
    viewGroup->addButton(gridBtn);

    headerLayout->addWidget(listBtn);
    headerLayout->addWidget(gridBtn);
    cardLayout->addLayout(headerLayout);

    m_stackedWidget = new QStackedWidget(this);

    m_table = new QTableWidget(this);
    m_table->setColumnCount(8);
    m_table->setShowGrid(false);
    m_table->setAlternatingRowColors(false);
    m_table->setSelectionBehavior(QAbstractItemView::SelectRows);
    m_table->setSelectionMode(QAbstractItemView::SingleSelection);
    m_table->setFocusPolicy(Qt::NoFocus);
    m_table->setVerticalScrollMode(QAbstractItemView::ScrollPerPixel);
    m_table->verticalScrollBar()->setSingleStep(m_table->fontMetrics().lineSpacing() * 2);
    m_table->setStyleSheet(
        "QTableWidget { background-color: transparent; border: none; outline: none; }"
        "QHeaderView::section { background-color: transparent; color: #7E8494; font-weight: bold; padding: 6px; border: none; border-bottom: 1px solid #1E2538; }"
        "QTableWidget::item { border-bottom: 1px solid rgba(255,255,255,0.04); }"
    );

    QStringList headers = {"#", "Title", "Mood", "Artist", "Album", "", "", ""};
    m_table->setHorizontalHeaderLabels(headers);
    m_table->horizontalHeader()->setDefaultAlignment(Qt::AlignCenter);
    m_table->horizontalHeader()->setHighlightSections(false);
    m_table->horizontalHeader()->setFixedHeight(38);
    m_table->verticalHeader()->setVisible(false);
    m_table->verticalHeader()->setDefaultSectionSize(54);

    for (int i = 0; i < m_table->columnCount(); ++i) {
        if (auto* h = m_table->horizontalHeaderItem(i)) {
            h->setTextAlignment(Qt::AlignCenter);
        }
    }
    const auto& currentTheme = ThemeManager::instance().currentTheme();
    if (auto* h = m_table->horizontalHeaderItem(5)) {
        h->setIcon(ThemeManager::tintedIcon(":/resources/icons/recently_played.png", currentTheme.iconColor));
        h->setTextAlignment(Qt::AlignCenter);
    }
    if (auto* h = m_table->horizontalHeaderItem(6)) {
        h->setIcon(ThemeManager::tintedIcon(":/resources/icons/favorite.png", currentTheme.iconColor));
        h->setTextAlignment(Qt::AlignCenter);
    }

    m_table->horizontalHeader()->setSectionResizeMode(0, QHeaderView::Fixed);
    m_table->setColumnWidth(0, 54);

    m_table->horizontalHeader()->setSectionResizeMode(1, QHeaderView::Stretch);

    m_table->horizontalHeader()->setSectionResizeMode(2, QHeaderView::Interactive);
    m_table->setColumnWidth(2, 110);

    m_table->horizontalHeader()->setSectionResizeMode(3, QHeaderView::Interactive);
    m_table->setColumnWidth(3, 130);

    m_table->horizontalHeader()->setSectionResizeMode(4, QHeaderView::Interactive);
    m_table->setColumnWidth(4, 130);

    m_table->horizontalHeader()->setSectionResizeMode(5, QHeaderView::Fixed);
    m_table->setColumnWidth(5, 55);

    m_table->horizontalHeader()->setSectionResizeMode(6, QHeaderView::Fixed);
    m_table->setColumnWidth(6, 40);

    m_table->horizontalHeader()->setSectionResizeMode(7, QHeaderView::Fixed);
    m_table->setColumnWidth(7, 48);

    m_table->setColumnHidden(2, !AppSettings::instance().isMoodColumnEnabled());
    initTableDelegate();

    m_stackedWidget->addWidget(m_table);

    m_gridWidget = new MediaGridWidget(this);
    m_stackedWidget->addWidget(m_gridWidget);

    cardLayout->addWidget(m_stackedWidget, 1);
    mainLayout->addWidget(cardFrame, 1);

    connect(listBtn, &QPushButton::clicked, this, [this]() {
        m_stackedWidget->setCurrentIndex(0);
    });
    connect(gridBtn, &QPushButton::clicked, this, [this]() {
        if (!m_gridPopulated) {
            populateGridFromTable();
        }
        m_stackedWidget->setCurrentIndex(1);
        QTimer::singleShot(0, this, &SongsTableWidget::updateGridResponsive);
    });

    m_table->setMouseTracking(true);
    m_table->viewport()->setMouseTracking(true);
    m_table->viewport()->installEventFilter(this);

    connect(m_table, &QTableWidget::cellEntered, this, &SongsTableWidget::onCellEntered);
    connect(m_table, &QTableWidget::itemSelectionChanged, this, &SongsTableWidget::updateRowStyles);

    connect(m_table, &QTableWidget::cellClicked, this, [this](int row, int col) {
        Q_UNUSED(col);
        if (auto* item = m_table->item(row, 0)) {
            bool ok = false;
            int songId = item->data(Qt::UserRole).toInt(&ok);
            if (ok) {
                emit songSelected(songId);
                return;
            }
        }
        emit songSelected(row);
    });
    connect(m_table, &QTableWidget::cellDoubleClicked, this, [this](int row, int col) {
        Q_UNUSED(col);
        if (auto* item = m_table->item(row, 0)) {
            bool ok = false;
            int songId = item->data(Qt::UserRole).toInt(&ok);
            if (ok) {
                emit songSelected(songId);
                return;
            }
        }
        emit songSelected(row);
    });

    m_table->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(m_table, &QTableWidget::customContextMenuRequested, this, [this](const QPoint& pos) {
        QTableWidgetItem* item = m_table->itemAt(pos);
        if (!item) return;
        int row = item->row();
        int songId = m_table->item(row, 0) ? m_table->item(row, 0)->data(Qt::UserRole).toInt() : -1;
        if (songId == -1) return;

        QMenu menu;
        QAction* addToQueueAct = menu.addAction(tr("Add to Queue"));
        QMenu* addToPlaylistMenu = menu.addMenu(tr("Add to Playlist"));
        populateAddToPlaylistMenu(addToPlaylistMenu, songId);
        QAction* editTagsAct = menu.addAction(tr("Edit Metadata Tags..."));
        QAction* scanLoudnessAct = menu.addAction(tr("Scan ReplayGain / Loudness..."));
        QAction* infoAct = menu.addAction(tr("Track Info"));
        menu.addSeparator();
        QAction* removeFromLibAct = menu.addAction(tr("Remove from Library"));
        QAction* chosen = menu.exec(m_table->viewport()->mapToGlobal(pos));
        if (chosen == editTagsAct) {
            openTagEditorDialog(row);
        } else if (chosen == scanLoudnessAct) {
            QVector<int> targetIds;
            for (auto* item : m_table->selectedItems()) {
                if (item->column() == 0) {
                    bool ok = false;
                    int id = item->data(Qt::UserRole).toInt(&ok);
                    if (ok && !targetIds.contains(id)) targetIds.append(id);
                }
            }
            if (targetIds.isEmpty()) {
                targetIds.append(songId);
            }
            openLoudnessScannerDialog(targetIds);
        } else if (chosen == addToQueueAct) {
            qDebug() << "Add to Queue clicked for song" << songId;
        } else if (chosen == infoAct) {
            openTagEditorDialog(row);
        } else if (chosen == removeFromLibAct) {
            const auto& cb = GuiBridgeManager::instance().callbacks();
            if (cb.on_remove_from_library) cb.on_remove_from_library(songId);
        }
    });

    connect(m_gridWidget, &MediaGridWidget::cardActivated, this, [this](int songId) {
        int row = m_songIdToRow.value(songId, -1);
        if (row >= 0) emit songSelected(row);
    });

    connect(&CoverLoader::instance(), &CoverLoader::coverReady,
            this, [this](const QString& path, int size, const QPixmap&) {
        Q_UNUSED(size);
        if (path.isEmpty() || m_stackedWidget->currentIndex() != 0) return;
        QRect visible = m_table->viewport()->rect();
        int top = m_table->rowAt(visible.top());
        int bot = m_table->rowAt(visible.bottom());
        if (top < 0) top = 0;
        if (bot < 0) bot = m_table->rowCount() - 1;
        for (int row = top; row <= bot; ++row) {
            if (auto* firstItem = m_table->item(row, 0)) {
                QString rowPath = firstItem->data(Qt::UserRole + 1).toString();
                if (rowPath == path) {
                    if (auto* titleCont = m_table->cellWidget(row, 1)) {
                        auto labels = titleCont->findChildren<QLabel*>();
                        for (QLabel* l : labels) {
                            if (l->objectName() != "SongTitleLabel") {
                                l->setPixmap(loadThumbnail(path));
                                break;
                            }
                        }
                    }
                }
            }
        }
    }, Qt::QueuedConnection);

    connect(m_table->verticalScrollBar(), &QScrollBar::valueChanged, this, [this]() {
        QTimer::singleShot(40, this, &SongsTableWidget::loadVisibleThumbnails);
    });
}

void SongsTableWidget::clearSongs() {
    m_actionButtons.clear();
    m_eqIcons.clear();
    m_songIdToRow.clear();
    m_rows.clear();
    m_gridPopulated = false;
    m_table->setRowCount(0);
    m_gridWidget->clearGrid();
    m_playingTrackIdx = -1;
    m_hoveredRow = -1;
    m_previousHoveredRow = -1;
    m_songCountDirty = false;
    if (m_songCountLabel) {
        m_songCountLabel->setText("0 songs");
    }
}

void SongsTableWidget::setOptimizedMode(bool enabled) {
    if (enabled) {
        CoverLoader::instance().clearCache();
        CoverLoader::instance().setCacheLimitKb(2048);
        int rows = m_table->rowCount();
        for (int row = 0; row < rows; ++row) {
            if (auto* titleCont = m_table->cellWidget(row, 1)) {
                auto labels = titleCont->findChildren<QLabel*>();
                for (QLabel* l : labels) {
                    if (l->objectName() != "SongTitleLabel") {
                        l->setPixmap(loadThumbnail(""));
                        l->setVisible(true);
                        break;
                    }
                }
            }
        }
    } else {
        CoverLoader::instance().setCacheLimitKb(15 * 1024);
        loadVisibleThumbnails();
    }
    if (m_gridPopulated) {
        m_gridWidget->setOptimizedMode(enabled);
    }
}

void SongsTableWidget::addSong(int index, int songId, bool isFavorite, const QString& title, const QString& artist, const QString& album, const QString& duration, const QString& coverPath) {
    SongRow row;
    row.displayIndex = index;
    row.songId = songId;
    row.isFavorite = isFavorite;
    row.title = title;
    row.artist = artist;
    row.album = album;
    row.duration = duration;
    row.coverPath = coverPath;
    m_rows.append(row);

    int rowIdx = m_table->rowCount();
    m_table->insertRow(rowIdx);
    m_table->setRowHeight(rowIdx, 54);

    m_songIdToRow.insert(songId, rowIdx);

    QString indexText = index > 0 ? QString::number(index) : QString("-");
    auto* itemIndex = new QTableWidgetItem(indexText);
    itemIndex->setData(Qt::UserRole, songId);
    itemIndex->setData(Qt::UserRole + 1, coverPath);
    itemIndex->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    itemIndex->setFlags(itemIndex->flags() ^ Qt::ItemIsEditable);
    m_table->setItem(rowIdx, 0, itemIndex);

    auto* titleContainer = new QWidget(this);
    auto* titleLayout = new QHBoxLayout(titleContainer);
    titleLayout->setContentsMargins(5, 0, 5, 0);
    titleLayout->setSpacing(12);
    titleLayout->setAlignment(Qt::AlignVCenter);

    auto* thumbLabel = new QLabel(titleContainer);
    thumbLabel->setObjectName("SongRowThumbLabel");
    thumbLabel->setProperty("coverPath", coverPath);
    thumbLabel->setFixedSize(44, 44);
    thumbLabel->setAlignment(Qt::AlignCenter);
    thumbLabel->setScaledContents(false);
    thumbLabel->setPixmap(loadThumbnail(coverPath, true));

    auto* titleLabel = new QLabel(title, titleContainer);
    titleLabel->setObjectName("SongTitleLabel");

    auto* eqIcon = new PlayingEqualizerIcon(titleContainer);
    eqIcon->setFixedSize(16, 12);
    eqIcon->setVisible(false);
    m_eqIcons.append(eqIcon);

    titleLayout->addWidget(thumbLabel);
    titleLayout->addWidget(titleLabel);
    titleLayout->addWidget(eqIcon);
    titleLayout->addStretch();

    m_table->setCellWidget(rowIdx, 1, titleContainer);
    titleContainer->setAttribute(Qt::WA_TransparentForMouseEvents);

    QString moodStr = (!m_rows.isEmpty()) ? m_rows.last().mood : "";
    auto* moodContainer = new QWidget(this);
    moodContainer->setAttribute(Qt::WA_TransparentForMouseEvents);
    auto* moodLayout = new QHBoxLayout(moodContainer);
    moodLayout->setContentsMargins(0, 0, 0, 0);
    moodLayout->setAlignment(Qt::AlignCenter);
    if (!moodStr.isEmpty()) {
        auto* moodBadge = new QLabel(moodContainer);
        moodBadge->setObjectName("SongMoodBadge");
        moodBadge->setProperty("mood", moodStr.toLower().trimmed());
        moodBadge->setText(moodStr.toUpper().trimmed());
        moodLayout->addWidget(moodBadge);
    }
    m_table->setCellWidget(rowIdx, 2, moodContainer);

    auto* itemArtist = new QTableWidgetItem(artist);
    itemArtist->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    itemArtist->setFlags(itemArtist->flags() ^ Qt::ItemIsEditable);
    m_table->setItem(rowIdx, 3, itemArtist);

    auto* itemAlbum = new QTableWidgetItem(album);
    itemAlbum->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    itemAlbum->setFlags(itemAlbum->flags() ^ Qt::ItemIsEditable);
    m_table->setItem(rowIdx, 4, itemAlbum);

    auto* itemDuration = new QTableWidgetItem(duration);
    itemDuration->setTextAlignment(Qt::AlignCenter);
    itemDuration->setFlags(itemDuration->flags() ^ Qt::ItemIsEditable);
    m_table->setItem(rowIdx, 5, itemDuration);

    auto* favBtn = new QPushButton(isFavorite ? "♥" : "♡", this);
    favBtn->setObjectName("FavBtn");
    favBtn->setCursor(Qt::PointingHandCursor);
    favBtn->setFocusPolicy(Qt::NoFocus);
    favBtn->setProperty("rowIdx", rowIdx);
    favBtn->setProperty("colIdx", 6);
    favBtn->setProperty("favorite", isFavorite);
    favBtn->installEventFilter(this);
    favBtn->setToolTip(isFavorite ? "Remove from Favorites" : "Add to Favorites");
    connect(favBtn, &QPushButton::clicked, this, [this, songId, favBtn, rowIdx]() {
        m_table->clearSelection();
        bool currentlyFav = (favBtn->text() == "♥");
        bool newFav = !currentlyFav;
        favBtn->setText(newFav ? "♥" : "♡");
        favBtn->setProperty("favorite", newFav);
        favBtn->style()->unpolish(favBtn);
        favBtn->style()->polish(favBtn);
        favBtn->setToolTip(newFav ? "Remove from Favorites" : "Add to Favorites");
        const auto& cb = GuiBridgeManager::instance().callbacks();
        if (cb.on_toggle_favorite) cb.on_toggle_favorite(songId);
        refreshSingleRowStyle(rowIdx);
    });
    auto* favContainer = new QWidget(this);
    favContainer->setFocusPolicy(Qt::NoFocus);
    favContainer->setProperty("rowIdx", rowIdx);
    favContainer->setProperty("colIdx", 6);
    favContainer->installEventFilter(this);
    auto* favLayout = new QHBoxLayout(favContainer);
    favLayout->setContentsMargins(0, 0, 0, 0);
    favLayout->setAlignment(Qt::AlignCenter);
    favLayout->addWidget(favBtn);
    m_table->setCellWidget(rowIdx, 6, favContainer);

    auto* actionBtn = new QPushButton(this);
    actionBtn->setObjectName("IconButton");
    actionBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/more.png", ThemeManager::instance().currentTheme().iconColor));
    actionBtn->setIconSize(QSize(14, 14));
    actionBtn->setFixedSize(26, 26);
    actionBtn->setFocusPolicy(Qt::NoFocus);
    actionBtn->setProperty("rowIdx", rowIdx);
    actionBtn->setProperty("colIdx", 7);
    actionBtn->installEventFilter(this);
    actionBtn->setToolTip("Track Options & Actions");
    connect(actionBtn, &QPushButton::clicked, this, [this, songId, rowIdx]() {
        m_table->clearSelection();
        onCellEntered(rowIdx, 7);
        QMenu menu;
        QAction* addToQueueAct = menu.addAction(tr("Add to Queue"));
        QMenu* addToPlaylistMenu = menu.addMenu(tr("Add to Playlist"));
        populateAddToPlaylistMenu(addToPlaylistMenu, songId);
        QAction* editTagsAct = menu.addAction(tr("Edit Metadata Tags..."));
        QAction* scanLoudnessAct = menu.addAction(tr("Scan ReplayGain / Loudness..."));
        QAction* infoAct = menu.addAction(tr("Track Info"));
        menu.addSeparator();
        QAction* removeFromLibAct = menu.addAction(tr("Remove from Library"));
        QAction* chosen = menu.exec(QCursor::pos());
        m_table->clearSelection();
        refreshSingleRowStyle(rowIdx);
        if (chosen == editTagsAct) {
            openTagEditorDialog(rowIdx);
        } else if (chosen == scanLoudnessAct) {
            openLoudnessScannerDialog(QVector<int>() << songId);
        } else if (chosen == addToQueueAct) {
            qDebug() << "Add to Queue clicked for song" << songId;
        } else if (chosen == infoAct) {
            openTagEditorDialog(rowIdx);
        } else if (chosen == removeFromLibAct) {
            const auto& cb1 = GuiBridgeManager::instance().callbacks();
            if (cb1.on_remove_from_library) cb1.on_remove_from_library(songId);
        }
    });
    auto* actionContainer = new QWidget(this);
    actionContainer->setFocusPolicy(Qt::NoFocus);
    actionContainer->setProperty("rowIdx", rowIdx);
    actionContainer->setProperty("colIdx", 7);
    actionContainer->installEventFilter(this);
    auto* actionLayout = new QHBoxLayout(actionContainer);
    actionLayout->setContentsMargins(0, 0, 0, 0);
    actionLayout->setAlignment(Qt::AlignCenter);
    actionLayout->addWidget(actionBtn);
    m_table->setCellWidget(rowIdx, 7, actionContainer);
    m_actionButtons.append(actionBtn);

    titleContainer->setAttribute(Qt::WA_TransparentForMouseEvents);

    if (songId == m_playingSongId && m_playingSongId != -1) {
        m_playingTrackIdx = rowIdx;
        eqIcon->setVisible(true);
        eqIcon->setPlaying(m_isPlaying);
        titleLabel->setProperty("playing", true);
        titleLabel->style()->unpolish(titleLabel);
        titleLabel->style()->polish(titleLabel);
    } else {
        titleLabel->setProperty("playing", false);
    }

    if (m_songCountLabel) {
        m_songCountDirty = true;
        QTimer::singleShot(100, this, &SongsTableWidget::flushSongCount);
    }

    if (m_gridPopulated) {
        m_gridWidget->addCard(songId, title, artist, coverPath);
    }
}

void SongsTableWidget::setSongsBatch(QVector<SongRow> rows) {
    m_table->setUpdatesEnabled(false);
    if (m_table->viewport()) m_table->viewport()->setUpdatesEnabled(false);
    m_table->blockSignals(true);

    const bool wasGridPopulated = m_gridPopulated;

    clearSongs();
    m_rows = std::move(rows);
    int n = m_rows.size();
    if (n > 0) {
        m_table->setRowCount(n);
        for (int i = 0; i < n; ++i) {
            const SongRow& r = m_rows[i];
            m_table->setRowHeight(i, 54);
            m_songIdToRow.insert(r.songId, i);

            QString indexText = r.displayIndex > 0 ? QString::number(r.displayIndex) : QString("-");
            auto* itemIndex = new QTableWidgetItem(indexText);
            itemIndex->setData(Qt::UserRole, r.songId);
            itemIndex->setData(Qt::UserRole + 1, r.coverPath);
            itemIndex->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
            itemIndex->setFlags(itemIndex->flags() ^ Qt::ItemIsEditable);
            m_table->setItem(i, 0, itemIndex);

            auto* titleContainer = new QWidget(this);
            auto* titleLayout = new QHBoxLayout(titleContainer);
            titleLayout->setContentsMargins(5, 0, 5, 0);
            titleLayout->setSpacing(12);
            titleLayout->setAlignment(Qt::AlignVCenter);

            auto* thumbLabel = new QLabel(titleContainer);
            thumbLabel->setObjectName("SongRowThumbLabel");
            thumbLabel->setProperty("coverPath", r.coverPath);
            thumbLabel->setFixedSize(44, 44);
            thumbLabel->setAlignment(Qt::AlignCenter);
            thumbLabel->setScaledContents(false);
            thumbLabel->setPixmap(loadThumbnail(r.coverPath, false));

            auto* titleLabel = new QLabel(r.title, titleContainer);
            titleLabel->setObjectName("SongTitleLabel");

            auto* eqIcon = new PlayingEqualizerIcon(titleContainer);
            eqIcon->setFixedSize(16, 12);
            eqIcon->setVisible(false);
            m_eqIcons.append(eqIcon);

            titleLayout->addWidget(thumbLabel);
            titleLayout->addWidget(titleLabel);
            titleLayout->addWidget(eqIcon);
            titleLayout->addStretch();

            m_table->setCellWidget(i, 1, titleContainer);
            titleContainer->setAttribute(Qt::WA_TransparentForMouseEvents);

            auto* moodContainer = new QWidget(this);
            moodContainer->setAttribute(Qt::WA_TransparentForMouseEvents);
            auto* moodLayout = new QHBoxLayout(moodContainer);
            moodLayout->setContentsMargins(0, 0, 0, 0);
            moodLayout->setAlignment(Qt::AlignCenter);
            if (!r.mood.isEmpty()) {
                auto* moodBadge = new QLabel(moodContainer);
                moodBadge->setObjectName("SongMoodBadge");
                moodBadge->setProperty("mood", r.mood.toLower().trimmed());
                moodBadge->setText(r.mood.toUpper().trimmed());
                moodLayout->addWidget(moodBadge);
            }
            m_table->setCellWidget(i, 2, moodContainer);

            auto* itemArtist = new QTableWidgetItem(r.artist);
            itemArtist->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
            itemArtist->setFlags(itemArtist->flags() ^ Qt::ItemIsEditable);
            m_table->setItem(i, 3, itemArtist);

            auto* itemAlbum = new QTableWidgetItem(r.album);
            itemAlbum->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
            itemAlbum->setFlags(itemAlbum->flags() ^ Qt::ItemIsEditable);
            m_table->setItem(i, 4, itemAlbum);

            auto* itemDuration = new QTableWidgetItem(r.duration);
            itemDuration->setTextAlignment(Qt::AlignCenter);
            itemDuration->setFlags(itemDuration->flags() ^ Qt::ItemIsEditable);
            m_table->setItem(i, 5, itemDuration);

            auto* favBtn = new QPushButton(r.isFavorite ? "♥" : "♡", this);
            favBtn->setObjectName("FavBtn");
            favBtn->setCursor(Qt::PointingHandCursor);
            favBtn->setFocusPolicy(Qt::NoFocus);
            favBtn->setProperty("rowIdx", i);
            favBtn->setProperty("colIdx", 6);
            favBtn->setProperty("favorite", r.isFavorite);
            favBtn->installEventFilter(this);
            favBtn->setToolTip(r.isFavorite ? "Remove from Favorites" : "Add to Favorites");
            connect(favBtn, &QPushButton::clicked, this, [this, songId = r.songId, favBtn, rowIdx = i]() {
                m_table->clearSelection();
                bool currentlyFav = (favBtn->text() == "♥");
                bool newFav = !currentlyFav;
                favBtn->setText(newFav ? "♥" : "♡");
                favBtn->setProperty("favorite", newFav);
                favBtn->style()->unpolish(favBtn);
                favBtn->style()->polish(favBtn);
                favBtn->setToolTip(newFav ? "Remove from Favorites" : "Add to Favorites");
                const auto& cb = GuiBridgeManager::instance().callbacks();
                if (cb.on_toggle_favorite) cb.on_toggle_favorite(songId);
                refreshSingleRowStyle(rowIdx);
            });
            auto* favContainer = new QWidget(this);
            favContainer->setFocusPolicy(Qt::NoFocus);
            favContainer->setProperty("rowIdx", i);
            favContainer->setProperty("colIdx", 6);
            favContainer->installEventFilter(this);
            auto* favLayout = new QHBoxLayout(favContainer);
            favLayout->setContentsMargins(0, 0, 0, 0);
            favLayout->setAlignment(Qt::AlignCenter);
            favLayout->addWidget(favBtn);
            m_table->setCellWidget(i, 6, favContainer);

            auto* actionBtn = new QPushButton(this);
            actionBtn->setObjectName("IconButton");
            actionBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/more.png", ThemeManager::instance().currentTheme().iconColor));
            actionBtn->setIconSize(QSize(14, 14));
            actionBtn->setFixedSize(26, 26);
            actionBtn->setFocusPolicy(Qt::NoFocus);
            actionBtn->setProperty("rowIdx", i);
            actionBtn->setProperty("colIdx", 7);
            actionBtn->installEventFilter(this);
            actionBtn->setToolTip("Track Options & Actions");
            connect(actionBtn, &QPushButton::clicked, this, [this, songId = r.songId, rowIdx = i]() {
                m_table->clearSelection();
                onCellEntered(rowIdx, 7);
                QMenu menu;
                QAction* addToQueueAct = menu.addAction(tr("Add to Queue"));
                QMenu* addToPlaylistMenu = menu.addMenu(tr("Add to Playlist"));
                populateAddToPlaylistMenu(addToPlaylistMenu, songId);
                QAction* editTagsAct = menu.addAction(tr("Edit Metadata Tags..."));
                QAction* scanLoudnessAct = menu.addAction(tr("Scan ReplayGain / Loudness..."));
                QAction* infoAct = menu.addAction(tr("Track Info"));
                menu.addSeparator();
                QAction* removeFromLibAct = menu.addAction(tr("Remove from Library"));
                QAction* chosen = menu.exec(QCursor::pos());
                m_table->clearSelection();
                refreshSingleRowStyle(rowIdx);
                if (chosen == editTagsAct) {
                    openTagEditorDialog(rowIdx);
                } else if (chosen == scanLoudnessAct) {
                    openLoudnessScannerDialog(QVector<int>() << songId);
                } else if (chosen == addToQueueAct) {
                    qDebug() << "Add to Queue clicked for song" << songId;
                } else if (chosen == infoAct) {
                    openTagEditorDialog(rowIdx);
                } else if (chosen == removeFromLibAct) {
                    const auto& cb1 = GuiBridgeManager::instance().callbacks();
                    if (cb1.on_remove_from_library) cb1.on_remove_from_library(songId);
                }
            });
            auto* actionContainer = new QWidget(this);
            actionContainer->setFocusPolicy(Qt::NoFocus);
            actionContainer->setProperty("rowIdx", i);
            actionContainer->setProperty("colIdx", 7);
            actionContainer->installEventFilter(this);
            auto* actionLayout = new QHBoxLayout(actionContainer);
            actionLayout->setContentsMargins(0, 0, 0, 0);
            actionLayout->setAlignment(Qt::AlignCenter);
            actionLayout->addWidget(actionBtn);
            m_table->setCellWidget(i, 7, actionContainer);
            m_actionButtons.append(actionBtn);

            if (r.songId == m_playingSongId && m_playingSongId != -1) {
                m_playingTrackIdx = i;
                eqIcon->setVisible(true);
                eqIcon->setPlaying(m_isPlaying);
                titleLabel->setProperty("playing", true);
                titleLabel->style()->unpolish(titleLabel);
                titleLabel->style()->polish(titleLabel);
            } else {
                titleLabel->setProperty("playing", false);
            }
        }
    }

    if (m_playingTrackIdx >= 0) {
        refreshSingleRowStyle(m_playingTrackIdx);
        if (AppSettings::instance().isCursorFollowsPlayback()) {
            QTimer::singleShot(60, this, &SongsTableWidget::scrollToActive);
        }
    }

    m_table->blockSignals(false);
    m_table->setUpdatesEnabled(true);
    if (m_table->viewport()) m_table->viewport()->setUpdatesEnabled(true);

    if (m_songCountLabel) {
        m_songCountLabel->setText(QString::number(n) + " songs");
    }
    m_songCountDirty = false;

    if (wasGridPopulated) {
        populateGridFromTable();
    }

    if (m_table->viewport()) m_table->viewport()->update();
    QTimer::singleShot(0, this, &SongsTableWidget::loadVisibleThumbnails);
}

void SongsTableWidget::flushSongCount() {
    if (m_songCountLabel && m_songCountDirty) {
        m_songCountDirty = false;
        m_songCountLabel->setText(QString::number(m_table->rowCount()) + " songs");
    }
}
