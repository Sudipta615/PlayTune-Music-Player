#include "settingspage.h"
#include "apptheme.h"
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QListWidget>

void SettingsPageWidget::clearFolderList() {
    if (m_foldersListWidget) m_foldersListWidget->clear();
}

void SettingsPageWidget::addFolderToList(int id, const QString& path, const QString& name, int /*trackCount*/) {
    if (!m_foldersListWidget) return;
    auto* item = new QListWidgetItem(m_foldersListWidget);
    item->setSizeHint(QSize(0, 50));
    item->setData(Qt::UserRole, id);

    auto* rowWidget = new QWidget(m_foldersListWidget);
    rowWidget->setStyleSheet("background: transparent;");
    auto* rowLayout = new QHBoxLayout(rowWidget);
    rowLayout->setContentsMargins(16, 8, 16, 8);
    rowLayout->setSpacing(12);

    auto* textLabel = new QLabel(QString("📁 %1  —  %2").arg(name, path), rowWidget);
    textLabel->setToolTip(path);

    auto* delBtn = new QPushButton(rowWidget);
    delBtn->setObjectName("DeleteFolderBtn");
    delBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/close.png",
        ThemeManager::instance().currentTheme().iconColor));
    delBtn->setIconSize(QSize(16, 16));
    delBtn->setFixedSize(32, 32);
    delBtn->setCursor(Qt::PointingHandCursor);
    delBtn->setToolTip("Remove Folder and All Songs Inside");
    delBtn->setStyleSheet(
        "QPushButton { background-color: transparent; border: none; border-radius: 6px; padding: 0px; }"
        "QPushButton:hover { background-color: rgba(229, 57, 53, 0.75); }"
    );

    connect(delBtn, &QPushButton::clicked, this, [this, id]() {
        emit deleteFolderRequested(id);
    });

    rowLayout->addWidget(textLabel, 1);
    rowLayout->addWidget(delBtn, 0, Qt::AlignRight | Qt::AlignVCenter);

    m_foldersListWidget->addItem(item);
    m_foldersListWidget->setItemWidget(item, rowWidget);
}
