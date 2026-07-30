#include "playlistcreatedialog.h"

#include <QVBoxLayout>
#include <QHBoxLayout>

PlaylistCreateDialog::PlaylistCreateDialog(Mode mode, const QString& existingName, QWidget* parent)
    : QDialog(parent), m_mode(mode) {
    setWindowFlags(Qt::FramelessWindowHint | Qt::Dialog);
    setAttribute(Qt::WA_TranslucentBackground, true);
    setupUi();
    if (!existingName.isEmpty()) {
        m_nameEdit->setText(existingName);
        m_nameEdit->selectAll();
    }
}

void PlaylistCreateDialog::setupUi() {
    auto* outerLayout = new QVBoxLayout(this);
    outerLayout->setContentsMargins(0, 0, 0, 0);

    auto* cardFrame = new QFrame(this);
    cardFrame->setObjectName("DialogCard");
    cardFrame->setStyleSheet(
        "QFrame#DialogCard {"
        "  background-color: #0F121D;"
        "  border: 1px solid #1E2538;"
        "  border-radius: 16px;"
        "}"
    );

    auto* layout = new QVBoxLayout(cardFrame);
    layout->setContentsMargins(24, 20, 24, 20);
    layout->setSpacing(12);
    outerLayout->addWidget(cardFrame);

    auto* titleLabel = new QLabel(m_mode == Mode::Create ? "Create New Playlist" : "Rename Playlist");
    titleLabel->setStyleSheet("font-size: 18px; font-weight: 600; color: #F0F0F5;");
    layout->addWidget(titleLabel);

    auto* hintLabel = new QLabel(m_mode == Mode::Create
        ? "Enter a name for your new playlist. You can rename it later."
        : "Enter a new name for this playlist.");
    hintLabel->setStyleSheet("color: #9AA0AC; font-size: 12px;");
    hintLabel->setWordWrap(true);
    layout->addWidget(hintLabel);

    m_nameEdit = new QLineEdit;
    m_nameEdit->setPlaceholderText("Playlist name");
    m_nameEdit->setStyleSheet(
        "QLineEdit { background-color: #14101E; color: #F0F0F5; border: 1px solid #2A1E3E; "
        "border-radius: 6px; padding: 10px 14px; font-size: 14px; }"
        "QLineEdit:focus { border-color: #FF2A7A; }");
    layout->addWidget(m_nameEdit);

    auto* btnRow = new QHBoxLayout;
    btnRow->addStretch();

    auto* cancelBtn = new QPushButton("Cancel");
    cancelBtn->setStyleSheet(
        "QPushButton { background-color: transparent; color: #9AA0AC; border: none; "
        "padding: 8px 16px; }"
        "QPushButton:hover { color: #F0F0F5; }");
    cancelBtn->setCursor(Qt::PointingHandCursor);
    connect(cancelBtn, &QPushButton::clicked, this, &QDialog::reject);
    btnRow->addWidget(cancelBtn);

    auto* okBtn = new QPushButton(m_mode == Mode::Create ? "Create" : "Rename");
    okBtn->setStyleSheet(
        "QPushButton { background-color: #7B1FA2; color: white; border: none; "
        "border-radius: 6px; padding: 8px 18px; font-weight: 600; }"
        "QPushButton:hover { background-color: #9C27B0; }"
        "QPushButton:disabled { background-color: #3A2A4E; color: #888; }");
    okBtn->setCursor(Qt::PointingHandCursor);
    okBtn->setEnabled(false);
    connect(m_nameEdit, &QLineEdit::textChanged, this, [okBtn](const QString& text) {
        okBtn->setEnabled(!text.trimmed().isEmpty());
    });
    connect(okBtn, &QPushButton::clicked, this, [this]() {
        QString name = m_nameEdit->text().trimmed();
        if (!name.isEmpty()) {
            emit nameSubmitted(name);
            accept();
        }
    });
    btnRow->addWidget(okBtn);

    layout->addLayout(btnRow);

    // Enter key submits.
    connect(m_nameEdit, &QLineEdit::returnPressed, this, [this, okBtn]() {
        if (okBtn->isEnabled()) okBtn->click();
    });
}

QString PlaylistCreateDialog::playlistName() const {
    return m_nameEdit->text().trimmed();
}
