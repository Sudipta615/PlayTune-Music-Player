#include "apptheme.h"
#include "coverloader.h"
#include <QSettings>
#include <QDateTime>
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
    // ─── 1. Dark Mode (Default) ─────────────────────────────────────────────
    ThemePalette dark;
    dark.id = "dark";
    dark.name = "Dark Mode";
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
    dark.cardBgGradStart = QColor("#141828");
    dark.cardBgGradEnd   = QColor("#0C0F1E");
    m_themes["dark"] = dark;

    // ─── 2. Light Mode ──────────────────────────────────────────────────────
    ThemePalette light;
    light.id = "light";
    light.name = "Light Mode";
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
    light.cardBgGradStart = QColor("#1E1B38");
    light.cardBgGradEnd   = QColor("#100E24");
    m_themes["light"] = light;

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
    list.append(qMakePair(QString("dark"),  QString("Dark Mode")));
    list.append(qMakePair(QString("light"), QString("Light Mode")));
    return list;
}

void ThemeManager::setTheme(const QString& themeId) {
    if (!m_themes.contains(themeId)) {
        qWarning() << "ThemeManager::setTheme - Unknown theme ID:" << themeId;
        return;
    }

    if (m_currentPalette.id == themeId) {
        return; // Skip redundant theme switch
    }

    static bool s_isChangingTheme = false;
    static QString s_pendingThemeId;

    if (s_isChangingTheme) {
        s_pendingThemeId = themeId;
        return; // Queue the latest requested theme without crashing
    }
    s_isChangingTheme = true;

    qint64 t0 = QDateTime::currentMSecsSinceEpoch();
    qDebug() << "[ThemeManager] setTheme START for theme:" << themeId;

    m_currentPalette = m_themes[themeId];

    QSettings settings("PlayTune", "Settings");
    settings.setValue("theme_id", themeId);
    settings.setValue("theme_text", m_currentPalette.name);
    qDebug() << "[ThemeManager] settings saved in" << (QDateTime::currentMSecsSinceEpoch() - t0) << "ms";

    if (qApp) {
        QFont font("Outfit", 10);
        font.setStyleHint(QFont::SansSerif);
        qApp->setFont(font);

        QPalette pal = qApp->palette();
        pal.setColor(QPalette::Window, m_currentPalette.windowBg);
        pal.setColor(QPalette::WindowText, m_currentPalette.primaryText);
        pal.setColor(QPalette::Base, m_currentPalette.cardBg);
        pal.setColor(QPalette::AlternateBase, m_currentPalette.headerBg);
        pal.setColor(QPalette::Text, m_currentPalette.primaryText);
        pal.setColor(QPalette::Button, m_currentPalette.headerBg);
        pal.setColor(QPalette::ButtonText, m_currentPalette.primaryText);
        pal.setColor(QPalette::Highlight, m_currentPalette.primaryAccent);
        pal.setColor(QPalette::HighlightedText, QColor("#FFFFFF"));
        qApp->setPalette(pal);

        qint64 t1 = QDateTime::currentMSecsSinceEpoch();
        QWidgetList topLevels = QApplication::topLevelWidgets();
        for (QWidget* w : topLevels) {
            if (w) w->setUpdatesEnabled(false);
        }

        QString qss = generateStylesheet();
        qDebug() << "[ThemeManager] generateStylesheet took" << (QDateTime::currentMSecsSinceEpoch() - t1) << "ms";

        qint64 t2 = QDateTime::currentMSecsSinceEpoch();
        qApp->setStyleSheet(qss);
        qDebug() << "[ThemeManager] qApp->setStyleSheet took" << (QDateTime::currentMSecsSinceEpoch() - t2) << "ms";

        qint64 t3 = QDateTime::currentMSecsSinceEpoch();
        emit themeChanged(m_currentPalette);
        qDebug() << "[ThemeManager] emit themeChanged took" << (QDateTime::currentMSecsSinceEpoch() - t3) << "ms";

        qint64 t4 = QDateTime::currentMSecsSinceEpoch();
        for (QWidget* w : topLevels) {
            if (w) {
                w->setUpdatesEnabled(true);
                w->update();
            }
        }
        qDebug() << "[ThemeManager] topLevels update took" << (QDateTime::currentMSecsSinceEpoch() - t4) << "ms";
    } else {
        emit themeChanged(m_currentPalette);
    }
    qDebug() << "[ThemeManager] TOTAL setTheme time:" << (QDateTime::currentMSecsSinceEpoch() - t0) << "ms";
    s_isChangingTheme = false;

    if (!s_pendingThemeId.isEmpty()) {
        QString next = s_pendingThemeId;
        s_pendingThemeId.clear();
        QMetaObject::invokeMethod(this, [this, next]() {
            setTheme(next);
        }, Qt::QueuedConnection);
    }
}

// ─── Icon Tinting ───────────────────────────────────────────────────────────

QIcon ThemeManager::tintedIcon(const QString& resourcePath, const QColor& color) {
    static QHash<QString, QIcon> s_iconCache;
    QString cacheKey = resourcePath + "_" + color.name() + "_" + QString::number(color.alpha());
    auto it = s_iconCache.find(cacheKey);
    if (it != s_iconCache.end()) {
        return it.value();
    }
    QIcon tinted = tintedIcon(QIcon(resourcePath), color);
    s_iconCache.insert(cacheKey, tinted);
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

static QString buildStylesheetForTheme(const ThemePalette& p) {
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

    QString baseCss = QString(R"(
/* Dynamic PlayTune Stylesheet — Active Theme: %1 */

QMainWindow, QDialog {
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

QFrame#SongsCard {
    background-color: %12;
    border: 1px solid %13;
    border-radius: 16px;
}

QFrame#SettingsCard {
    background-color: %12;
    border: 1px solid %13;
    border-radius: 12px;
}

QFrame#SettingsDivider {
    background-color: %13;
    min-height: 1px;
    max-height: 1px;
    border: none;
}

QLabel#SettingsSectionHeader {
    font-size: 11px;
    font-weight: 700;
    color: %11;
    letter-spacing: 1px;
    border: none;
    background: transparent;
}

QLabel#SettingsTitleLabel {
    font-size: 14px;
    font-weight: 600;
    color: %7;
    border: none;
    background: transparent;
}

QLabel#SettingsSubLabel {
    font-size: 12px;
    color: %8;
    border: none;
    background: transparent;
}

QFrame#AlbumsCard, QFrame#ArtistsCard, QFrame#FoldersCard {
    background-color: %12;
    border: 1px solid %13;
    border-radius: 16px;
}

QLabel#ViewTitleLabel {
    font-size: 22px;
    font-weight: 600;
    color: %7;
    padding-bottom: 8px;
    border: none;
    background: transparent;
}

QPushButton#FavBtn {
    border: none;
    background: transparent;
    color: #7E8494;
    font-size: 16px;
}
QPushButton#FavBtn:hover {
    color: #FF2A7A;
}
QPushButton#FavBtn[favorite="true"] {
    color: #FF2A7A;
}

QFrame#CardFrame {
    background-color: %12;
    border: 1px solid %13;
    border-radius: 14px;
}
QFrame#CardFrame:hover {
    background-color: %9;
    border: 1px solid %4;
}
QFrame#CardFrame[playing="true"] {
    background-color: %10;
    border: 1px solid %11;
}

QLabel#CardTitleLabel {
    font-weight: 600;
    font-size: 11px;
    color: %2;
    background: transparent;
    border: none;
}
QLabel#CardTitleLabel[playing="true"] {
    color: %11;
}

QLabel#CardSubtitleLabel {
    font-size: 10px;
    color: %8;
    background: transparent;
    border: none;
}

QLabel#SongTitleLabel {
    font-size: 13px;
    font-weight: 500;
    color: %7;
    background: transparent;
    border: none;
}

QLabel#SongTitleLabel[playing="true"] {
    font-weight: bold;
    color: %11;
}

QPushButton#ToggleRightTopBtn {
    background-color: %12;
    color: %2;
    border: 1px solid %13;
    border-radius: 6px;
    font-size: 12px;
    font-weight: bold;
}
QPushButton#ToggleRightTopBtn:hover {
    background-color: %9;
    border-color: %4;
    color: %7;
}

QFrame#WindowSeparator {
    color: %27;
    background-color: %27;
    min-width: 1px;
    max-width: 1px;
    border: none;
}

QFrame#CardSeparator,
QFrame[frameShape="4"],
QFrame[frameShape="5"] {
    color: %27;
    background-color: %27;
    border: none;
    max-height: 1px;
    min-height: 1px;
    height: 1px;
}

QListWidget#SidebarPlaylists {
    background-color: transparent;
    border: none;
    color: %2;
    font-size: 13px;
    outline: none;
}
QListWidget#SidebarPlaylists::item {
    padding: 6px 8px;
    border-radius: 4px;
}
QListWidget#SidebarPlaylists::item:hover {
    background-color: %9;
    color: %7;
}
QListWidget#SidebarPlaylists::item:selected {
    background-color: %9;
    color: %11;
    font-weight: bold;
}

QLabel#MiniTitle {
    font-size: 13px;
    font-weight: bold;
    color: %7;
}
QLabel#MiniArtistAlbum {
    font-size: 11px;
    color: %8;
}
QLabel#QueueFooterLabel {
    color: %8;
    font-size: 11px;
    margin-left: 5px;
}
QLabel#QueueHeaderLabel {
    color: %8;
    font-size: 11px;
    font-weight: bold;
}
QLabel#QueueVolumeLabel {
    color: %8;
    font-size: 11px;
    min-width: 32px;
    font-weight: 500;
}
QListWidget#QueueLyricsList {
    background-color: %12;
    border-radius: 10px;
    border: 1px solid %13;
    outline: 0;
}
QListWidget#QueueLyricsList::item {
    padding: 10px 8px;
}
QListWidget#QueueLyricsList::item:hover {
    background: %9;
    border-radius: 6px;
}
QListWidget#QueueLyricsList::item:selected {
    background: transparent;
}
QLabel#QueueUnsyncedLyrics {
    background-color: %12;
    border-radius: 10px;
    border: 1px solid %13;
    padding: 20px;
    color: %7;
    font-size: 13px;
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
    .arg(npGrooveColor)             // %26
    .arg(p.separatorColor.name());  // %27

    QString moodPillCss;
    if (p.isLight) {
        moodPillCss = QStringLiteral(
"QLabel#SongMoodBadge {\n"
"    border-radius: 6px;\n"
"    padding: 3px 8px;\n"
"    font-size: 10px;\n"
"    font-weight: bold;\n"
"    background-color: rgba(124, 58, 237, 0.16);\n"
"    border: 1px solid rgba(124, 58, 237, 0.40);\n"
"    color: #6D28D9;\n"
"}\n"
"QLabel#SongMoodBadge[mood=\"energetic\"], QLabel#SongMoodBadge[mood=\"sleep\"], QLabel#SongMoodBadge[mood=\"lofi\"] {\n"
"    background-color: rgba(124, 58, 237, 0.16);\n"
"    border: 1px solid rgba(124, 58, 237, 0.40);\n"
"    color: #6D28D9;\n"
"}\n"
"QLabel#SongMoodBadge[mood=\"romantic\"] {\n"
"    background-color: rgba(236, 72, 153, 0.16);\n"
"    border: 1px solid rgba(236, 72, 153, 0.40);\n"
"    color: #BE185D;\n"
"}\n"
"QLabel#SongMoodBadge[mood=\"happy\"] {\n"
"    background-color: rgba(234, 179, 8, 0.22);\n"
"    border: 1px solid rgba(202, 138, 4, 0.55);\n"
"    color: #854D0E;\n"
"}\n"
"QLabel#SongMoodBadge[mood=\"calm\"] {\n"
"    background-color: rgba(6, 182, 212, 0.16);\n"
"    border: 1px solid rgba(6, 182, 212, 0.40);\n"
"    color: #0369A1;\n"
"}\n"
"QLabel#SongMoodBadge[mood=\"party\"] {\n"
"    background-color: rgba(168, 85, 247, 0.16);\n"
"    border: 1px solid rgba(168, 85, 247, 0.40);\n"
"    color: #7E22CE;\n"
"}\n"
"QLabel#SongMoodBadge[mood=\"nostalgic\"] {\n"
"    background-color: rgba(217, 119, 6, 0.16);\n"
"    border: 1px solid rgba(217, 119, 6, 0.40);\n"
"    color: #C2410C;\n"
"}\n"
"QLabel#SongMoodBadge[mood=\"sad\"] {\n"
"    background-color: rgba(99, 102, 241, 0.16);\n"
"    border: 1px solid rgba(99, 102, 241, 0.40);\n"
"    color: #4338CA;\n"
"}\n"
        );
    } else {
        moodPillCss = QStringLiteral(
"QLabel#SongMoodBadge {\n"
"    border-radius: 6px;\n"
"    padding: 3px 8px;\n"
"    font-size: 10px;\n"
"    font-weight: bold;\n"
"    background-color: rgba(168, 85, 247, 0.22);\n"
"    border: 1px solid rgba(192, 132, 252, 0.65);\n"
"    color: #F3E8FF;\n"
"}\n"
"QLabel#SongMoodBadge[mood=\"energetic\"], QLabel#SongMoodBadge[mood=\"sleep\"], QLabel#SongMoodBadge[mood=\"lofi\"] {\n"
"    background-color: rgba(124, 58, 237, 0.25);\n"
"    border: 1px solid rgba(167, 139, 250, 0.70);\n"
"    color: #E9D5FF;\n"
"}\n"
"QLabel#SongMoodBadge[mood=\"romantic\"] {\n"
"    background-color: rgba(236, 72, 153, 0.25);\n"
"    border: 1px solid rgba(244, 114, 182, 0.70);\n"
"    color: #FBCFE8;\n"
"}\n"
"QLabel#SongMoodBadge[mood=\"happy\"] {\n"
"    background-color: rgba(234, 179, 8, 0.25);\n"
"    border: 1px solid rgba(250, 204, 21, 0.70);\n"
"    color: #FEF08A;\n"
"}\n"
"QLabel#SongMoodBadge[mood=\"calm\"] {\n"
"    background-color: rgba(6, 182, 212, 0.25);\n"
"    border: 1px solid rgba(56, 189, 248, 0.70);\n"
"    color: #BAE6FD;\n"
"}\n"
"QLabel#SongMoodBadge[mood=\"party\"] {\n"
"    background-color: rgba(168, 85, 247, 0.25);\n"
"    border: 1px solid rgba(192, 132, 252, 0.70);\n"
"    color: #F3E8FF;\n"
"}\n"
"QLabel#SongMoodBadge[mood=\"nostalgic\"] {\n"
"    background-color: rgba(217, 119, 6, 0.25);\n"
"    border: 1px solid rgba(251, 146, 60, 0.70);\n"
"    color: #FFEDD5;\n"
"}\n"
"QLabel#SongMoodBadge[mood=\"sad\"] {\n"
"    background-color: rgba(99, 102, 241, 0.25);\n"
"    border: 1px solid rgba(129, 140, 248, 0.70);\n"
"    color: #E0E7FF;\n"
"}\n"
        );
    }

    return baseCss + "\n" + moodPillCss;
}

QString ThemeManager::generateStylesheet() const {
    static QString s_darkQss;
    static QString s_lightQss;

    if (m_currentPalette.isLight) {
        if (s_lightQss.isEmpty()) {
            s_lightQss = buildStylesheetForTheme(m_themes["light"]);
        }
        return s_lightQss;
    } else {
        if (s_darkQss.isEmpty()) {
            s_darkQss = buildStylesheetForTheme(m_themes["dark"]);
        }
        return s_darkQss;
    }
}

QPixmap ThemeManager::defaultAlbumArt(int size) const {
    static QPixmap s_baseCover;
    if (s_baseCover.isNull()) {
        s_baseCover.load(":/resources/images/placeholder_cover.png");
        if (s_baseCover.isNull()) {
            s_baseCover.load(":/resources/icons/logo.png");
        }
        if (s_baseCover.isNull()) {
            s_baseCover.load(":/resources/icons/playtune_logo.png");
        }
    }

    if (s_baseCover.isNull()) {
        return QPixmap();
    }

    if (size <= 0 || (s_baseCover.width() == size && s_baseCover.height() == size)) {
        return s_baseCover;
    }

    return s_baseCover.scaled(size, size, Qt::KeepAspectRatioByExpanding, Qt::SmoothTransformation);
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
