#include "songstable.h"
#include "gui_bridge_p.h"
#include "custom_widgets.h"
#include "coverloader.h"
#include "appsettings.h"
#include "tageditordialog.h"
#include "loudnessscannerdialog.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QComboBox>
#include <QPushButton>
#include <QIcon>
#include <QPainter>
#include <QStyle>
#include <QTableWidgetItem>
#include <cstdlib>
#include <random>
#include <QTimerEvent>
#include <QButtonGroup>
#include <QApplication>
#include <QCursor>
#include <QPainterPath>
#include <QStyledItemDelegate>
#include <QMenu>
#include <QAction>
#include <QDebug>
#include <QSettings>
#include <QSet>
#include <QTimer>
#include <QShowEvent>
#include <QScrollBar>
#include <QFile>
#include <QPixmapCache>
#include <QMutex>

// ===========================================================================
// Local helpers
// ===========================================================================

namespace {

static void applyMoodPillStyle(QLabel* badge, const QString& moodName) {
    if (!badge || moodName.trimmed().isEmpty()) return;
    QString lower = moodName.toLower().trimmed();

    QString bg = "rgba(168, 85, 247, 0.22)";
    QString border = "rgba(192, 132, 252, 0.65)";
    QString text = "#F3E8FF";

    if (lower == "energetic") {
        bg = "rgba(124, 58, 237, 0.25)";
        border = "rgba(167, 139, 250, 0.70)";
        text = "#E9D5FF";
    } else if (lower == "romantic") {
        bg = "rgba(236, 72, 153, 0.25)";
        border = "rgba(244, 114, 182, 0.70)";
        text = "#FBCFE8";
    } else if (lower == "happy") {
        bg = "rgba(234, 179, 8, 0.25)";
        border = "rgba(250, 204, 21, 0.70)";
        text = "#FEF08A";
    } else if (lower == "calm") {
        bg = "rgba(6, 182, 212, 0.25)";
        border = "rgba(56, 189, 248, 0.70)";
        text = "#BAE6FD";
    } else if (lower == "party") {
        bg = "rgba(168, 85, 247, 0.25)";
        border = "rgba(192, 132, 252, 0.70)";
        text = "#F3E8FF";
    } else if (lower == "nostalgic") {
        bg = "rgba(217, 119, 6, 0.25)";
        border = "rgba(251, 146, 60, 0.70)";
        text = "#FFEDD5";
    } else if (lower == "sad") {
        bg = "rgba(99, 102, 241, 0.25)";
        border = "rgba(129, 140, 248, 0.70)";
        text = "#E0E7FF";
    } else if (lower == "sleep" || lower == "lofi") {
        bg = "rgba(139, 92, 246, 0.25)";
        border = "rgba(167, 139, 250, 0.70)";
        text = "#EDE9FE";
    }

    badge->setText(moodName.toUpper().trimmed());
    badge->setStyleSheet(QString(
        "QLabel {"
        "   background-color: %1;"
        "   color: %2;"
        "   border: 1px solid %3;"
        "   border-radius: 6px;"
        "   padding: 3px 8px;"
        "   font-size: 10px;"
        "   font-weight: 700;"
        "}"
    ).arg(bg, text, border));
}

/// Build a square 44×44 rounded thumbnail for the songs table. Goes
/// through the shared CoverLoader cache so the same pixmap is reused
/// by the queue widget and the grid cards.
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

    // Cache miss: return fallback. Only trigger async load if requestAsync is true (visible rows).
    QPixmap fallback;
    CoverLoader::instance().resolveOrFallback(coverPath, 44, fallback);
    if (requestAsync && !coverPath.isEmpty()) {
        CoverLoader::instance().requestAsync(coverPath, 44);
    }
    // Build a rounded fallback from the default cover so it looks the
    // same shape as a real cover.
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

// ==========================================
// PlayingEqualizerIcon Implementation
// ==========================================
PlayingEqualizerIcon::PlayingEqualizerIcon(QWidget* parent) : QWidget(parent) {
    m_timerId = -1;
}

void PlayingEqualizerIcon::setPlaying(bool playing) {
    m_isPlaying = playing;
    if (m_isPlaying) {
        if (m_timerId == -1) m_timerId = startTimer(70);
    } else {
        if (m_timerId != -1) {
            killTimer(m_timerId);
            m_timerId = -1;
        }
        m_heights = {0.2f, 0.2f, 0.2f};
        update();
    }
}

void PlayingEqualizerIcon::timerEvent(QTimerEvent* event) {
    if (event->timerId() == m_timerId) {
        for (int i = 0; i < 3; ++i) {
            // Generate subtle random bounce targets between 0.15 and 0.85
            if (qAbs(m_heights[i] - m_targetHeights[i]) < 0.05f) {
                m_targetHeights[i] = 0.15f + static_cast<float>(std::rand() % 70) / 100.0f;
            }
            // Linear interpolate
            m_heights[i] = m_heights[i] * 0.6f + m_targetHeights[i] * 0.4f;
        }
        update();
    } else {
        QWidget::timerEvent(event);
    }
}

void PlayingEqualizerIcon::paintEvent(QPaintEvent* event) {
    Q_UNUSED(event);
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);

    double w = width();
    double h = height();
    double barW = 3.0;
    double spacing = 2.0;
    double startX = (w - (3 * barW + 2 * spacing)) / 2.0;

    const auto& p = ThemeManager::instance().currentTheme();
    painter.setBrush(p.secondaryAccent);
    painter.setPen(Qt::NoPen);

    for (int i = 0; i < 3; ++i) {
        double barH = qMax(2.0, static_cast<double>(m_heights[i]) * h);
        double x = startX + i * (barW + spacing);
        double y = h - barH;
        painter.drawRoundedRect(QRectF(x, y, barW, barH), 1.5, 1.5);
    }
}


// ==========================================
// SongTableRowDelegate Implementation
// ==========================================
class SongTableRowDelegate : public QStyledItemDelegate {
public:
    explicit SongTableRowDelegate(SongsTableWidget* owner, QObject* parent = nullptr)
        : QStyledItemDelegate(parent), m_owner(owner) {}

    void paint(QPainter* painter, const QStyleOptionViewItem& option, const QModelIndex& index) const override {
        painter->save();
        int row = index.row();
        bool isPlaying = (row == m_owner->playingTrackIdx());
        bool isHovered = (row == m_owner->hoveredRow());
        bool isSelected = false;
        if (auto* firstItem = m_owner->tableWidget()->item(row, 0)) {
            isSelected = firstItem->isSelected();
        }

        const auto& p = ThemeManager::instance().currentTheme();

        QColor bgColor = Qt::transparent;
        if (isPlaying) {
            bgColor = p.itemSelectedBg;
        } else if (isHovered) {
            bgColor = p.itemHoverBg;
        } else if (isSelected) {
            bgColor = p.itemSelectedBg;
        }

        if (bgColor.isValid() && bgColor != Qt::transparent) {
            painter->save();
            painter->setRenderHint(QPainter::Antialiasing, true);
            painter->setPen(Qt::NoPen);
            painter->setBrush(bgColor);

            int totalWidth = m_owner->tableWidget()->viewport()->width();
            QRectF fullRowRect(4, option.rect.top() + 2, totalWidth - 8, option.rect.height() - 4);

            painter->setClipRect(option.rect);
            painter->drawRoundedRect(fullRowRect, 10, 10);
            painter->restore();
        }

        QStyleOptionViewItem opt = option;
        // Clear default Qt selection & focus flags so no white/blue system box is ever drawn
        opt.state &= ~QStyle::State_Selected;
        opt.state &= ~QStyle::State_HasFocus;

        QColor textColor;
        if (isPlaying) {
            textColor = p.secondaryAccent;
        } else {
            textColor = p.secondaryText;
        }

        opt.palette.setColor(QPalette::Text, textColor);
        opt.palette.setColor(QPalette::WindowText, textColor);
        opt.palette.setColor(QPalette::HighlightedText, textColor);

        QStyledItemDelegate::paint(painter, opt, index);
        painter->restore();
    }

private:
    SongsTableWidget* m_owner = nullptr;
};

// ==========================================
// SongsTableWidget Implementation
// ==========================================
SongsTableWidget::SongsTableWidget(QWidget* parent) : QWidget(parent) {
    setupUi();
}

void SongsTableWidget::setupUi() {
    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);

    auto* cardFrame = new QFrame(this);
    cardFrame->setObjectName("SongsCard");

    auto applyCardStyle = [cardFrame](const ThemePalette& p) {
        cardFrame->setStyleSheet(QString(
            "QFrame#SongsCard {"
            "   background-color: %1;"
            "   border: 1px solid %2;"
            "   border-radius: 16px;"
            "}"
        ).arg(p.cardBg.name(), p.cardBorder.name()));
    };
    applyCardStyle(ThemeManager::instance().currentTheme());
    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this, applyCardStyle](const ThemePalette& p) {
        applyCardStyle(p);
        if (m_table) {
            int rows = m_table->rowCount();
            for (int r = 0; r < rows; ++r) {
                if (auto* titleWidget = m_table->cellWidget(r, 1)) {
                    if (auto* thumbLabel = titleWidget->findChild<QLabel*>("SongRowThumbLabel")) {
                        QString path = thumbLabel->property("coverPath").toString();
                        thumbLabel->setPixmap(loadThumbnail(path, false));
                    }
                }
                if (auto* actionWidget = m_table->cellWidget(r, 7)) {
                    if (auto* actionBtn = actionWidget->findChild<QPushButton*>()) {
                        actionBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/more.png", p.iconColor));
                    }
                }
            }
            m_table->viewport()->update();
            loadVisibleThumbnails();
        }
    });

    auto* cardLayout = new QVBoxLayout(cardFrame);
    cardLayout->setContentsMargins(16, 16, 16, 16);
    cardLayout->setSpacing(12);

    // 1. Header Row
    auto* headerLayout = new QHBoxLayout();
    headerLayout->setContentsMargins(0, 0, 0, 5);

    m_backBtn = new QPushButton("‹  Back", this);
    m_backBtn->setCursor(Qt::PointingHandCursor);
    m_backBtn->setVisible(false);
    connect(m_backBtn, &QPushButton::clicked, this, &SongsTableWidget::backButtonClicked);
    // Back button color from theme
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

    // Sort Dropdown
    auto* sortCombo = new QComboBox(this);
    ThemeManager::setupComboBox(sortCombo);
    sortCombo->addItem("Sort by: Title");
    sortCombo->addItem("Sort by: Artist");
    sortCombo->addItem("Sort by: Mood");
    sortCombo->addItem("Sort by: Date");
    sortCombo->setToolTip("Sort Library Songs by Title, Artist, Mood, or Date Added");
    connect(sortCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this, [this](int idx) {
        if (m_rows.isEmpty()) return;
        if (idx == 0) { // Title
            std::sort(m_rows.begin(), m_rows.end(), [](const SongRow& a, const SongRow& b) {
                return a.title.localeAwareCompare(b.title) < 0;
            });
        } else if (idx == 1) { // Artist
            std::sort(m_rows.begin(), m_rows.end(), [](const SongRow& a, const SongRow& b) {
                return a.artist.localeAwareCompare(b.artist) < 0;
            });
        } else if (idx == 2) { // Mood
            std::sort(m_rows.begin(), m_rows.end(), [](const SongRow& a, const SongRow& b) {
                if (a.mood.isEmpty() != b.mood.isEmpty()) {
                    return !a.mood.isEmpty(); // non-empty moods first
                }
                return a.mood.localeAwareCompare(b.mood) < 0;
            });
        } else if (idx == 3) { // Date / Song ID
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

    // View List / Grid buttons
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

    // Auto-exclusive button group for list/grid toggle buttons
    auto* viewGroup = new QButtonGroup(this);
    viewGroup->setExclusive(true);
    viewGroup->addButton(listBtn);
    viewGroup->addButton(gridBtn);

    headerLayout->addWidget(listBtn);
    headerLayout->addWidget(gridBtn);
    cardLayout->addLayout(headerLayout);

    // 2. Stacked Widget to hold both views
    m_stackedWidget = new QStackedWidget(this);

    // Table view creation: 8 columns (Col 2 is dedicated Mood column)
    m_table = new QTableWidget(this);
    m_table->setColumnCount(8);
    m_table->setShowGrid(false);
    m_table->setAlternatingRowColors(false);
    m_table->setSelectionBehavior(QAbstractItemView::SelectRows);
    m_table->setSelectionMode(QAbstractItemView::SingleSelection);
    m_table->setFocusPolicy(Qt::NoFocus);
    m_table->setVerticalScrollMode(QAbstractItemView::ScrollPerPixel);
    m_table->setStyleSheet(
        "QTableWidget { background-color: transparent; border: none; outline: none; }"
        "QHeaderView::section { background-color: transparent; color: #7E8494; font-weight: bold; padding: 6px; border: none; border-bottom: 1px solid #1E2538; }"
        "QTableWidget::item { border-bottom: 1px solid rgba(255,255,255,0.04); }"
    );

    // Set horizontal headers
    QStringList headers = {"#", "Title", "Mood", "Artist", "Album", "", "", ""};
    m_table->setHorizontalHeaderLabels(headers);
    m_table->horizontalHeader()->setDefaultAlignment(Qt::AlignCenter);
    m_table->horizontalHeader()->setHighlightSections(false);
    m_table->horizontalHeader()->setFixedHeight(38);
    m_table->verticalHeader()->setVisible(false);
    m_table->verticalHeader()->setDefaultSectionSize(54); // Spacious 54px row height

    for (int i = 0; i < m_table->columnCount(); ++i) {
        if (auto* h = m_table->horizontalHeaderItem(i)) {
            h->setTextAlignment(Qt::AlignCenter);
        }
    }
    if (auto* h = m_table->horizontalHeaderItem(5)) {
        h->setIcon(QIcon(":/resources/icons/recently_played.png"));
    }
    if (auto* h = m_table->horizontalHeaderItem(6)) {
        h->setIcon(QIcon(":/resources/icons/favorite.png"));
    }

    // Set Column Widths
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

    m_table->setItemDelegate(new SongTableRowDelegate(this, m_table));

    m_stackedWidget->addWidget(m_table);

    // Grid View setup — uses the shared MediaGridWidget so the look &
    // behaviour is identical to the Albums and Artists tabs.
    m_gridWidget = new MediaGridWidget(this);
    m_stackedWidget->addWidget(m_gridWidget);

    cardLayout->addWidget(m_stackedWidget, 1);
    mainLayout->addWidget(cardFrame, 1);

    // View toggling logic. The grid is populated lazily on first switch
    // to grid view — this is critical for memory: a 10 000-track library
    // would otherwise hold 10 000 SongGridCard widgets + 10 000 scaled
    // cover pixmaps (~676 MB) even when the user never left the table
    // view.
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

    // Mouse tracking for unified row hover highlight
    m_table->setMouseTracking(true);
    m_table->viewport()->setMouseTracking(true);
    m_table->viewport()->installEventFilter(this);

    connect(m_table, &QTableWidget::cellEntered, this, &SongsTableWidget::onCellEntered);
    connect(m_table, &QTableWidget::itemSelectionChanged, this, &SongsTableWidget::updateRowStyles);

    // Connect selection triggers for table view.
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

    // Same for grid view
    connect(m_gridWidget, &MediaGridWidget::cardActivated, this, [this](int songId) {
        // Map songId → row index so the existing songSelected(row) contract
        // is preserved.
        int row = m_songIdToRow.value(songId, -1);
        if (row >= 0) emit songSelected(row);
    });

    // When CoverLoader finishes an async cover load, repaint any visible
    // rows whose cover_path matches. This is what makes covers "pop in"
    // as the user scrolls, without ever blocking the GUI thread on I/O.
    connect(&CoverLoader::instance(), &CoverLoader::coverReady,
            this, [this](const QString& path, int size, const QPixmap&) {
        Q_UNUSED(size);
        if (path.isEmpty() || m_stackedWidget->currentIndex() != 0) return;
        // Only the visible viewport rows need repainting; QRect
        // intersection with viewport()->rect() filters the rest.
        QRect visible = m_table->viewport()->rect();
        int top = m_table->rowAt(visible.top());
        int bot = m_table->rowAt(visible.bottom());
        if (top < 0) top = 0;
        if (bot < 0) bot = m_table->rowCount() - 1;
        for (int row = top; row <= bot; ++row) {
            if (auto* firstItem = m_table->item(row, 0)) {
                QString rowPath = firstItem->data(Qt::UserRole + 1).toString();
                if (rowPath == path) {
                    // Refresh the thumbnail in the title column.
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

    // When the user scrolls the table, refresh thumbnails for rows whose
    // async cover loads completed while they were off-screen. Without this,
    // covers that finished loading between scrolls would remain as the
    // default album art until the next full table rebuild.
    connect(m_table->verticalScrollBar(), &QScrollBar::valueChanged, this, [this]() {
        loadVisibleThumbnails();
    });
}

void SongsTableWidget::clearSongs() {
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

QPixmap SongsTableWidget::getThumbnail(const QString& title) {
    Q_UNUSED(title);
    return getDefaultAlbumArt();
}

void SongsTableWidget::setOptimizedMode(bool enabled) {
    int rows = m_table->rowCount();
    for (int row = 0; row < rows; ++row) {
        if (auto* titleCont = m_table->cellWidget(row, 1)) {
            auto labels = titleCont->findChildren<QLabel*>();
            for (QLabel* l : labels) {
                if (l->objectName() != "SongTitleLabel") {
                    if (enabled) {
                        l->setPixmap(loadThumbnail(""));
                        l->setVisible(true);
                    } else {
                        if (auto* firstItem = m_table->item(row, 0)) {
                            QString cp = firstItem->data(Qt::UserRole + 1).toString();
                            if (!cp.isEmpty()) {
                                CoverLoader::instance().requestAsync(cp, 44);
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    // In Optimized Mode flush the pixmap cache for table covers; the
    // Now Playing card manages its own cover outside of CoverLoader so
    // it is unaffected.
    if (enabled) {
        CoverLoader::instance().clearCache();
        CoverLoader::instance().setCacheLimitKb(2048);
    } else {
        CoverLoader::instance().setCacheLimitKb(15 * 1024);
    }
    // Also apply to the grid view cards if it was already populated.
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

    // 0. Index Column
    QString indexText = index > 0 ? QString::number(index) : QString("-");
    auto* itemIndex = new QTableWidgetItem(indexText);
    itemIndex->setData(Qt::UserRole, songId);
    itemIndex->setData(Qt::UserRole + 1, coverPath);
    itemIndex->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    itemIndex->setFlags(itemIndex->flags() ^ Qt::ItemIsEditable);
    m_table->setItem(rowIdx, 0, itemIndex);

    // 1. Title Column (Custom Container Widget)
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
    titleLabel->setStyleSheet("font-weight: 500; font-size: 13px;");

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

    // 2. Dedicated Mood Column
    QString moodStr = (!m_rows.isEmpty()) ? m_rows.last().mood : "";
    auto* moodContainer = new QWidget(this);
    moodContainer->setAttribute(Qt::WA_TransparentForMouseEvents);
    auto* moodLayout = new QHBoxLayout(moodContainer);
    moodLayout->setContentsMargins(0, 0, 0, 0);
    moodLayout->setAlignment(Qt::AlignCenter);
    if (!moodStr.isEmpty()) {
        auto* moodBadge = new QLabel(moodContainer);
        moodBadge->setObjectName("SongMoodBadge");
        applyMoodPillStyle(moodBadge, moodStr);
        moodLayout->addWidget(moodBadge);
    }
    m_table->setCellWidget(rowIdx, 2, moodContainer);

    // 3. Artist Column
    auto* itemArtist = new QTableWidgetItem(artist);
    itemArtist->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    itemArtist->setFlags(itemArtist->flags() ^ Qt::ItemIsEditable);
    m_table->setItem(rowIdx, 3, itemArtist);

    // 4. Album Column
    auto* itemAlbum = new QTableWidgetItem(album);
    itemAlbum->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    itemAlbum->setFlags(itemAlbum->flags() ^ Qt::ItemIsEditable);
    m_table->setItem(rowIdx, 4, itemAlbum);

    // 5. Duration Column
    auto* itemDuration = new QTableWidgetItem(duration);
    itemDuration->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    itemDuration->setFlags(itemDuration->flags() ^ Qt::ItemIsEditable);
    m_table->setItem(rowIdx, 5, itemDuration);

    // 6. Favorite Heart Button
    auto* favBtn = new QPushButton(isFavorite ? "♥" : "♡", this);
    favBtn->setObjectName("FavBtn");
    favBtn->setCursor(Qt::PointingHandCursor);
    favBtn->setFocusPolicy(Qt::NoFocus);
    favBtn->setProperty("rowIdx", rowIdx);
    favBtn->setProperty("colIdx", 6);
    favBtn->installEventFilter(this);
    favBtn->setStyleSheet(isFavorite ? "QPushButton { border: none; background: transparent; color: #FF2A7A; font-size: 16px; }" : "QPushButton { border: none; background: transparent; color: #7E8494; font-size: 16px; }");
    favBtn->setToolTip(isFavorite ? "Remove from Favorites" : "Add to Favorites");
    connect(favBtn, &QPushButton::clicked, this, [this, songId, favBtn, rowIdx]() {
        m_table->clearSelection();
        bool currentlyFav = (favBtn->text() == "♥");
        bool newFav = !currentlyFav;
        favBtn->setText(newFav ? "♥" : "♡");
        favBtn->setStyleSheet(newFav ? "QPushButton { border: none; background: transparent; color: #FF2A7A; font-size: 16px; }" : "QPushButton { border: none; background: transparent; color: #7E8494; font-size: 16px; }");
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

    // 7. Three-dot Action Menu
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

    // Enable hover transparency on titleContainer
    titleContainer->setAttribute(Qt::WA_TransparentForMouseEvents);

    // If this row matches the currently-playing song, mark it as such.
    if (songId == m_playingSongId && m_playingSongId != -1) {
        m_playingTrackIdx = rowIdx;
        eqIcon->setVisible(true);
        eqIcon->setPlaying(m_isPlaying);
    }

    if (m_songCountLabel) {
        m_songCountDirty = true;
        QTimer::singleShot(100, this, &SongsTableWidget::flushSongCount);
    }

    // If the grid was already populated (user is currently viewing it),
    // append to the grid as well.
    if (m_gridPopulated) {
        m_gridWidget->addCard(songId, title, artist, coverPath);
    }
}

void SongsTableWidget::setSongsBatch(QVector<SongRow> rows) {
    // Transactional rebuild: clear, then bulk-insert. Signalling and
    // repainting are suppressed for the duration; this is the single
    // most important optimisation for large libraries — it converts
    // O(n) widget-create + cover-load calls (each costing ~0.5 ms on
    // the GUI thread) into a single ~50 ms transaction for 1 000 rows.
    m_table->setUpdatesEnabled(false);
    if (m_table->viewport()) m_table->viewport()->setUpdatesEnabled(false);
    m_table->blockSignals(true);

    // Remember whether the grid view was active so we can repopulate it
    // after the rebuild. clearSongs() resets m_gridPopulated to false.
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

            // 0. Index Column
            QString indexText = r.displayIndex > 0 ? QString::number(r.displayIndex) : QString("-");
            auto* itemIndex = new QTableWidgetItem(indexText);
            itemIndex->setData(Qt::UserRole, r.songId);
            itemIndex->setData(Qt::UserRole + 1, r.coverPath);
            itemIndex->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
            itemIndex->setFlags(itemIndex->flags() ^ Qt::ItemIsEditable);
            m_table->setItem(i, 0, itemIndex);

            // 1. Title Column (Custom Container Widget)
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
            titleLabel->setStyleSheet("font-weight: 500; font-size: 13px;");

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

            // 2. Dedicated Mood Column
            auto* moodContainer = new QWidget(this);
            moodContainer->setAttribute(Qt::WA_TransparentForMouseEvents);
            auto* moodLayout = new QHBoxLayout(moodContainer);
            moodLayout->setContentsMargins(0, 0, 0, 0);
            moodLayout->setAlignment(Qt::AlignCenter);
            if (!r.mood.isEmpty()) {
                auto* moodBadge = new QLabel(moodContainer);
                moodBadge->setObjectName("SongMoodBadge");
                applyMoodPillStyle(moodBadge, r.mood);
                moodLayout->addWidget(moodBadge);
            }
            m_table->setCellWidget(i, 2, moodContainer);

            // 3. Artist Column
            auto* itemArtist = new QTableWidgetItem(r.artist);
            itemArtist->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
            itemArtist->setFlags(itemArtist->flags() ^ Qt::ItemIsEditable);
            m_table->setItem(i, 3, itemArtist);

            // 4. Album Column
            auto* itemAlbum = new QTableWidgetItem(r.album);
            itemAlbum->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
            itemAlbum->setFlags(itemAlbum->flags() ^ Qt::ItemIsEditable);
            m_table->setItem(i, 4, itemAlbum);

            // 5. Duration Column
            auto* itemDuration = new QTableWidgetItem(r.duration);
            itemDuration->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
            itemDuration->setFlags(itemDuration->flags() ^ Qt::ItemIsEditable);
            m_table->setItem(i, 5, itemDuration);

            // 6. Favorite Heart Button
            auto* favBtn = new QPushButton(r.isFavorite ? "♥" : "♡", this);
            favBtn->setObjectName("FavBtn");
            favBtn->setCursor(Qt::PointingHandCursor);
            favBtn->setFocusPolicy(Qt::NoFocus);
            favBtn->setProperty("rowIdx", i);
            favBtn->setProperty("colIdx", 6);
            favBtn->installEventFilter(this);
            favBtn->setStyleSheet(r.isFavorite ? "QPushButton { border: none; background: transparent; color: #FF2A7A; font-size: 16px; }" : "QPushButton { border: none; background: transparent; color: #7E8494; font-size: 16px; }");
            favBtn->setToolTip(r.isFavorite ? "Remove from Favorites" : "Add to Favorites");
            connect(favBtn, &QPushButton::clicked, this, [this, songId = r.songId, favBtn, rowIdx = i]() {
                m_table->clearSelection();
                bool currentlyFav = (favBtn->text() == "♥");
                bool newFav = !currentlyFav;
                favBtn->setText(newFav ? "♥" : "♡");
                favBtn->setStyleSheet(newFav ? "QPushButton { border: none; background: transparent; color: #FF2A7A; font-size: 16px; }" : "QPushButton { border: none; background: transparent; color: #7E8494; font-size: 16px; }");
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

            // 7. Three-dot Action Menu
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

            // Restore playing-track highlight if this row matches.
            if (r.songId == m_playingSongId && m_playingSongId != -1) {
                m_playingTrackIdx = i;
                eqIcon->setVisible(true);
                eqIcon->setPlaying(m_isPlaying);
            }
        }
    }

    m_table->blockSignals(false);
    if (m_table->viewport()) m_table->viewport()->setUpdatesEnabled(true);
    m_table->setUpdatesEnabled(true);

    if (m_songCountLabel) {
        m_songCountLabel->setText(QString::number(n) + " songs");
    }
    m_songCountDirty = false;

    // If the grid view was previously populated (user was looking at it,
    // or had switched to it at least once), repopulate it now so it
    // reflects the new data. populateGridFromTable() clears the grid
    // internally, so we don't need to call clearGrid() here.
    if (wasGridPopulated) {
        populateGridFromTable();
    }

    // Repaint the viewport to reflect the new content and load covers for visible rows.
    if (m_table->viewport()) m_table->viewport()->update();
    QTimer::singleShot(0, this, &SongsTableWidget::loadVisibleThumbnails);
}

void SongsTableWidget::populateGridFromTable() {
    m_gridWidget->beginBatchAppend();
    m_gridWidget->clearGrid();
    for (const SongRow& r : m_rows) {
        m_gridWidget->addCard(r.songId, r.title, r.artist, r.coverPath, true);
    }
    m_gridPopulated = true;
    if (m_playingSongId > 0) {
        int idx = m_songIdToRow.value(m_playingSongId, -1);
        if (idx >= 0) m_gridWidget->setPlayingIndex(idx);
    }
    m_gridWidget->endBatchAppend();
}

void SongsTableWidget::flushSongCount() {
    if (m_songCountLabel && m_songCountDirty) {
        m_songCountDirty = false;
        m_songCountLabel->setText(QString::number(m_table->rowCount()) + " songs");
    }
}

void SongsTableWidget::setPlayingSongId(int songId, bool playing) {
    m_playingSongId = songId;
    m_isPlaying = playing;

    int newPlayingRow = m_songIdToRow.value(songId, -1);

    if (m_playingTrackIdx >= 0 && m_playingTrackIdx != newPlayingRow) {
        if (m_playingTrackIdx < m_eqIcons.size()) {
            m_eqIcons[m_playingTrackIdx]->setVisible(false);
            m_eqIcons[m_playingTrackIdx]->setPlaying(false);
        }
    }

    int oldRow = m_playingTrackIdx;
    m_playingTrackIdx = newPlayingRow;

    if (m_playingTrackIdx >= 0 && m_playingTrackIdx < m_table->rowCount()) {
        if (m_playingTrackIdx < m_eqIcons.size()) {
            m_eqIcons[m_playingTrackIdx]->setVisible(true);
            m_eqIcons[m_playingTrackIdx]->setPlaying(m_isPlaying);
        }
    }

    // O(1) style refresh: only the previously-playing and newly-playing
    // rows changed. The old implementation iterated every row, which on
    // a 10 000-track table added ~5 ms of jitter on every track change.
    if (oldRow >= 0) refreshSingleRowStyle(oldRow);
    if (m_playingTrackIdx >= 0 && m_playingTrackIdx != oldRow) {
        refreshSingleRowStyle(m_playingTrackIdx);
    }

    // Mirror the highlight to the grid (only if populated).
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

void SongsTableWidget::onCellEntered(int row, int col) {
    Q_UNUSED(col);
    if (m_hoveredRow != row) {
        m_previousHoveredRow = m_hoveredRow;
        m_hoveredRow = row;
        // O(1): only the previously-hovered and newly-hovered rows
        // need their style refreshed. The delegate already handles the
        // actual painting; this just triggers a repaint of those two
        // rows.
        if (m_previousHoveredRow >= 0) {
            refreshSingleRowStyle(m_previousHoveredRow);
        }
        if (m_hoveredRow >= 0) {
            refreshSingleRowStyle(m_hoveredRow);
        }
    }
}

void SongsTableWidget::refreshSingleRowStyle(int row) {
    if (row < 0 || row >= m_table->rowCount()) return;
    // Trigger a repaint of just this row's viewport region. The delegate
    // will re-evaluate the hovered/playing state and paint accordingly.
    QRect r = m_table->visualRect(m_table->model()->index(row, 0));
    int totalWidth = m_table->viewport()->width();
    r.setX(0);
    r.setWidth(totalWidth);
    m_table->viewport()->update(r);
}

void SongsTableWidget::updateRowStyles() {
    // Selection changed — refresh only the visible rows. The delegate
    // already does the right thing; we just need to trigger repaints.
    QRect visible = m_table->viewport()->rect();
    int top = m_table->rowAt(visible.top());
    int bot = m_table->rowAt(visible.bottom());
    if (top < 0) top = 0;
    if (bot < 0) bot = m_table->rowCount() - 1;
    for (int row = top; row <= bot; ++row) {
        refreshSingleRowStyle(row);
    }
}

bool SongsTableWidget::eventFilter(QObject* watched, QEvent* event) {
    if (watched == m_table->viewport()) {
        if (event->type() == QEvent::Leave) {
            // Check if cursor is actually outside the table bounds
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
            // Smaller mobile width: hide Album, Duration, and 3-dots action menu
            m_table->setColumnHidden(4, true);  // Album
            m_table->setColumnHidden(5, true);  // Duration
            m_table->setColumnHidden(7, true);  // 3 dots
        } else if (width < 680) {
            // Tablet width: hide Album, keep Duration and 3-dots visible
            m_table->setColumnHidden(4, true);  // Album
            m_table->setColumnHidden(5, false); // Duration
            m_table->setColumnHidden(7, false); // 3 dots
        } else {
            // Standard & Fullscreen desktop: show all columns including 3-dots
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
                        l->setPixmap(loadThumbnail(coverPath, true));
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

    // Update the cached row data + the grid card if populated.
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
        // Rebuild the affected card by removing + re-inserting it.
        // (MediaGridWidget doesn't expose a per-item update API yet.)
        // For simplicity we just clear + repopulate the grid; this is
        // O(n) but tag edits are rare.
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

void SongsTableWidget::updateGridResponsive() {
    if (m_gridWidget) m_gridWidget->updateGridResponsive();
}

void SongsTableWidget::resizeEvent(QResizeEvent* event) {
    QWidget::resizeEvent(event);
    // Defer to next event-loop tick so we don't block the resize loop.
    QTimer::singleShot(0, this, [this]() {
        if (m_gridWidget) m_gridWidget->updateGridResponsive();
    });
}

void SongsTableWidget::showEvent(QShowEvent* event) {
    QWidget::showEvent(event);
    QTimer::singleShot(0, this, [this]() {
        if (m_gridWidget) m_gridWidget->updateGridResponsive();
        loadVisibleThumbnails();
    });
}

void SongsTableWidget::loadVisibleThumbnails() {
    if (!m_table || !m_table->viewport() || AppSettings::instance().isOptimizedMode()) return;
    QRect visible = m_table->viewport()->rect();
    int top = m_table->rowAt(visible.top());
    int bot = m_table->rowAt(visible.bottom());
    if (top < 0) top = 0;
    if (bot < 0) bot = m_table->rowCount() - 1;
    for (int row = top; row <= bot; ++row) {
        if (auto* firstItem = m_table->item(row, 0)) {
            QString coverPath = firstItem->data(Qt::UserRole + 1).toString();
            if (coverPath.isEmpty()) continue;
            QPixmap rounded;
            if (CoverLoader::instance().tryGetRounded(coverPath, 44, 8, rounded)) {
                if (auto* titleCont = m_table->cellWidget(row, 1)) {
                    auto labels = titleCont->findChildren<QLabel*>();
                    for (QLabel* l : labels) {
                        if (l->objectName() != "SongTitleLabel") {
                            l->setPixmap(rounded);
                            break;
                        }
                    }
                }
            } else {
                CoverLoader::instance().requestAsync(coverPath, 44);
            }
        }
    }
}
