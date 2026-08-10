#include "settingspage.h"
#include "apptheme.h"
#include <QComboBox>
#include <QSignalBlocker>
#include <QAbstractItemView>

void SettingsPageWidget::clearAudioDeviceList() {
    if (!m_deviceCombo) return;
    if (m_deviceCombo->view() && m_deviceCombo->view()->isVisible()) return;
    QSignalBlocker b(m_deviceCombo);
    m_deviceCombo->clear();
}

void SettingsPageWidget::addAudioDeviceToList(const QString& name, bool isCurrent) {
    if (!m_deviceCombo) return;
    if (m_deviceCombo->view() && m_deviceCombo->view()->isVisible()) return;
    QSignalBlocker b(m_deviceCombo);
    int existingIdx = m_deviceCombo->findText(name);
    if (existingIdx >= 0) {
        if (isCurrent || (!m_currentDevice.isEmpty() && name == m_currentDevice)) {
            if (m_deviceCombo->currentIndex() != existingIdx) {
                m_deviceCombo->setCurrentIndex(existingIdx);
                m_currentDevice = name;
            }
        }
        return;
    }
    m_deviceCombo->addItem(name);
    if (isCurrent || (!m_currentDevice.isEmpty() && name == m_currentDevice)) {
        m_deviceCombo->setCurrentIndex(m_deviceCombo->count() - 1);
        m_currentDevice = name;
    }
}
