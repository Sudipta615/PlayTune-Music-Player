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

    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this](const ThemePalette& p) {
        updateThemeStyles(p);
    });
    updateThemeStyles(ThemeManager::instance().currentTheme());
}

void PlaylistCreateDialog::setupUi() {
    auto* outerLayout = new QVBoxLayout(this);
    outerLayout->setContentsMargins(0, 0, 0, 0);

    m_cardFrame = new QFrame(this);
    m_cardFrame->setObjectName("DialogCard");

    auto* layout = new QVBoxLayout(m_cardFrame);
    layout->setContentsMargins(24, 20, 24, 20);
    layout->setSpacing(12);
    outerLayout->addWidget(m_cardFrame);

    m_titleLabel = new QLabel(m_mode == Mode::Create ? "Create New Playlist" : "Rename Playlist", m_cardFrame);
    layout->addWidget(m_titleLabel);

    m_hintLabel = new QLabel(m_mode == Mode::Create
        ? "Enter a name for your new playlist. You can rename it later."
        : "Enter a new name for this playlist.", m_cardFrame);
    m_hintLabel->setWordWrap(true);
    layout->addWidget(m_hintLabel);

    m_nameEdit = new QLineEdit(m_cardFrame);
    m_nameEdit->setPlaceholderText("Playlist name");
    layout->addWidget(m_nameEdit);

    auto* btnRow = new QHBoxLayout;
    btnRow->addStretch();

    m_cancelBtn = new QPushButton("Cancel", m_cardFrame);
    m_cancelBtn->setCursor(Qt::PointingHandCursor);
    connect(m_cancelBtn, &QPushButton::clicked, this, &QDialog::reject);
    btnRow->addWidget(m_cancelBtn);

    m_okBtn = new QPushButton(m_mode == Mode::Create ? "Create" : "Rename", m_cardFrame);
    m_okBtn->setCursor(Qt::PointingHandCursor);
    m_okBtn->setEnabled(false);
    connect(m_nameEdit, &QLineEdit::textChanged, this, [this](const QString& text) {
        if (m_okBtn) m_okBtn->setEnabled(!text.trimmed().isEmpty());
    });
    connect(m_okBtn, &QPushButton::clicked, this, [this]() {
        QString name = m_nameEdit->text().trimmed();
        if (!name.isEmpty()) {
            emit nameSubmitted(name);
            accept();
        }
    });
    btnRow->addWidget(m_okBtn);

    layout->addLayout(btnRow);

    // Enter key submits.
    connect(m_nameEdit, &QLineEdit::returnPressed, this, [this]() {
        if (m_okBtn && m_okBtn->isEnabled()) m_okBtn->click();
    });
}

void PlaylistCreateDialog::updateThemeStyles(const ThemePalette& p) {
    if (m_cardFrame) {
        m_cardFrame->setStyleSheet(QString(
            "QFrame#DialogCard {"
            "  background-color: %1;"
            "  border: 1.5px solid %2;"
            "  border-radius: 16px;"
            "}"
        ).arg(p.cardBg.name(), p.cardBorder.name()));
    }
    if (m_titleLabel) {
        m_titleLabel->setStyleSheet(QString("font-size: 18px; font-weight: 600; color: %1; background: transparent;").arg(p.primaryText.name()));
    }
    if (m_hintLabel) {
        m_hintLabel->setStyleSheet(QString("font-size: 12px; color: %1; background: transparent;").arg(p.mutedText.name()));
    }
    if (m_nameEdit) {
        m_nameEdit->setStyleSheet(QString(
            "QLineEdit { background-color: %1; color: %2; border: 1px solid %3; "
            "border-radius: 6px; padding: 10px 14px; font-size: 14px; }"
            "QLineEdit:focus { border-color: %4; }"
        ).arg(p.headerBg.name(), p.primaryText.name(), p.cardBorder.name(), p.primaryAccent.name()));
    }
    if (m_cancelBtn) {
        m_cancelBtn->setStyleSheet(QString(
            "QPushButton { background-color: transparent; color: %1; border: none; padding: 8px 16px; font-size: 13px; font-weight: 500; }"
            "QPushButton:hover { color: %2; }"
        ).arg(p.mutedText.name(), p.primaryText.name()));
    }
    if (m_okBtn) {
        m_okBtn->setStyleSheet(QString(
            "QPushButton { background-color: %1; color: #FFFFFF; border: none; border-radius: 6px; padding: 8px 18px; font-weight: 600; font-size: 13px; }"
            "QPushButton:hover { background-color: %2; }"
            "QPushButton:disabled { background-color: %3; color: %4; }"
        ).arg(p.primaryAccent.name(), p.secondaryAccent.name(), p.itemHoverBg.name(), p.mutedText.name()));
    }
}

QString PlaylistCreateDialog::playlistName() const {
    return m_nameEdit->text().trimmed();
}
