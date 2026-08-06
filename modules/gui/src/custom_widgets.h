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

#include <QSlider>
#include <QMouseEvent>
#include <QStyleOptionSlider>
#include <QStyle>
#include <algorithm>

// Modern QSlider subclass that jumps directly to clicked position on Left-Click
class ClickableSlider : public QSlider {
    Q_OBJECT
public:
    explicit ClickableSlider(Qt::Orientation orientation, QWidget* parent = nullptr)
        : QSlider(orientation, parent) {}
    explicit ClickableSlider(QWidget* parent = nullptr)
        : QSlider(parent) {}
    ~ClickableSlider() override = default;

protected:
    void mousePressEvent(QMouseEvent* event) override {
        if (event->button() == Qt::LeftButton) {
            int val = pixelPosToRangeValue(event->pos());
            setValue(val);
            emit sliderMoved(val);
            QSlider::mousePressEvent(event);
            setValue(val);
            emit sliderMoved(val);
        } else {
            QSlider::mousePressEvent(event);
        }
    }

private:
    int pixelPosToRangeValue(const QPoint& pos) const {
        QStyleOptionSlider opt;
        initStyleOption(&opt);
        QRect handleRect = style()->subControlRect(QStyle::CC_Slider, &opt, QStyle::SC_SliderHandle, this);
        QRect grooveRect = style()->subControlRect(QStyle::CC_Slider, &opt, QStyle::SC_SliderGroove, this);

        int val = minimum();
        if (orientation() == Qt::Horizontal) {
            int sliderMin = grooveRect.x() + handleRect.width() / 2;
            int sliderMax = grooveRect.x() + grooveRect.width() - handleRect.width() / 2;
            int usableWidth = sliderMax - sliderMin;
            if (usableWidth > 0) {
                double ratio = qBound(0.0, static_cast<double>(pos.x() - sliderMin) / usableWidth, 1.0);
                if (opt.upsideDown) {
                    ratio = 1.0 - ratio;
                }
                val = minimum() + qRound(ratio * (maximum() - minimum()));
            } else {
                double ratio = qBound(0.0, static_cast<double>(pos.x()) / qMax(1, width()), 1.0);
                val = minimum() + qRound(ratio * (maximum() - minimum()));
            }
        } else {
            int sliderMin = grooveRect.y() + handleRect.height() / 2;
            int sliderMax = grooveRect.y() + grooveRect.height() - handleRect.height() / 2;
            int usableHeight = sliderMax - sliderMin;
            if (usableHeight > 0) {
                double ratio = qBound(0.0, static_cast<double>(sliderMax - pos.y()) / usableHeight, 1.0);
                if (opt.upsideDown) {
                    ratio = 1.0 - ratio;
                }
                val = minimum() + qRound(ratio * (maximum() - minimum()));
            } else {
                double ratio = qBound(0.0, static_cast<double>(height() - pos.y()) / qMax(1, height()), 1.0);
                val = minimum() + qRound(ratio * (maximum() - minimum()));
            }
        }
        return val;
    }
};

#include <QOpenGLWidget>
#include <QOpenGLFunctions>

// Waveform Seekbar Visualizer (GPU-Accelerated with CPU QPainter Fallback)
class WaveformVisualizer : public QOpenGLWidget, protected QOpenGLFunctions {
    Q_OBJECT
public:
    explicit WaveformVisualizer(QWidget* parent = nullptr);
    ~WaveformVisualizer() override = default;

    void setPlaybackProgress(double progress); // 0.0 to 1.0
    void updateBuffer(const QVector<float>& buffer);
    void setPlaying(bool playing);
    void setGpuAccelerationEnabled(bool enabled);

    bool isGpuAccelerated() const { return m_gpuAccelerated && m_gpuUserSettingEnabled; }

signals:
    void seekRequested(double ratio); // 0.0 to 1.0

protected:
    void initializeGL() override;
    void resizeGL(int w, int h) override;
    void paintGL() override;
    void paintEvent(QPaintEvent* event) override;
    void timerEvent(QTimerEvent* event) override;
    void hideEvent(QHideEvent* event) override;
    void showEvent(QShowEvent* event) override;
    void mousePressEvent(QMouseEvent* event) override;

private:
    void generateDefaultWaveform();
    void renderWaveform(QPainter* painter);

    bool m_gpuInitialized = false;
    bool m_gpuAccelerated = false;
    bool m_gpuUserSettingEnabled = false;
    QVector<float> m_heights;      // height of each bar (0.0 to 1.0)
    QVector<float> m_levels;       // animated current level of each bar
    double m_progress = 0.0;       // playback position (0.0 to 1.0)
    bool m_isPlaying = false;
    int m_timerId = -1;
    bool m_timerWasActive = false;
    double m_animationPhase = 0.0;
};

#include "apptheme.h"

inline QPixmap getDefaultAlbumArt() {
    return ThemeManager::instance().defaultAlbumArt();
}

#endif // CUSTOM_WIDGETS_H
