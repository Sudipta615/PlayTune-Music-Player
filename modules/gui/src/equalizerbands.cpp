#include "equalizerwindow.h"
#include "apptheme.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QFrame>
#include <QLabel>
#include <QPushButton>
#include <QSlider>
#include <QIcon>
#include <QDoubleSpinBox>
#include <QComboBox>

QWidget* EqualizerWindow::createParamSliderCard(const QString& name, double minVal, double maxVal, double defaultVal, const QString& unit, int paramIdx) {
    auto* card = new QFrame(m_eqContentWidget);
    card->setObjectName("EqControlPanel");
    m_paramCards.append(card);
    
    auto* cardLayout = new QVBoxLayout(card);
    cardLayout->setContentsMargins(15, 10, 15, 10);
    cardLayout->setSpacing(2);

    auto* headerLayout = new QHBoxLayout();
    
    auto* labelName = new QLabel(name, card);
    m_paramNameLabels.append(labelName);
    
    auto* infoBtn = new QPushButton(card);
    infoBtn->setIcon(QIcon(":/resources/icons/info.png"));
    infoBtn->setIconSize(QSize(12, 12));
    infoBtn->setFixedSize(16, 16);
    infoBtn->setToolTip("Information about " + name + " parameter adjustment (" + QString::number(minVal) + unit + " to " + QString::number(maxVal) + unit + ")");
    m_paramInfoBtns.append(infoBtn);

    headerLayout->addWidget(labelName);
    headerLayout->addWidget(infoBtn);
    headerLayout->addStretch();
    cardLayout->addLayout(headerLayout);

    auto* sliderLayout = new QHBoxLayout();
    sliderLayout->setSpacing(8);

    auto* minLabel = new QLabel(QString::number(minVal) + unit, card);
    m_paramMinLabels.append(minLabel);
    
    auto* slider = new QSlider(Qt::Horizontal, card);
    
    if (minVal == -12.0) {
        slider->setRange(-120, 120);
        slider->setValue(static_cast<int>(defaultVal * 10));
        m_bassSlider = (paramIdx == 0) ? slider : m_bassSlider;
        m_trebleSlider = (paramIdx == 1) ? slider : m_trebleSlider;
    } else if (minVal == 0.0) {
        slider->setRange(0, 200);
        slider->setValue(static_cast<int>(defaultVal));
        m_stereoSlider = slider;
    } else {
        slider->setRange(-100, 100);
        slider->setValue(static_cast<int>(defaultVal * 100));
        m_balanceSlider = slider;
    }

    auto* maxLabel = new QLabel(QString("%1%2%3").arg(maxVal > 0 && unit == "dB" ? "+" : "").arg(maxVal).arg(unit), card);
    m_paramMaxLabels.append(maxLabel);

    sliderLayout->addWidget(minLabel);
    sliderLayout->addWidget(slider);
    sliderLayout->addWidget(maxLabel);
    cardLayout->addLayout(sliderLayout);

    auto* valLabel = new QLabel(card);
    valLabel->setObjectName("EqValueLabel");
    valLabel->setAlignment(Qt::AlignCenter);
    cardLayout->addWidget(valLabel);

    auto updateValLabel = [valLabel, minVal, unit, slider, name](double val) {
        QString text;
        if (minVal == -12.0) {
            text = QString("%1%2dB").arg(val >= 0 ? "+" : "").arg(val, 0, 'f', 1);
        } else if (minVal == 0.0) {
            text = QString("%1%").arg(val, 0, 'f', 0);
        } else {
            text = QString("%1%2").arg(val >= 0 ? "+" : "").arg(val, 0, 'f', 2);
        }
        valLabel->setText(text);
        if (slider) slider->setToolTip("Adjust " + name + ": " + text);
    };
    updateValLabel(defaultVal);

    if (paramIdx == 0) m_bassValueLabel = valLabel;
    else if (paramIdx == 1) m_trebleValueLabel = valLabel;
    else if (paramIdx == 2) m_stereoValueLabel = valLabel;
    else if (paramIdx == 3) m_balanceValueLabel = valLabel;

    connect(slider, &QSlider::valueChanged, this, [this, paramIdx, minVal, updateValLabel](int val) {
        double realVal = val;
        if (minVal == -12.0) realVal = val / 10.0;
        else if (minVal == -1.0) realVal = val / 100.0;
        
        updateValLabel(realVal);
        emit sliderParamChanged(paramIdx, realVal);
        scheduleSave();
    });

    return card;
}

void EqualizerWindow::updateAdvancedBandControls(int bandIdx) {
    if (bandIdx < 0 || bandIdx >= m_advBands.size()) return;
    m_currentAdvBandIdx = bandIdx;
    const AdvBandState& b = m_advBands[bandIdx];

    if (m_advFilterTypeCombo) {
        QSignalBlocker blocker(m_advFilterTypeCombo);
        m_advFilterTypeCombo->setCurrentIndex(b.filterType);
    }
    if (m_advFreqSlider && m_advFreqSpin) {
        QSignalBlocker blocker1(m_advFreqSlider);
        QSignalBlocker blocker2(m_advFreqSpin);
        m_advFreqSlider->setValue(qRound(b.freqHz));
        m_advFreqSpin->setValue(b.freqHz);
    }
    if (m_advQSlider && m_advQSpin) {
        QSignalBlocker blocker1(m_advQSlider);
        QSignalBlocker blocker2(m_advQSpin);
        m_advQSlider->setValue(qRound(b.q * 10.0));
        m_advQSpin->setValue(b.q);
    }
    if (m_advGainSlider && m_advGainLabel) {
        QSignalBlocker blocker(m_advGainSlider);
        m_advGainSlider->setValue(qRound(b.gainDb * 10.0));
        m_advGainLabel->setText(QString("%1%2 dB").arg(b.gainDb >= 0 ? "+" : "").arg(b.gainDb, 0, 'f', 1));
    }
    if (m_advBandToggle) {
        QSignalBlocker blocker(m_advBandToggle);
        m_advBandToggle->setChecked(b.enabled);
    }
}
