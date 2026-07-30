#ifndef KARAOKEDIALOG_H
#define KARAOKEDIALOG_H

#include <QDialog>
#include <QListWidget>
#include <QLabel>
#include <QPixmap>
#include <QPushButton>
#include <QVector>
#include "lrcparser.h"

class KaraokeDialog : public QDialog {
    Q_OBJECT
public:
    explicit KaraokeDialog(QWidget *parent = nullptr);
    ~KaraokeDialog() override = default;

    void setTrackInfo(const QString& title, const QString& artist, const QString& coverPath);
    void setLyrics(const QString& syncedLrc, const QString& unsyncedLyrics);
    void updateProgress(double elapsedSeconds);

signals:
    void seekRequested(double seconds);

protected:
    void paintEvent(QPaintEvent* event) override;
    void keyPressEvent(QKeyEvent* event) override;

private slots:
    void onLineItemClicked(QListWidgetItem* item);

private:
    QLabel* m_titleLabel;
    QLabel* m_artistLabel;
    QPushButton* m_closeButton;
    QListWidget* m_listWidget;
    QLabel* m_unsyncedLabel;

    QPixmap m_blurredCover;
    QVector<LrcLine> m_lines;
    int m_activeIndex;
    bool m_isSynced;
};

#endif // KARAOKEDIALOG_H
