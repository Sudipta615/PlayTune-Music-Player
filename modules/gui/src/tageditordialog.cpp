#include "tageditordialog.h"
#include "gui_bridge.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QGridLayout>
#include <QFrame>
#include <QFileDialog>
#include <QMessageBox>
#include <QIcon>
#include <QPixmap>
#include <QFileInfo>

TagEditorDialog::TagEditorDialog(const TagEditorTrackData& data, QWidget* parent)
    : QDialog(parent), m_originalData(data), m_currentCoverPath(data.cover_path)
{
    setObjectName("TagEditorDialog");
    setWindowIcon(QIcon(":/resources/icons/playtune_logo.png"));
    setWindowFlags(Qt::Dialog | Qt::FramelessWindowHint);
    setAttribute(Qt::WA_StyledBackground, true);

    resize(700, 520);
    setMinimumSize(660, 480);

    setupUi();
    updateCoverPreview();
}

void TagEditorDialog::setupUi() {
    setStyleSheet(
        "QDialog#TagEditorDialog { background-color: #12151F; border: 1px solid #242A3D; border-radius: 10px; }"
        "QLabel { color: #FFFFFF; font-size: 13px; }"
        "QLineEdit {"
        "   background-color: #181B28; border: 1px solid #242A3D; border-radius: 6px;"
        "   padding: 6px 10px; color: #FFFFFF; font-size: 13px; selection-background-color: #FF2A7A;"
        "}"
        "QLineEdit:focus { border: 1px solid #00E5FF; background-color: #1E2233; }"
        "QSpinBox {"
        "   background-color: #181B28; border: 1px solid #242A3D; border-radius: 6px;"
        "   padding: 6px 8px; padding-right: 24px; color: #FFFFFF; font-size: 13px; selection-background-color: #FF2A7A;"
        "}"
        "QSpinBox:focus { border: 1px solid #00E5FF; background-color: #1E2233; }"
        "QSpinBox::up-button { subcontrol-origin: border; subcontrol-position: top right; width: 22px; height: 14px; border-left: 1px solid #242A3D; border-bottom: 1px solid #242A3D; border-top-right-radius: 5px; background-color: #242A3D; }"
        "QSpinBox::up-button:hover { background-color: #7B1FA2; }"
        "QSpinBox::down-button { subcontrol-origin: border; subcontrol-position: bottom right; width: 22px; height: 14px; border-left: 1px solid #242A3D; border-bottom-right-radius: 5px; background-color: #242A3D; }"
        "QSpinBox::down-button:hover { background-color: #7B1FA2; }"
        "QPushButton {"
        "   font-size: 13px; font-weight: bold; color: #FFFFFF;"
        "   background-color: #181B28; border: 1px solid #242A3D; border-radius: 6px;"
        "   padding: 8px 16px; outline: none;"
        "}"
        "QPushButton:hover { background-color: #232736; border-color: #38415C; }"
        "QPushButton#SaveButton {"
        "   background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #FF2A7A, stop:1 #8B26B6);"
        "   border: 1px solid #FF2A7A; color: #FFFFFF;"
        "}"
        "QPushButton#SaveButton:hover {"
        "   background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #FF458D, stop:1 #9E3AC7);"
        "}"
        "QPushButton#RemoveCoverBtn:hover { background-color: #3D1022; border-color: #FF2A7A; color: #FF6688; }"
    );

    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(20, 20, 20, 20);
    mainLayout->setSpacing(16);

    // 1. Header Bar
    auto* headerLayout = new QHBoxLayout();
    headerLayout->setContentsMargins(0, 0, 0, 5);
    headerLayout->setSpacing(10);

    auto* iconLabel = new QLabel(this);
    iconLabel->setPixmap(QIcon(":/resources/icons/playtune_logo.png").pixmap(20, 20));
    
    auto* titleLabel = new QLabel("Metadata Tag Editor", this);
    titleLabel->setStyleSheet("font-size: 17px; font-weight: bold; color: #FFFFFF;");

    auto* closeBtn = new QPushButton(this);
    closeBtn->setIcon(QIcon(":/resources/icons/close.png"));
    closeBtn->setIconSize(QSize(14, 14));
    closeBtn->setFixedSize(28, 28);
    closeBtn->setStyleSheet("QPushButton { border: none; background: transparent; padding: 0px; } QPushButton:hover { background-color: #3D1022; border-radius: 14px; }");
    connect(closeBtn, &QPushButton::clicked, this, &QDialog::reject);

    headerLayout->addWidget(iconLabel);
    headerLayout->addWidget(titleLabel);
    headerLayout->addStretch();
    headerLayout->addWidget(closeBtn);
    mainLayout->addLayout(headerLayout);

    // 2. Card Panel container for Form + Cover art
    auto* cardFrame = new QFrame(this);
    cardFrame->setStyleSheet("QFrame { background-color: #161926; border: 1px solid #242A3D; border-radius: 8px; }");
    auto* cardLayout = new QHBoxLayout(cardFrame);
    cardLayout->setContentsMargins(18, 18, 18, 18);
    cardLayout->setSpacing(24);

    // Left Column: Cover Art & Cover Buttons
    auto* leftLayout = new QVBoxLayout();
    leftLayout->setSpacing(12);

    m_coverPreview = new QLabel(cardFrame);
    m_coverPreview->setFixedSize(200, 200);
    m_coverPreview->setAlignment(Qt::AlignCenter);
    m_coverPreview->setStyleSheet("QLabel { background-color: #10121C; border: 1px solid #2C334A; border-radius: 8px; }");
    leftLayout->addWidget(m_coverPreview, 0, Qt::AlignHCenter);

    auto* changeCoverBtn = new QPushButton("Change Cover...", cardFrame);
    auto* removeCoverBtn = new QPushButton("Remove Cover", cardFrame);
    removeCoverBtn->setObjectName("RemoveCoverBtn");

    connect(changeCoverBtn, &QPushButton::clicked, this, &TagEditorDialog::onChangeCoverClicked);
    connect(removeCoverBtn, &QPushButton::clicked, this, &TagEditorDialog::onRemoveCoverClicked);

    leftLayout->addWidget(changeCoverBtn);
    leftLayout->addWidget(removeCoverBtn);
    leftLayout->addStretch();

    cardLayout->addLayout(leftLayout);

    // Right Column: Form Grid
    auto* gridLayout = new QGridLayout();
    gridLayout->setSpacing(12);
    gridLayout->setContentsMargins(0, 0, 0, 0);

    auto addRow = [&](int row, const QString& labelText, QWidget* field) {
        auto* lbl = new QLabel(labelText, cardFrame);
        lbl->setStyleSheet("font-weight: 600; color: #A0A6B8;");
        gridLayout->addWidget(lbl, row, 0, Qt::AlignRight | Qt::AlignVCenter);
        gridLayout->addWidget(field, row, 1);
    };

    m_titleEdit = new QLineEdit(m_originalData.title, cardFrame);
    m_artistEdit = new QLineEdit(m_originalData.artist, cardFrame);
    m_albumEdit = new QLineEdit(m_originalData.album, cardFrame);
    m_albumArtistEdit = new QLineEdit(m_originalData.album_artist, cardFrame);
    m_genreEdit = new QLineEdit(m_originalData.genre, cardFrame);

    m_yearSpin = new QSpinBox(cardFrame);
    m_yearSpin->setRange(0, 9999);
    m_yearSpin->setSpecialValueText("Unset");
    m_yearSpin->setValue(m_originalData.year);

    m_trackSpin = new QSpinBox(cardFrame);
    m_trackSpin->setRange(0, 999);
    m_trackSpin->setSpecialValueText("Unset");
    m_trackSpin->setValue(m_originalData.track_number);

    m_discSpin = new QSpinBox(cardFrame);
    m_discSpin->setRange(0, 99);
    m_discSpin->setSpecialValueText("Unset");
    m_discSpin->setValue(m_originalData.disc_number);

    addRow(0, "Title:", m_titleEdit);
    addRow(1, "Artist:", m_artistEdit);
    addRow(2, "Album:", m_albumEdit);
    addRow(3, "Album Artist:", m_albumArtistEdit);
    addRow(4, "Genre:", m_genreEdit);
    addRow(5, "Year:", m_yearSpin);
    addRow(6, "Track #:", m_trackSpin);
    addRow(7, "Disc #:", m_discSpin);

    cardLayout->addLayout(gridLayout, 1);
    mainLayout->addWidget(cardFrame, 1);

    // 3. Bottom Actions Bar
    auto* bottomLayout = new QHBoxLayout();
    bottomLayout->setSpacing(12);

    auto* resetBtn = new QPushButton("Reset", this);
    connect(resetBtn, &QPushButton::clicked, this, [this]() {
        m_titleEdit->setText(m_originalData.title);
        m_artistEdit->setText(m_originalData.artist);
        m_albumEdit->setText(m_originalData.album);
        m_albumArtistEdit->setText(m_originalData.album_artist);
        m_genreEdit->setText(m_originalData.genre);
        m_yearSpin->setValue(m_originalData.year);
        m_trackSpin->setValue(m_originalData.track_number);
        m_discSpin->setValue(m_originalData.disc_number);
        m_currentCoverPath = m_originalData.cover_path;
        updateCoverPreview();
    });

    auto* cancelBtn = new QPushButton("Cancel", this);
    connect(cancelBtn, &QPushButton::clicked, this, &QDialog::reject);

    auto* saveBtn = new QPushButton("Save Tags", this);
    saveBtn->setObjectName("SaveButton");
    connect(saveBtn, &QPushButton::clicked, this, &TagEditorDialog::onSaveClicked);

    bottomLayout->addWidget(resetBtn);
    bottomLayout->addStretch();
    bottomLayout->addWidget(cancelBtn);
    bottomLayout->addWidget(saveBtn);
    mainLayout->addLayout(bottomLayout);
}

void TagEditorDialog::updateCoverPreview() {
    if (m_currentCoverPath.isEmpty() || !QFileInfo::exists(m_currentCoverPath)) {
        m_coverPreview->setText("No Cover Art");
        m_coverPreview->setPixmap(QPixmap());
        return;
    }

    QPixmap pix(m_currentCoverPath);
    if (pix.isNull()) {
        m_coverPreview->setText("Invalid Image");
        return;
    }

    m_coverPreview->setText("");
    m_coverPreview->setPixmap(pix.scaled(196, 196, Qt::KeepAspectRatio, Qt::SmoothTransformation));
}

void TagEditorDialog::onChangeCoverClicked() {
    QString path = QFileDialog::getOpenFileName(
        this,
        "Select Album Cover Art",
        QString(),
        "Images (*.png *.jpg *.jpeg *.webp)"
    );

    if (!path.isEmpty()) {
        m_currentCoverPath = path;
        updateCoverPreview();
    }
}

void TagEditorDialog::onRemoveCoverClicked() {
    m_currentCoverPath = "";
    updateCoverPreview();
}

void TagEditorDialog::onSaveClicked() {
    QByteArray titleUtf = m_titleEdit->text().trimmed().toUtf8();
    QByteArray artistUtf = m_artistEdit->text().trimmed().toUtf8();
    QByteArray albumUtf = m_albumEdit->text().trimmed().toUtf8();
    QByteArray albumArtistUtf = m_albumArtistEdit->text().trimmed().toUtf8();
    QByteArray genreUtf = m_genreEdit->text().trimmed().toUtf8();

    // Determine cover path argument:
    // If m_currentCoverPath == m_originalData.cover_path -> null (no change)
    // If m_currentCoverPath.isEmpty() -> empty string "" (remove cover)
    // Else -> new file path string
    QByteArray coverUtf;
    const char* coverPtr = nullptr;
    if (m_currentCoverPath != m_originalData.cover_path) {
        coverUtf = m_currentCoverPath.toUtf8();
        coverPtr = coverUtf.constData();
    }

    FfiTagEditRequest req;
    req.track_id = m_originalData.track_id;
    req.title = titleUtf.constData();
    req.artist = artistUtf.constData();
    req.album = albumUtf.constData();
    req.album_artist = albumArtistUtf.constData();
    req.genre = genreUtf.constData();
    req.year = m_yearSpin->value();
    req.track_number = m_trackSpin->value();
    req.disc_number = m_discSpin->value();
    req.cover_image_path = coverPtr;

    int res = playtune_update_track_tags(&req);
    if (res != 1) {
        QMessageBox::warning(
            this,
            "Tag Editor Error",
            "Failed to save metadata tags. Ensure the audio file is not read-only or locked by another process."
        );
        return;
    }

    accept();
}

void TagEditorDialog::mousePressEvent(QMouseEvent* event) {
    if (event->button() == Qt::LeftButton && event->pos().y() < 45) {
        m_dragPosition = event->globalPosition().toPoint() - frameGeometry().topLeft();
        event->accept();
    } else {
        QDialog::mousePressEvent(event);
    }
}

void TagEditorDialog::mouseMoveEvent(QMouseEvent* event) {
    if (event->buttons() & Qt::LeftButton && !m_dragPosition.isNull() && event->pos().y() < 45) {
        move(event->globalPosition().toPoint() - m_dragPosition);
        event->accept();
    } else {
        QDialog::mouseMoveEvent(event);
    }
}
