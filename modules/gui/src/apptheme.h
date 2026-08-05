#ifndef APPTHEME_H
#define APPTHEME_H

#include <QObject>
#include <QColor>
#include <QString>
#include <QPixmap>
#include <QMap>
#include <QList>
#include <QPair>

struct ThemePalette {
    QString id;                 // e.g. "light", "dark", "teal", "amber", "cyan", "crimson"
    QString name;               // e.g. "Light Premium", "Dark Premium (Default)", etc.
    bool isLight = false;

    // Main App Surface Backgrounds
    QColor windowBg;            // Center panel / App window background
    QColor sidebarBg;           // Left sidebar background
    QColor queueBg;             // Right sidebar background
    QColor cardBg;              // Settings card, Dialog container background
    QColor cardBorder;          // Card / Frame border color
    QColor headerBg;            // Header / SearchBar background
    QColor separatorColor;      // Layout divider line color

    // Text & Labels
    QColor primaryText;         // Main headers, titles
    QColor secondaryText;       // Subtitles, table rows, artist/album
    QColor mutedText;           // Header uppercase labels, placeholder text, disabled text

    // Dual Accents
    QColor primaryAccent;       // Main accent color
    QColor secondaryAccent;     // Secondary accent color

    // Interactive States & Components
    QColor itemHoverBg;         // Table/list row hover color
    QColor itemSelectedBg;      // Table/list row selection color
    QColor scrollbarHandle;     // Scrollbar handle
    QColor tooltipBg;           // QToolTip background
    QColor tooltipBorder;       // QToolTip border

    // Artwork & Placeholder Gradients
    QColor placeholderGradStart;
    QColor placeholderGradEnd;
    QColor cardBgGradStart;     // Fallback NowPlayingCard background gradient start
    QColor cardBgGradEnd;       // Fallback NowPlayingCard background gradient end
};

class ThemeManager : public QObject {
    Q_OBJECT
public:
    static ThemeManager& instance();

    const ThemePalette& currentTheme() const { return m_currentPalette; }
    QString currentThemeId() const { return m_currentPalette.id; }
    
    // Returns available themes (id -> display name)
    QList<QPair<QString, QString>> availableThemes() const;

    // Switch theme by ID ("light", "dark", "teal", "amber", "cyan", "crimson")
    void setTheme(const QString& themeId);

    // Generate dynamic application QSS string for current active theme
    QString generateStylesheet() const;

    // Generate theme-aware default album artwork placeholder
    QPixmap defaultAlbumArt(int size = 300) const;

signals:
    void themeChanged(const ThemePalette& palette);

private:
    explicit ThemeManager(QObject* parent = nullptr);
    void registerThemes();
    void loadSavedTheme();

    QMap<QString, ThemePalette> m_themes;
    QList<QString> m_themesOrder;   // registration order (drives availableThemes())
    ThemePalette m_currentPalette;
};

#endif // APPTHEME_H
