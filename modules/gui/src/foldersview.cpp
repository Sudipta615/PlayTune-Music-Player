#include "foldersview.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QHeaderView>
#include <QTableWidgetItem>
#include <QIcon>

FoldersViewWidget::FoldersViewWidget(QWidget* parent) : QWidget(parent) {
    setObjectName("FoldersView");
    setAttribute(Qt::WA_StyledBackground, true);
    setupUi();
}

void FoldersViewWidget::setupUi() {
    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);

    m_stackedWidget = new QStackedWidget(this);

    // ── Page 0: Folders List Page (inside FoldersCard Frame) ───────────────────
    auto* page0Card = new QFrame(m_stackedWidget);
    page0Card->setObjectName("FoldersCard");
    page0Card->setStyleSheet(
        "QFrame#FoldersCard {"
        "   background-color: #0F121D;"
        "   border: 1px solid #1E2538;"
        "   border-radius: 16px;"
        "}"
    );

    auto* page0Layout = new QVBoxLayout(page0Card);
    page0Layout->setContentsMargins(16, 16, 16, 16);
    page0Layout->setSpacing(12);

    auto* headerLayout = new QHBoxLayout();
    auto* titleLabel = new QLabel("Imported Folders", page0Card);
    titleLabel->setStyleSheet("font-size: 20px; font-weight: bold; color: #FFFFFF;");
    headerLayout->addWidget(titleLabel);
    headerLayout->addStretch();
    page0Layout->addLayout(headerLayout);

    // 2 columns - Folder Name and Track count
    m_table = new QTableWidget(0, 2, page0Card);
    m_table->setObjectName("FoldersTable");
    m_table->setFrameShape(QFrame::NoFrame);
    m_table->setShowGrid(false);
    m_table->setSelectionBehavior(QAbstractItemView::SelectRows);
    m_table->setSelectionMode(QAbstractItemView::SingleSelection);
    m_table->setFocusPolicy(Qt::NoFocus);
    m_table->verticalHeader()->setVisible(false);

    QStringList headers = {"FOLDER NAME", "TRACKS"};
    m_table->setHorizontalHeaderLabels(headers);
    m_table->horizontalHeader()->setDefaultAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    m_table->horizontalHeader()->setStretchLastSection(false);
    m_table->horizontalHeader()->setSectionResizeMode(0, QHeaderView::Stretch);
    m_table->horizontalHeader()->setSectionResizeMode(1, QHeaderView::Fixed);
    m_table->setColumnWidth(1, 120);

    m_table->setStyleSheet(
        "QTableWidget { background-color: transparent; color: #FFFFFF; font-size: 14px; border: none; }"
        "QHeaderView::section { background-color: transparent; color: #7E8494; font-size: 11px; font-weight: bold; padding: 8px 12px; border: none; border-bottom: 1px solid #1E2538; }"
        "QTableWidget::item { border-bottom: 1px solid #161C2B; padding-left: 8px; padding-right: 8px; }"
        "QTableWidget::item:selected { background-color: #1B1130; color: #FF2A7A; border-radius: 8px; }"
        "QTableWidget::item:hover { background-color: #1A122B; border-radius: 8px; }"
    );

    page0Layout->addWidget(m_table);
    m_stackedWidget->addWidget(page0Card); // index 0

    connect(m_table, &QTableWidget::cellClicked, this, [this](int row, int col) {
        Q_UNUSED(col);
        auto* item = m_table->item(row, 0);
        if (item) {
            int folderId = item->data(Qt::UserRole).toInt();
            QString folderName = item->text();
            showFolderSongs(folderId, folderName);
            emit folderSelected(folderId);
        }
    });

    // ── Page 1: Folder Songs Page (Direct SongsTableWidget with Header Back Button) ─────
    m_folderSongsTable = new SongsTableWidget(m_stackedWidget);
    m_stackedWidget->addWidget(m_folderSongsTable); // index 1

    connect(m_folderSongsTable, &SongsTableWidget::songSelected, this, [this](int index) {
        emit songSelected(index);
    });
    connect(m_folderSongsTable, &SongsTableWidget::backButtonClicked, this, [this]() {
        showFolderList();
        emit backToFoldersClicked();
    });

    mainLayout->addWidget(m_stackedWidget);
}

void FoldersViewWidget::showFolderList() {
    if (m_folderSongsTable) {
        m_folderSongsTable->setBackButtonVisible(false);
    }
    if (m_stackedWidget) {
        m_stackedWidget->setCurrentIndex(0);
    }
}

void FoldersViewWidget::showFolderSongs(int /*folderId*/, const QString& /*folderName*/) {
    if (m_folderSongsTable) {
        m_folderSongsTable->setBackButtonVisible(true, "‹  Folders");
    }
    if (m_stackedWidget) {
        m_stackedWidget->setCurrentIndex(1);
    }
}

void FoldersViewWidget::clearFolders() {
    if (m_table) {
        m_table->setRowCount(0);
    }
    showFolderList();
}

void FoldersViewWidget::addFolderRow(int id, const QString& path, const QString& name, int trackCount) {
    if (!m_table) return;
    int row = m_table->rowCount();
    m_table->insertRow(row);
    m_table->setRowHeight(row, 48);

    auto* nameItem = new QTableWidgetItem(name);
    nameItem->setData(Qt::UserRole, id);
    nameItem->setFlags(nameItem->flags() & ~Qt::ItemIsEditable);
    nameItem->setIcon(QIcon(":/resources/icons/folders.png"));
    nameItem->setToolTip(path);
    nameItem->setTextAlignment(Qt::AlignLeft | Qt::AlignVCenter);

    auto* countItem = new QTableWidgetItem(QString::number(trackCount) + " tracks");
    countItem->setFlags(countItem->flags() & ~Qt::ItemIsEditable);
    countItem->setTextAlignment(Qt::AlignRight | Qt::AlignVCenter);
    countItem->setForeground(QBrush(QColor("#7E8494")));

    m_table->setItem(row, 0, nameItem);
    m_table->setItem(row, 1, countItem);
}

void FoldersViewWidget::clearSongs() {
    if (m_folderSongsTable) m_folderSongsTable->clearSongs();
}

void FoldersViewWidget::addSong(int index, int songId, bool isFavorite, const QString& title, const QString& artist, const QString& album, const QString& duration, const QString& coverPath) {
    if (m_folderSongsTable) {
        m_folderSongsTable->addSong(index, songId, isFavorite, title, artist, album, duration, coverPath);
    }
}

void FoldersViewWidget::setPlayingState(bool playing) {
    if (m_folderSongsTable) {
        m_folderSongsTable->setPlayingTrack(-2, playing);
    }
}

void FoldersViewWidget::setActiveIndex(int index, bool playing) {
    setPlayingSongId(index, playing);
}

void FoldersViewWidget::setPlayingSongId(int songId, bool playing) {
    if (m_folderSongsTable) {
        m_folderSongsTable->setPlayingSongId(songId, playing);
    }
}

void FoldersViewWidget::updateTrackRow(int songId, const QString& title, const QString& artist, const QString& album, const QString& duration, const QString& coverPath) {
    if (m_folderSongsTable) {
        m_folderSongsTable->updateTrackRow(songId, title, artist, album, duration, coverPath);
    }
}
