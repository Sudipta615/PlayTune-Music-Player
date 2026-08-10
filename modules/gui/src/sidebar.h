#ifndef SIDEBAR_H
#define SIDEBAR_H

#include <QWidget>
#include <QButtonGroup>
#include <QAbstractButton>
#include <QPushButton>
#include <QLabel>
#include <QListWidget>
#include <QList>
#include <QStringList>

class SidebarWidget : public QWidget {
    Q_OBJECT
public:
    explicit SidebarWidget(QWidget* parent = nullptr);
    ~SidebarWidget() override = default;

    void setActiveNav(int id) {
        if (m_navGroup) {
            QAbstractButton* btn = m_navGroup->button(id);
            if (btn) {
                btn->setChecked(true);
            } else {
                qWarning("SidebarWidget::setActiveNav: no button with id %d", id);
            }
        }
    }

    void setCollapsed(bool collapsed);
    bool isCollapsed() const { return m_isCollapsed; }
    void setSidebarWidths(int expandedWidth, int collapsedWidth);

    /// Clear and rebuild the dynamic playlist list.
    void clearPlaylists();
    /// Add a row to the playlist list. `playlist_id` is the DB row id.
    void addPlaylistRow(int playlist_id, const QString& name, int track_count, double duration_secs);

signals:
    void homeClicked();
    void albumsClicked();
    void artistsClicked();
    void foldersClicked();
    void settingsClicked();
    void addPlaylistClicked();
    void favoritesClicked();
    void recentlyPlayedClicked();
    void mostPlayedClicked();
    void collapsedToggled(bool collapsed);
    /// Emitted when the user clicks a user-defined playlist row.
    void playlistSelected(int playlist_id);
    /// Emitted when the user right-clicks a playlist and picks Rename.
    void playlistRenameRequested(int playlist_id, const QString& current_name);
    /// Emitted when the user right-clicks a playlist and picks Delete.
    void playlistDeleteRequested(int playlist_id);

protected:
    bool eventFilter(QObject* watched, QEvent* event) override;

private:
    void setupUi();
    void applyNavButtonStyles();
    QButtonGroup* m_navGroup = nullptr;
    QWidget* m_logoContainer = nullptr;
    QLabel* m_logoIcon = nullptr;
    QLabel* m_logoText = nullptr;
    QLabel* m_sectionLabel = nullptr;
    QPushButton* m_addPlaylistBtn = nullptr;
    QList<QPushButton*> m_allNavButtons;
    QStringList m_allNavTexts;
    bool m_isCollapsed = false;
    int m_expandedWidth = 200;
    int m_collapsedWidth = 64;
    QListWidget* m_playlistList = nullptr;
};

#endif // SIDEBAR_H
