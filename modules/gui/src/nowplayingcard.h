#ifndef NOWPLAYINGCARD_H
#define NOWPLAYINGCARD_H

#include <QFrame>
#include <QLabel>
#include <QPushButton>
#include <QSlider>
#include "custom_widgets.h"

#include <QVariantAnimation>
#include <QColor>
#include <QPointer>

class NowPlayingCard : public QFrame {
    Q_OBJECT
public:
    explicit NowPlayingCard(QWidget* parent = nullptr);
    ~NowPlayingCard() override = default;

    // Setters to update from FFI
    void setTrackInfo(const QString& title, const QString& artist, const QString& album, const QString& coverPath);
    void setPlayState(bool playing);
    void setPlaybackProgress(double elapsed, double total);
    void updateVisualizer(const QVector<float>& data);
    /// Update the playback speed label (e.g. "1.00x"). Pass 1.0 to hide.
    void setSpeedLabel(double speed);
    /// Update the sleep timer countdown display. Pass 0 to hide.
    void setSleepTimerRemaining(int seconds_remaining);
    /// Apply or remove Optimized Mode optimizations (live, no restart needed).
    void setOptimizedMode(bool enabled);

    // Getter for keyboard seek
    double elapsedSeconds() const { return m_elapsedSeconds; }
    double totalSeconds() const { return m_totalDuration; }
    bool isPlaying() const { return m_isPlaying; }

signals:
    void playPauseClicked();
    void prevClicked();
    void nextClicked();
    void seekRequested(double seconds);
    void eqClicked();
    void editTagsClicked();
    void repeatClicked(bool checked = false);
    void shuffleClicked(bool checked = false);
    /// Emitted when the user drags the speed slider. 1.0 = normal speed.
    void speedChanged(double speed);
    /// Emitted when the user clicks the sleep timer button.
    void sleepTimerClicked();

protected:
    void resizeEvent(QResizeEvent* event) override;
    bool eventFilter(QObject* watched, QEvent* event) override;

private:
    void setupUi();
    QString formatTime(double seconds);
    void updateCoverPixmap();
    void applyCardStyle(const QColor& c1, const QColor& c2, const QColor& border);
    void animateToColors(const QColor& targetC1, const QColor& targetC2, const QColor& targetBorder);
    void applyLabelStyles(const ThemePalette& p);

    QLabel* m_coverLabel = nullptr;
    QLabel* m_nowPlayingLabel = nullptr;
    QLabel* m_titleLabel = nullptr;
    QLabel* m_artistLabel = nullptr;
    QLabel* m_albumLabel = nullptr;
    QPushButton* m_editTagsBtn = nullptr;

    WaveformVisualizer* m_visualizer = nullptr;
    ClickableSlider* m_seekBar = nullptr;
    QLabel* m_timeElapsed = nullptr;
    QLabel* m_timeTotal = nullptr;

    QPushButton* m_playPauseBtn = nullptr;
    QPushButton* m_prevBtn = nullptr;
    QPushButton* m_nextBtn = nullptr;
    QPushButton* m_repeatBtn = nullptr;
    QPushButton* m_shuffleBtn = nullptr;
    QPushButton* m_eqBtn = nullptr;
    QPushButton* m_sleepTimerBtn = nullptr;
    QLabel* m_sleepTimerLabel = nullptr;
    int m_sleepTimerRemaining = 0;

    double m_totalDuration = 0.0;
    double m_elapsedSeconds = 0.0;
    bool m_isSliderPressed = false;
    bool m_isPlaying = false;

    // Smooth backdrop gradient animation
    QPointer<QVariantAnimation> m_colorAnimation;
    QColor m_currentC1 = QColor("#151624");
    QColor m_currentC2 = QColor("#0F111D");
    QColor m_currentBorderColor = QColor("#23283E");

    // Responsive sizing members
    bool         m_hasCustomCover = false;
    QPixmap      m_coverPixmap;
    int          m_coverSize = 140;
    bool         m_optimizedMode = false;
    class QVBoxLayout* m_rightLayout = nullptr;
    class QHBoxLayout* m_controlsLayout = nullptr;
};

#endif // NOWPLAYINGCARD_H
