#include "sleeptimerdialog.h"

#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QGridLayout>

SleepTimerDialog::SleepTimerDialog(QWidget* parent)
    : QDialog(parent) {
    setWindowFlags(Qt::FramelessWindowHint | Qt::Dialog);
    setAttribute(Qt::WA_TranslucentBackground, true);
    setupUi();

    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this](const ThemePalette& p) {
        updateThemeStyles(p);
    });
    updateThemeStyles(ThemeManager::instance().currentTheme());
}

void SleepTimerDialog::setupUi() {
    auto* outerLayout = new QVBoxLayout(this);
    outerLayout->setContentsMargins(0, 0, 0, 0);

    m_cardFrame = new QFrame(this);
    m_cardFrame->setObjectName("DialogCard");

    auto* layout = new QVBoxLayout(m_cardFrame);
    layout->setContentsMargins(24, 20, 24, 20);
    layout->setSpacing(12);
    outerLayout->addWidget(m_cardFrame);

    m_titleLabel = new QLabel("Sleep Timer", m_cardFrame);
    layout->addWidget(m_titleLabel);

    m_hintLabel = new QLabel(
        "PlayTune will pause playback after the selected duration.\n"
        "Choose a preset or enter a custom value (1–120 minutes).", m_cardFrame);
    m_hintLabel->setWordWrap(true);
    layout->addWidget(m_hintLabel);

    // Preset grid.
    auto* presetLayout = new QGridLayout;
    presetLayout->setSpacing(8);
    QList<int> presets = {15, 30, 45, 60, 90};
    for (int i = 0; i < presets.size(); ++i) {
        int mins = presets[i];
        auto* btn = new QPushButton(QString("%1 min").arg(mins), m_cardFrame);
        btn->setCursor(Qt::PointingHandCursor);
        connect(btn, &QPushButton::clicked, this, [this, mins]() { pickPreset(mins); });
        presetLayout->addWidget(btn, i / 3, i % 3);
        m_presetBtns.append(btn);
    }
    layout->addLayout(presetLayout);

    // Custom value row.
    auto* customRow = new QHBoxLayout;
    m_customLabel = new QLabel("Custom:", m_cardFrame);
    customRow->addWidget(m_customLabel);

    m_customSpin = new QSpinBox(m_cardFrame);
    m_customSpin->setRange(1, 120);
    m_customSpin->setValue(20);
    m_customSpin->setSuffix(" min");
    customRow->addWidget(m_customSpin);
    customRow->addStretch();

    m_customBtn = new QPushButton("Start", m_cardFrame);
    m_customBtn->setCursor(Qt::PointingHandCursor);
    connect(m_customBtn, &QPushButton::clicked, this, &SleepTimerDialog::pickCustom);
    customRow->addWidget(m_customBtn);
    layout->addLayout(customRow);

    // Cancel row.
    m_cancelBtn = new QPushButton("Cancel Active Timer", m_cardFrame);
    m_cancelBtn->setCursor(Qt::PointingHandCursor);
    connect(m_cancelBtn, &QPushButton::clicked, this, &SleepTimerDialog::cancelTimer);
    layout->addWidget(m_cancelBtn);

    // Close button.
    m_closeBtn = new QPushButton("Close", m_cardFrame);
    m_closeBtn->setCursor(Qt::PointingHandCursor);
    connect(m_closeBtn, &QPushButton::clicked, this, &QDialog::accept);
    layout->addWidget(m_closeBtn, 0, Qt::AlignRight);
}

void SleepTimerDialog::updateThemeStyles(const ThemePalette& p) {
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
    if (m_customLabel) {
        m_customLabel->setStyleSheet(QString("font-size: 13px; font-weight: 500; color: %1; background: transparent;").arg(p.secondaryText.name()));
    }

    const QString presetBtnStyle = QString(
        "QPushButton { background-color: %1; color: %2; border: 1px solid %3; "
        "border-radius: 6px; padding: 10px 14px; font-size: 13px; font-weight: 500; }"
        "QPushButton:hover { background-color: %4; border-color: %5; color: %6; }"
    ).arg(p.headerBg.name(), p.primaryText.name(), p.cardBorder.name(),
          p.itemHoverBg.name(), p.primaryAccent.name(), p.secondaryAccent.name());

    for (auto* btn : m_presetBtns) {
        if (btn) btn->setStyleSheet(presetBtnStyle);
    }

    if (m_customSpin) {
        m_customSpin->setStyleSheet(QString(
            "QSpinBox { background-color: %1; color: %2; border: 1px solid %3; "
            "border-radius: 6px; padding: 4px 8px; padding-right: 24px; font-size: 13px; }"
            "QSpinBox::up-button { subcontrol-origin: border; subcontrol-position: top right; width: 22px; height: 14px; border-left: 1px solid %3; border-bottom: 1px solid %3; border-top-right-radius: 5px; background-color: %4; }"
            "QSpinBox::up-button:hover { background-color: %5; }"
            "QSpinBox::down-button { subcontrol-origin: border; subcontrol-position: bottom right; width: 22px; height: 14px; border-left: 1px solid %3; border-bottom-right-radius: 5px; background-color: %4; }"
            "QSpinBox::down-button:hover { background-color: %5; }"
        ).arg(p.headerBg.name(), p.primaryText.name(), p.cardBorder.name(), p.itemHoverBg.name(), p.primaryAccent.name()));
    }

    if (m_customBtn) {
        m_customBtn->setStyleSheet(QString(
            "QPushButton { background-color: %1; color: #FFFFFF; border: none; "
            "border-radius: 6px; padding: 8px 16px; font-weight: 600; font-size: 13px; }"
            "QPushButton:hover { background-color: %2; }"
        ).arg(p.primaryAccent.name(), p.secondaryAccent.name()));
    }

    if (m_cancelBtn) {
        m_cancelBtn->setStyleSheet(QString(
            "QPushButton { background-color: transparent; color: %1; border: 1px solid %1; "
            "border-radius: 6px; padding: 8px 14px; font-size: 13px; font-weight: 500; }"
            "QPushButton:hover { background-color: %2; }"
        ).arg(p.secondaryAccent.name(), p.itemHoverBg.name()));
    }

    if (m_closeBtn) {
        m_closeBtn->setStyleSheet(QString(
            "QPushButton { background-color: transparent; color: %1; border: none; "
            "padding: 6px; font-size: 13px; font-weight: 500; }"
            "QPushButton:hover { color: %2; }"
        ).arg(p.mutedText.name(), p.primaryText.name()));
    }
}

void SleepTimerDialog::pickPreset(int minutes) {
    emit durationSelected(minutes);
    accept();
}

void SleepTimerDialog::pickCustom() {
    emit durationSelected(m_customSpin->value());
    accept();
}

void SleepTimerDialog::cancelTimer() {
    emit durationSelected(0);
    accept();
}
