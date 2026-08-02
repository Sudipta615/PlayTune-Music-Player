#ifndef SETTINGSPAGE_H
#define SETTINGSPAGE_H

#include <QWidget>
#include <QComboBox>
#include <QPushButton>
#include <QListWidget>
#include <QSpinBox>
#include "custom_widgets.h"

class SettingsPageWidget : public QWidget {
    Q_OBJECT
public:
    explicit SettingsPageWidget(QWidget* parent = nullptr);
    ~SettingsPageWidget() override = default;

    bool isTooltipsEnabled() const;
    bool isOptimizedMode() const { return m_optimizedMode; }
    bool isCrossfadeEnabled() const { return m_crossfadeEnabled; }
    bool isNormalizeEnabled() const { return m_normalizeEnabled; }
    bool isGaplessEnabled() const { return m_gaplessEnabled; }
    bool isCursorFollowsPlayback() const { return m_cursorFollows; }
    bool isNotificationsEnabled() const { return m_notificationsEnabled; }
    bool isTrayEnabled() const { return m_trayEnabled; }
    bool isMinimizeToTray() const { return m_minimizeToTray; }

signals:
    void themeChanged(const QString& themeName);
    void tooltipsToggled(bool enabled);
    void crossfadeToggled(bool enabled);
    void normalizeToggled(bool enabled);
    void gaplessToggled(bool enabled);
    void cursorFollowsToggled(bool enabled);
    void notificationsToggled(bool enabled);
    void trayToggled(bool enabled);
    void minimizeToTrayToggled(bool enabled);
    void crossfadeDurationChanged(int duration_ms);
    void outputBackendChanged(int backend);
    void outputDeviceChanged(const QString& deviceName);
    void optimizedModeToggled(bool enabled);
    void addSongsRequested();
    void addFoldersRequested();
    void deleteFolderRequested(int folderId);
    void importM3URequested();
    void exportM3URequested();

public slots:
    // Update folder list in Settings from bridge signals
    void clearFolderList();
    void addFolderToList(int id, const QString& path, const QString& name, int trackCount);
    void clearAudioDeviceList();
    void addAudioDeviceToList(const QString& name, bool isCurrent);

protected:
    void showEvent(QShowEvent* event) override;

private:
    void setupUi();
    void loadSettings();
    void saveSettings();

    QComboBox* m_themeCombo = nullptr;
    QComboBox* m_backendCombo = nullptr;
    QComboBox* m_deviceCombo = nullptr;
    ToggleSwitch* m_tooltipToggle = nullptr;
    ToggleSwitch* m_crossfadeToggle = nullptr;
    ToggleSwitch* m_normalizeToggle = nullptr;
    ToggleSwitch* m_gaplessToggle = nullptr;
    ToggleSwitch* m_cursorFollowToggle = nullptr;
    ToggleSwitch* m_notificationsToggle = nullptr;
    ToggleSwitch* m_trayToggle = nullptr;
    ToggleSwitch* m_minimizeToTrayToggle = nullptr;
    ToggleSwitch* m_optimizedModeToggle = nullptr;
    QPushButton*  m_loudnessScanBtn = nullptr;
    QSpinBox* m_crossfadeDurationSpin = nullptr;
    QPushButton* m_addSongsBtn = nullptr;
    QPushButton* m_addFoldersBtn = nullptr;
    QPushButton* m_importM3UBtn = nullptr;
    QPushButton* m_exportM3UBtn = nullptr;
    QListWidget* m_foldersListWidget = nullptr;  

    bool m_tooltipsEnabled = true;
    bool m_crossfadeEnabled = false;
    bool m_normalizeEnabled = false;
    bool m_gaplessEnabled = true;
    bool m_cursorFollows = false;
    bool m_notificationsEnabled = true;
    bool m_trayEnabled = false;
    bool m_minimizeToTray = false;
    bool m_optimizedMode = false;
    int m_currentBackend = 0;
    QString m_currentDevice = "Default / Automatic";
    QString m_currentTheme = "Dark (Default)";
};

#endif // SETTINGSPAGE_H
