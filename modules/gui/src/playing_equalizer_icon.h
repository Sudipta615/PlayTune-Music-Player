#ifndef PLAYINGEQUALIZERICON_H
#define PLAYINGEQUALIZERICON_H

#include <QWidget>
#include <QVector>
#include <QPaintEvent>
#include <QTimerEvent>

// The 3-bar moving equalizer visualizer next to the playing song title
class PlayingEqualizerIcon : public QWidget {
    Q_OBJECT
public:
    explicit PlayingEqualizerIcon(QWidget* parent = nullptr);
    ~PlayingEqualizerIcon() override = default;

    void setPlaying(bool playing);

protected:
    void paintEvent(QPaintEvent* event) override;
    void timerEvent(QTimerEvent* event) override;

private:
    bool m_isPlaying = true;
    int m_timerId = -1;
    QVector<float> m_heights = {0.3f, 0.7f, 0.5f};
    float m_targetHeights[3] = {0.3f, 0.7f, 0.5f};
};

#endif // PLAYINGEQUALIZERICON_H
