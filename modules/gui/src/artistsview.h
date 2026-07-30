#ifndef ARTISTSVIEW_H
#define ARTISTSVIEW_H

#include <QWidget>
#include <QStackedWidget>
#include <QListWidget>
#include <QTableWidget>
#include <QLabel>
#include <QPushButton>
#include <QVector>

#include "mediagridview.h"

class SongsTableWidget;

/// Three-tier artist browser:
///   page 0 = alphabetical artist grid (covers + name + counts)
///   page 1 = filtered songs table for the selected artist
///
/// Clicking an artist in the grid emits `artistSelected(artist_id)` and
/// the backend responds by pushing that artist's albums / songs to the
/// inner songs table. Clicking "< Back" returns to the grid.
class ArtistsViewWidget : public QWidget {
    Q_OBJECT
public:
    explicit ArtistsViewWidget(QWidget* parent = nullptr);

    void clearArtists();
    void addArtistRow(int artist_id, const QString& name, int album_count, int track_count, const QString& cover_path = QString());
    void clearAlbumsInArtist() {}
    void addAlbumInArtist(int, const QString&, const QString&, int, double) {}

    void showArtistGrid();
    void showArtistSongs(int artist_id, const QString& artist_name);

    // Forwarded slots for the inner songs table.
    void clearSongs();
    void addSong(int index, int song_id, bool is_favorite, const QString& title,
                 const QString& artist, const QString& album, const QString& duration,
                 const QString& cover_path);
    void setPlayingState(bool playing);
    void setActiveIndex(int index, bool playing);
    void setPlayingSongId(int songId, bool playing);

    void updateGridResponsive();

signals:
    void artistSelected(int artist_id);
    void backToArtistsClicked();
    void songSelected(int index);

protected:
    void resizeEvent(QResizeEvent* event) override;
    void showEvent(QShowEvent* event) override;

private:
    void setupUi();

    QStackedWidget* m_stackedWidget = nullptr;
    MediaGridWidget* m_artistGrid = nullptr;
    SongsTableWidget* m_artistSongsTable = nullptr;
    QLabel* m_songsTitleLabel = nullptr;
    QPushButton* m_backBtn = nullptr;
    int m_currentArtistId = -1;
};

#endif // ARTISTSVIEW_H
