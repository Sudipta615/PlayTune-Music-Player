#include "loudnessscannerdialog.h"
#include "gui_bridge_p.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QFrame>
#include <QMessageBox>
#include <QIcon>
#include <QCloseEvent>
#include <QMetaObject>

LoudnessScannerDialog::LoudnessScannerDialog(const QVector<int>& trackIds, QWidget* parent)
    : QDialog(parent), m_targetTrackIds(trackIds)
{
    setObjectName("LoudnessScannerDialog");
    setWindowIcon(QIcon(":/resources/icons/playtune_logo.png"));
    setWindowFlags(Qt::Dialog | Qt::FramelessWindowHint);
    setAttribute(Qt::WA_StyledBackground, true);

    resize(850, 560);
    setMinimumSize(720, 480);

    setupUi();

    connect(&GuiBridgeManager::instance(), &GuiBridgeManager::loudnessScanProgress, this, &LoudnessScannerDialog::onScanProgress);
    connect(&GuiBridgeManager::instance(), &GuiBridgeManager::loudnessScanTrackResult, this, &LoudnessScannerDialog::onTrackResult);
    connect(&GuiBridgeManager::instance(), &GuiBridgeManager::loudnessScanFinished, this, &LoudnessScannerDialog::onScanFinished);

    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this](const ThemePalette& p) {
        updateThemeStyles(p);
    });
    updateThemeStyles(ThemeManager::instance().currentTheme());

    // Start background scan
    m_scanning = true;
    m_writeBtn->setEnabled(false);
    m_statusLabel->setText("Starting EBU R128 loudness scanner worker...");
    m_progressBar->setValue(0);

    const int* idsPtr = m_targetTrackIds.isEmpty() ? nullptr : m_targetTrackIds.constData();
    playtune_start_loudness_scan(idsPtr, m_targetTrackIds.size());
}

LoudnessScannerDialog::~LoudnessScannerDialog() {
    if (m_scanning) {
        playtune_cancel_loudness_scan();
    }
}

void LoudnessScannerDialog::updateThemeStyles(const ThemePalette& p) {
    setStyleSheet(QString(
        "QDialog#LoudnessScannerDialog { background-color: %1; border: 1.5px solid %2; border-radius: 14px; }"
        "QLabel { color: %3; font-size: 13px; background: transparent; }"
        "QProgressBar {"
        "   border: 1px solid %2; border-radius: 6px; background-color: %4;"
        "   text-align: center; color: %3; font-weight: bold; height: 18px;"
        "}"
        "QProgressBar::chunk {"
        "   background-color: %5;"
        "   border-radius: 5px;"
        "}"
        "QTableWidget {"
        "   background-color: %4; border: 1px solid %2; border-radius: 8px;"
        "   color: %3; gridline-color: %2; font-size: 12px; selection-background-color: %6;"
        "}"
        "QHeaderView::section {"
        "   background-color: %4; color: %7; font-weight: bold; border: none; border-bottom: 1px solid %2; padding: 6px;"
        "}"
        "QPushButton {"
        "   font-size: 13px; font-weight: bold; color: %3;"
        "   background-color: %4; border: 1px solid %2; border-radius: 6px;"
        "   padding: 8px 16px; outline: none;"
        "}"
        "QPushButton:hover { background-color: %6; border-color: %5; color: %8; }"
        "QPushButton:disabled { color: %7; border-color: %2; background-color: %1; }"
        "QPushButton#WriteButton {"
        "   background-color: %5; border: none; color: #FFFFFF;"
        "}"
        "QPushButton#WriteButton:hover {"
        "   background-color: %8;"
        "}"
        "QPushButton#WriteButton:disabled {"
        "   background-color: %4; border: 1px solid %2; color: %7;"
        "}"
    ).arg(p.cardBg.name(), p.cardBorder.name(), p.primaryText.name(),
          p.headerBg.name(), p.primaryAccent.name(), p.itemHoverBg.name(), p.mutedText.name(), p.secondaryAccent.name()));
}

void LoudnessScannerDialog::setupUi() {

    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(20, 20, 20, 20);
    mainLayout->setSpacing(16);

    // 1. Header Bar
    auto* headerLayout = new QHBoxLayout();
    headerLayout->setContentsMargins(0, 0, 0, 5);
    headerLayout->setSpacing(10);

    auto* iconLabel = new QLabel(this);
    iconLabel->setPixmap(QIcon(":/resources/icons/playtune_logo.png").pixmap(20, 20));

    auto* titleLabel = new QLabel("ReplayGain / Loudness Scanner & Writer", this);
    titleLabel->setStyleSheet("font-size: 17px; font-weight: bold; color: #FFFFFF;");

    auto* closeBtn = new QPushButton(this);
    closeBtn->setIcon(QIcon(":/resources/icons/close.png"));
    closeBtn->setIconSize(QSize(14, 14));
    closeBtn->setFixedSize(28, 28);
    closeBtn->setStyleSheet("QPushButton { border: none; background: transparent; padding: 0px; } QPushButton:hover { background-color: #3D1022; border-radius: 14px; }");
    connect(closeBtn, &QPushButton::clicked, this, &LoudnessScannerDialog::onCancelOrCloseClicked);

    headerLayout->addWidget(iconLabel);
    headerLayout->addWidget(titleLabel);
    headerLayout->addStretch();
    headerLayout->addWidget(closeBtn);
    mainLayout->addLayout(headerLayout);

    // 2. Progress Section
    auto* progressFrame = new QFrame(this);
    progressFrame->setStyleSheet("QFrame { background-color: #161926; border: 1px solid #242A3D; border-radius: 8px; }");
    auto* progressLayout = new QVBoxLayout(progressFrame);
    progressLayout->setContentsMargins(14, 12, 14, 12);
    progressLayout->setSpacing(8);

    m_statusLabel = new QLabel("Initializing scanner...", progressFrame);
    m_statusLabel->setStyleSheet("color: #00E5FF; font-weight: 600;");

    m_progressBar = new QProgressBar(progressFrame);
    m_progressBar->setRange(0, 100);
    m_progressBar->setValue(0);

    progressLayout->addWidget(m_statusLabel);
    progressLayout->addWidget(m_progressBar);
    mainLayout->addWidget(progressFrame);

    // 3. Results Table
    m_resultsTable = new QTableWidget(0, 5, this);
    m_resultsTable->setHorizontalHeaderLabels({"Song Title", "LUFS (Int.)", "Peak", "ReplayGain (dB)", "EBU R128 (dB)"});
    m_resultsTable->horizontalHeader()->setSectionResizeMode(0, QHeaderView::Stretch);
    m_resultsTable->horizontalHeader()->setSectionResizeMode(1, QHeaderView::ResizeToContents);
    m_resultsTable->horizontalHeader()->setSectionResizeMode(2, QHeaderView::ResizeToContents);
    m_resultsTable->horizontalHeader()->setSectionResizeMode(3, QHeaderView::ResizeToContents);
    m_resultsTable->horizontalHeader()->setSectionResizeMode(4, QHeaderView::ResizeToContents);
    m_resultsTable->setSelectionBehavior(QAbstractItemView::SelectRows);
    m_resultsTable->setEditTriggers(QAbstractItemView::NoEditTriggers);
    m_resultsTable->setShowGrid(true);

    mainLayout->addWidget(m_resultsTable, 1);

    // 4. Bottom Actions
    auto* bottomLayout = new QHBoxLayout();
    bottomLayout->setSpacing(12);

    auto* targetInfoLbl = new QLabel(this);
    targetInfoLbl->setText("Target: -18.0 LUFS (ReplayGain 2.0) / -23.0 LUFS (EBU R128)");
    targetInfoLbl->setStyleSheet("color: #8A92A6; font-size: 11px;");

    m_cancelCloseBtn = new QPushButton("Cancel Scan", this);
    connect(m_cancelCloseBtn, &QPushButton::clicked, this, &LoudnessScannerDialog::onCancelOrCloseClicked);

    m_writeBtn = new QPushButton("Write Tags to Files", this);
    m_writeBtn->setObjectName("WriteButton");
    connect(m_writeBtn, &QPushButton::clicked, this, &LoudnessScannerDialog::onWriteTagsClicked);

    bottomLayout->addWidget(targetInfoLbl);
    bottomLayout->addStretch();
    bottomLayout->addWidget(m_cancelCloseBtn);
    bottomLayout->addWidget(m_writeBtn);
    mainLayout->addLayout(bottomLayout);
}

void LoudnessScannerDialog::onScanProgress(int current, int total, const QString& current_file) {
    if (total > 0) {
        if (m_progressBar->maximum() != total) {
            m_progressBar->setMaximum(total);
        }
        m_progressBar->setValue(current);
    }
    m_statusLabel->setText(QString("Scanning %1 of %2: %3").arg(current).arg(total).arg(current_file));
}

void LoudnessScannerDialog::onTrackResult(int track_id, float lufs, float peak, float rg_gain_db, float r128_gain_db) {
    ScannedTrackItem item;
    item.track_id = track_id;
    item.title = m_statusLabel->text().section(": ", 1); // fallback title or we can get title from current progress
    if (item.title.isEmpty()) item.title = QString("Track #%1").arg(track_id);
    item.lufs = lufs;
    item.peak = peak;
    item.rg_gain_db = rg_gain_db;
    item.r128_gain_db = r128_gain_db;
    m_results.append(item);

    int row = m_resultsTable->rowCount();
    m_resultsTable->insertRow(row);

    auto* titleItem = new QTableWidgetItem(item.title);
    auto* lufsItem = new QTableWidgetItem(QString("%1 LUFS").arg(lufs, 0, 'f', 1));
    lufsItem->setTextAlignment(Qt::AlignCenter);

    auto* peakItem = new QTableWidgetItem(QString("%1").arg(peak, 0, 'f', 4));
    peakItem->setTextAlignment(Qt::AlignCenter);

    auto* rgItem = new QTableWidgetItem(QString("%1 dB").arg(rg_gain_db >= 0 ? "+" + QString::number(rg_gain_db, 'f', 2) : QString::number(rg_gain_db, 'f', 2)));
    rgItem->setTextAlignment(Qt::AlignCenter);
    rgItem->setForeground(QBrush(QColor("#00E5FF")));

    auto* r128Item = new QTableWidgetItem(QString("%1 dB").arg(r128_gain_db >= 0 ? "+" + QString::number(r128_gain_db, 'f', 2) : QString::number(r128_gain_db, 'f', 2)));
    r128Item->setTextAlignment(Qt::AlignCenter);

    m_resultsTable->setItem(row, 0, titleItem);
    m_resultsTable->setItem(row, 1, lufsItem);
    m_resultsTable->setItem(row, 2, peakItem);
    m_resultsTable->setItem(row, 3, rgItem);
    m_resultsTable->setItem(row, 4, r128Item);
}

void LoudnessScannerDialog::onScanFinished(bool success, const QString& error_msg) {
    m_scanning = false;
    if (m_progressBar->maximum() > 0 && success) {
        m_progressBar->setValue(m_progressBar->maximum());
    }
    m_cancelCloseBtn->setText("Close");

    if (!success && !error_msg.isEmpty()) {
        m_statusLabel->setText(QString("Scan stopped/error: %1").arg(error_msg));
        m_statusLabel->setStyleSheet("color: #FF5252; font-weight: 600;");
    } else {
        m_statusLabel->setText(QString("Scan complete! Analyzed %1 tracks.").arg(m_results.size()));
        m_statusLabel->setStyleSheet("color: #00E5FF; font-weight: 600;");
        if (!m_results.isEmpty()) {
            m_writeBtn->setEnabled(true);
        }
    }
}

void LoudnessScannerDialog::onCancelOrCloseClicked() {
    if (m_scanning) {
        m_scanning = false;
        playtune_cancel_loudness_scan();
        m_statusLabel->setText("Cancelling scan...");
        m_cancelCloseBtn->setText("Close");
    } else {
        reject();
    }
}

void LoudnessScannerDialog::onWriteTagsClicked() {
    if (m_results.isEmpty() || m_scanning) return;

    m_writeBtn->setEnabled(false);
    m_cancelCloseBtn->setEnabled(false);
    m_statusLabel->setText("Writing ReplayGain & EBU R128 tags to audio files and database...");
    m_statusLabel->setStyleSheet("color: #FFB300; font-weight: 600;");

    QVector<FfiLoudnessWriteItem> ffiItems;
    ffiItems.reserve(m_results.size());
    for (const auto& r : m_results) {
        FfiLoudnessWriteItem item;
        item.track_id = r.track_id;
        item.lufs = r.lufs;
        item.peak = r.peak;
        item.rg_gain_db = r.rg_gain_db;
        item.r128_gain_db = r.r128_gain_db;
        ffiItems.append(item);
    }

    int res = playtune_write_loudness_results(ffiItems.constData(), ffiItems.size());
    if (res == 1) {
        m_statusLabel->setText(QString("Successfully wrote tags to %1 files and updated database!").arg(ffiItems.size()));
        m_statusLabel->setStyleSheet("color: #00E5FF; font-weight: bold;");
        QMessageBox::information(this, "Success", "All loudness and ReplayGain tags written successfully!");
        accept();
    } else {
        m_statusLabel->setText("Finished writing with some errors/warnings. Check logs for details.");
        m_statusLabel->setStyleSheet("color: #FFB300; font-weight: bold;");
        m_cancelCloseBtn->setEnabled(true);
    }
}

void LoudnessScannerDialog::closeEvent(QCloseEvent* event) {
    if (m_scanning) {
        playtune_cancel_loudness_scan();
    }
    QDialog::closeEvent(event);
}

void LoudnessScannerDialog::mousePressEvent(QMouseEvent* event) {
    if (event->button() == Qt::LeftButton && event->pos().y() < 45) {
        m_dragPosition = event->globalPosition().toPoint() - frameGeometry().topLeft();
        event->accept();
    } else {
        QDialog::mousePressEvent(event);
    }
}

void LoudnessScannerDialog::mouseMoveEvent(QMouseEvent* event) {
    if (event->buttons() & Qt::LeftButton && !m_dragPosition.isNull() && event->pos().y() < 45) {
        move(event->globalPosition().toPoint() - m_dragPosition);
        event->accept();
    } else {
        QDialog::mouseMoveEvent(event);
    }
}
