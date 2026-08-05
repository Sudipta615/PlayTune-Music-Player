#include "apptheme.h"
#include <QSettings>
#include <QApplication>
#include <QWidget>
#include <QPainter>
#include <QLinearGradient>
#include <QImage>
#include <QPixmap>
#include <QDebug>
#include <QComboBox>
#include <QListView>
#include <QStyledItemDelegate>

ThemeManager& ThemeManager::instance() {
    static ThemeManager inst;
    return inst;
}

ThemeManager::ThemeManager(QObject* parent) : QObject(parent) {
    registerThemes();
    loadSavedTheme();
}

void ThemeManager::registerThemes() {
    // ─── 1. Dark Premium (Purple) ───────────────────────────────────────────
    ThemePalette dark;
    dark.id = "dark";
    dark.name = "Dark Premium (Purple)";
    dark.isLight = false;
    dark.windowBg        = QColor("#0C0F1A");
    dark.sidebarBg       = QColor("#090C1C");
    dark.queueBg         = QColor("#080B18");
    dark.cardBg          = QColor("#111525");
    dark.cardBorder      = QColor("#1C2240");
    dark.headerBg        = QColor("#0D1120");
    dark.separatorColor  = QColor("#1A2038");
    dark.primaryText     = QColor("#F0F2FF");
    dark.secondaryText   = QColor("#C8CCDF");
    dark.mutedText       = QColor("#636B84");
    dark.primaryAccent   = QColor("#8B2FC9");   // Rich deep purple
    dark.secondaryAccent = QColor("#FF2D78");   // Vivid hot pink
    dark.iconColor       = QColor("#E0E4FF");   // Near-white with slight blue tint
    dark.itemHoverBg     = QColor("#221A46");   // Distinct purple-tinted hover
    dark.itemSelectedBg  = QColor("#38276D");
    dark.scrollbarHandle = QColor("#222842");
    dark.tooltipBg       = QColor("#0D1120");
    dark.tooltipBorder   = QColor("#8B2FC9");
    dark.placeholderGradStart = QColor("#8B2FC9");
    dark.placeholderGradEnd   = QColor("#FF2D78");
    dark.cardBgGradStart = QColor("#141828");
    dark.cardBgGradEnd   = QColor("#0C0F1E");
    m_themes["dark"] = dark;

    // ─── 2. Light Premium ──────────────────────────────────────────────────
    ThemePalette light;
    light.id = "light";
    light.name = "Light Premium";
    light.isLight = true;
    light.windowBg       = QColor("#F4F6FB");   // Soft cool white, NOT pure white
    light.sidebarBg      = QColor("#EAEEF8");   // Slightly blue-grey sidebar
    light.queueBg        = QColor("#ECF0F9");   // Queue panel: soft lavender-grey
    light.cardBg         = QColor("#FFFFFF");   // Cards pure white for contrast
    light.cardBorder     = QColor("#D8DDED");   // Soft blue-grey border
    light.headerBg       = QColor("#F0F3FA");   // Search bar background
    light.separatorColor = QColor("#D4D9EC");
    light.primaryText    = QColor("#0F1630");   // Deep navy-almost-black
    light.secondaryText  = QColor("#2D3558");   // Medium dark navy
    light.mutedText      = QColor("#7882A8");   // Medium blue-grey muted
    light.primaryAccent  = QColor("#6C2BD9");   // Rich violet (readable on white)
    light.secondaryAccent= QColor("#E91E8C");   // Vivid magenta-pink
    light.iconColor      = QColor("#2D3558");   // Dark navy for icon tinting in light mode
    light.itemHoverBg    = QColor("#E2E8F8");   // Soft blue hover
    light.itemSelectedBg = QColor("#CBD6F6");   // Slightly deeper blue selection
    light.scrollbarHandle= QColor("#B8C3DF");
    light.tooltipBg      = QColor("#1A2040");   // Dark tooltip on light bg
    light.tooltipBorder  = QColor("#6C2BD9");
    light.placeholderGradStart = QColor("#6C2BD9");
    light.placeholderGradEnd   = QColor("#E91E8C");
    light.cardBgGradStart = QColor("#1E1B38");
    light.cardBgGradEnd   = QColor("#100E24");
    m_themes["light"] = light;

    // ─── 3. Emerald Green ──────────────────────────────────────────────────
    ThemePalette teal;
    teal.id = "teal";
    teal.name = "Emerald Green";
    teal.isLight = false;
    teal.windowBg       = QColor("#050F0C");   // Very deep emerald black
    teal.sidebarBg      = QColor("#030C09");
    teal.queueBg        = QColor("#030B08");
    teal.cardBg         = QColor("#081810");
    teal.cardBorder     = QColor("#0E2D1C");
    teal.headerBg       = QColor("#061410");
    teal.separatorColor = QColor("#0C2818");
    teal.primaryText    = QColor("#E8FFF6");
    teal.secondaryText  = QColor("#B8E8D4");
    teal.mutedText      = QColor("#3D7A5C");
    teal.primaryAccent  = QColor("#00C876");   // Rich emerald green
    teal.secondaryAccent= QColor("#00FFB0");   // Bright mint highlight
    teal.iconColor      = QColor("#C8F5E2");   // Mint-white icon tint
    teal.itemHoverBg    = QColor("#0E3B24");   // Rich emerald hover
    teal.itemSelectedBg = QColor("#165938");
    teal.scrollbarHandle= QColor("#112E1E");
    teal.tooltipBg      = QColor("#071A10");
    teal.tooltipBorder  = QColor("#00C876");
    teal.placeholderGradStart = QColor("#00C876");
    teal.placeholderGradEnd   = QColor("#00FFB0");
    teal.cardBgGradStart = QColor("#091F14");
    teal.cardBgGradEnd   = QColor("#040E09");
    m_themes["teal"] = teal;

    // ─── 4. Sunset Amber ───────────────────────────────────────────────────
    ThemePalette amber;
    amber.id = "amber";
    amber.name = "Sunset Amber";
    amber.isLight = false;
    amber.windowBg       = QColor("#0F0900");   // Ultra-deep warm black
    amber.sidebarBg      = QColor("#0C0700");
    amber.queueBg        = QColor("#0B0600");
    amber.cardBg         = QColor("#180E00");
    amber.cardBorder     = QColor("#2D1E00");
    amber.headerBg       = QColor("#140B00");
    amber.separatorColor = QColor("#261900");
    amber.primaryText    = QColor("#FFF8EC");
    amber.secondaryText  = QColor("#F0D9B0");
    amber.mutedText      = QColor("#7A5A2A");
    amber.primaryAccent  = QColor("#FF7700");   // Vivid burnt orange
    amber.secondaryAccent= QColor("#FFB300");   // Rich golden amber
    amber.iconColor      = QColor("#FFE4A0");   // Warm golden icon tint
    amber.itemHoverBg    = QColor("#382200");   // Rich amber hover
    amber.itemSelectedBg = QColor("#543300");
    amber.scrollbarHandle= QColor("#302010");
    amber.tooltipBg      = QColor("#1A0F00");
    amber.tooltipBorder  = QColor("#FF7700");
    amber.placeholderGradStart = QColor("#FF7700");
    amber.placeholderGradEnd   = QColor("#FFB300");
    amber.cardBgGradStart = QColor("#221400");
    amber.cardBgGradEnd   = QColor("#100800");
    m_themes["amber"] = amber;

    // ─── 5. Electric Cyan ──────────────────────────────────────────────────
    ThemePalette cyan;
    cyan.id = "cyan";
    cyan.name = "Electric Cyan";
    cyan.isLight = false;
    cyan.windowBg       = QColor("#030A14");
    cyan.sidebarBg      = QColor("#02070F");
    cyan.queueBg        = QColor("#020610");
    cyan.cardBg         = QColor("#07111E");
    cyan.cardBorder     = QColor("#0E2038");
    cyan.headerBg       = QColor("#050E1A");
    cyan.separatorColor = QColor("#0C2040");
    cyan.primaryText    = QColor("#E8F4FF");
    cyan.secondaryText  = QColor("#B0D0F0");
    cyan.mutedText      = QColor("#3A5F8A");
    cyan.primaryAccent  = QColor("#00CFFF");   // Electric cyan
    cyan.secondaryAccent= QColor("#1A6EFF");   // Sapphire blue
    cyan.iconColor      = QColor("#A8E8FF");   // Cyan-white icon tint
    cyan.itemHoverBg    = QColor("#0E2E52");   // Rich cyan hover
    cyan.itemSelectedBg = QColor("#16467A");
    cyan.scrollbarHandle= QColor("#102840");
    cyan.tooltipBg      = QColor("#040F20");
    cyan.tooltipBorder  = QColor("#00CFFF");
    cyan.placeholderGradStart = QColor("#00CFFF");
    cyan.placeholderGradEnd   = QColor("#1A6EFF");
    cyan.cardBgGradStart = QColor("#091828");
    cyan.cardBgGradEnd   = QColor("#040C18");
    m_themes["cyan"] = cyan;

    // ─── 6. Crimson Rose ───────────────────────────────────────────────────
    ThemePalette crimson;
    crimson.id = "crimson";
    crimson.name = "Crimson Rose";
    crimson.isLight = false;
    crimson.windowBg       = QColor("#100306");   // Ultra-deep crimson black
    crimson.sidebarBg      = QColor("#0D0204");
    crimson.queueBg        = QColor("#0C0204");
    crimson.cardBg         = QColor("#1A060C");
    crimson.cardBorder     = QColor("#320D18");
    crimson.headerBg       = QColor("#150408");
    crimson.separatorColor = QColor("#2E0C16");
    crimson.primaryText    = QColor("#FFE8EF");
    crimson.secondaryText  = QColor("#F0BFD0");
    crimson.mutedText      = QColor("#7A3048");
    crimson.primaryAccent  = QColor("#FF1A4A");   // Deep crimson-red
    crimson.secondaryAccent= QColor("#FF4E90");   // Rose-pink highlight
    crimson.iconColor      = QColor("#FFB8CC");   // Rose-white icon tint
    crimson.itemHoverBg    = QColor("#3B0B1A");   // Rich crimson hover
    crimson.itemSelectedBg = QColor("#5C1229");
    crimson.scrollbarHandle= QColor("#380E1A");
    crimson.tooltipBg      = QColor("#1C0508");
    crimson.tooltipBorder  = QColor("#FF1A4A");
    crimson.placeholderGradStart = QColor("#FF1A4A");
    crimson.placeholderGradEnd   = QColor("#FF4E90");
    crimson.cardBgGradStart = QColor("#230A12");
    crimson.cardBgGradEnd   = QColor("#100306");
    m_themes["crimson"] = crimson;

    m_currentPalette = m_themes["dark"];
}

void ThemeManager::loadSavedTheme() {
    QSettings settings("PlayTune", "Settings");
    QString themeId = settings.value("theme_id", "dark").toString();
    if (!m_themes.contains(themeId)) {
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
    list.append(qMakePair(QString("dark"),    QString("Dark Premium (Purple)")));
    list.append(qMakePair(QString("light"),   QString("Light Premium")));
    list.append(qMakePair(QString("teal"),    QString("Emerald Green")));
    list.append(qMakePair(QString("amber"),   QString("Sunset Amber")));
    list.append(qMakePair(QString("cyan"),    QString("Electric Cyan")));
    list.append(qMakePair(QString("crimson"), QString("Crimson Rose")));
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
        QWidgetList topLevels = QApplication::topLevelWidgets();
        for (QWidget* w : topLevels) {
            if (w) w->setUpdatesEnabled(false);
        }

        qApp->setStyleSheet(generateStylesheet());

        for (QWidget* w : topLevels) {
            if (w) {
                w->setUpdatesEnabled(true);
                w->update();
            }
        }
    }

    emit themeChanged(m_currentPalette);
}

// ─── Icon Tinting ───────────────────────────────────────────────────────────

QIcon ThemeManager::tintedIcon(const QString& resourcePath, const QColor& color) {
    static QMap<QString, QIcon> s_iconCache;
    QString cacheKey = resourcePath + "_" + color.name() + "_" + QString::number(color.alpha());
    if (s_iconCache.contains(cacheKey)) {
        return s_iconCache[cacheKey];
    }
    QIcon tinted = tintedIcon(QIcon(resourcePath), color);
    s_iconCache[cacheKey] = tinted;
    return tinted;
}

QIcon ThemeManager::tintedIcon(const QIcon& source, const QColor& color) {
    if (source.isNull()) return source;
    // Get the largest pixmap available
    QPixmap src = source.pixmap(128, 128);
    if (src.isNull()) return source;

    QImage img = src.toImage().convertToFormat(QImage::Format_ARGB32);
    int w = img.width();
    int h = img.height();

    int tr = color.red();
    int tg = color.green();
    int tb = color.blue();

    for (int y = 0; y < h; ++y) {
        QRgb* line = reinterpret_cast<QRgb*>(img.scanLine(y));
        for (int x = 0; x < w; ++x) {
            int alpha = qAlpha(line[x]);
            if (alpha > 0) {
                // Replace all opaque pixels with the target color, preserving alpha
                line[x] = qRgba(tr, tg, tb, alpha);
            }
        }
    }

    QIcon result;
    // Build icon at several sizes for crisp rendering
    for (int s : {16, 18, 22, 24, 32}) {
        result.addPixmap(QPixmap::fromImage(img.scaled(s, s,
            Qt::KeepAspectRatio, Qt::SmoothTransformation)));
    }
    return result;
}

// ─── Stylesheet Generation ──────────────────────────────────────────────────

QString ThemeManager::generateStylesheet() const {
    const auto& p = m_currentPalette;

    // Light-mode conditional values for the NowPlayingCard labels
    // On dark themes: labels have dark-glass backgrounds and white text
    // On light themes: labels are clean pill-shaped with theme colors
    QString npArtistBg, npArtistBorder, npArtistColor;
    QString npAlbumBg, npAlbumBorder, npAlbumColor;
    QString npTimeBg, npTimeBorder, npTimeColor;
    QString npTitleColor;
    QString npGrooveColor;

    npTitleColor    = "#FFFFFF";
    npArtistBg      = "rgba(8, 10, 20, 0.50)";
    npArtistBorder  = "rgba(255, 255, 255, 0.15)";
    npArtistColor   = "#F0F2FF";
    npAlbumBg       = "rgba(8, 10, 20, 0.38)";
    npAlbumBorder   = "rgba(255, 255, 255, 0.12)";
    npAlbumColor    = "#D0D5F0";
    npTimeBg        = "rgba(8, 10, 20, 0.58)";
    npTimeBorder    = "rgba(255, 255, 255, 0.18)";
    npTimeColor     = "#FFFFFF";
    npGrooveColor   = "rgba(255, 255, 255, 0.28)";

    // Sidebar checked state for light mode: use accent bg tint instead of pure accent fill
    QString sidebarCheckedBg = p.isLight
        ? p.itemSelectedBg.name()
        : p.itemHoverBg.name();
    QString sidebarCheckedColor = p.secondaryAccent.name();

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
    background-color: %10;
    color: %11;
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
    background-color: %12;
    border: 1px solid %13;
    border-radius: 10px;
    color: %7;
    padding: 8px 12px;
    font-size: 13px;
}

QLineEdit#SearchBar:placeholder {
    color: %8;
}

QLineEdit#SearchBar:hover {
    border: 1px solid %4;
}

QLineEdit#SearchBar:focus {
    border: 1px solid %4;
}

QLabel#QueueRowTitleLabel {
    font-size: 12px;
    font-weight: 500;
    color: %7;
    background: transparent;
    margin: 0px;
    padding: 0px;
}

QLabel#QueueRowArtistLabel {
    font-size: 10px;
    color: %8;
    background: transparent;
    margin: 0px;
    padding: 0px;
}

QLabel#NowPlayingTitle {
    font-size: 22px;
    font-weight: bold;
    color: %14;
}

QLabel#NowPlayingArtist {
    font-size: 12px;
    font-weight: 500;
    color: %15;
    background-color: %16;
    border: 1px solid %17;
    border-radius: 6px;
    padding: 3px 8px;
}

QLabel#NowPlayingAlbum {
    font-size: 12px;
    font-weight: 500;
    color: %18;
    background-color: %19;
    border: 1px solid %20;
    border-radius: 6px;
    padding: 3px 8px;
}

QLabel#TimeLabel {
    font-size: 11px;
    font-weight: 600;
    color: %21;
    background-color: %22;
    border: 1px solid %23;
    border-radius: 6px;
    padding: 3px 6px;
}

QPushButton#MediaControlBtn {
    background: transparent;
    border: none;
    border-radius: 6px;
    padding: 6px;
}

QPushButton#MediaControlBtn:hover {
    background-color: %4;
    border-radius: 8px;
}

QPushButton#MediaControlBtn:checked {
    background-color: %4;
    border-radius: 8px;
}

QPushButton#MediaControlBtn:checked:hover {
    background-color: %4;
}

QPushButton#PlayPauseBtn {
    background-color: %4;
    border: none;
    border-radius: 20px;
    padding: 8px;
}

QPushButton#PlayPauseBtn:hover {
    background-color: %4;
    opacity: 0.85;
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
    background-color: %12;
    border: 1px solid %13;
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
    background-color: %12;
    border: 1px solid %13;
    border-radius: 8px;
    color: %7;
    outline: none;
    padding: 4px;
    selection-background-color: %9;
    selection-color: %11;
    show-decoration-selected: 1;
}

QComboBox QAbstractItemView::item {
    padding: 7px 12px;
    border-radius: 5px;
    min-height: 26px;
    color: %2;
    background-color: transparent;
}

QComboBox QAbstractItemView::item:hover,
QComboBox QAbstractItemView::item:selected {
    background-color: %9;
    color: %11;
    font-weight: bold;
}

QSpinBox, QDoubleSpinBox {
    background-color: %12;
    color: %7;
    border: 1px solid %13;
    border-radius: 8px;
    padding: 4px 8px;
    padding-right: 24px;
    font-size: 13px;
    font-weight: 500;
}

QSpinBox:focus, QDoubleSpinBox:focus {
    border-color: %4;
}

QSpinBox::up-button, QDoubleSpinBox::up-button {
    subcontrol-origin: border;
    subcontrol-position: top right;
    width: 22px;
    height: 14px;
    border-left: 1px solid %13;
    border-bottom: 1px solid %13;
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
    border-left: 1px solid %13;
    border-bottom-right-radius: 7px;
    background-color: %9;
}

QSpinBox::down-button:hover, QDoubleSpinBox::down-button:hover {
    background-color: %4;
}

QPushButton#IconButton {
    background-color: %12;
    border: 1px solid %13;
    border-radius: 8px;
    padding: 6px;
}

QPushButton#IconButton:hover {
    background-color: %9;
}

QPushButton#IconButton:checked {
    background-color: %9;
    border-color: %4;
}

QTableWidget {
    background-color: transparent;
    gridline-color: transparent;
    border: none;
    outline: none;
}

QTableWidget::item {
    border-bottom: 1px solid %13;
    padding: 4px 10px;
    outline: none;
}

QTableWidget::item:focus, QTableWidget::item:selected {
    border: none;
    outline: none;
    background-color: transparent;
}

QHeaderView {
    background-color: %12;
    border-radius: 10px;
    border: 1px solid %13;
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
    background: %24;
    min-height: 20px;
    border-radius: 3px;
}

QScrollBar::handle:vertical:hover {
    background: %4;
}

QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {
    border: none;
    background: none;
    height: 0px;
}

QScrollBar:horizontal {
    border: none;
    background: transparent;
    height: 6px;
    margin: 0px;
}

QScrollBar::handle:horizontal {
    background: %24;
    min-width: 20px;
    border-radius: 3px;
}

QScrollBar::handle:horizontal:hover {
    background: %4;
}

QScrollBar::add-line:horizontal, QScrollBar::sub-line:horizontal {
    border: none;
    background: none;
    width: 0px;
}

QWidget#RightSidebarFrame {
    background-color: %25;
    border: none;
}

QFrame#TabContainer {
    background-color: %12;
    border: 1px solid %13;
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
    background: %13;
    border-radius: 2px;
}

QSlider::groove:horizontal:hover {
    background: %13;
}

QSlider::sub-page:horizontal {
    background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 %4, stop:1 %11);
    border-radius: 2px;
}

QSlider::sub-page:horizontal:disabled {
    background: %24;
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
    background: %11;
}

QSlider::handle:horizontal:disabled {
    background: %8;
}

QFrame#NowPlayingCard QSlider::groove:horizontal {
    height: 4px;
    background: %26;
    border-radius: 2px;
}

QFrame#NowPlayingCard QSlider::groove:horizontal:hover {
    background: %26;
}

#EqualizerWindow {
    background-color: %5;
    border: 1.5px solid %13;
    border-radius: 14px;
}

QFrame#EqControlPanel {
    background-color: %25;
    border: 1px solid %13;
    border-radius: 12px;
}

QPushButton#PresetBtn {
    background-color: %12;
    border: 1px solid %13;
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

QPushButton#PresetBtn:disabled {
    background-color: %5;
    border-color: %13;
    color: %8;
    opacity: 0.5;
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

QPushButton#ResetBtn:disabled {
    border-color: %13;
    color: %8;
}

QMenu {
    background-color: %12;
    border: 1px solid %13;
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
    color: %11;
}

QMenu::separator {
    height: 1px;
    background: %13;
    margin: 4px 8px;
}
)")
    .arg(p.name)                    // %1
    .arg(p.secondaryText.name())    // %2
    .arg(p.tooltipBg.name())        // %3
    .arg(p.primaryAccent.name())    // %4
    .arg(p.windowBg.name())         // %5
    .arg(p.sidebarBg.name())        // %6
    .arg(p.primaryText.name())      // %7
    .arg(p.mutedText.name())        // %8
    .arg(p.itemHoverBg.name())      // %9
    .arg(sidebarCheckedBg)          // %10
    .arg(p.secondaryAccent.name())  // %11
    .arg(p.headerBg.name())         // %12
    .arg(p.cardBorder.name())       // %13
    .arg(npTitleColor)              // %14
    .arg(npArtistColor)             // %15
    .arg(npArtistBg)                // %16
    .arg(npArtistBorder)            // %17
    .arg(npAlbumColor)              // %18
    .arg(npAlbumBg)                 // %19
    .arg(npAlbumBorder)             // %20
    .arg(npTimeColor)               // %21
    .arg(npTimeBg)                  // %22
    .arg(npTimeBorder)              // %23
    .arg(p.scrollbarHandle.name())  // %24
    .arg(p.queueBg.name())          // %25
    .arg(npGrooveColor);            // %26
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
    painter.setPen(QPen(QColor(255, 255, 255, 40), 1.5));
    painter.setBrush(Qt::NoBrush);
    painter.drawRect(1, 1, size - 2, size - 2);

    // Center logo
    QPixmap logo(":/resources/icons/playtune_logo.png");
    if (!logo.isNull()) {
        int logoSize = size * 0.52;
        QPixmap scaledLogo = logo.scaled(logoSize, logoSize, Qt::KeepAspectRatio, Qt::SmoothTransformation);
        int x = (size - scaledLogo.width()) / 2;
        int y = (size - scaledLogo.height()) / 2;
        painter.drawPixmap(x, y, scaledLogo);
    }
    return cover;
}

// ─── Custom ComboBox Delegate with Hover Support ─────────────────────────────

class ComboBoxItemDelegate : public QStyledItemDelegate {
public:
    explicit ComboBoxItemDelegate(QObject* parent = nullptr) : QStyledItemDelegate(parent) {}

    void paint(QPainter* painter, const QStyleOptionViewItem& option, const QModelIndex& index) const override {
        QStyleOptionViewItem opt = option;
        initStyleOption(&opt, index);

        painter->save();
        painter->setRenderHint(QPainter::Antialiasing);

        const auto& palette = ThemeManager::instance().currentTheme();
        bool isHoveredOrSelected = (opt.state & QStyle::State_Selected) || (opt.state & QStyle::State_MouseOver);

        // Fill item background
        QRect bgRect = opt.rect.adjusted(2, 2, -2, -2);
        if (isHoveredOrSelected) {
            painter->setPen(Qt::NoPen);
            painter->setBrush(palette.itemSelectedBg.isValid() ? palette.itemSelectedBg : palette.itemHoverBg);
            painter->drawRoundedRect(bgRect, 6, 6);
        } else {
            painter->setPen(Qt::NoPen);
            painter->setBrush(Qt::transparent);
            painter->drawRoundedRect(bgRect, 6, 6);
        }

        // Draw icon if present
        QVariant iconVar = index.data(Qt::DecorationRole);
        int textLeftMargin = 12;
        if (iconVar.canConvert<QIcon>()) {
            QIcon icon = qvariant_cast<QIcon>(iconVar);
            if (!icon.isNull()) {
                int iconSize = 16;
                int iconY = opt.rect.top() + (opt.rect.height() - iconSize) / 2;
                icon.paint(painter, opt.rect.left() + 10, iconY, iconSize, iconSize);
                textLeftMargin = 34;
            }
        }

        // Draw text
        QRect textRect = opt.rect.adjusted(textLeftMargin, 0, -12, 0);
        QFont font = opt.font;
        if (isHoveredOrSelected) font.setBold(true);
        painter->setFont(font);

        if (isHoveredOrSelected) {
            painter->setPen(palette.secondaryAccent.isValid() ? palette.secondaryAccent : palette.primaryAccent);
        } else {
            painter->setPen(palette.primaryText);
        }

        QString text = index.data(Qt::DisplayRole).toString();
        painter->drawText(textRect, Qt::AlignLeft | Qt::AlignVCenter, text);

        painter->restore();
    }

    QSize sizeHint(const QStyleOptionViewItem& option, const QModelIndex& index) const override {
        QSize sz = QStyledItemDelegate::sizeHint(option, index);
        sz.setHeight(qMax(sz.height(), 34));
        return sz;
    }
};

void ThemeManager::setupComboBox(QComboBox* combo) {
    if (!combo) return;

    QAbstractItemView* view = combo->view();
    if (view) {
        view->setObjectName("ComboPopupView");
        view->setMouseTracking(true);
        if (view->viewport()) {
            view->viewport()->setAttribute(Qt::WA_Hover, true);
            view->viewport()->setMouseTracking(true);
        }

        auto updatePopupStyle = [view](const ThemePalette& p) {
            if (!view) return;
            view->setStyleSheet(QString(
                "QAbstractItemView {"
                "  background-color: %1;"
                "  border: 1.5px solid %2;"
                "  border-radius: 10px;"
                "  outline: none;"
                "  padding: 4px;"
                "  selection-background-color: %3;"
                "  selection-color: %4;"
                "}"
            ).arg(p.headerBg.name(), p.cardBorder.name(), p.itemSelectedBg.name(), p.secondaryAccent.name()));
        };

        updatePopupStyle(ThemeManager::instance().currentTheme());

        connect(&ThemeManager::instance(), &ThemeManager::themeChanged, view, [updatePopupStyle, view](const ThemePalette& p) {
            updatePopupStyle(p);
            if (view->viewport()) view->viewport()->update();
        });
    }

    combo->setItemDelegate(new ComboBoxItemDelegate(combo));
}
