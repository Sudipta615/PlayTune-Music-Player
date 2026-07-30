#ifndef CUSTOM_WIDGETS_H
#define CUSTOM_WIDGETS_H

#include <QWidget>
#include <QAbstractButton>
#include <QVariantAnimation>
#include <QVector>
#include <QPointF>

// Animated modern Toggle Switch (replaces standard checkbox)
class ToggleSwitch : public QWidget {
    Q_OBJECT
    Q_PROPERTY(qreal offset READ offset WRITE setOffset)
public:
    explicit ToggleSwitch(QWidget* parent = nullptr);
    ~ToggleSwitch() override = default;

    bool isChecked() const { return m_checked; }
    void setChecked(bool checked);

    QSize sizeHint() const override;

    qreal offset() const { return m_offset; }
    void setOffset(qreal o) { m_offset = o; update(); }

signals:
    void toggled(bool checked);

protected:
    void paintEvent(QPaintEvent* event) override;
    void mousePressEvent(QMouseEvent* event) override;

private:
    bool m_checked = false;
    qreal m_offset = 0.0;
    QVariantAnimation* m_anim = nullptr;
};

// Custom interactive 10-band spline-based Equalizer chart
class EqualizerCurveWidget : public QWidget {
    Q_OBJECT
public:
    explicit EqualizerCurveWidget(QWidget* parent = nullptr);
    ~EqualizerCurveWidget() override = default;

    // Get current gains in dB
    QVector<double> getGains() const { return m_gains; }

    // Set value of a specific band (-12.0 to +12.0)
    void setBandValue(int bandIdx, double db);
    void setGains(const QVector<double>& gains);

signals:
    void bandChanged(int bandIdx, double db);

protected:
    void paintEvent(QPaintEvent* event) override;
    void mousePressEvent(QMouseEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;
    void resizeEvent(QResizeEvent* event) override;

private:
    int getBandIndexAt(const QPoint& pos) const;
    double yToDb(double y) const;
    double dbToY(double db) const;

    QVector<double> m_gains; // 10 values, -12.0 to 12.0
    int m_activeBand = -1;   // currently dragged band

    // Frequency labels matching mockup
    const QVector<QString> m_frequencies = {
        "32", "64", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"
    };
};

// Waveform Seekbar Visualizer
class WaveformVisualizer : public QWidget {
    Q_OBJECT
public:
    explicit WaveformVisualizer(QWidget* parent = nullptr);
    ~WaveformVisualizer() override = default;

    void setPlaybackProgress(double progress); // 0.0 to 1.0
    void updateBuffer(const QVector<float>& buffer);
    void setPlaying(bool playing);

protected:
    void paintEvent(QPaintEvent* event) override;
    void timerEvent(QTimerEvent* event) override;
    void hideEvent(QHideEvent* event) override;
    void showEvent(QShowEvent* event) override;

private:
    void generateDefaultWaveform();

    QVector<float> m_heights;      // height of each bar (0.0 to 1.0)
    QVector<float> m_levels;       // animated current level of each bar
    double m_progress = 0.0;       // playback position (0.0 to 1.0)
    bool m_isPlaying = false;
    int m_timerId = -1;
    bool m_timerWasActive = false;
    double m_animationPhase = 0.0;
};

#include <QPixmap>
#include <QPainter>
#include <QLinearGradient>

inline QPixmap getDefaultAlbumArt() {
    static QPixmap cachedCover;
    if (!cachedCover.isNull()) {
        return cachedCover;
    }
    const int size = 300;
    cachedCover = QPixmap(size, size);
    cachedCover.fill(Qt::transparent);

    QPainter painter(&cachedCover);
    painter.setRenderHint(QPainter::Antialiasing);
    painter.setRenderHint(QPainter::SmoothPixmapTransform);

    // Sleek, professional dark gradient tile background
    QLinearGradient bgGrad(0, 0, size, size);
    bgGrad.setColorAt(0.0, QColor("#1F2338"));
    bgGrad.setColorAt(1.0, QColor("#141624"));
    painter.fillRect(0, 0, size, size, bgGrad);

    // Subtle inner border / highlight
    painter.setPen(QPen(QColor("#2E3350"), 2));
    painter.setBrush(Qt::NoBrush);
    painter.drawRect(1, 1, size - 2, size - 2);

    // Center the app logo (`:/resources/icons/playtune_logo.png`)
    QPixmap logo(":/resources/icons/playtune_logo.png");
    if (!logo.isNull()) {
        int logoSize = size * 0.55; // ~165px
        QPixmap scaledLogo = logo.scaled(logoSize, logoSize, Qt::KeepAspectRatio, Qt::SmoothTransformation);
        int x = (size - scaledLogo.width()) / 2;
        int y = (size - scaledLogo.height()) / 2;
        painter.drawPixmap(x, y, scaledLogo);
    }
    return cachedCover;
}

#endif // CUSTOM_WIDGETS_H
