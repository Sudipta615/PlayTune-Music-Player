#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <QLineEdit>
#include <QStackedWidget>
#include <QKeyEvent>
#include <QTimer>
#include <QSystemTrayIcon>
#include <QMenu>
#include "sidebar.h"
#include "nowplayingcard.h"
#include "songstable.h"
#include "queuewidget.h"
#include "equalizerwindow.h"
#include "settingspage.h"
#include "foldersview.h"
#include "albumsview.h"
#include "artistsview.h"

#include <QFileInfo>

class ToolTipController;

class MainWindow : public QMainWindow {
    Q_OBJECT
public:
    explicit MainWindow(QWidget* parent = nullptr);
    ~MainWindow() override = default;

protected:
    void keyPressEvent(QKeyEvent* event) override;
    void resizeEvent(QResizeEvent* event) override;
    void changeEvent(QEvent* event) override;
    void closeEvent(QCloseEvent* event) override;

private slots:
    void onTrackMetadataUpdated(int track_id, const QString& title, const QString& artist, const QString& album, const QString& duration_str, const QString& cover_path);

private:
    void setupUi();
    void connectBridge();
    void loadStyleSheet();
    void setupKeyboardShortcuts();
    void updateSidebarDimensions();
    void showToast(const QString& message);  // Issue 10: in-app toast notification
    void updateLayoutForCurrentTab(int index);
    void setupSystemTray();
    void onPlaylistCreateRequested();
    void onPlaylistRenameRequested(int playlist_id, const QString& current_name);
    void onPlaylistDeleteRequested(int playlist_id);
    void onSleepTimerRequested();
    void onImportM3URequested();
    void onExportM3URequested();
    void forceAppIcon();

    /// Returns the SongsTableWidget that is currently visible to the user
    /// (Home tab → m_songsTable; Folders tab page 1 → m_foldersView's
    /// inner table; Albums tab page 1 → m_albumsView's inner table;
    /// Artists tab page 1 → m_artistsView's inner table). Falls back to
    /// m_songsTable when no inner table is visible (e.g. on Settings).
    ///
    /// Used by the songsCleared / songAdded / songsBatchReplaced signal
    /// dispatchers so songs events are routed to ONE view instead of
    /// four — which was the single biggest source of redundant widget
    /// creation and cover-load work on the GUI thread (4× cost for
    /// every refresh_ui call).
    SongsTableWidget* activeSongsTable() const;

    // Components
    SidebarWidget* m_sidebar = nullptr;
    QLineEdit* m_searchBar = nullptr;
    QAction* m_searchAction = nullptr;
    QPushButton* m_toggleRightTopBtn = nullptr;
    NowPlayingCard* m_nowPlayingCard = nullptr;
    QStackedWidget* m_contentStack = nullptr;
    SongsTableWidget* m_songsTable = nullptr;
    SettingsPageWidget* m_settingsPage = nullptr;
    FoldersViewWidget* m_foldersView = nullptr;
    AlbumsViewWidget* m_albumsView = nullptr;
    ArtistsViewWidget* m_artistsView = nullptr;
    QueueWidget* m_queueWidget = nullptr;

    // Equalizer popup panel
    EqualizerWindow* m_eqWindow = nullptr;

    // System tray
    QSystemTrayIcon* m_trayIcon = nullptr;
    QMenu* m_trayMenu = nullptr;
    QAction* m_trayPlayPauseAction = nullptr;
    QAction* m_trayNextAction = nullptr;
    QAction* m_trayPrevAction = nullptr;
    QAction* m_trayShowHideAction = nullptr;
    QAction* m_trayQuitAction = nullptr;
    bool m_trayEnabled = false;
    bool m_minimizeToTray = false;
    bool m_trayBalloonShown = false;

    QTimer* m_searchTimer = nullptr;
    ToolTipController* m_toolTipController = nullptr;

    // Keyboard shortcut state
    double m_currentVolume = 0.75;  // 0.0 – 1.0, mirrors the volume slider
    bool   m_isMuted       = false;
    double m_volumeBeforeMute = 0.75;

    // Responsive / layout helper components & flags
    QFrame*      m_sep1 = nullptr;
    QFrame*      m_sep2 = nullptr;
    bool         m_inResizeEvent = false;
    bool         m_sidebarCollapsedByUser = false;
    bool         m_queueHiddenByUser = false;
    bool         m_wasMaximizedBeforeMinimize = false;
    bool         m_wasFullScreenBeforeMinimize = false;
};

#endif // MAINWINDOW_H
