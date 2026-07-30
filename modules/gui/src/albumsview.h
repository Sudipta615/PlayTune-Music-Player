#ifndef ALBUMSVIEW_H
#define ALBUMSVIEW_H

#include <QWidget>
#include <QStackedWidget>
#include <QTableWidget>
#include <QLabel>
#include <QPushButton>
#include <QListWidget>
#include <QVector>

#include "mediagridview.h"

class SongsTableWidget;

/// Two-page stacked widget mirroring the FoldersView pattern:
///   page 0 = grid of album covers (with title/artist/track count)
///   page 1 = filtered songs table for the selected album
///
/// Clicking an album in page 0 emits `albumSelected(album_id)` so the
/// bridge can filter the songs table; clicking "< Back" returns to the
/// grid. The same SongsTableWidget instance is reused for the inner
/// songs list so all keyboard / context-menu behaviour is identical to
/// the main Songs tab.
class AlbumsViewWidget : public QWidget {
    Q_OBJECT
public:
    explicit AlbumsViewWidget(QWidget* parent = nullptr);

    /// Clear and rebuild the album grid.
    void clearAlbums();
    /// Add one album row to the grid. `album_id` is a stable track id
    /// used by the backend to look up the album name.
    void addAlbumRow(int album_id, const QString& name, const QString& artist,
                     int track_count, double duration_secs, int year,
                     const QString& cover_path = QString());

    /// Switch back to the grid page.
    void showAlbumGrid();
    /// Switch to the songs page and update the title.
    void showAlbumSongs(int album_id, const QString& album_name);

    // Forwarded slots so the bridge can populate the inner songs table.
    void clearSongs();
    void addSong(int index, int song_id, bool is_favorite, const QString& title,
                 const QString& artist, const QString& album, const QString& duration,
                 const QString& cover_path);
    void setPlayingState(bool playing);
    void setActiveIndex(int index, bool playing);
    void setPlayingSongId(int songId, bool playing);

    void updateGridResponsive();

signals:
    /// Emitted when the user clicks an album cover. The backend should
    /// respond by refreshing the songs table for that album.
    void albumSelected(int album_id);
    void backToAlbumsClicked();
    void songSelected(int index);
    void deleteFolderRequested(int folder_id);  // unused, kept for compat

protected:
    void resizeEvent(QResizeEvent* event) override;
    void showEvent(QShowEvent* event) override;

private:
    void setupUi();

    QStackedWidget* m_stackedWidget = nullptr;
    MediaGridWidget* m_albumGrid = nullptr;
    SongsTableWidget* m_albumSongsTable = nullptr;
    QLabel* m_songsTitleLabel = nullptr;
    QPushButton* m_backBtn = nullptr;
};

#endif // ALBUMSVIEW_H
