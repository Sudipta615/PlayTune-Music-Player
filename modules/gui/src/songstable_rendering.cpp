#include "songstable.h"
#include "coverloader.h"
#include "appsettings.h"
#include "apptheme.h"
#include "custom_widgets.h"
#include <QPainter>
#include <QPainterPath>
#include <QStyledItemDelegate>
#include <QScrollBar>
#include <QTimer>

namespace {

static QPixmap loadThumbnailHelper(const QString& coverPath, bool requestAsync = false) {
    if (AppSettings::instance().isOptimizedMode()) {
        QPixmap def = getDefaultAlbumArt();
        QPixmap target(44, 44);
        target.fill(Qt::transparent);
        QPainter painter(&target);
        painter.setRenderHint(QPainter::Antialiasing, true);
        painter.setRenderHint(QPainter::SmoothPixmapTransform, true);
        QPainterPath path;
        path.addRoundedRect(0, 0, 44, 44, 8, 8);
        painter.setClipPath(path);
        QPixmap scaled = def.scaled(44, 44, Qt::KeepAspectRatioByExpanding, Qt::SmoothTransformation);
        painter.drawPixmap((44 - scaled.width()) / 2, (44 - scaled.height()) / 2, scaled);
        return target;
    }
    QPixmap rounded;
    if (CoverLoader::instance().tryGetRounded(coverPath, 44, 8, rounded)) {
        return rounded;
    }

    QPixmap fallback;
    CoverLoader::instance().resolveOrFallback(coverPath, 44, fallback);
    if (requestAsync && !coverPath.isEmpty()) {
        CoverLoader::instance().requestAsync(coverPath, 44);
    }
    QPixmap target(44, 44);
    target.fill(Qt::transparent);
    QPainter painter(&target);
    painter.setRenderHint(QPainter::Antialiasing, true);
    painter.setRenderHint(QPainter::SmoothPixmapTransform, true);
    QPainterPath path;
    path.addRoundedRect(0, 0, 44, 44, 8, 8);
    painter.setClipPath(path);
    QPixmap scaled = fallback.scaled(44, 44, Qt::KeepAspectRatioByExpanding, Qt::SmoothTransformation);
    painter.drawPixmap((44 - scaled.width()) / 2, (44 - scaled.height()) / 2, scaled);
    return target;
}

} // namespace

class SongTableRowDelegate : public QStyledItemDelegate {
public:
    explicit SongTableRowDelegate(SongsTableWidget* owner, QObject* parent = nullptr)
        : QStyledItemDelegate(parent), m_owner(owner) {}

    void paint(QPainter* painter, const QStyleOptionViewItem& option, const QModelIndex& index) const override {
        painter->save();
        int row = index.row();
        bool isPlaying = (row == m_owner->playingTrackIdx());
        bool isHovered = (row == m_owner->hoveredRow());
        bool isSelected = false;
        if (auto* firstItem = m_owner->tableWidget()->item(row, 0)) {
            isSelected = firstItem->isSelected();
        }

        const auto& p = ThemeManager::instance().currentTheme();

        QColor bgColor = Qt::transparent;
        if (isPlaying) {
            bgColor = p.itemSelectedBg;
        } else if (isHovered) {
            bgColor = p.itemHoverBg;
        } else if (isSelected) {
            bgColor = p.itemSelectedBg;
        }

        if (bgColor.isValid() && bgColor != Qt::transparent) {
            painter->save();
            painter->setRenderHint(QPainter::Antialiasing, true);
            painter->setPen(Qt::NoPen);
            painter->setBrush(bgColor);

            int totalWidth = m_owner->tableWidget()->viewport()->width();
            QRectF fullRowRect(4, option.rect.top() + 2, totalWidth - 8, option.rect.height() - 4);

            painter->setClipRect(option.rect);
            painter->drawRoundedRect(fullRowRect, 10, 10);
            painter->restore();
        }

        QStyleOptionViewItem opt = option;
        opt.state &= ~QStyle::State_Selected;
        opt.state &= ~QStyle::State_HasFocus;

        QColor textColor;
        if (isPlaying) {
            textColor = p.secondaryAccent;
        } else {
            textColor = p.secondaryText;
        }

        opt.palette.setColor(QPalette::Text, textColor);
        opt.palette.setColor(QPalette::WindowText, textColor);
        opt.palette.setColor(QPalette::HighlightedText, textColor);

        QStyledItemDelegate::paint(painter, opt, index);
        painter->restore();
    }

private:
    SongsTableWidget* m_owner = nullptr;
};

void SongsTableWidget::initTableDelegate() {
    if (m_table) {
        m_table->setItemDelegate(new SongTableRowDelegate(this, m_table));
    }
}

QPixmap SongsTableWidget::getThumbnail(const QString& title) {
    Q_UNUSED(title);
    return getDefaultAlbumArt();
}

void SongsTableWidget::populateGridFromTable() {
    m_gridWidget->beginBatchAppend();
    m_gridWidget->clearGrid();
    for (const SongRow& r : m_rows) {
        m_gridWidget->addCard(r.songId, r.title, r.artist, r.coverPath, true);
    }
    m_gridPopulated = true;
    if (m_playingSongId > 0) {
        int idx = m_songIdToRow.value(m_playingSongId, -1);
        if (idx >= 0) m_gridWidget->setPlayingIndex(idx);
    }
    m_gridWidget->endBatchAppend();
}

void SongsTableWidget::onCellEntered(int row, int col) {
    Q_UNUSED(col);
    if (m_hoveredRow != row) {
        m_previousHoveredRow = m_hoveredRow;
        m_hoveredRow = row;
        if (m_previousHoveredRow >= 0) {
            refreshSingleRowStyle(m_previousHoveredRow);
        }
        if (m_hoveredRow >= 0) {
            refreshSingleRowStyle(m_hoveredRow);
        }
    }
}

void SongsTableWidget::refreshSingleRowStyle(int row) {
    if (row < 0 || row >= m_table->rowCount()) return;
    QRect r = m_table->visualRect(m_table->model()->index(row, 0));
    int totalWidth = m_table->viewport()->width();
    r.setX(0);
    r.setWidth(totalWidth);
    m_table->viewport()->update(r);
}

void SongsTableWidget::updateRowStyles() {
    QRect visible = m_table->viewport()->rect();
    int top = m_table->rowAt(visible.top());
    int bot = m_table->rowAt(visible.bottom());
    if (top < 0) top = 0;
    if (bot < 0) bot = m_table->rowCount() - 1;
    for (int row = top; row <= bot; ++row) {
        refreshSingleRowStyle(row);
    }
}

void SongsTableWidget::updateGridResponsive() {
    if (m_gridWidget) m_gridWidget->updateGridResponsive();
}

void SongsTableWidget::loadVisibleThumbnails() {
    if (!m_table || !m_table->viewport() || AppSettings::instance().isOptimizedMode()) return;
    QRect visible = m_table->viewport()->rect();
    int top = m_table->rowAt(visible.top());
    int bot = m_table->rowAt(visible.bottom());
    if (top < 0) top = 0;
    if (bot < 0) bot = m_table->rowCount() - 1;
    for (int row = top; row <= bot; ++row) {
        if (auto* firstItem = m_table->item(row, 0)) {
            QString coverPath = firstItem->data(Qt::UserRole + 1).toString();
            if (coverPath.isEmpty()) continue;
            QPixmap rounded;
            if (CoverLoader::instance().tryGetRounded(coverPath, 44, 8, rounded)) {
                if (auto* titleCont = m_table->cellWidget(row, 1)) {
                    auto labels = titleCont->findChildren<QLabel*>();
                    for (QLabel* l : labels) {
                        if (l->objectName() != "SongTitleLabel") {
                            l->setPixmap(rounded);
                            break;
                        }
                    }
                }
            } else {
                CoverLoader::instance().requestAsync(coverPath, 44);
            }
        }
    }
}
