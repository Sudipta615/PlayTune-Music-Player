#include "playing_equalizer_icon.h"
#include "apptheme.h"
#include <QPainter>
#include <cstdlib>

PlayingEqualizerIcon::PlayingEqualizerIcon(QWidget* parent) : QWidget(parent) {
    m_timerId = -1;
}

void PlayingEqualizerIcon::setPlaying(bool playing) {
    m_isPlaying = playing;
    if (m_isPlaying) {
        if (m_timerId == -1) m_timerId = startTimer(70);
    } else {
        if (m_timerId != -1) {
            killTimer(m_timerId);
            m_timerId = -1;
        }
        m_heights = {0.2f, 0.2f, 0.2f};
        update();
    }
}

void PlayingEqualizerIcon::timerEvent(QTimerEvent* event) {
    if (event->timerId() == m_timerId) {
        for (int i = 0; i < 3; ++i) {
            if (qAbs(m_heights[i] - m_targetHeights[i]) < 0.05f) {
                m_targetHeights[i] = 0.15f + static_cast<float>(std::rand() % 70) / 100.0f;
            }
            m_heights[i] = m_heights[i] * 0.6f + m_targetHeights[i] * 0.4f;
        }
        update();
    } else {
        QWidget::timerEvent(event);
    }
}

void PlayingEqualizerIcon::paintEvent(QPaintEvent* event) {
    Q_UNUSED(event);
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);

    double w = width();
    double h = height();
    double barW = 3.0;
    double spacing = 2.0;
    double startX = (w - (3 * barW + 2 * spacing)) / 2.0;

    const auto& p = ThemeManager::instance().currentTheme();
    painter.setBrush(p.secondaryAccent);
    painter.setPen(Qt::NoPen);

    for (int i = 0; i < 3; ++i) {
        double barH = qMax(2.0, static_cast<double>(m_heights[i]) * h);
        double x = startX + i * (barW + spacing);
        double y = h - barH;
        painter.drawRoundedRect(QRectF(x, y, barW, barH), 1.5, 1.5);
    }
}
