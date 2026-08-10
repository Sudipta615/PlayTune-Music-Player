#ifndef SETTINGSPAGE_H
#define SETTINGSPAGE_H

#include <QWidget>
#include <QComboBox>
#include <QPushButton>
#include <QListWidget>
#include <QSpinBox>
#include <QLabel>
#include <QFrame>
#include "custom_widgets.h"

class SettingsPageWidget : public QWidget {
    Q_OBJECT
public:
    explicit SettingsPageWidget(QWidget* parent = nullptr);
    ~SettingsPageWidget() override = default;

    bool isTooltipsEnabled() const;
    bool isMoodColumnEnabled() const { return m_moodColumnEnabled; }
    bool isOptimizedMode() const { return m_optimizedMode; }
    bool isGpuRenderingEnabled() const { return m_gpuRendering; }
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
    void moodColumnToggled(bool enabled);
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
    void gpuRenderingToggled(bool enabled);
    void addSongsRequested();
    void addFoldersRequested();
    void deleteFolderRequested(int folderId);
    void importM3URequested();
    void exportM3URequested();

public slots:
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
    void updateThemeStyles(const ThemePalette& p);

    // ── Controls ─────────────────────────────────────────────────────────
    QComboBox*    m_themeCombo           = nullptr;
    QComboBox*    m_backendCombo         = nullptr;
    QComboBox*    m_deviceCombo          = nullptr;
    ToggleSwitch* m_tooltipToggle        = nullptr;
    ToggleSwitch* m_moodColumnToggle     = nullptr;
    ToggleSwitch* m_crossfadeToggle      = nullptr;
    ToggleSwitch* m_normalizeToggle      = nullptr;
    ToggleSwitch* m_gaplessToggle        = nullptr;
    ToggleSwitch* m_cursorFollowToggle   = nullptr;
    ToggleSwitch* m_notificationsToggle  = nullptr;
    ToggleSwitch* m_trayToggle           = nullptr;
    ToggleSwitch* m_minimizeToTrayToggle = nullptr;
    ToggleSwitch* m_optimizedModeToggle  = nullptr;
    ToggleSwitch* m_gpuRenderingToggle   = nullptr;
    QPushButton*  m_loudnessScanBtn      = nullptr;
    QSpinBox*     m_crossfadeDurationSpin= nullptr;
    QPushButton*  m_addSongsBtn          = nullptr;
    QPushButton*  m_addFoldersBtn        = nullptr;
    QPushButton*  m_importM3UBtn         = nullptr;
    QPushButton*  m_exportM3UBtn         = nullptr;
    QListWidget*  m_foldersListWidget    = nullptr;

    // ── Cached label/frame references for O(1) theme updates ─────────────
    QLabel*  m_pageTitle        = nullptr;
    QLabel*  m_pageSub          = nullptr;
    // Section header labels (APPEARANCE, ADD MUSIC, etc.)
    QList<QLabel*> m_sectionHeaders;
    // Setting row title labels (bold text on left of each row)
    QList<QLabel*> m_settingTitleLabels;
    // Setting row subtitle labels (small muted text)
    QList<QLabel*> m_settingSubLabels;
    // Horizontal separator frames inside cards
    QList<QFrame*> m_cardSeparators;
    // All SettingsCard frames
    QList<QFrame*> m_settingsCards;
    // Performance card (has accent border — stored separately)
    QFrame* m_performanceCard   = nullptr;
    // The info box label inside performance card
    QLabel* m_perfInfoLabel     = nullptr;

    // ── Settings state ────────────────────────────────────────────────────
    bool    m_tooltipsEnabled   = true;
    bool    m_moodColumnEnabled = true;
    bool    m_crossfadeEnabled  = false;
    bool    m_normalizeEnabled  = false;
    bool    m_gaplessEnabled    = true;
    bool    m_cursorFollows     = false;
    bool    m_notificationsEnabled = true;
    bool    m_trayEnabled       = false;
    bool    m_minimizeToTray    = false;
    bool    m_optimizedMode     = false;
    bool    m_gpuRendering      = false;
    int     m_currentBackend    = 0;
    QString m_currentDevice     = "Default / Automatic";
    QString m_currentTheme      = "Dark Premium (Purple)";
};

#endif // SETTINGSPAGE_H
