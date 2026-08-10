#include "equalizerwindow.h"

void EqualizerWindow::selectPresetButton(int presetIdx) {
    if (presetIdx >= 0 && presetIdx < m_presetBtns.size()) {
        m_presetBtns[presetIdx]->setChecked(true);
    }
}

void EqualizerWindow::setPresetGains(int presetIdx) {
    QVector<double> gains(10, 0.0);
    switch (presetIdx) {
        case 0: // Flat
            gains.fill(0.0);
            break;
        case 1: // Pop
            gains = {-1.0, 1.5, 2.5, 3.0, 1.0, -1.0, -1.5, -1.5, -1.0, -1.0};
            break;
        case 2: // Rock
            gains = {4.0, 3.0, -2.0, -4.0, -1.5, 1.0, 3.0, 4.5, 5.0, 5.0};
            break;
        case 3: // Jazz
            gains = {3.0, 2.0, 1.0, 1.5, -1.0, -1.0, 0.0, 1.5, 2.5, 3.0};
            break;
        case 4: // Classical
            gains = {3.5, 2.5, 2.0, 1.5, -1.0, -1.0, -0.5, 1.0, 2.0, 2.5};
            break;
        case 5: // Electronic
            gains = {4.5, 3.5, 1.0, 0.0, -2.0, 1.5, 1.0, 1.0, 3.5, 4.0};
            break;
        case 6: // Hip Hop
            gains = {5.0, 4.0, 1.5, 2.5, -1.0, -1.5, 0.5, -0.5, 2.0, 3.0};
            break;
        default: // Custom
            return; // Don't alter gains
    }
    m_curveWidget->setGains(gains);
    for (int i = 0; i < 10; ++i) {
        emit bandChanged(i, gains[i]);
    }
}

void EqualizerWindow::setEqGains(const QVector<double>& gains) {
    m_curveWidget->setGains(gains);
}

void EqualizerWindow::setEqEnabled(bool enabled) {
    QSignalBlocker block(m_enableToggle);
    m_enableToggle->setChecked(enabled);
    m_eqContentWidget->setEnabled(enabled);
}
