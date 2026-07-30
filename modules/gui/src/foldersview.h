#ifndef FOLDERSVIEW_H
#define FOLDERSVIEW_H

#include <QWidget>
#include <QTableWidget>
#include <QPushButton>
#include <QStackedWidget>
#include <QLabel>
#include "songstable.h"

class FoldersViewWidget : public QWidget {
    Q_OBJECT
public:
    explicit FoldersViewWidget(QWidget* parent = nullptr);
    ~FoldersViewWidget() override = default;

    void clearFolders();
    void addFolderRow(int id, const QString& path, const QString& name, int trackCount);

    void showFolderList();
    void showFolderSongs(int folderId, const QString& folderName);

public slots:
    void clearSongs();
    void addSong(int index, int songId, bool isFavorite, const QString& title, const QString& artist, const QString& album, const QString& duration, const QString& coverPath);
    void setPlayingState(bool playing);
    void setActiveIndex(int index, bool playing);
    void setPlayingSongId(int songId, bool playing);
    void updateTrackRow(int songId, const QString& title, const QString& artist, const QString& album, const QString& duration, const QString& coverPath);

signals:
    void folderSelected(int folderId);
    void deleteFolderRequested(int folderId);
    void backToFoldersClicked();
    void songSelected(int index);

private:
    void setupUi();
    QStackedWidget* m_stackedWidget = nullptr;
    QTableWidget* m_table = nullptr;
    SongsTableWidget* m_folderSongsTable = nullptr;
    QLabel* m_songsTitleLabel = nullptr;
};

#endif // FOLDERSVIEW_H
