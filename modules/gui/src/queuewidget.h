#ifndef QUEUEWIDGET_H
#define QUEUEWIDGET_H

#include <QWidget>
#include <QTableWidget>
#include <QLabel>
#include <QPushButton>
#include <QSlider>
#include <QButtonGroup>
#include <QStackedWidget>
#include <QTextEdit>
#include <QTimer>

#include <QListWidget>
#include <QVector>
#include <QDropEvent>
#include "karaokedialog.h"
#include "lrcparser.h"

class DragDropQueueTableWidget : public QTableWidget {
    Q_OBJECT
public:
    explicit DragDropQueueTableWidget(QWidget* parent = nullptr) : QTableWidget(parent) {
        setDragEnabled(true);
        setAcceptDrops(true);
        if (viewport()) viewport()->setAcceptDrops(true);
        setDropIndicatorShown(true);
        setDragDropMode(QAbstractItemView::InternalMove);
        setSelectionMode(QAbstractItemView::SingleSelection);
        setSelectionBehavior(QAbstractItemView::SelectRows);
        setDefaultDropAction(Qt::MoveAction);
    }

signals:
    void rowMoved(int fromRow, int toRow);

protected:
    void dropEvent(QDropEvent* event) override {
        int fromRow = currentRow();
        QModelIndex targetIndex = indexAt(event->position().toPoint());
        int toRow = targetIndex.isValid() ? targetIndex.row() : rowCount() - 1;

        if (fromRow >= 0 && toRow >= 0 && fromRow != toRow) {
            emit rowMoved(fromRow, toRow);
        }
        event->accept();
    }
};

class QueueWidget : public QWidget {
    Q_OBJECT
public:
    explicit QueueWidget(QWidget* parent = nullptr);
    ~QueueWidget() override = default;

    // Setters to update from FFI
    void setTrackInfo(const QString& title, const QString& artist, const QString& album, const QString& coverPath);
    void clearQueue();
    void addQueueSong(int index, const QString& title, const QString& artist, const QString& duration, const QString& coverPath);
    void setVolume(double level);

    void setTrackLyrics(int trackId, const QString& syncedLrc, const QString& unsyncedLyrics);
    void updatePlaybackProgress(double elapsedSeconds);
    void reorderQueueRow(int fromRow, int toRow);
    /// Apply or remove Optimized Mode live (hides/shows thumbnail labels).
    void setOptimizedMode(bool enabled);

    int hoveredRow() const { return m_hoveredRow; }
    QTableWidget* queueTable() const { return m_queueTable; }

signals:
    void clearQueueClicked();
    void volumeChanged(double level);
    void queueSongSelected(int index);
    void seekRequested(double seconds);
    void toggleRightSidebarRequested();

protected:
    bool eventFilter(QObject* watched, QEvent* event) override;

private slots:
    void onLyricsLineClicked(QListWidgetItem* item);
    void onExpandKaraokeClicked();

private:
    void setupUi();
    QPixmap getThumbnail(const QString& title);

    QPushButton* m_toggleRightBtn = nullptr;
    QPushButton* m_queueTab = nullptr;
    QPushButton* m_lyricsTab = nullptr;
    QButtonGroup* m_tabGroup = nullptr;

    // Mini Now Playing details
    QLabel* m_miniCover = nullptr;
    QLabel* m_miniTitle = nullptr;
    QLabel* m_miniArtistAlbum = nullptr;
<<<<<<< HEAD
    QLabel* m_npHeaderLabel = nullptr;  // "Now Playing" section header
    QLabel* m_upNextLabel = nullptr;    // "Up Next" section header
=======
    QString m_miniCoverPath;      // last cover path shown on the mini card
>>>>>>> mulberry-calendula

    // Stacked widget for Queue vs Lyrics
    QStackedWidget* m_contentStack = nullptr;

    // Up Next list
    QTableWidget* m_queueTable = nullptr;
    QLabel* m_footerLabel = nullptr; // e.g. "8 songs * 33:44"
    int m_hoveredRow = -1;
    int m_totalQueueSeconds = 0;

    // Lyrics tab
    int m_currentTrackId = -1;
    bool m_isSyncedLyrics = false;
    QVector<LrcLine> m_lyricsLines;
    int m_activeLyricIndex = -1;

    QListWidget* m_lyricsListWidget = nullptr;
    QLabel* m_unsyncedLyricsLabel = nullptr;
    QPushButton* m_karaokeButton = nullptr;
    KaraokeDialog* m_karaokeDialog = nullptr;

    // Volume controls
    QPushButton* m_volumeIcon = nullptr;
    QSlider* m_volumeSlider = nullptr;
    QLabel* m_volumeLabel = nullptr;
    int m_lastVolumeBeforeMute = 80;
    QTimer* m_volumeThrottleTimer = nullptr;
    int m_lastEmittedVolume = -1;
};

#endif // QUEUEWIDGET_H
