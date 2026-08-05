#include "apptheme.h"
#include <QSettings>
#include <QApplication>
#include <QPainter>
#include <QLinearGradient>
#include <QDebug>

ThemeManager& ThemeManager::instance() {
    static ThemeManager inst;
    return inst;
}

ThemeManager::ThemeManager(QObject* parent) : QObject(parent) {
    registerThemes();
    loadSavedTheme();
}

void ThemeManager::registerThemes() {
    // 1. Dark Premium (Default / Purple)
    ThemePalette dark;
    dark.id = "dark";
    dark.name = "Dark Premium (Purple)";
    dark.isLight = false;
    dark.windowBg = QColor("#0E111A");
    dark.sidebarBg = QColor("#0B0F21");
    dark.queueBg = QColor("#0A0E1C");
    dark.cardBg = QColor("#121624");
    dark.cardBorder = QColor("#1E2436");
    dark.headerBg = QColor("#0E121B");
    dark.separatorColor = QColor("#242A3D");
    dark.primaryText = QColor("#FFFFFF");
    dark.secondaryText = QColor("#E1E4EB");
    dark.mutedText = QColor("#7E8494");
    dark.primaryAccent = QColor("#7B1FA2");     // Deep Purple
    dark.secondaryAccent = QColor("#FF2A7A");   // Vivid Pink
    dark.itemHoverBg = QColor("#1B1130");
    dark.itemSelectedBg = QColor("#2A1645");
    dark.scrollbarHandle = QColor("#252833");
    dark.tooltipBg = QColor("#131622");
    dark.tooltipBorder = QColor("#7B1FA2");
    dark.placeholderGradStart = QColor("#7B1FA2");
    dark.placeholderGradEnd = QColor("#FF2A7A");
    dark.cardBgGradStart = QColor("#151624");
    dark.cardBgGradEnd = QColor("#0F111D");
    m_themes["dark"] = dark;
    m_themesOrder.append("dark");

    // 2. Light Premium
    ThemePalette light;
    light.id = "light";
    light.name = "Light Premium";
    light.isLight = true;
    light.windowBg = QColor("#F1F5F9");
    light.sidebarBg = QColor("#E2E8F0");
    light.queueBg = QColor("#E2E8F0");
    light.cardBg = QColor("#FFFFFF");
    light.cardBorder = QColor("#CBD5E1");
    light.headerBg = QColor("#F8FAFC");
    light.separatorColor = QColor("#CBD5E1");
    light.primaryText = QColor("#0F172A");      // Very dark slate
    light.secondaryText = QColor("#334155");    // Dark slate gray
    light.mutedText = QColor("#64748B");        // Medium slate
    light.primaryAccent = QColor("#7C3AED");    // Royal Violet
    light.secondaryAccent = QColor("#D946EF");  // Vivid Fuchsia/Pink
    light.itemHoverBg = QColor("#F1F5F9");
    light.itemSelectedBg = QColor("#E2E8F0");
    light.scrollbarHandle = QColor("#94A3B8");
    light.tooltipBg = QColor("#0F172A");
    light.tooltipBorder = QColor("#7C3AED");
    light.placeholderGradStart = QColor("#7C3AED");
    light.placeholderGradEnd = QColor("#D946EF");
    light.cardBgGradStart = QColor("#E2E8F0");
    light.cardBgGradEnd = QColor("#CBD5E1");
    m_themes["light"] = light;
    m_themesOrder.append("light");

    // 3. Emerald Teal
    ThemePalette teal;
    teal.id = "teal";
    teal.name = "Emerald Teal";
    teal.isLight = false;
    teal.windowBg = QColor("#051014");
    teal.sidebarBg = QColor("#030B0E");
    teal.queueBg = QColor("#030A0D");
    teal.cardBg = QColor("#09181D");
    teal.cardBorder = QColor("#112830");
    teal.headerBg = QColor("#07151B");
    teal.separatorColor = QColor("#112D36");
    teal.primaryText = QColor("#FFFFFF");
    teal.secondaryText = QColor("#D1F5F2");
    teal.mutedText = QColor("#4A7A77");
    teal.primaryAccent = QColor("#00D294");     // Mint Emerald
    teal.secondaryAccent = QColor("#00E5FF");   // Aqua Cyan
    teal.itemHoverBg = QColor("#0D2B30");
    teal.itemSelectedBg = QColor("#0F3D45");
    teal.scrollbarHandle = QColor("#143840");
    teal.tooltipBg = QColor("#081C22");
    teal.tooltipBorder = QColor("#00D294");
    teal.placeholderGradStart = QColor("#00D294");
    teal.placeholderGradEnd = QColor("#00E5FF");
    teal.cardBgGradStart = QColor("#0B2228");
    teal.cardBgGradEnd = QColor("#06161B");
    m_themes["teal"] = teal;
    m_themesOrder.append("teal");

    // 4. Sunset Amber
    ThemePalette amber;
    amber.id = "amber";
    amber.name = "Sunset Amber";
    amber.isLight = false;
    amber.windowBg = QColor("#110B07");
    amber.sidebarBg = QColor("#0C0704");
    amber.queueBg = QColor("#0B0604");
    amber.cardBg = QColor("#1A110B");
    amber.cardBorder = QColor("#2E1E14");
    amber.headerBg = QColor("#160E08");
    amber.separatorColor = QColor("#2E1F16");
    amber.primaryText = QColor("#FFFFFF");
    amber.secondaryText = QColor("#F5E8DF");
    amber.mutedText = QColor("#8F7261");
    amber.primaryAccent = QColor("#FF5722");   // Vivid Orange
    amber.secondaryAccent = QColor("#FF9800"); // Warm Amber/Gold
    amber.itemHoverBg = QColor("#2C1A0F");
    amber.itemSelectedBg = QColor("#402413");
    amber.scrollbarHandle = QColor("#3B281B");
    amber.tooltipBg = QColor("#1D120B");
    amber.tooltipBorder = QColor("#FF5722");
    amber.placeholderGradStart = QColor("#FF5722");
    amber.placeholderGradEnd = QColor("#FF9800");
    amber.cardBgGradStart = QColor("#24170F");
    amber.cardBgGradEnd = QColor("#140C07");
    m_themes["amber"] = amber;
    m_themesOrder.append("amber");

    // 5. Electric Cyan
    ThemePalette cyan;
    cyan.id = "cyan";
    cyan.name = "Electric Cyan";
    cyan.isLight = false;
    cyan.windowBg = QColor("#040A12");
    cyan.sidebarBg = QColor("#02070E");
    cyan.queueBg = QColor("#02060D");
    cyan.cardBg = QColor("#081322");
    cyan.cardBorder = QColor("#102138");
    cyan.headerBg = QColor("#060F1B");
    cyan.separatorColor = QColor("#10243E");
    cyan.primaryText = QColor("#FFFFFF");
    cyan.secondaryText = QColor("#D6E9FF");
    cyan.mutedText = QColor("#4D749B");
    cyan.primaryAccent = QColor("#00D2FF");    // Electric Cyan
    cyan.secondaryAccent = QColor("#2563EB");  // Sapphire Blue
    cyan.itemHoverBg = QColor("#0E233D");
    cyan.itemSelectedBg = QColor("#123359");
    cyan.scrollbarHandle = QColor("#142F52");
    cyan.tooltipBg = QColor("#071526");
    cyan.tooltipBorder = QColor("#00D2FF");
    cyan.placeholderGradStart = QColor("#00D2FF");
    cyan.placeholderGradEnd = QColor("#2563EB");
    cyan.cardBgGradStart = QColor("#0B1B30");
    cyan.cardBgGradEnd = QColor("#050E1A");
    m_themes["cyan"] = cyan;
    m_themesOrder.append("cyan");

    // 6. Crimson Rose
    ThemePalette crimson;
    crimson.id = "crimson";
    crimson.name = "Crimson Rose";
    crimson.isLight = false;
    crimson.windowBg = QColor("#120408");
    crimson.sidebarBg = QColor("#0D0205");
    crimson.queueBg = QColor("#0C0205");
    crimson.cardBg = QColor("#1B080F");
    crimson.cardBorder = QColor("#30101C");
    crimson.headerBg = QColor("#16050A");
    crimson.separatorColor = QColor("#33121F");
    crimson.primaryText = QColor("#FFFFFF");
    crimson.secondaryText = QColor("#FCE6EF");
    crimson.mutedText = QColor("#8D5167");
    crimson.primaryAccent = QColor("#FF1744");  // Crimson Red
    crimson.secondaryAccent = QColor("#D81B60");// Rose Magenta
    crimson.itemHoverBg = QColor("#2E0D19");
    crimson.itemSelectedBg = QColor("#471226");
    crimson.scrollbarHandle = QColor("#3D1524");
    crimson.tooltipBg = QColor("#210710");
    crimson.tooltipBorder = QColor("#FF1744");
    crimson.placeholderGradStart = QColor("#FF1744");
    crimson.placeholderGradEnd = QColor("#D81B60");
    crimson.cardBgGradStart = QColor("#260B15");
    crimson.cardBgGradEnd = QColor("#130409");
    m_themes["crimson"] = crimson;
    m_themesOrder.append("crimson");

    m_currentPalette = m_themes["dark"];
}

void ThemeManager::loadSavedTheme() {
    QSettings settings("PlayTune", "Settings");
    QString themeId = settings.value("theme_id", "dark").toString();
    if (!m_themes.contains(themeId)) {
        // Fallback: check theme_text
        QString themeText = settings.value("theme_text", "").toString();
        for (auto it = m_themes.begin(); it != m_themes.end(); ++it) {
            if (it.value().name == themeText) {
                themeId = it.key();
                break;
            }
        }
    }
    if (!m_themes.contains(themeId)) themeId = "dark";
    m_currentPalette = m_themes[themeId];
}

QList<QPair<QString, QString>> ThemeManager::availableThemes() const {
    QList<QPair<QString, QString>> list;
    // Preserve the registration order rather than relying on the QMap's
    // alphabetical key ordering, and keep this list in sync with the
    // themes actually registered in registerThemes() (single source of truth).
    for (const QString& id : m_themesOrder) {
        const ThemePalette& t = m_themes[id];
        list.append(qMakePair(id, t.name));
    }
    return list;
}

void ThemeManager::setTheme(const QString& themeId) {
    if (!m_themes.contains(themeId)) {
        qWarning() << "ThemeManager::setTheme - Unknown theme ID:" << themeId;
        return;
    }

    m_currentPalette = m_themes[themeId];

    QSettings settings("PlayTune", "Settings");
    settings.setValue("theme_id", themeId);
    settings.setValue("theme_text", m_currentPalette.name);

    if (qApp) {
        qApp->setStyleSheet(generateStylesheet());
    }

    emit themeChanged(m_currentPalette);
}

QString ThemeManager::generateStylesheet() const {
    const auto& p = m_currentPalette;

    return QString(R"(
/* Dynamic PlayTune Stylesheet — Active Theme: %1 */

QWidget {
    font-family: "Outfit", "Inter", "Segoe UI", sans-serif;
    color: %2;
}

QToolTip {
    background-color: %3;
    color: #FFFFFF;
    border: 1px solid %4;
    border-radius: 6px;
    padding: 6px 10px;
    font-size: 12px;
    font-weight: 500;
}

QMainWindow, QWidget#CenterPanel {
    background-color: %5;
}

QWidget#SidebarFrame {
    background-color: %6;
    border: none;
}

QLabel#LogoLabel {
    font-size: 18px;
    font-weight: bold;
    color: %7;
    padding-left: 5px;
}

QPushButton#SidebarBtn {
    background: transparent;
    color: %8;
    border: none;
    text-align: left;
    padding: 10px 15px;
    font-size: 14px;
    border-radius: 8px;
    font-weight: 500;
}

QPushButton#SidebarBtn:hover {
    background-color: %9;
    color: %7;
}

QPushButton#SidebarBtn:checked {
    background-color: %9;
    color: %10;
    font-weight: bold;
}

QLabel#SectionHeader {
    color: %8;
    font-size: 11px;
    font-weight: bold;
    text-transform: uppercase;
    margin-top: 15px;
    margin-bottom: 5px;
    padding-left: 10px;
}

QLineEdit#SearchBar {
    background-color: %11;
    border: 1px solid %12;
    border-radius: 10px;
    color: %7;
    padding: 8px 12px;
    font-size: 13px;
}

QLineEdit#SearchBar:hover {
    border: 1px solid %4;
}

QLineEdit#SearchBar:focus {
    border: 1px solid %10;
}

QLabel#ContentHeader {
    font-size: 20px;
    font-weight: bold;
    color: %7;
}
    background: transparent;
    border: none;
    border-radius: 6px;
    padding: 6px;
}

QPushButton#MediaControlBtn:hover {
    background-color: %9;
}

QPushButton#MediaControlBtn:checked {
    background-color: %10;
    border-radius: 8px;
}

QPushButton#MediaControlBtn:checked:hover {
    background-color: %10;
}

QPushButton#PlayPauseBtn {
    background-color: %4;
    border: none;
    border-radius: 20px;
    padding: 8px;
}

QPushButton#PlayPauseBtn:hover {
    background-color: %10;
}

QLabel#ContentHeader {
    font-size: 20px;
    font-weight: bold;
    color: %7;
}

QLabel#ContentSubHeader {
    font-size: 13px;
    color: %8;
    margin-left: 8px;
}

QComboBox {
    background-color: %11;
    border: 1px solid %12;
    border-radius: 8px;
    padding: 6px 12px;
    color: %7;
    font-size: 13px;
}

QComboBox:hover {
    border: 1px solid %4;
}

QComboBox::drop-down {
    border: none;
    width: 20px;
}

QComboBox QAbstractItemView {
    background-color: %11;
    border: 1px solid %12;
    selection-background-color: %9;
    selection-color: %10;
    color: %7;
    outline: none;
    padding: 4px;
}

QComboBox QAbstractItemView::item {
    padding: 6px 12px;
    border-radius: 4px;
    color: %2;
    background-color: transparent;
}

QComboBox QAbstractItemView::item:hover {
    background-color: %9;
    color: %7;
}

QComboBox QAbstractItemView::item:selected {
    background-color: %9;
    color: %10;
    font-weight: bold;
}

QSpinBox, QDoubleSpinBox {
    background-color: %11;
    color: %7;
    border: 1px solid %12;
    border-radius: 8px;
    padding: 4px 8px;
    padding-right: 24px;
    font-size: 13px;
    font-weight: 500;
}

QSpinBox:focus, QDoubleSpinBox:focus {
    border-color: %10;
}

QSpinBox::up-button, QDoubleSpinBox::up-button {
    subcontrol-origin: border;
    subcontrol-position: top right;
    width: 22px;
    height: 14px;
    border-left: 1px solid %12;
    border-bottom: 1px solid %12;
    border-top-right-radius: 7px;
    background-color: %9;
}

QSpinBox::up-button:hover, QDoubleSpinBox::up-button:hover {
    background-color: %4;
}

QSpinBox::down-button, QDoubleSpinBox::down-button {
    subcontrol-origin: border;
    subcontrol-position: bottom right;
    width: 22px;
    height: 14px;
    border-left: 1px solid %12;
    border-bottom-right-radius: 7px;
    background-color: %9;
}

QSpinBox::down-button:hover, QDoubleSpinBox::down-button:hover {
    background-color: %4;
}

QPushButton#IconButton {
    background-color: %11;
    border: 1px solid %12;
    border-radius: 8px;
    padding: 6px;
}

QPushButton#IconButton:hover {
    background-color: %9;
}

QPushButton#IconButton:checked {
    background-color: %9;
    border-color: %10;
}

QTableWidget {
    background-color: transparent;
    gridline-color: transparent;
    border: none;
    outline: none;
}

QTableWidget::item {
    border-bottom: 1px solid %12;
    padding: 4px 10px;
    outline: none;
}

QTableWidget::item:focus, QTableWidget::item:selected {
    border: none;
    outline: none;
}

QHeaderView {
    background-color: %11;
    border-radius: 10px;
    border: 1px solid %12;
    margin: 0px;
}

QHeaderView::section {
    background-color: transparent;
    color: %8;
    padding: 8px 10px;
    border: none;
    font-size: 11px;
    font-weight: bold;
    text-transform: uppercase;
}

QScrollBar:vertical {
    border: none;
    background: transparent;
    width: 6px;
    margin: 0px;
}

QScrollBar::handle:vertical {
    background: %13;
    min-height: 20px;
    border-radius: 3px;
}

QScrollBar::handle:vertical:hover {
    background: %4;
}

QWidget#RightSidebarFrame {
    background-color: %14;
    border: none;
}

QFrame#TabContainer {
    background-color: %11;
    border: 1px solid %12;
    border-radius: 10px;
    padding: 2px;
}

QPushButton#TabBtn {
    background: transparent;
    color: %8;
    border: none;
    border-radius: 8px;
    padding: 6px 12px;
    font-size: 13px;
    font-weight: 500;
}

QPushButton#TabBtn:hover {
    color: %7;
    background-color: %9;
}

QPushButton#TabBtn:checked {
    background-color: %4;
    color: #FFFFFF;
    font-weight: bold;
}

QSlider::groove:horizontal {
    height: 4px;
    background: %12;
    border-radius: 2px;
}

QSlider::sub-page:horizontal {
    background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 %4, stop:1 %10);
    border-radius: 2px;
}

QSlider::handle:horizontal {
    background: #FFFFFF;
    width: 12px;
    height: 12px;
    margin-top: -4px;
    margin-bottom: -4px;
    border-radius: 6px;
}

QSlider::handle:horizontal:hover {
    background: %10;
}

QFrame#NowPlayingCard QSlider::groove:horizontal {
    height: 4px;
    background: rgba(255, 255, 255, 0.28);
    border-radius: 2px;
}

#EqualizerWindow {
    background-color: %5;
    border: 1.5px solid %12;
    border-radius: 14px;
}

QFrame#EqControlPanel {
    background-color: %14;
    border: 1px solid %12;
    border-radius: 12px;
}

QPushButton#PresetBtn {
    background-color: %11;
    border: 1px solid %12;
    border-radius: 8px;
    padding: 8px 16px;
    color: %8;
    font-size: 13px;
}

QPushButton#PresetBtn:hover {
    background-color: %9;
    color: %7;
}

QPushButton#PresetBtn:checked {
    background-color: %4;
    border-color: %4;
    color: #FFFFFF;
    font-weight: bold;
}

QPushButton#ResetBtn {
    background-color: transparent;
    border: 1px solid %4;
    border-radius: 8px;
    padding: 8px 16px;
    color: %4;
    font-size: 13px;
}

QPushButton#ResetBtn:hover {
    background-color: %9;
    color: %7;
}

QMenu {
    background-color: %11;
    border: 1px solid %12;
    border-radius: 12px;
    padding: 6px;
}

QMenu::item {
    background-color: transparent;
    color: %2;
    border-radius: 6px;
    padding: 8px 16px;
    font-size: 13px;
}

QMenu::item:selected {
    background-color: %9;
    color: %10;
}

QMenu::separator {
    height: 1px;
    background: %12;
    margin: 4px 8px;
}
)")
    .arg(p.name)                           // %1
    .arg(p.secondaryText.name())           // %2
    .arg(p.tooltipBg.name())               // %3
    .arg(p.primaryAccent.name())           // %4
    .arg(p.windowBg.name())                // %5
    .arg(p.sidebarBg.name())               // %6
    .arg(p.primaryText.name())             // %7
    .arg(p.mutedText.name())               // %8
    .arg(p.itemHoverBg.name())             // %9
    .arg(p.secondaryAccent.name())         // %10
    .arg(p.headerBg.name())                // %11
    .arg(p.cardBorder.name())              // %12
    .arg(p.scrollbarHandle.name())         // %13
    .arg(p.queueBg.name());                // %14
}

QPixmap ThemeManager::defaultAlbumArt(int size) const {
    QPixmap cover(size, size);
    cover.fill(Qt::transparent);

    QPainter painter(&cover);
    painter.setRenderHint(QPainter::Antialiasing);
    painter.setRenderHint(QPainter::SmoothPixmapTransform);

    // Theme-aware dual-accent gradient tile background
    QLinearGradient bgGrad(0, 0, size, size);
    bgGrad.setColorAt(0.0, m_currentPalette.placeholderGradStart);
    bgGrad.setColorAt(1.0, m_currentPalette.placeholderGradEnd);
    painter.fillRect(0, 0, size, size, bgGrad);

    // Subtle inner border / glass glow
    painter.setPen(QPen(QColor(255, 255, 255, 45), 2));
    painter.setBrush(Qt::NoBrush);
    painter.drawRect(1, 1, size - 2, size - 2);

    // Center logo
    QPixmap logo(":/resources/icons/playtune_logo.png");
    if (!logo.isNull()) {
        int logoSize = size * 0.55;
        QPixmap scaledLogo = logo.scaled(logoSize, logoSize, Qt::KeepAspectRatio, Qt::SmoothTransformation);
        int x = (size - scaledLogo.width()) / 2;
        int y = (size - scaledLogo.height()) / 2;
        painter.drawPixmap(x, y, scaledLogo);
    }
    return cover;
}
