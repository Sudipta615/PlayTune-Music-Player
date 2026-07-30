#include "karaokedialog.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QPainter>
#include <QPainterPath>
#include <QKeyEvent>
#include <QImage>
#include <QScrollBar>
#include <QFile>
#include <QFont>

KaraokeDialog::KaraokeDialog(QWidget *parent)
    : QDialog(parent),
      m_activeIndex(-1),
      m_isSynced(false)
{
    setWindowFlags(Qt::Dialog | Qt::FramelessWindowHint | Qt::Window);
    setAttribute(Qt::WA_TranslucentBackground, false);
    resize(1024, 768);

    QVBoxLayout* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(40, 30, 40, 40);
    mainLayout->setSpacing(20);

    // Top Bar: Track Info & Close Button
    QHBoxLayout* topLayout = new QHBoxLayout();
    QVBoxLayout* infoLayout = new QVBoxLayout();
    infoLayout->setSpacing(4);

    m_titleLabel = new QLabel(QStringLiteral("No Track Playing"), this);
    m_titleLabel->setFont(QFont(QStringLiteral("Inter"), 24, QFont::Bold));
    m_titleLabel->setStyleSheet(QStringLiteral("color: #ffffff;"));

    m_artistLabel = new QLabel(QStringLiteral("PlayTune Library"), this);
    m_artistLabel->setFont(QFont(QStringLiteral("Inter"), 16, QFont::Medium));
    m_artistLabel->setStyleSheet(QStringLiteral("color: rgba(255, 255, 255, 0.7);"));

    infoLayout->addWidget(m_titleLabel);
    infoLayout->addWidget(m_artistLabel);
    topLayout->addLayout(infoLayout);
    topLayout->addStretch();

    m_closeButton = new QPushButton(QStringLiteral("EXIT ⛶"), this);
    m_closeButton->setCursor(Qt::PointingHandCursor);
    m_closeButton->setStyleSheet(QStringLiteral(
        "QPushButton {"
        "  background-color: rgba(255, 255, 255, 0.1);"
        "  color: #ffffff;"
        "  border: 1px solid rgba(255, 255, 255, 0.2);"
        "  border-radius: 20px;"
        "  padding: 8px 20px;"
        "  font-weight: bold;"
        "  font-size: 14px;"
        "}"
        "QPushButton:hover {"
        "  background-color: #ff4b4b;"
        "  border-color: #ff4b4b;"
        "  color: #ffffff;"
        "}"
    ));
    connect(m_closeButton, &QPushButton::clicked, this, &KaraokeDialog::accept);
    topLayout->addWidget(m_closeButton);

    mainLayout->addLayout(topLayout);

    // Center: Synced Lyrics ListWidget or Unsynced Label
    m_listWidget = new QListWidget(this);
    m_listWidget->setFrameShape(QFrame::NoFrame);
    m_listWidget->setStyleSheet(QStringLiteral(
        "QListWidget {"
        "  background: transparent;"
        "  border: none;"
        "  outline: 0;"
        "}"
        "QListWidget::item {"
        "  padding: 14px 10px;"
        "}"
        "QListWidget::item:hover {"
        "  background: rgba(0, 229, 255, 0.08);"
        "  border-radius: 12px;"
        "}"
        "QListWidget::item:selected {"
        "  background: transparent;"
        "}"
    ));
    m_listWidget->setVerticalScrollMode(QAbstractItemView::ScrollPerPixel);
    m_listWidget->setFocusPolicy(Qt::NoFocus);
    m_listWidget->verticalScrollBar()->setStyleSheet(QStringLiteral(
        "QScrollBar:vertical {"
        "  border: none;"
        "  background: rgba(255, 255, 255, 0.05);"
        "  width: 6px;"
        "  border-radius: 3px;"
        "}"
        "QScrollBar::handle:vertical {"
        "  background: rgba(255, 255, 255, 0.3);"
        "  min-height: 20px;"
        "  border-radius: 3px;"
        "}"
        "QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {"
        "  height: 0px;"
        "}"
    ));
    connect(m_listWidget, &QListWidget::itemClicked, this, &KaraokeDialog::onLineItemClicked);
    mainLayout->addWidget(m_listWidget, 1);

    m_unsyncedLabel = new QLabel(this);
    m_unsyncedLabel->setAlignment(Qt::AlignCenter);
    m_unsyncedLabel->setWordWrap(true);
    m_unsyncedLabel->setFont(QFont(QStringLiteral("Inter"), 18, QFont::Medium));
    m_unsyncedLabel->setStyleSheet(QStringLiteral("color: rgba(255, 255, 255, 0.85); padding: 30px;"));
    m_unsyncedLabel->hide();
    mainLayout->addWidget(m_unsyncedLabel, 1);
}

void KaraokeDialog::paintEvent(QPaintEvent* event) {
    Q_UNUSED(event);
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    painter.setRenderHint(QPainter::SmoothPixmapTransform);

    if (!m_blurredCover.isNull()) {
        painter.drawPixmap(rect(), m_blurredCover);
        // Dark glass overlay for neon contrast
        painter.fillRect(rect(), QColor(10, 14, 23, 215));
    } else {
        // Deep vibrant dark background
        QLinearGradient grad(0, 0, width(), height());
        grad.setColorAt(0, QColor(15, 23, 42));
        grad.setColorAt(1, QColor(8, 12, 22));
        painter.fillRect(rect(), grad);
    }
}

void KaraokeDialog::keyPressEvent(QKeyEvent* event) {
    if (event->key() == Qt::Key_Escape) {
        accept();
    } else {
        QDialog::keyPressEvent(event);
    }
}

void KaraokeDialog::setTrackInfo(const QString& title, const QString& artist, const QString& coverPath) {
    if (!title.isEmpty()) m_titleLabel->setText(title);
    if (!artist.isEmpty()) m_artistLabel->setText(artist);

    m_blurredCover = QPixmap();
    if (!coverPath.isEmpty() && QFile::exists(coverPath)) {
        QImage img(coverPath);
        if (!img.isNull()) {
            // Scale down drastically and scale back up to generate a smooth Gaussian-like blur
            QImage smallImg = img.scaled(48, 48, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
            m_blurredCover = QPixmap::fromImage(smallImg.scaled(1920, 1080, Qt::IgnoreAspectRatio, Qt::SmoothTransformation));
        }
    }
    update();
}

void KaraokeDialog::setLyrics(const QString& syncedLrc, const QString& unsyncedLyrics) {
    m_lines.clear();
    m_listWidget->clear();
    m_activeIndex = -1;

    if (!syncedLrc.trimmed().isEmpty()) {
        m_lines = LrcParser::parse(syncedLrc);
    }

    if (!m_lines.isEmpty()) {
        m_isSynced = true;
        m_listWidget->show();
        m_unsyncedLabel->hide();

        for (const LrcLine& line : m_lines) {
            QListWidgetItem* item = new QListWidgetItem(line.text, m_listWidget);
            item->setTextAlignment(Qt::AlignCenter);
            item->setFont(QFont(QStringLiteral("Inter"), 18, QFont::Medium));
            item->setForeground(QBrush(QColor(255, 255, 255, 120)));
            item->setData(Qt::UserRole, line.timestampSeconds);
        }
    } else if (!unsyncedLyrics.trimmed().isEmpty()) {
        m_isSynced = false;
        m_listWidget->hide();
        m_unsyncedLabel->setText(unsyncedLyrics.trimmed());
        m_unsyncedLabel->show();
    } else {
        m_isSynced = false;
        m_listWidget->hide();
        m_unsyncedLabel->setText(QStringLiteral("🎵 No lyrics found for this track.\nAdd an .lrc file in the same folder or embed lyrics via Tag Editor."));
        m_unsyncedLabel->show();
    }
}

void KaraokeDialog::updateProgress(double elapsedSeconds) {
    if (!m_isSynced || m_lines.isEmpty()) return;

    int newIndex = LrcParser::findActiveLineIndex(m_lines, elapsedSeconds);
    if (newIndex != m_activeIndex && newIndex >= 0 && newIndex < m_listWidget->count()) {
        // Reset previous active line
        if (m_activeIndex >= 0 && m_activeIndex < m_listWidget->count()) {
            QListWidgetItem* prevItem = m_listWidget->item(m_activeIndex);
            prevItem->setFont(QFont(QStringLiteral("Inter"), 18, QFont::Medium));
            prevItem->setForeground(QBrush(QColor(255, 255, 255, 120)));
        }

        m_activeIndex = newIndex;

        // Highlight new active line with glowing neon cyan typography
        QListWidgetItem* currItem = m_listWidget->item(m_activeIndex);
        currItem->setFont(QFont(QStringLiteral("Inter"), 26, QFont::Bold));
        currItem->setForeground(QBrush(QColor(QStringLiteral("#00e5ff"))));

        m_listWidget->scrollToItem(currItem, QAbstractItemView::PositionAtCenter);
    }
}

void KaraokeDialog::onLineItemClicked(QListWidgetItem* item) {
    if (!m_isSynced || !item) return;
    QVariant tsData = item->data(Qt::UserRole);
    if (tsData.isValid()) {
        double seconds = tsData.toDouble();
        emit seekRequested(seconds);
    }
}
