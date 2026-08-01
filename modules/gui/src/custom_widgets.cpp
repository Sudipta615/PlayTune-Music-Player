#include "custom_widgets.h"
#include <QPainter>
#include <QPainterPath>
#include <QMouseEvent>
#include <QVariantAnimation>
#include <cmath>
#include <QDebug>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

// ==========================================
// ToggleSwitch Implementation
// ==========================================
ToggleSwitch::ToggleSwitch(QWidget* parent) : QWidget(parent) {
    setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);

    m_anim = new QVariantAnimation(this);
    m_anim->setDuration(150);

    connect(m_anim, &QVariantAnimation::valueChanged, this, [this](const QVariant& value) {
        setOffset(value.toReal());
    });
}

QSize ToggleSwitch::sizeHint() const {
    return {38, 20};
}

void ToggleSwitch::setChecked(bool checked) {
    if (m_checked == checked) return;
    m_checked = checked;

    if (isVisible()) {
        m_anim->stop();
        m_anim->setStartValue(m_offset);
        m_anim->setEndValue(m_checked ? 1.0 : 0.0);
        m_anim->start();
    } else {
        m_offset = m_checked ? 1.0 : 0.0;
        update();
    }

    emit toggled(m_checked);
}

void ToggleSwitch::mousePressEvent(QMouseEvent* event) {
    if (event->button() == Qt::LeftButton) {
        setChecked(!m_checked);
        event->accept();
    } else {
        QWidget::mousePressEvent(event);
    }
}

void ToggleSwitch::paintEvent(QPaintEvent* event) {
    Q_UNUSED(event);
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);

    QRectF r = rect();
    qreal radius = r.height() / 2.0;

    // Draw track
    QPainterPath trackPath;
    trackPath.addRoundedRect(r, radius, radius);

    QLinearGradient trackGradient(r.topLeft(), r.topRight());
    if (m_checked || m_anim->state() == QAbstractAnimation::Running) {
        // Linear gradient pink to purple matching the design
        trackGradient.setColorAt(0.0, QColor("#FF2A7A"));
        trackGradient.setColorAt(1.0, QColor("#7B1FA2"));
        painter.setBrush(trackGradient);
    } else {
        painter.setBrush(QColor("#161922"));
    }
    painter.setPen(Qt::NoPen);
    painter.drawPath(trackPath);

    // Draw knob
    qreal knobDiameter = r.height() - 4.0;
    qreal minX = 2.0;
    qreal maxX = r.width() - knobDiameter - 2.0;
    qreal knobX = minX + m_offset * (maxX - minX);

    QRectF knobRect(knobX, 2.0, knobDiameter, knobDiameter);
    painter.setBrush(Qt::white);
    painter.drawEllipse(knobRect);
}



// ==========================================
// EqualizerCurveWidget Implementation
// ==========================================
EqualizerCurveWidget::EqualizerCurveWidget(QWidget* parent) : QWidget(parent) {
    m_gains.fill(0.0, 10); // 10 bands default to 0dB
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    setMouseTracking(true); // Track mouse hover to show pointer hand
    setToolTip("Click and drag handles to adjust frequency band gain (-12 dB to +12 dB)");
}

void EqualizerCurveWidget::setBandValue(int bandIdx, double db) {
    if (bandIdx >= 0 && bandIdx < m_gains.size()) {
        m_gains[bandIdx] = qBound(-12.0, db, 12.0);
        update();
    }
}

void EqualizerCurveWidget::setGains(const QVector<double>& gains) {
    if (gains.size() != m_gains.size()) {
        qWarning() << "EqualizerCurveWidget::setGains: expected"
                   << m_gains.size() << "gains, got" << gains.size() << "— ignored";
        return;
    }
    m_gains = gains;
    update();
}

double EqualizerCurveWidget::yToDb(double y) const {
    double top = 30.0;
    double bottom = height() - 40.0;
    double range = bottom - top;
    if (range <= 0) return 0.0;

    double ratio = (y - top) / range; // 0.0 at top (+12dB), 1.0 at bottom (-12dB)
    return 12.0 - ratio * 24.0;
}

double EqualizerCurveWidget::dbToY(double db) const {
    double top = 30.0;
    double bottom = height() - 40.0;
    double range = bottom - top;

    double ratio = (12.0 - db) / 24.0; // 0.0 at 12dB, 1.0 at -12dB
    return top + ratio * range;
}

int EqualizerCurveWidget::getBandIndexAt(const QPoint& pos) const {
    double leftMargin = 50.0;
    double chartWidth = width() - leftMargin - 30.0;
    double spacing = chartWidth / 11.0;

    int bestIdx = -1;
    double bestDist = 25.0; // click threshold in pixels

    for (int i = 0; i < 10; ++i) {
        double cx = leftMargin + (i + 1) * spacing;
        double cy = dbToY(m_gains[i]);
        double dist = std::hypot(pos.x() - cx, pos.y() - cy);
        if (dist < bestDist) {
            bestDist = dist;
            bestIdx = i;
        }
    }
    return bestIdx;
}

void EqualizerCurveWidget::mousePressEvent(QMouseEvent* event) {
    if (!isEnabled()) return;
    m_activeBand = getBandIndexAt(event->pos());
    if (m_activeBand != -1) {
        double newDb = yToDb(event->pos().y());
        setBandValue(m_activeBand, newDb);
        emit bandChanged(m_activeBand, m_gains[m_activeBand]);
    }
}

void EqualizerCurveWidget::mouseMoveEvent(QMouseEvent* event) {
    if (!isEnabled()) return;
    if (m_activeBand != -1) {
        double newDb = yToDb(event->pos().y());
        setBandValue(m_activeBand, newDb);
        emit bandChanged(m_activeBand, m_gains[m_activeBand]);
    } else {
        // Show pointer hand when hovering over an active node
        int bandIdx = getBandIndexAt(event->pos());
        if (bandIdx != -1) {
            setCursor(Qt::PointingHandCursor);
        } else {
            unsetCursor();
        }
    }
}

void EqualizerCurveWidget::mouseReleaseEvent(QMouseEvent* event) {
    Q_UNUSED(event);
    m_activeBand = -1;
}

void EqualizerCurveWidget::resizeEvent(QResizeEvent* event) {
    QWidget::resizeEvent(event);
}

void EqualizerCurveWidget::paintEvent(QPaintEvent* event) {
    Q_UNUSED(event);
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);

    bool active = isEnabled();

    double leftMargin = 50.0;
    double rightMargin = 30.0;
    double topMargin = 30.0;
    double bottomMargin = 40.0;

    double chartWidth = width() - leftMargin - rightMargin;
    double chartHeight = height() - topMargin - bottomMargin;
    double spacing = chartWidth / 11.0;

    // Draw horizontal grid lines (+12dB, +6dB, 0dB, -6dB, -12dB)
    const QVector<int> dbLevels = {12, 6, 0, -6, -12};
    QPen gridPen(QColor("#1A1D26"), 1, Qt::DashLine);
    QPen textPen(active ? QColor("#7E8494") : QColor("#4C5264"));

    for (int db : dbLevels) {
        double y = dbToY(db);
        painter.setPen(gridPen);
        painter.drawLine(QPointF(leftMargin, y), QPointF(width() - rightMargin, y));

        painter.setPen(textPen);
        QString label = QString("%1%2dB").arg(db > 0 ? "+" : "").arg(db);
        painter.drawText(QRectF(10, y - 10, leftMargin - 15, 20), Qt::AlignRight | Qt::AlignVCenter, label);
    }

    // Draw visual hint text near top right
    painter.setPen(QColor("#4C5264"));
    painter.drawText(QRectF(width() - 170, 8, 140, 20), Qt::AlignRight | Qt::AlignVCenter, active ? "Drag nodes to adjust" : "Equalizer Off");

    // Prepare node coordinates
    QVector<QPointF> points;
    points.reserve(10);
    for (int i = 0; i < 10; ++i) {
        double x = leftMargin + (i + 1) * spacing;
        double y = dbToY(m_gains[i]);
        points.append(QPointF(x, y));
    }

    double yBottom = dbToY(-12.0);

    // Draw vertical frequency tracks & gradient fills
    for (int i = 0; i < 10; ++i) {
        double cx = points[i].x();
        double cy = points[i].y();

        // Background slider line track
        painter.setPen(QPen(QColor("#1B1E28"), 2));
        painter.drawLine(QPointF(cx, topMargin), QPointF(cx, yBottom));

        // Draw filled gradient bar below knob
        if (cy < yBottom) {
            QLinearGradient barGradient(QPointF(cx, cy), QPointF(cx, yBottom));
            if (active) {
                barGradient.setColorAt(0.0, QColor("#FF2A7A"));
                barGradient.setColorAt(1.0, QColor("#7B1FA2"));
            } else {
                barGradient.setColorAt(0.0, QColor("#4C5264"));
                barGradient.setColorAt(1.0, QColor("#252833"));
            }

            painter.setPen(QPen(barGradient, 3));
            painter.drawLine(QPointF(cx, cy), QPointF(cx, yBottom));
        }

        // Draw frequency labels at bottom
        painter.setPen(textPen);
        painter.drawText(QRectF(cx - spacing/2.0, height() - bottomMargin + 10, spacing, 25),
                         Qt::AlignCenter, m_frequencies[i]);
    }

    // Draw smooth Catmull-Rom spline connecting all EQ nodes
    if (points.size() >= 2) {
        QPainterPath splinePath;
        splinePath.moveTo(points[0]);

        for (int i = 0; i < points.size() - 1; ++i) {
            QPointF p0 = points[qMax(i - 1, 0)];
            QPointF p1 = points[i];
            QPointF p2 = points[i + 1];
            QPointF p3 = points[qMin(i + 2, points.size() - 1)];

            // Bezier control points calculation
            QPointF cp1 = p1 + (p2 - p0) / 6.0;
            QPointF cp2 = p2 - (p3 - p1) / 6.0;

            splinePath.cubicTo(cp1, cp2, p2);
        }

        painter.setPen(active ? QPen(QColor("#FF2A7A"), 2) : QPen(QColor("#4C5264"), 2));
        painter.setBrush(Qt::NoBrush);
        painter.drawPath(splinePath);
    }

    // Draw the nodes (white circles with pink glowing shadows)
    for (int i = 0; i < 10; ++i) {
        double cx = points[i].x();
        double cy = points[i].y();

        if (active) {
            // Pink glow shadow
            painter.setBrush(QColor(255, 42, 122, 70));
            painter.setPen(Qt::NoPen);
            painter.drawEllipse(QPointF(cx, cy), 8, 8);

            // White core knob
            painter.setBrush(Qt::white);
            painter.drawEllipse(QPointF(cx, cy), 5, 5);
        } else {
            // Muted grey core knob, no glow
            painter.setBrush(QColor("#4C5264"));
            painter.setPen(Qt::NoPen);
            painter.drawEllipse(QPointF(cx, cy), 5, 5);
        }
    }
}


// ==========================================
// WaveformVisualizer Implementation
// ==========================================
WaveformVisualizer::WaveformVisualizer(QWidget* parent) : QWidget(parent) {
    m_heights.fill(0.1f, 44);
    m_levels.fill(0.1f, 44);
    generateDefaultWaveform();
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    setMinimumHeight(26);
    setMaximumHeight(34);
}

void WaveformVisualizer::setPlaybackProgress(double progress) {
    m_progress = qBound(0.0, progress, 1.0);
    update();
}

void WaveformVisualizer::setPlaying(bool playing) {
    m_isPlaying = playing;
    if (m_isPlaying) {
        if (m_timerId == -1) {
            m_timerId = startTimer(33); // ~30 FPS micro-animations
        }
    } else {
        if (m_timerId != -1) {
            killTimer(m_timerId);
            m_timerId = -1;
        }
    }
}

void WaveformVisualizer::updateBuffer(const QVector<float>& buffer) {
    if (buffer.size() == m_heights.size()) {
        m_heights = buffer;
    } else if (!buffer.isEmpty()) {
        // Interpolate buffer to match visualizer length
        int targetSize = m_heights.size();
        for (int i = 0; i < targetSize; ++i) {
            float srcIdx = (float)i / targetSize * buffer.size();
            int idx1 = (int)srcIdx;
            int idx2 = qMin(idx1 + 1, buffer.size() - 1);
            float fract = srcIdx - idx1;
            m_heights[i] = buffer[idx1] * (1.0f - fract) + buffer[idx2] * fract;
        }
    }
    update();
}

void WaveformVisualizer::generateDefaultWaveform() {
    // Generates a mock waveform that looks organic using overlapping sines
    int size = m_heights.size();
    for (int i = 0; i < size; ++i) {
        double x = (double)i / size;
        double val = 0.15 +
                     0.40 * std::sin(x * M_PI) +
                     0.25 * std::sin(x * 4 * M_PI) * std::cos(x * 2.5 * M_PI) +
                     0.15 * std::sin(x * 12 * M_PI + 1.2);
        m_heights[i] = static_cast<float>(qBound(0.1, std::abs(val), 0.95));
        m_levels[i] = m_heights[i];
    }
}

void WaveformVisualizer::timerEvent(QTimerEvent* event) {
    if (event->timerId() == m_timerId) {
        m_animationPhase += 0.15;
        int size = m_levels.size();
        for (int i = 0; i < size; ++i) {
            // Smoothly track the spectrum heights from Rust engine
            m_levels[i] = m_levels[i] * 0.75f + m_heights[i] * 0.25f;
            
            // Add a subtle micro-vibrancy if playing and bars are low
            if (m_isPlaying && m_heights[i] < 0.15f) {
                double wave = std::sin(i * 0.6 + m_animationPhase) * 0.03;
                m_levels[i] = qBound(0.05f, m_levels[i] + (float)wave, 1.0f);
            }
        }
        update();
    } else {
        QWidget::timerEvent(event);
    }
}

void WaveformVisualizer::hideEvent(QHideEvent* event) {
    if (m_timerId != -1) {
        killTimer(m_timerId);
        m_timerId = -1;
        m_timerWasActive = true;
    }
    QWidget::hideEvent(event);
}

void WaveformVisualizer::showEvent(QShowEvent* event) {
    if (m_timerWasActive || m_isPlaying) {
        if (m_timerId == -1) {
            m_timerId = startTimer(33);
        }
        m_timerWasActive = false;
    }
    QWidget::showEvent(event);
}

void WaveformVisualizer::mousePressEvent(QMouseEvent* event) {
    if (event->button() == Qt::LeftButton) {
        double w = width();
        if (w > 0.0) {
            double ratio = qBound(0.0, static_cast<double>(event->pos().x()) / w, 1.0);
            setPlaybackProgress(ratio);
            emit seekRequested(ratio);
        }
        event->accept();
    } else {
        QWidget::mousePressEvent(event);
    }
}

void WaveformVisualizer::paintEvent(QPaintEvent* event) {
    Q_UNUSED(event);
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);

    int numBars = m_levels.size();
    double w = width();
    double h = height();

    double totalSpacing = w - 4.0; // padding margins
    double barWidth = (totalSpacing / numBars) - 1.5;
    if (barWidth < 1.0) barWidth = 1.0;
    double step = totalSpacing / numBars;

    QLinearGradient activeGradient(QPointF(0, 0), QPointF(w, 0));
    activeGradient.setColorAt(0.0, QColor("#FF2A7A")); // Magenta highlight
    activeGradient.setColorAt(1.0, QColor("#7B1FA2")); // Purple gradient

    QColor inactiveColor("#252833");

    for (int i = 0; i < numBars; ++i) {
        double barHeight = m_levels[i] * h;
        double bx = 2.0 + i * step;
        double by = (h - barHeight) / 2.0;

        QRectF barRect(bx, by, barWidth, barHeight);

        double progressFraction = (double)i / numBars;
        if (progressFraction <= m_progress) {
            // Draw filled active bar
            painter.setBrush(activeGradient);
        } else {
            // Draw dark inactive bar
            painter.setBrush(inactiveColor);
        }
        painter.setPen(Qt::NoPen);
        painter.drawRoundedRect(barRect, barWidth / 2.0, barWidth / 2.0);
    }
}
