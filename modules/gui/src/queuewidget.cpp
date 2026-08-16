#include "queuewidget.h"
#include "coverloader.h"
#include "custom_widgets.h"
#include "gui_bridge_p.h"
#include "appsettings.h"
#include "apptheme.h"
#include <QFile>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QTableWidgetItem>
#include <QIcon>
#include <QPainter>
#include <QPainterPath>
#include <QDebug>
#include <QStyledItemDelegate>
#include <QEvent>
#include <QSettings>
#include <QScrollBar>
#include <QPixmapCache>

// Helper to round pixmaps
static QPixmap getRoundedPixmap(const QPixmap& src, int size, int radius) {
    if (src.isNull()) return src;
    QPixmap target(size, size);
    target.fill(Qt::transparent);
    QPainter painter(&target);
    painter.setRenderHint(QPainter::Antialiasing, true);
    painter.setRenderHint(QPainter::SmoothPixmapTransform, true);
    QPainterPath path;
    path.addRoundedRect(0, 0, size, size, radius, radius);
    painter.setClipPath(path);

    QPixmap expanded = src.scaled(size, size, Qt::KeepAspectRatioByExpanding, Qt::SmoothTransformation);
    int ex = (size - expanded.width()) / 2;
    int ey = (size - expanded.height()) / 2;
    painter.drawPixmap(ex, ey, expanded);

    return target;
}

// ==========================================
// QueueTableRowDelegate Implementation
// ==========================================
class QueueTableRowDelegate : public QStyledItemDelegate {
public:
    explicit QueueTableRowDelegate(QueueWidget* owner, QObject* parent = nullptr)
        : QStyledItemDelegate(parent), m_owner(owner) {}

    void paint(QPainter* painter, const QStyleOptionViewItem& option, const QModelIndex& index) const override {
        painter->save();
        int row = index.row();
        bool isHovered = (row == m_owner->hoveredRow());
        bool isSelected = false;
        if (auto* firstItem = m_owner->queueTable()->item(row, 0)) {
            isSelected = firstItem->isSelected();
        }

        const auto& p = ThemeManager::instance().currentTheme();

        QColor bgColor = Qt::transparent;
        if (isHovered) {
            bgColor = p.itemHoverBg;
        } else if (isSelected) {
            bgColor = p.itemSelectedBg;
        }

        if (bgColor.isValid() && bgColor != Qt::transparent) {
            painter->setRenderHint(QPainter::Antialiasing, true);
            painter->setPen(Qt::NoPen);
            painter->setBrush(bgColor);

            int totalWidth = m_owner->queueTable()->viewport()->width();
            QRectF fullRowRect(2, option.rect.top() + 1, totalWidth - 4, option.rect.height() - 2);

            painter->setClipRect(option.rect);
            painter->drawRoundedRect(fullRowRect, 8, 8);
        }

        QStyleOptionViewItem opt = option;
        opt.state &= ~QStyle::State_Selected;
        opt.state &= ~QStyle::State_HasFocus;
        QColor textColor = p.secondaryText;
        opt.palette.setColor(QPalette::Text, textColor);
        opt.palette.setColor(QPalette::WindowText, textColor);
        opt.palette.setColor(QPalette::HighlightedText, textColor);

        QStyledItemDelegate::paint(painter, opt, index);
        painter->restore();
    }

private:
    QueueWidget* m_owner = nullptr;
};

QueueWidget::QueueWidget(QWidget* parent) : QWidget(parent) {
    setObjectName("RightSidebarFrame");
    setAttribute(Qt::WA_StyledBackground, true);
    setupUi();
    m_karaokeDialog = new KaraokeDialog(this);
    connect(m_karaokeDialog, &KaraokeDialog::seekRequested, this, &QueueWidget::seekRequested);
}

void QueueWidget::setupUi() {
    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(15, 20, 15, 15);
    mainLayout->setSpacing(15);

    // 1. Queue / Lyrics Tab Switcher
    auto* tabContainer = new QFrame(this);
    tabContainer->setObjectName("TabContainer");
    auto* tabLayout = new QHBoxLayout(tabContainer);
    tabLayout->setContentsMargins(2, 2, 2, 2);
    tabLayout->setSpacing(0);

    m_toggleRightBtn = new QPushButton(">", tabContainer);
    m_toggleRightBtn->setFixedSize(28, 28);
    m_toggleRightBtn->setCursor(Qt::PointingHandCursor);
    m_toggleRightBtn->setToolTip("Collapse Right Sidebar (Q)");
    {
        const auto& p = ThemeManager::instance().currentTheme();
        m_toggleRightBtn->setStyleSheet(QString(
            "QPushButton { border: none; background: transparent; color: %1; font-size: 14px; font-weight: bold; border-radius: 6px; }"
            "QPushButton:hover { background-color: %2; color: %3; }"
        ).arg(p.mutedText.name(), p.itemHoverBg.name(), p.primaryText.name()));
    }
    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this](const ThemePalette& p) {
        if (m_toggleRightBtn) {
            m_toggleRightBtn->setStyleSheet(QString(
                "QPushButton { border: none; background: transparent; color: %1; font-size: 14px; font-weight: bold; border-radius: 6px; }"
                "QPushButton:hover { background-color: %2; color: %3; }"
            ).arg(p.mutedText.name(), p.itemHoverBg.name(), p.primaryText.name()));
        }
    });
    connect(m_toggleRightBtn, &QPushButton::clicked, this, &QueueWidget::toggleRightSidebarRequested);

    m_queueTab = new QPushButton("Queue", tabContainer);
    m_queueTab->setObjectName("TabBtn");
    m_queueTab->setCheckable(true);
    m_queueTab->setChecked(true);
    m_queueTab->setToolTip("View Up Next Playback Queue");

    m_lyricsTab = new QPushButton("Lyrics", tabContainer);
    m_lyricsTab->setObjectName("TabBtn");
    m_lyricsTab->setCheckable(true);
    m_lyricsTab->setToolTip("View Synchronized Track Lyrics");

    tabLayout->addWidget(m_toggleRightBtn);
    tabLayout->addWidget(m_queueTab);
    tabLayout->addWidget(m_lyricsTab);

    m_tabGroup = new QButtonGroup(this);
    m_tabGroup->setExclusive(true);
    m_tabGroup->addButton(m_queueTab, 0);
    m_tabGroup->addButton(m_lyricsTab, 1);

    mainLayout->addWidget(tabContainer);

    // 2. Now Playing Mini Card
    auto* npHeader = new QLabel("Now Playing", this);
    m_npHeaderLabel = npHeader;
    mainLayout->addWidget(npHeader);

    // upNextLabel: created here with 'this' as parent so it can be referenced
    // before queuePage is constructed. It will be reparented into queuePage's layout.
    m_upNextLabel = new QLabel("Up Next", this);

    auto* npCard = new QWidget(this);
    auto* npCardLayout = new QHBoxLayout(npCard);
    npCardLayout->setContentsMargins(5, 5, 5, 5);
    npCardLayout->setSpacing(10);

    m_miniCover = new QLabel(npCard);
    m_miniCover->setFixedSize(44, 44);
    QPixmap defaultCover = getDefaultAlbumArt();
    m_miniCover->setPixmap(getRoundedPixmap(defaultCover, 44, 8));

    auto* npInfoLayout = new QVBoxLayout();
    npInfoLayout->setSpacing(2);
    npInfoLayout->setContentsMargins(0, 0, 0, 0);

    m_miniTitle = new QLabel("No Track Playing", npCard);
    m_miniTitle->setObjectName("MiniTitle");
    m_miniArtistAlbum = new QLabel("PlayTune Music Player", npCard);
    m_miniArtistAlbum->setObjectName("MiniArtistAlbum");

    auto applyTheme = [this](const ThemePalette& p) {
        if (m_queueTable) {
            int count = m_queueTable->rowCount();
            for (int r = 0; r < count; ++r) {
                if (auto* itemIdx = m_queueTable->item(r, 0)) {
                    itemIdx->setForeground(p.mutedText);
                }
            }
            if (m_queueTable->viewport()) {
                m_queueTable->viewport()->update();
            }
        }
    };
    applyTheme(ThemeManager::instance().currentTheme());
    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [applyTheme](const ThemePalette& p) {
        applyTheme(p);
    });

    npInfoLayout->addWidget(m_miniTitle);
    npInfoLayout->addWidget(m_miniArtistAlbum);

    npCardLayout->addWidget(m_miniCover);
    npCardLayout->addLayout(npInfoLayout);
    npCardLayout->addStretch();

    mainLayout->addWidget(npCard);

    // 3. Stacked Widget for Queue vs Lyrics
    m_contentStack = new QStackedWidget(this);

    auto* queuePage = new QWidget(this);
    auto* queuePageLayout = new QVBoxLayout(queuePage);
    queuePageLayout->setContentsMargins(0, 0, 0, 0);
    queuePageLayout->setSpacing(10);

    auto* upNextHeaderLayout = new QHBoxLayout();
    // m_upNextLabel was already created above; add it to queuePage layout now
    upNextHeaderLayout->addWidget(m_upNextLabel);
    
    auto* clearBtn = new QPushButton("Clear", queuePage);
    clearBtn->setObjectName("ResetBtn");
    clearBtn->setFixedSize(50, 24);
    clearBtn->setStyleSheet("QPushButton { font-size: 11px; padding: 2px; }");
    clearBtn->setToolTip("Clear All Tracks from Up Next Queue");

    upNextHeaderLayout->addStretch();
    upNextHeaderLayout->addWidget(clearBtn);
    queuePageLayout->addLayout(upNextHeaderLayout);

    connect(clearBtn, &QPushButton::clicked, this, [this]() {
        emit clearQueueClicked();
    });

    auto* dragDropTable = new DragDropQueueTableWidget(queuePage);
    m_queueTable = dragDropTable;
    connect(dragDropTable, &DragDropQueueTableWidget::rowMoved, this, &QueueWidget::reorderQueueRow);
    m_queueTable->setColumnCount(2);
    m_queueTable->setShowGrid(false);
    m_queueTable->setAlternatingRowColors(false);
    m_queueTable->setSelectionBehavior(QAbstractItemView::SelectRows);
    m_queueTable->setSelectionMode(QAbstractItemView::SingleSelection);
    m_queueTable->setFocusPolicy(Qt::NoFocus);
    m_queueTable->verticalHeader()->setVisible(false);
    m_queueTable->horizontalHeader()->setVisible(false);
    m_queueTable->setVerticalScrollMode(QAbstractItemView::ScrollPerPixel);
    m_queueTable->verticalScrollBar()->setSingleStep(m_queueTable->fontMetrics().lineSpacing() * 2);

    m_queueTable->horizontalHeader()->setSectionResizeMode(0, QHeaderView::Fixed);
    m_queueTable->setColumnWidth(0, 36);
    m_queueTable->horizontalHeader()->setSectionResizeMode(1, QHeaderView::Stretch);

    m_queueTable->setItemDelegate(new QueueTableRowDelegate(this, m_queueTable));
    m_queueTable->setMouseTracking(true);
    if (m_queueTable->viewport()) {
        m_queueTable->viewport()->setMouseTracking(true);
        m_queueTable->viewport()->installEventFilter(this);
    }
    connect(m_queueTable, &QTableWidget::cellEntered, this, [this](int row, int col) {
        Q_UNUSED(col);
        if (m_hoveredRow != row) {
            m_hoveredRow = row;
            if (m_queueTable && m_queueTable->viewport()) m_queueTable->viewport()->update();
        }
    });

    queuePageLayout->addWidget(m_queueTable);

    connect(m_queueTable, &QTableWidget::cellClicked, this, [this](int row, int col) {
        Q_UNUSED(col);
        if (auto* item = m_queueTable->item(row, 0)) {
            bool ok = false;
            int songIdx = item->data(Qt::UserRole).toInt(&ok);
            if (ok) {
                emit queueSongSelected(songIdx);
                return;
            }
        }
        emit queueSongSelected(row);
    });
    connect(m_queueTable, &QTableWidget::cellDoubleClicked, this, [this](int row, int col) {
        Q_UNUSED(col);
        if (auto* item = m_queueTable->item(row, 0)) {
            bool ok = false;
            int songIdx = item->data(Qt::UserRole).toInt(&ok);
            if (ok) {
                emit queueSongSelected(songIdx);
                return;
            }
        }
        emit queueSongSelected(row);
    });

    m_footerLabel = new QLabel("0 songs • 0:00", queuePage);
    m_footerLabel->setStyleSheet("color: #4C5264; font-size: 11px; margin-left: 5px;");
    queuePageLayout->addWidget(m_footerLabel);

    // Lyrics Page
    auto* lyricsPage = new QWidget(this);
    auto* lyricsLayout = new QVBoxLayout(lyricsPage);
    lyricsLayout->setContentsMargins(4, 4, 4, 4);
    lyricsLayout->setSpacing(10);

    m_karaokeButton = new QPushButton(QStringLiteral("⛶ Expand Karaoke Mode"), lyricsPage);
    m_karaokeButton->setCursor(Qt::PointingHandCursor);
    m_karaokeButton->setStyleSheet(QStringLiteral(
        "QPushButton {"
        "  background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #00e5ff, stop:1 #0088ff);"
        "  color: #080c16;"
        "  border: none;"
        "  border-radius: 8px;"
        "  padding: 8px 12px;"
        "  font-weight: bold;"
        "  font-size: 13px;"
        "}"
        "QPushButton:hover {"
        "  background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #33edff, stop:1 #3399ff);"
        "}"
    ));
    connect(m_karaokeButton, &QPushButton::clicked, this, &QueueWidget::onExpandKaraokeClicked);
    lyricsLayout->addWidget(m_karaokeButton);

    m_lyricsListWidget = new QListWidget(lyricsPage);
    m_lyricsListWidget->setFrameShape(QFrame::NoFrame);
    m_lyricsListWidget->setObjectName("QueueLyricsList");
    m_lyricsListWidget->setVerticalScrollMode(QAbstractItemView::ScrollPerPixel);
    m_lyricsListWidget->verticalScrollBar()->setSingleStep(m_lyricsListWidget->fontMetrics().lineSpacing() * 2);
    m_lyricsListWidget->setFocusPolicy(Qt::NoFocus);
    connect(m_lyricsListWidget, &QListWidget::itemClicked, this, &QueueWidget::onLyricsLineClicked);
    lyricsLayout->addWidget(m_lyricsListWidget, 1);

    m_unsyncedLyricsLabel = new QLabel(lyricsPage);
    m_unsyncedLyricsLabel->setObjectName("QueueUnsyncedLyrics");
    m_unsyncedLyricsLabel->setAlignment(Qt::AlignCenter);
    m_unsyncedLyricsLabel->setWordWrap(true);
    m_unsyncedLyricsLabel->hide();
    lyricsLayout->addWidget(m_unsyncedLyricsLabel, 1);

    m_contentStack->addWidget(queuePage);
    m_contentStack->addWidget(lyricsPage);
    mainLayout->addWidget(m_contentStack, 1);

    connect(m_tabGroup, &QButtonGroup::idClicked, m_contentStack, &QStackedWidget::setCurrentIndex);

    // 6. Volume Control layout (Bottom)
    auto* volumeLayout = new QHBoxLayout();
    volumeLayout->setSpacing(10);
    volumeLayout->setContentsMargins(5, 10, 5, 5);

    m_volumeIcon = new QPushButton(this);
    m_volumeIcon->setObjectName("IconButton");
    m_volumeIcon->setIcon(ThemeManager::tintedIcon(":/resources/icons/volume.png",
        ThemeManager::instance().currentTheme().iconColor));
    m_volumeIcon->setIconSize(QSize(18, 18));
    m_volumeIcon->setFixedSize(30, 30);
    m_volumeIcon->setToolTip("Toggle Mute Audio (M)");
    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this](const ThemePalette& p) {
        if (m_volumeIcon) m_volumeIcon->setIcon(ThemeManager::tintedIcon(":/resources/icons/volume.png", p.iconColor));
    });

    m_volumeSlider = new ClickableSlider(Qt::Horizontal, this);
    m_volumeSlider->setObjectName("VolumeSlider");
    m_volumeSlider->setRange(0, 100);

    // Create label first so it can be updated in the QSettings restore block
    m_volumeLabel = new QLabel("75%", this);
    m_volumeLabel->setStyleSheet("color: #7E8494; font-size: 11px; min-width: 32px; font-weight: 500;");

    // Issue 11: Restore persisted volume (default 75%)
    {
        QSettings s("PlayTune", "Settings");
        int savedVol = s.value("volume", 75).toInt();
        if (savedVol < 0) savedVol = 0;
        if (savedVol > 100) savedVol = 100;
        m_lastVolumeBeforeMute = savedVol > 0 ? savedVol : 75;
        m_volumeSlider->setValue(savedVol);
        m_volumeSlider->setToolTip(QString("Adjust Master Volume: %1% (\u2191 / \u2193)").arg(savedVol));
        m_volumeLabel->setText(savedVol == 0 ? "Muted" : QString("%1%").arg(savedVol));
    }

    volumeLayout->addWidget(m_volumeIcon);
    volumeLayout->addWidget(m_volumeSlider);
    volumeLayout->addWidget(m_volumeLabel);
    mainLayout->addLayout(volumeLayout);

    m_volumeThrottleTimer = new QTimer(this);
    m_volumeThrottleTimer->setInterval(30);
    m_volumeThrottleTimer->setSingleShot(false);

    connect(m_volumeThrottleTimer, &QTimer::timeout, this, [this]() {
        if (!m_volumeSlider) return;
        int val = m_volumeSlider->value();
        if (val != m_lastEmittedVolume) {
            m_lastEmittedVolume = val;
            emit volumeChanged(static_cast<double>(val) / 100.0);
        }
        if (!m_volumeSlider->isSliderDown()) {
            m_volumeThrottleTimer->stop();
            QSettings s("PlayTune", "Settings");
            s.setValue("volume", val);
        }
    });

    connect(m_volumeSlider, &QSlider::valueChanged, this, [this](int val) {
        if (val == 0) {
            m_volumeLabel->setText("Muted");
        } else {
            m_volumeLabel->setText(QString("%1%").arg(val));
        }
        m_volumeSlider->setToolTip(QString("Adjust Master Volume: %1% (\u2191 / \u2193)").arg(val));
        if (val > 0) m_lastVolumeBeforeMute = val;
        m_volumeIcon->setToolTip(val == 0 ? "Unmute Audio (M)" : "Toggle Mute Audio (M)");

        if (!m_volumeThrottleTimer->isActive()) {
            m_lastEmittedVolume = val;
            emit volumeChanged(static_cast<double>(val) / 100.0);
            m_volumeThrottleTimer->start();
        }
    });

    connect(m_volumeSlider, &QSlider::sliderReleased, this, [this]() {
        if (m_volumeThrottleTimer->isActive()) {
            m_volumeThrottleTimer->stop();
        }
        int val = m_volumeSlider->value();
        if (val != m_lastEmittedVolume) {
            m_lastEmittedVolume = val;
            emit volumeChanged(static_cast<double>(val) / 100.0);
        }
        QSettings s("PlayTune", "Settings");
        s.setValue("volume", val);
    });

    connect(m_volumeIcon, &QPushButton::clicked, this, [this]() {
        if (m_volumeSlider->value() == 0) {
            m_volumeSlider->setValue(m_lastVolumeBeforeMute > 0 ? m_lastVolumeBeforeMute : 75);
        } else {
            m_lastVolumeBeforeMute = m_volumeSlider->value();
            m_volumeSlider->setValue(0);
        }
    });
}

void QueueWidget::setTrackInfo(const QString& title, const QString& artist, const QString& album, const QString& coverPath) {
    m_miniTitle->setText(title.isEmpty() ? "Unknown Title" : title);
    m_miniCoverPath = coverPath;

    QString artistAlbum = (artist.isEmpty() ? "Unknown Artist" : artist);
    if (!album.isEmpty()) {
        artistAlbum += " • " + album;
    }
    m_miniArtistAlbum->setText(artistAlbum);

    if (AppSettings::instance().isOptimizedMode()) {
        if (m_miniCover) m_miniCover->setVisible(false);
    } else {
        if (m_miniCover) {
            m_miniCover->setProperty("coverPath", coverPath);
            m_miniCover->setVisible(true);
        }
        QPixmap cover;
        if (!coverPath.isEmpty() && cover.load(coverPath)) {
            m_miniCover->setPixmap(getRoundedPixmap(cover, 44, 8));
        } else {
            m_miniCover->setPixmap(getRoundedPixmap(getThumbnail(title), 44, 8));
        }
    }

    if (m_karaokeDialog) {
        m_karaokeDialog->setTrackInfo(title, artist, coverPath);
    }
}


QPixmap QueueWidget::getThumbnail(const QString& title) {
    Q_UNUSED(title);
    return getDefaultAlbumArt();
}

void QueueWidget::setOptimizedMode(bool enabled) {
    // Hide/show the mini Now Playing cover.
    if (m_miniCover) m_miniCover->setVisible(!enabled);

    // Walk all queue rows and hide/show the thumbnail label in column 1.
    if (m_queueTable) {
        int rows = m_queueTable->rowCount();
        for (int row = 0; row < rows; ++row) {
            if (auto* details = m_queueTable->cellWidget(row, 1)) {
                auto labels = details->findChildren<QLabel*>();
                for (QLabel* l : labels) {
                    if (l->objectName() == "QueueRowThumbLabel" || (l->minimumWidth() <= 24 && l->maximumWidth() <= 24)) {
                        l->setVisible(!enabled);
                    }
                }
            }
        }
    }
}


void QueueWidget::clearQueue() {
    m_hoveredRow = -1;
    m_totalQueueSeconds = 0;
    m_queueTable->setRowCount(0);
    m_footerLabel->setText("0 songs • 0:00");
}

void QueueWidget::beginQueueUpdate() {
    if (m_queueTable) {
        m_queueTable->setUpdatesEnabled(false);
        if (m_queueTable->viewport()) m_queueTable->viewport()->setUpdatesEnabled(false);
        m_queueTable->blockSignals(true);
    }
}

void QueueWidget::endQueueUpdate() {
    if (m_queueTable) {
        m_queueTable->blockSignals(false);
        m_queueTable->setUpdatesEnabled(true);
        if (m_queueTable->viewport()) m_queueTable->viewport()->setUpdatesEnabled(true);
        if (m_queueTable->viewport()) m_queueTable->viewport()->update();
    }
}

void QueueWidget::setTrackLyrics(int trackId, const QString& syncedLrc, const QString& unsyncedLyrics) {
    m_currentTrackId = trackId;
    m_lyricsLines.clear();
    if (m_lyricsListWidget) m_lyricsListWidget->clear();
    m_activeLyricIndex = -1;

    if (m_karaokeDialog) {
        m_karaokeDialog->setLyrics(syncedLrc, unsyncedLyrics);
    }

    if (!syncedLrc.trimmed().isEmpty()) {
        m_lyricsLines = LrcParser::parse(syncedLrc);
    }

    if (!m_lyricsLines.isEmpty() && m_lyricsListWidget) {
        m_isSyncedLyrics = true;
        m_lyricsListWidget->show();
        if (m_unsyncedLyricsLabel) m_unsyncedLyricsLabel->hide();

        for (const LrcLine& line : m_lyricsLines) {
            QListWidgetItem* item = new QListWidgetItem(line.text, m_lyricsListWidget);
            item->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
            item->setFont(QFont(QStringLiteral("Inter"), 13, QFont::Medium));
            item->setForeground(QBrush(QColor(225, 228, 235, 140)));
            item->setData(Qt::UserRole, line.timestampSeconds);
        }
    } else if (!unsyncedLyrics.trimmed().isEmpty()) {
        m_isSyncedLyrics = false;
        if (m_lyricsListWidget) m_lyricsListWidget->hide();
        if (m_unsyncedLyricsLabel) {
            m_unsyncedLyricsLabel->setText(unsyncedLyrics.trimmed());
            m_unsyncedLyricsLabel->show();
        }
    } else {
        m_isSyncedLyrics = false;
        if (m_lyricsListWidget) m_lyricsListWidget->hide();
        if (m_unsyncedLyricsLabel) {
            m_unsyncedLyricsLabel->setText(QStringLiteral("🎵 No lyrics found for this track.\nAdd an .lrc file next to the song file or embed lyrics via right-click Tag Editor."));
            m_unsyncedLyricsLabel->show();
        }
    }
}

void QueueWidget::updatePlaybackProgress(double elapsedSeconds) {
    if (m_karaokeDialog && m_karaokeDialog->isVisible()) {
        m_karaokeDialog->updateProgress(elapsedSeconds);
    }

    if (!m_isSyncedLyrics || m_lyricsLines.isEmpty() || !m_lyricsListWidget) return;

    int newIndex = LrcParser::findActiveLineIndex(m_lyricsLines, elapsedSeconds);
    if (newIndex != m_activeLyricIndex && newIndex >= 0 && newIndex < m_lyricsListWidget->count()) {
        if (m_activeLyricIndex >= 0 && m_activeLyricIndex < m_lyricsListWidget->count()) {
            QListWidgetItem* prevItem = m_lyricsListWidget->item(m_activeLyricIndex);
            prevItem->setFont(QFont(QStringLiteral("Inter"), 13, QFont::Medium));
            prevItem->setForeground(QBrush(QColor(225, 228, 235, 140)));
        }

        m_activeLyricIndex = newIndex;

        QListWidgetItem* currItem = m_lyricsListWidget->item(m_activeLyricIndex);
        currItem->setFont(QFont(QStringLiteral("Inter"), 14, QFont::Bold));
        currItem->setForeground(QBrush(QColor(QStringLiteral("#00e5ff"))));

        m_lyricsListWidget->scrollToItem(currItem, QAbstractItemView::PositionAtCenter);
    }
}

void QueueWidget::onLyricsLineClicked(QListWidgetItem* item) {
    if (!m_isSyncedLyrics || !item) return;
    QVariant tsData = item->data(Qt::UserRole);
    if (tsData.isValid()) {
        double seconds = tsData.toDouble();
        emit seekRequested(seconds);
    }
}

void QueueWidget::onExpandKaraokeClicked() {
    if (m_karaokeDialog) {
        m_karaokeDialog->show();
        m_karaokeDialog->raise();
        m_karaokeDialog->activateWindow();
    }
}

bool QueueWidget::eventFilter(QObject* watched, QEvent* event) {
    if (m_queueTable && watched == m_queueTable->viewport()) {
        if (event->type() == QEvent::Leave) {
            if (m_hoveredRow != -1) {
                m_hoveredRow = -1;
                m_queueTable->viewport()->update();
            }
        }
    }
    return QWidget::eventFilter(watched, event);
}

void QueueWidget::addQueueSong(int index, const QString& title, const QString& artist, const QString& duration, const QString& coverPath) {
    int row = m_queueTable->rowCount();
    m_queueTable->insertRow(row);
    m_queueTable->setRowHeight(row, 46);

    // 0. Index Column
    auto* itemIndex = new QTableWidgetItem(QString::number(row + 1));
    itemIndex->setData(Qt::UserRole, index);
    itemIndex->setData(Qt::UserRole + 1, title);
    itemIndex->setData(Qt::UserRole + 2, artist);
    itemIndex->setData(Qt::UserRole + 3, duration);
    itemIndex->setData(Qt::UserRole + 4, coverPath);
    itemIndex->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    const auto& p = ThemeManager::instance().currentTheme();
    itemIndex->setForeground(p.mutedText);
    itemIndex->setFlags(itemIndex->flags() ^ Qt::ItemIsEditable);
    m_queueTable->setItem(row, 0, itemIndex);

    // 1. Details Column (Container Widget with Thumbnail + stack info)
    auto* detailsContainer = new QWidget(this);
    auto* detailsLayout = new QHBoxLayout(detailsContainer);
    detailsLayout->setContentsMargins(4, 2, 4, 2);
    detailsLayout->setSpacing(8);

    auto* thumbLabel = new QLabel(detailsContainer);
    thumbLabel->setObjectName("QueueRowThumbLabel");
    thumbLabel->setProperty("coverPath", coverPath);
    thumbLabel->setFixedSize(24, 24);

    if (AppSettings::instance().isOptimizedMode()) {
        thumbLabel->setVisible(false);
    } else {
        QPixmap cover;
        bool hasCover = false;
        if (!coverPath.isEmpty()) {
            QString cacheKey = QStringLiteral("thb24:") + coverPath;
            if (QPixmapCache::find(cacheKey, &cover)) {
                hasCover = true;
            } else if (cover.load(coverPath)) {
                QPixmapCache::insert(cacheKey, cover);
                hasCover = true;
            }
        }
        if (!hasCover) {
            cover = getThumbnail(title);
        }
        thumbLabel->setPixmap(getRoundedPixmap(cover, 24, 6));
    }


    auto* infoVLayout = new QVBoxLayout();
    infoVLayout->setSpacing(1);
    infoVLayout->setContentsMargins(0, 0, 0, 0);
    infoVLayout->setAlignment(Qt::AlignVCenter);
    auto* titleLabel = new QLabel(title, detailsContainer);
    titleLabel->setObjectName("QueueRowTitleLabel");
    
    auto* artistLabel = new QLabel(artist, detailsContainer);
    artistLabel->setObjectName("QueueRowArtistLabel");

    infoVLayout->addWidget(titleLabel);
    infoVLayout->addWidget(artistLabel);

    detailsLayout->addWidget(thumbLabel, 0, Qt::AlignVCenter);
    detailsLayout->addLayout(infoVLayout);
    detailsLayout->setAlignment(infoVLayout, Qt::AlignVCenter);
    detailsLayout->addStretch();

    detailsContainer->setAttribute(Qt::WA_TransparentForMouseEvents);
    detailsContainer->setStyleSheet("background: transparent;");

    m_queueTable->setCellWidget(row, 1, detailsContainer);

    // Duration column removed for Issue 4

    int totalSongs = m_queueTable->rowCount();
    // Parse the just-added row's duration string.
    int addedSeconds = 0;
    QStringList parts = duration.split(':');
    if (parts.size() == 2) {
        bool ok1, ok2;
        int m = parts[0].toInt(&ok1);
        int s = parts[1].toInt(&ok2);
        if (ok1 && ok2) addedSeconds = m * 60 + s;
    } else if (parts.size() == 3) {
        bool ok1, ok2, ok3;
        int h = parts[0].toInt(&ok1);
        int m = parts[1].toInt(&ok2);
        int s = parts[2].toInt(&ok3);
        if (ok1 && ok2 && ok3) addedSeconds = h * 3600 + m * 60 + s;
    }
    m_totalQueueSeconds += addedSeconds;
    int totalSeconds = m_totalQueueSeconds;
    int hours = totalSeconds / 3600;
    int mins = (totalSeconds % 3600) / 60;
    int secs = totalSeconds % 60;
    if (hours > 0) {
        m_footerLabel->setText(QString("%1 songs • %2:%3:%4")
                               .arg(totalSongs)
                               .arg(hours)
                               .arg(mins, 2, 10, QChar('0'))
                               .arg(secs, 2, 10, QChar('0')));
    } else {
        m_footerLabel->setText(QString("%1 songs • %2:%3")
                               .arg(totalSongs)
                               .arg(mins)
                               .arg(secs, 2, 10, QChar('0')));
    }
}

void QueueWidget::setVolume(double level) {
    if (m_volumeSlider) {
        QSignalBlocker blocker(m_volumeSlider);
        int val = qRound(level * 100.0);
        m_volumeSlider->setValue(val);
        if (val == 0) {
            m_volumeLabel->setText("Muted");
        } else {
            m_volumeLabel->setText(QString("%1%").arg(val));
            m_lastVolumeBeforeMute = val;
        }
        m_volumeSlider->setToolTip(QString("Adjust Master Volume: %1% (↑ / ↓)").arg(val));
        if (m_volumeIcon) m_volumeIcon->setToolTip(val == 0 ? "Unmute Audio (M)" : "Toggle Mute Audio (M)");
    }
}

void QueueWidget::reorderQueueRow(int fromRow, int toRow) {
    if (!m_queueTable || fromRow < 0 || toRow < 0 || fromRow >= m_queueTable->rowCount() || toRow >= m_queueTable->rowCount() || fromRow == toRow) {
        return;
    }

    auto* itemFrom = m_queueTable->item(fromRow, 0);
    if (!itemFrom) return;

    int songIndex = itemFrom->data(Qt::UserRole).toInt();
    QString title = itemFrom->data(Qt::UserRole + 1).toString();
    QString artist = itemFrom->data(Qt::UserRole + 2).toString();
    QString duration = itemFrom->data(Qt::UserRole + 3).toString();
    QString coverPath = itemFrom->data(Qt::UserRole + 4).toString();

    m_queueTable->removeRow(fromRow);
    toRow = qBound(0, toRow, m_queueTable->rowCount());

    m_queueTable->insertRow(toRow);
    m_queueTable->setRowHeight(toRow, 46);

    auto* itemIndex = new QTableWidgetItem(QString::number(toRow + 1));
    itemIndex->setData(Qt::UserRole, songIndex);
    itemIndex->setData(Qt::UserRole + 1, title);
    itemIndex->setData(Qt::UserRole + 2, artist);
    itemIndex->setData(Qt::UserRole + 3, duration);
    itemIndex->setData(Qt::UserRole + 4, coverPath);
    itemIndex->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    const auto& p = ThemeManager::instance().currentTheme();
    itemIndex->setForeground(p.mutedText);
    itemIndex->setFlags(itemIndex->flags() ^ Qt::ItemIsEditable);
    m_queueTable->setItem(toRow, 0, itemIndex);

    auto* detailsContainer = new QWidget(this);
    auto* detailsLayout = new QHBoxLayout(detailsContainer);
    detailsLayout->setContentsMargins(4, 2, 4, 2);
    detailsLayout->setSpacing(8);

    auto* thumbLabel = new QLabel(detailsContainer);
    thumbLabel->setObjectName("QueueRowThumbLabel");
    thumbLabel->setProperty("coverPath", coverPath);
    thumbLabel->setFixedSize(24, 24);
    QPixmap cover;
    if (!coverPath.isEmpty() && cover.load(coverPath)) {
        thumbLabel->setPixmap(getRoundedPixmap(cover, 24, 6));
    } else {
        thumbLabel->setPixmap(getRoundedPixmap(getThumbnail(title), 24, 6));
    }

    auto* infoVLayout = new QVBoxLayout();
    infoVLayout->setSpacing(1);
    infoVLayout->setContentsMargins(0, 0, 0, 0);
    infoVLayout->setAlignment(Qt::AlignVCenter);

    auto* titleLabel = new QLabel(title, detailsContainer);
    titleLabel->setObjectName("QueueRowTitleLabel");
    titleLabel->setStyleSheet(QString("font-size: 12px; font-weight: 500; color: %1; background: transparent; margin: 0px; padding: 0px;").arg(p.primaryText.name()));

    auto* artistLabel = new QLabel(artist, detailsContainer);
    artistLabel->setObjectName("QueueRowArtistLabel");
    artistLabel->setStyleSheet(QString("font-size: 10px; color: %1; background: transparent; margin: 0px; padding: 0px;").arg(p.mutedText.name()));

    infoVLayout->addWidget(titleLabel);
    infoVLayout->addWidget(artistLabel);

    detailsLayout->addWidget(thumbLabel, 0, Qt::AlignVCenter);
    detailsLayout->addLayout(infoVLayout);
    detailsLayout->setAlignment(infoVLayout, Qt::AlignVCenter);
    detailsLayout->addStretch();

    detailsContainer->setAttribute(Qt::WA_TransparentForMouseEvents);
    detailsContainer->setStyleSheet("background: transparent;");

    m_queueTable->setCellWidget(toRow, 1, detailsContainer);

    // Re-index all row number labels (1, 2, 3...)
    for (int r = 0; r < m_queueTable->rowCount(); ++r) {
        if (auto* it = m_queueTable->item(r, 0)) {
            it->setText(QString::number(r + 1));
        }
    }

    m_queueTable->selectRow(toRow);

    const auto& cb = GuiBridgeManager::instance().callbacks();
    if (cb.on_reorder_queue) {
        cb.on_reorder_queue(fromRow, toRow);
    }
}
