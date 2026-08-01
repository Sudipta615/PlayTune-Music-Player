#include "sleeptimerdialog.h"

#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QGridLayout>
#include <QSettings>

SleepTimerDialog::SleepTimerDialog(QWidget* parent)
    : QDialog(parent) {
    setWindowFlags(Qt::FramelessWindowHint | Qt::Dialog);
    setAttribute(Qt::WA_TranslucentBackground, true);
    setupUi();
}

void SleepTimerDialog::setupUi() {
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

    auto* title = new QLabel("Sleep Timer");
    title->setStyleSheet("font-size: 18px; font-weight: 600; color: #F0F0F5;");
    layout->addWidget(title);

    m_hintLabel = new QLabel(
        "PlayTune will pause playback after the selected duration.\n"
        "Choose a preset or enter a custom value (1–120 minutes).");
    m_hintLabel->setStyleSheet("color: #9AA0AC; font-size: 12px;");
    m_hintLabel->setWordWrap(true);
    layout->addWidget(m_hintLabel);

    // Preset grid.
    auto* presetLayout = new QGridLayout;
    presetLayout->setSpacing(8);
    QList<int> presets = {15, 30, 45, 60, 90};
    for (int i = 0; i < presets.size(); ++i) {
        int mins = presets[i];
        auto* btn = new QPushButton(QString("%1 min").arg(mins));
        btn->setStyleSheet(
            "QPushButton { background-color: #1B1130; color: #F0F0F5; border: 1px solid #2A1E3E; "
            "border-radius: 6px; padding: 10px 14px; font-size: 13px; }"
            "QPushButton:hover { background-color: #2A1E3E; border-color: #FF2A7A; }");
        btn->setCursor(Qt::PointingHandCursor);
        connect(btn, &QPushButton::clicked, this, [this, mins]() { pickPreset(mins); });
        presetLayout->addWidget(btn, i / 3, i % 3);
    }
    layout->addLayout(presetLayout);

    // Custom value row.
    auto* customRow = new QHBoxLayout;
    auto* customLabel = new QLabel("Custom:");
    customLabel->setStyleSheet("color: #C8C8D0; font-size: 13px;");
    customRow->addWidget(customLabel);
    m_customSpin = new QSpinBox;
    m_customSpin->setRange(1, 120);
    m_customSpin->setValue(20);
    m_customSpin->setSuffix(" min");
    m_customSpin->setStyleSheet(
        "QSpinBox { background-color: #14101E; color: #F0F0F5; border: 1px solid #2A1E3E; "
        "border-radius: 6px; padding: 4px 8px; padding-right: 24px; font-size: 13px; }"
        "QSpinBox::up-button { subcontrol-origin: border; subcontrol-position: top right; width: 22px; height: 14px; border-left: 1px solid #2A1E3E; border-bottom: 1px solid #2A1E3E; border-top-right-radius: 5px; background-color: #241D3B; }"
        "QSpinBox::up-button:hover { background-color: #7B1FA2; }"
        "QSpinBox::down-button { subcontrol-origin: border; subcontrol-position: bottom right; width: 22px; height: 14px; border-left: 1px solid #2A1E3E; border-bottom-right-radius: 5px; background-color: #241D3B; }"
        "QSpinBox::down-button:hover { background-color: #7B1FA2; }");
    customRow->addWidget(m_customSpin);
    customRow->addStretch();
    auto* customBtn = new QPushButton("Start");
    customBtn->setStyleSheet(
        "QPushButton { background-color: #7B1FA2; color: white; border: none; "
        "border-radius: 6px; padding: 8px 16px; font-weight: 600; }"
        "QPushButton:hover { background-color: #9C27B0; }");
    customBtn->setCursor(Qt::PointingHandCursor);
    connect(customBtn, &QPushButton::clicked, this, &SleepTimerDialog::pickCustom);
    customRow->addWidget(customBtn);
    layout->addLayout(customRow);

    // Cancel row.
    auto* cancelBtn = new QPushButton("Cancel Active Timer");
    cancelBtn->setStyleSheet(
        "QPushButton { background-color: transparent; color: #FF2A7A; border: 1px solid #FF2A7A; "
        "border-radius: 6px; padding: 8px 14px; }"
        "QPushButton:hover { background-color: rgba(255, 42, 122, 0.15); }");
    cancelBtn->setCursor(Qt::PointingHandCursor);
    connect(cancelBtn, &QPushButton::clicked, this, &SleepTimerDialog::cancelTimer);
    layout->addWidget(cancelBtn);

    // Close button.
    auto* closeBtn = new QPushButton("Close");
    closeBtn->setStyleSheet(
        "QPushButton { background-color: transparent; color: #9AA0AC; border: none; "
        "padding: 6px; }"
        "QPushButton:hover { color: #F0F0F5; }");
    closeBtn->setCursor(Qt::PointingHandCursor);
    connect(closeBtn, &QPushButton::clicked, this, &QDialog::accept);
    layout->addWidget(closeBtn, 0, Qt::AlignRight);
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
