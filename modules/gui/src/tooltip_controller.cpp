#include "tooltip_controller.h"
#include <QToolTip>
#include <QEvent>
#include <QWidget>
#include <QScreen>
#include <QGuiApplication>

ToolTipController::ToolTipController(QObject* parent) : QObject(parent) {}

void ToolTipController::setEnabled(bool enabled) {
    if (m_enabled == enabled) return;
    m_enabled = enabled;
    if (!m_enabled) {
        QToolTip::hideText();
    }
}

bool ToolTipController::isEnabled() const {
    return m_enabled;
}

bool ToolTipController::eventFilter(QObject* obj, QEvent* event) {
    // If tooltips are turned off, block all tooltip requests and ensure any visible tooltip is hidden.
    if (!m_enabled) {
        if (event->type() == QEvent::ToolTip) {
            QToolTip::hideText();
            return true; // Block event
        }
        if (event->type() == QEvent::Enter || event->type() == QEvent::MouseMove || event->type() == QEvent::Leave) {
            if (QToolTip::isVisible()) {
                QToolTip::hideText();
            }
        }
        return QObject::eventFilter(obj, event);
    }

    // When tooltips are ON:
    if (!obj->isWidgetType()) {
        return QObject::eventFilter(obj, event);
    }

    QWidget* w = qobject_cast<QWidget*>(obj);
    if (!w) {
        return QObject::eventFilter(obj, event);
    }

    // Walk up to find if w or any parent (up to the window level) has a tooltip
    QWidget* target = w;
    while (target && target->toolTip().isEmpty() && target->parentWidget() && !target->parentWidget()->isWindow()) {
        target = target->parentWidget();
    }

    const bool hasToolTip = target && !target->toolTip().isEmpty() && !target->isWindow();

    switch (event->type()) {
    case QEvent::Enter: {
        if (hasToolTip) {
            showInstantToolTip(target);
        } else if (QToolTip::isVisible()) {
            QToolTip::hideText();
        }
        break;
    }
    case QEvent::Leave:
    case QEvent::MouseButtonPress:
    case QEvent::WindowDeactivate:
    case QEvent::Hide: {
        if (QToolTip::isVisible()) {
            QToolTip::hideText();
        }
        break;
    }
    case QEvent::ToolTip: {
        if (hasToolTip) {
            // Ensure tooltip is shown instantly near the component rather than using Qt's delayed globalPos
            showInstantToolTip(target);
            return true; // Intercept to prevent Qt's default tooltip handler from moving/delaying it
        } else {
            QToolTip::hideText();
            return true;
        }
    }
    default:
        break;
    }

    return QObject::eventFilter(obj, event);
}

void ToolTipController::showInstantToolTip(QWidget* target) {
    if (!target || target->toolTip().isEmpty()) return;

    QPoint globalTopLeft = target->mapToGlobal(QPoint(0, 0));
    int th = target->height();

    // Position near the component (right below the component with a clean 4px vertical margin)
    QPoint pos = globalTopLeft + QPoint(8, th + 4);

    QScreen* screen = target->screen();
    if (!screen) screen = QGuiApplication::primaryScreen();
    if (screen) {
        QRect geom = screen->availableGeometry();
        // If placing below overflows screen bottom, place directly above the component
        if (pos.y() + 40 > geom.bottom()) {
            pos = globalTopLeft + QPoint(8, -32);
        }
        // Ensure horizontal positioning stays within screen bounds
        if (pos.x() + 260 > geom.right()) {
            pos.setX(qMax(geom.left() + 4, geom.right() - 260));
        }
    }

    QToolTip::showText(pos, target->toolTip(), target, target->rect());
}
