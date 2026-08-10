#include "mainwindow.h"
#include "gui_bridge_p.h"
#include <QShortcut>
#include <QSlider>
#include <QVBoxLayout>
#include <QGraphicsOpacityEffect>
#include <QPropertyAnimation>
#include <QWindowStateChangeEvent>

#if defined(_WIN32) || defined(WIN32)
#include <windows.h>
#endif

void MainWindow::setupKeyboardShortcuts() {
    auto* searchShortcut = new QShortcut(QKeySequence(Qt::CTRL | Qt::Key_F), this);
    connect(searchShortcut, &QShortcut::activated, this, [this]() {
        m_searchBar->setFocus();
        m_searchBar->selectAll();
    });

    setFocusPolicy(Qt::StrongFocus);
}

void MainWindow::showToast(const QString& message) {
    auto* toast = new QLabel(message, this);
    toast->setObjectName("ToastNotification");
    toast->setAlignment(Qt::AlignCenter);
    toast->setWordWrap(false);
    toast->setStyleSheet(
        "QLabel#ToastNotification {"
        "  background-color: #1E293B;"
        "  color: #E2E8F0;"
        "  border: 1px solid #334155;"
        "  border-radius: 10px;"
        "  padding: 10px 20px;"
        "  font-size: 13px;"
        "  font-weight: 600;"
        "}"
    );
    toast->adjustSize();
    toast->setFixedWidth(qMax(280, toast->sizeHint().width() + 40));

    int x = width() - toast->width() - 20;
    int y = height() - toast->height() - 24;
    toast->move(x, y);
    toast->raise();
    toast->show();

    auto* effect = new QGraphicsOpacityEffect(toast);
    toast->setGraphicsEffect(effect);
    auto* fadeIn = new QPropertyAnimation(effect, "opacity", toast);
    fadeIn->setDuration(250);
    fadeIn->setStartValue(0.0);
    fadeIn->setEndValue(1.0);
    fadeIn->start(QAbstractAnimation::DeleteWhenStopped);

    QTimer::singleShot(3500, toast, [toast, effect]() {
        auto* fadeOut = new QPropertyAnimation(effect, "opacity", toast);
        fadeOut->setDuration(350);
        fadeOut->setStartValue(1.0);
        fadeOut->setEndValue(0.0);
        connect(fadeOut, &QPropertyAnimation::finished, toast, &QLabel::deleteLater);
        fadeOut->start(QAbstractAnimation::DeleteWhenStopped);
    });
}

void MainWindow::keyPressEvent(QKeyEvent* event) {
    if (m_searchBar->hasFocus()) {
        QMainWindow::keyPressEvent(event);
        return;
    }

    const auto& cb = GuiBridgeManager::instance().callbacks();
    const bool  shift = event->modifiers() & Qt::ShiftModifier;

    switch (event->key()) {

    case Qt::Key_Space:
        if (cb.on_play_pause) cb.on_play_pause();
        event->accept();
        return;

    case Qt::Key_Right:
        if (shift) {
            double newPos = m_nowPlayingCard->elapsedSeconds() + 5.0;
            double total = m_nowPlayingCard->totalSeconds();
            if (total > 0.0) {
                newPos = qMin(newPos, total);
            }
            emit m_nowPlayingCard->seekRequested(newPos);
        } else {
            if (cb.on_next) cb.on_next();
        }
        event->accept();
        return;

    case Qt::Key_Left:
        if (shift) {
            double newPos = m_nowPlayingCard->elapsedSeconds() - 5.0;
            emit m_nowPlayingCard->seekRequested(qMax(0.0, newPos));
        } else {
            if (cb.on_prev) cb.on_prev();
        }
        event->accept();
        return;

    case Qt::Key_Up: {
        m_currentVolume = qMin(1.0, m_currentVolume + 0.05);
        m_isMuted = false;
        m_volumeBeforeMute = m_currentVolume;
        if (auto* slider = m_queueWidget->findChild<QSlider*>("VolumeSlider")) {
            slider->setValue(static_cast<int>(m_currentVolume * 100));
        } else if (cb.on_volume) {
            cb.on_volume(m_currentVolume);
        }
        event->accept();
        return;
    }
    case Qt::Key_Down: {
        m_currentVolume = qMax(0.0, m_currentVolume - 0.05);
        if (m_currentVolume == 0.0) m_isMuted = true;
        if (m_currentVolume > 0.0) m_volumeBeforeMute = m_currentVolume;
        if (auto* slider = m_queueWidget->findChild<QSlider*>("VolumeSlider")) {
            slider->setValue(static_cast<int>(m_currentVolume * 100));
        } else if (cb.on_volume) {
            cb.on_volume(m_currentVolume);
        }
        event->accept();
        return;
    }

    case Qt::Key_M: {
        if (m_isMuted) {
            m_isMuted = false;
            m_currentVolume = m_volumeBeforeMute;
        } else {
            m_volumeBeforeMute = m_currentVolume;
            m_isMuted = true;
            m_currentVolume = 0.0;
        }
        if (auto* slider = m_queueWidget->findChild<QSlider*>("VolumeSlider")) {
            slider->setValue(static_cast<int>(m_currentVolume * 100));
        } else if (cb.on_volume) {
            cb.on_volume(m_currentVolume);
        }
        event->accept();
        return;
    }

    case Qt::Key_R: {
        for (auto* btn : m_nowPlayingCard->findChildren<QPushButton*>("MediaControlBtn")) {
            if (btn->toolTip().startsWith("Repeat")) {
                btn->click();
                break;
            }
        }
        event->accept();
        return;
    }

    case Qt::Key_S: {
        for (auto* btn : m_nowPlayingCard->findChildren<QPushButton*>("MediaControlBtn")) {
            if (btn->toolTip().startsWith("Shuffle")) {
                btn->click();
                break;
            }
        }
        event->accept();
        return;
    }

    case Qt::Key_E: {
        if (m_eqWindow->isVisible()) {
            m_eqWindow->hide();
        } else {
            if ((m_eqWindow->pos().x() <= 0 && m_eqWindow->pos().y() <= 0) ||
                m_eqWindow->pos().x() + 100 > width() || m_eqWindow->pos().y() + 100 > height()) {
                int cx = qMax(0, (width() - m_eqWindow->width()) / 2);
                int cy = qMax(0, (height() - m_eqWindow->height()) / 2);
                m_eqWindow->move(cx, cy);
            }
            m_eqWindow->show();
            m_eqWindow->raise();
            m_eqWindow->activateWindow();
        }
        event->accept();
        return;
    }

    case Qt::Key_Q: {
        if (m_queueWidget) {
            bool visible = !m_queueWidget->isVisible();
            m_queueHiddenByUser = !visible;
            if (m_contentStack && m_contentStack->currentIndex() != 1) {
                m_queueWidget->setVisible(visible);
                if (m_sep2) m_sep2->setVisible(visible);
                if (m_toggleRightTopBtn) m_toggleRightTopBtn->setVisible(!visible);
            }
        }
        event->accept();
        return;
    }

    default:
        break;
    }

    QMainWindow::keyPressEvent(event);
}

void MainWindow::updateSidebarDimensions() {
    bool isLargeOrFullScreen = isFullScreen() || isMaximized() || width() >= 1600;
    int expandedLeft  = isLargeOrFullScreen ? 250 : 200;
    int collapsedLeft = isLargeOrFullScreen ? 80  : 64;
    int rightWidth    = isLargeOrFullScreen ? 350 : 290;

    if (m_sidebar) {
        m_sidebar->setSidebarWidths(expandedLeft, collapsedLeft);
    }
    if (m_queueWidget) {
        m_queueWidget->setFixedWidth(rightWidth);
    }
}

void MainWindow::changeEvent(QEvent* event) {
    QMainWindow::changeEvent(event);
    if (event && event->type() == QEvent::WindowStateChange) {
        auto* stateEvent = static_cast<QWindowStateChangeEvent*>(event);
        Qt::WindowStates oldState = stateEvent->oldState();
        Qt::WindowStates newState = windowState();

        if (newState & Qt::WindowMinimized) {
            if (oldState & Qt::WindowMaximized) {
                m_wasMaximizedBeforeMinimize = true;
            }
            if (oldState & Qt::WindowFullScreen) {
                m_wasFullScreenBeforeMinimize = true;
            }
        } else if (oldState & Qt::WindowMinimized) {
            if (m_wasMaximizedBeforeMinimize) {
                m_wasMaximizedBeforeMinimize = false;
                if (!(newState & Qt::WindowMaximized)) {
                    setWindowState((windowState() & ~Qt::WindowMinimized) | Qt::WindowMaximized);
                }
            } else if (m_wasFullScreenBeforeMinimize) {
                m_wasFullScreenBeforeMinimize = false;
                if (!(newState & Qt::WindowFullScreen)) {
                    setWindowState((windowState() & ~Qt::WindowMinimized) | Qt::WindowFullScreen);
                }
            }
        } else {
            if (!(newState & Qt::WindowMaximized)) {
                m_wasMaximizedBeforeMinimize = false;
            }
            if (!(newState & Qt::WindowFullScreen)) {
                m_wasFullScreenBeforeMinimize = false;
            }
        }

        updateSidebarDimensions();
    }
}

void MainWindow::resizeEvent(QResizeEvent* event) {
    QMainWindow::resizeEvent(event);
    int w = width();

    m_inResizeEvent = true;
    updateSidebarDimensions();

    if (w < 1050) {
        if (!m_sidebar->isCollapsed()) {
            m_sidebar->setCollapsed(true);
        }
    } else {
        if (!m_sidebarCollapsedByUser && m_sidebar->isCollapsed()) {
            m_sidebar->setCollapsed(false);
        }
    }

    if (w < 850) {
        if (m_queueWidget->isVisible()) {
            m_queueWidget->setVisible(false);
            if (m_sep2) m_sep2->setVisible(false);
            if (m_toggleRightTopBtn && m_contentStack && m_contentStack->currentIndex() != 1) {
                m_toggleRightTopBtn->setVisible(true);
            }
        }
    } else {
        if (!m_queueHiddenByUser && !m_queueWidget->isVisible()) {
            if (m_contentStack && m_contentStack->currentIndex() != 1) {
                m_queueWidget->setVisible(true);
                if (m_sep2) m_sep2->setVisible(true);
                if (m_toggleRightTopBtn) m_toggleRightTopBtn->setVisible(false);
            }
        }
    }

    int centerWidth = w - (m_sidebar->isVisible() ? m_sidebar->width() : 0) 
                        - (m_queueWidget->isVisible() ? m_queueWidget->width() : 0);
    if (m_songsTable) {
        m_songsTable->setResponsiveWidth(centerWidth);
    }

    if (auto* centralWidget = this->centralWidget()) {
        if (auto* centerPanel = centralWidget->findChild<QWidget*>("CenterPanel")) {
            if (auto* centerLayout = qobject_cast<QVBoxLayout*>(centerPanel->layout())) {
                int sideMargin = (w < 700) ? 10 : 20;
                centerLayout->setContentsMargins(sideMargin, sideMargin, sideMargin, sideMargin);
                centerLayout->setSpacing(sideMargin);
            }
        }
    }

    if (m_eqWindow && m_eqWindow->isVisible()) {
        QPoint eqPos = m_eqWindow->pos();
        int maxX = width() - m_eqWindow->width();
        int maxY = height() - m_eqWindow->height();
        eqPos.setX(qBound(0, eqPos.x(), qMax(0, maxX)));
        eqPos.setY(qBound(0, eqPos.y(), qMax(0, maxY)));
        m_eqWindow->move(eqPos);
    }

    m_inResizeEvent = false;
}

void MainWindow::forceAppIcon() {
    QIcon icon = windowIcon();
    if (icon.isNull()) return;

    setWindowIcon(icon);

#if defined(_WIN32) || defined(WIN32)
    HWND hwnd = reinterpret_cast<HWND>(this->winId());
    if (!hwnd) return;

    static const int sizes[] = {16, 32, 48, 64, 128, 256};
    for (int size : sizes) {
        QPixmap px = icon.pixmap(size, size);
        if (px.isNull()) continue;
        QImage img = px.toImage().convertToFormat(QImage::Format_ARGB32);
        if (img.isNull()) continue;
        HICON hIcon = img.toHICON();
        if (hIcon) {
            if (size <= 32) {
                SendMessage(hwnd, WM_SETICON, ICON_SMALL, reinterpret_cast<LPARAM>(hIcon));
            }
            SendMessage(hwnd, WM_SETICON, ICON_BIG, reinterpret_cast<LPARAM>(hIcon));
            DestroyIcon(hIcon);
        }
    }
#endif
}
