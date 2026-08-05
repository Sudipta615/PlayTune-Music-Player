#include "settingspage.h"
#include "loudnessscannerdialog.h"
#include "appsettings.h"
#include "apptheme.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QFrame>
#include <QIcon>
#include <QSize>
#include <QSettings>
#include <QShowEvent>
#include <QListWidgetItem>
#include <QScrollArea>
#include <QSignalBlocker>
#include <QApplication>

SettingsPageWidget::SettingsPageWidget(QWidget* parent) : QWidget(parent) {
    setObjectName("SettingsPage");
    setAttribute(Qt::WA_StyledBackground, true);
    setupUi();
    loadSettings();

    updateThemeStyles(ThemeManager::instance().currentTheme());
    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this](const ThemePalette& p) {
        updateThemeStyles(p);
    });
}

void SettingsPageWidget::setupUi() {
    auto* rootLayout = new QVBoxLayout(this);
    rootLayout->setContentsMargins(0, 0, 0, 0);
    rootLayout->setSpacing(0);

    // ─── Scroll Area Container ────────────────────────────────────────────
    // Using QScrollArea ensures that on smaller or standard window heights, the cards never
    // squish vertically, overlap, or clip. Each card gets full, comfortable breathing room.
    auto* scrollArea = new QScrollArea(this);
    scrollArea->setObjectName("SettingsScrollArea");
    scrollArea->setWidgetResizable(true);
    scrollArea->setFrameShape(QFrame::NoFrame);
    scrollArea->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    scrollArea->setStyleSheet(
        "QScrollArea#SettingsScrollArea { background-color: transparent; border: none; }"
        "QScrollArea#SettingsScrollArea > QWidget > QWidget { background-color: transparent; }"
    );

    auto* scrollContent = new QWidget(scrollArea);
    scrollContent->setObjectName("SettingsScrollContent");
    scrollContent->setStyleSheet("QWidget#SettingsScrollContent { background-color: transparent; }");
    scrollArea->setWidget(scrollContent);
    rootLayout->addWidget(scrollArea);

    auto* mainLayout = new QVBoxLayout(scrollContent);
    mainLayout->setContentsMargins(30, 24, 30, 30);
    mainLayout->setSpacing(20);

    // Shared styling tokens for cards and dividers
    const QString cardStyle =
        "QFrame#SettingsCard {"
        "  background-color: #121624;"
        "  border: 1px solid #1E2436;"
        "  border-radius: 14px;"
        "}";
    const QString sepStyle =
        "QFrame { background-color: #1E2436; max-height: 1px; min-height: 1px; border: none; }";

    // Helper: Creates a clean row with title + description on the left, control vertically centered on the right
    auto createSettingRow = [cardStyle](QFrame* parentCard, const QString& title, const QString& subtitle, QWidget* control) -> QLayout* {
        auto* row = new QHBoxLayout();
        row->setContentsMargins(0, 4, 0, 4);
        row->setSpacing(16);

        auto* textCol = new QVBoxLayout();
        textCol->setSpacing(2);
        auto* titleLbl = new QLabel(title, parentCard);
        titleLbl->setStyleSheet("font-size: 14px; font-weight: 600; color: #FFFFFF; border: none; background: transparent;");
        auto* subLbl = new QLabel(subtitle, parentCard);
        subLbl->setStyleSheet("font-size: 12px; color: #8E95AD; border: none; background: transparent;");
        subLbl->setWordWrap(true);
        textCol->addWidget(titleLbl);
        textCol->addWidget(subLbl);

        row->addLayout(textCol, 1);
        row->addWidget(control, 0, Qt::AlignVCenter | Qt::AlignRight);
        return row;
    };

    // ─── Page Header ──────────────────────────────────────────────────────
    {
        auto* headerLayout = new QVBoxLayout();
        headerLayout->setSpacing(4);
        auto* pageTitle = new QLabel("Settings & Library Management", scrollContent);
        pageTitle->setStyleSheet("font-size: 24px; font-weight: 800; color: #FFFFFF; letter-spacing: -0.5px; border: none; background: transparent;");
        auto* pageSub = new QLabel("Configure your audio processing preferences, visual themes, and local music collection directories.", scrollContent);
        pageSub->setStyleSheet("font-size: 13px; color: #8E95AD; border: none; background: transparent;");
        headerLayout->addWidget(pageTitle);
        headerLayout->addWidget(pageSub);
        mainLayout->addLayout(headerLayout);
    }

    // ─── 2-Column Split Layout ───────────────────────────────────────────
    auto* columnsLayout = new QHBoxLayout();
    columnsLayout->setSpacing(20);
    columnsLayout->setAlignment(Qt::AlignTop);

    auto* leftCol = new QVBoxLayout();
    leftCol->setSpacing(20);
    leftCol->setAlignment(Qt::AlignTop);

    auto* rightCol = new QVBoxLayout();
    rightCol->setSpacing(20);
    rightCol->setAlignment(Qt::AlignTop);

    // ─── LEFT COLUMN ──────────────────────────────────────────────────────

    // Card 1: Appearance & Theme (Left)
    {
        auto* card = new QFrame(scrollContent);
        card->setObjectName("SettingsCard");
        card->setFrameShape(QFrame::NoFrame);
        card->setStyleSheet(cardStyle);
        auto* cl = new QVBoxLayout(card);
        cl->setContentsMargins(22, 18, 22, 22);
        cl->setSpacing(14);

        auto* hdr = new QLabel("APPEARANCE & THEME", card);
        hdr->setStyleSheet("font-size: 11px; font-weight: 700; color: #FF2A7A; letter-spacing: 1px; border: none; background: transparent;");
        cl->addWidget(hdr);
        auto* sep = new QFrame(card); sep->setFrameShape(QFrame::HLine);
        sep->setStyleSheet(sepStyle); cl->addWidget(sep);

        m_themeCombo = new QComboBox(card);
        m_themeCombo->setObjectName("ThemeComboBox");
        for (const auto& pair : ThemeManager::instance().availableThemes()) {
            m_themeCombo->addItem(pair.second, pair.first);
        }
        m_themeCombo->setFixedHeight(34);
        m_themeCombo->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
        m_themeCombo->setStyleSheet(
            "QComboBox { background-color: #1A2030; color: #FFFFFF; border: 1px solid #28324A;"
            "  border-radius: 8px; padding: 4px 12px; font-size: 12px; font-weight: 500; }"
            "QComboBox:hover { border-color: #FF2A7A; }"
            "QComboBox::drop-down { border: none; width: 24px; }"
            "QComboBox QAbstractItemView { background-color: #1A2030; color: #FFFFFF;"
            "  selection-background-color: #FF2A7A; selection-color: #FFFFFF; border: 1px solid #28324A; border-radius: 6px; padding: 4px; }"
        );
        m_themeCombo->setToolTip("Select Application Visual Theme");
        cl->addLayout(createSettingRow(card, "Application Theme", "Select the visual color scheme across all windows and controls.", m_themeCombo));

        auto* div1 = new QFrame(card); div1->setFrameShape(QFrame::HLine);
        div1->setStyleSheet(sepStyle); cl->addWidget(div1);

        m_tooltipToggle = new ToggleSwitch(card);
        m_tooltipToggle->setToolTip("Enable or Disable Descriptive Tooltips Across the Application");
        cl->addLayout(createSettingRow(card, "UI Tooltips & Hints", "Display descriptive popups when hovering controls.", m_tooltipToggle));

        connect(m_tooltipToggle, &ToggleSwitch::toggled, this, [this](bool on) {
            m_tooltipsEnabled = on; saveSettings(); emit tooltipsToggled(on);
        });
        connect(m_themeCombo, &QComboBox::activated, this, [this](int idx) {
            m_currentTheme = m_themeCombo->itemText(idx);
            QString themeId = m_themeCombo->itemData(idx).toString();
            saveSettings();
            ThemeManager::instance().setTheme(themeId);
        });

        leftCol->addWidget(card);
    }

    // Card 0-B: Performance & Resource Usage (Left, TOP of left column)
    {
        auto* card = new QFrame(scrollContent);
        card->setObjectName("SettingsCard");
        card->setFrameShape(QFrame::NoFrame);
        card->setStyleSheet(
            "QFrame#SettingsCard {"
            "  background-color: #121624;"
            "  border: 1.5px solid #7C4A00;"
            "  border-radius: 14px;"
            "}"
        );
        auto* cl = new QVBoxLayout(card);
        cl->setContentsMargins(22, 18, 22, 22);
        cl->setSpacing(14);

        auto* hdr = new QLabel("\u26a1  PERFORMANCE & RESOURCE USAGE", card);
        hdr->setStyleSheet("font-size: 11px; font-weight: 700; color: #F59E0B; letter-spacing: 1px; border: none; background: transparent;");
        cl->addWidget(hdr);
        auto* sep = new QFrame(card); sep->setFrameShape(QFrame::HLine);
        sep->setStyleSheet(sepStyle); cl->addWidget(sep);

        auto* infoBox = new QLabel(
            "Disables: Spectrum Visualizer \u00b7 Cover Art in all Library tabs \u00b7 "
            "Drop Shadows & Color Animations \u00b7 Loudness Scanner. "
            "EQ DSP is also bypassed when all bands are at 0 dB. "
            "Now Playing card cover art is always preserved.",
            card
        );
        infoBox->setWordWrap(true);
        infoBox->setStyleSheet(
            "font-size: 12px; color: #F59E0B; background-color: rgba(245,158,11,0.08); "
            "border: 1px solid rgba(245,158,11,0.25); border-radius: 8px; padding: 8px 10px;"
        );
        cl->addWidget(infoBox);

        m_optimizedModeToggle = new ToggleSwitch(card);
        m_optimizedModeToggle->setToolTip(
            "Enable Optimized Mode: reduces CPU and RAM usage with no impact on audio quality"
        );
        cl->addLayout(createSettingRow(
            card,
            "Optimized Mode",
            "Minimize CPU & RAM usage. Ideal for low-power devices or background listening.",
            m_optimizedModeToggle
        ));

        connect(m_optimizedModeToggle, &ToggleSwitch::toggled, this, [this](bool on) {
            m_optimizedMode = on;
            AppSettings::instance().setOptimizedMode(on);
            if (m_loudnessScanBtn) {
                m_loudnessScanBtn->setEnabled(!on);
                m_loudnessScanBtn->setToolTip(
                    on ? "Disabled in Optimized Mode" : "Scan Library for ReplayGain..."
                );
            }
            if (m_tooltipToggle) {
                m_tooltipToggle->setEnabled(!on);
                m_tooltipToggle->setToolTip(
                    on ? "Tooltips hints are disabled in Optimized Mode" : "Display descriptive popups when hovering controls."
                );
            }
            saveSettings();
            emit optimizedModeToggled(on);
        });

        rightCol->addWidget(card);
    }


    // Card 2: Add Music To Library (Left)

    {
        auto* card = new QFrame(scrollContent);
        card->setObjectName("SettingsCard");
        card->setFrameShape(QFrame::NoFrame);
        card->setStyleSheet(cardStyle);
        auto* cl = new QVBoxLayout(card);
        cl->setContentsMargins(22, 18, 22, 22);
        cl->setSpacing(14);

        auto* hdr = new QLabel("ADD MUSIC TO LIBRARY", card);
        hdr->setStyleSheet("font-size: 11px; font-weight: 700; color: #38BDF8; letter-spacing: 1px; border: none; background: transparent;");
        cl->addWidget(hdr);
        auto* sep = new QFrame(card); sep->setFrameShape(QFrame::HLine);
        sep->setStyleSheet(sepStyle); cl->addWidget(sep);

        auto* desc = new QLabel(
            "Import audio tracks or scan directories directly into your local PlayTune collection. "
            "Supported formats: MP3, FLAC, WAV, M4A, OGG, and AAC.",
            card
        );
        desc->setStyleSheet("font-size: 13px; color: #8E95AD; border: none; background: transparent;");
        desc->setWordWrap(true);
        cl->addWidget(desc);

        auto* btnGrid = new QGridLayout();
        btnGrid->setSpacing(10);

        m_addSongsBtn = new QPushButton("  Add Songs", card);
        m_addSongsBtn->setObjectName("AddMusicBtn");
        m_addSongsBtn->setIcon(QIcon(":/resources/icons/plus.png"));
        m_addSongsBtn->setIconSize(QSize(14, 14));
        m_addSongsBtn->setCursor(Qt::PointingHandCursor);
        m_addSongsBtn->setFixedHeight(36);
        m_addSongsBtn->setStyleSheet(
            "QPushButton { background-color: #7B1FA2; color: #FFFFFF; font-size: 13px;"
            "  font-weight: 600; border: none; border-radius: 8px; padding: 0px 14px; }"
            "QPushButton:hover { background-color: #9C27B0; }"
            "QPushButton:pressed { background-color: #6A1B9A; }"
        );
        m_addSongsBtn->setToolTip("Select and Import Individual Audio Tracks (.mp3, .flac, .wav, .m4a)");

        m_addFoldersBtn = new QPushButton("  Add Folder", card);
        m_addFoldersBtn->setObjectName("AddMusicBtn");
        m_addFoldersBtn->setIcon(QIcon(":/resources/icons/folders.png"));
        m_addFoldersBtn->setIconSize(QSize(14, 14));
        m_addFoldersBtn->setCursor(Qt::PointingHandCursor);
        m_addFoldersBtn->setFixedHeight(36);
        m_addFoldersBtn->setStyleSheet(
            "QPushButton { background-color: #1A2030; color: #E1E4EB; font-size: 13px;"
            "  font-weight: 600; border: 1px solid #28324A; border-radius: 8px; padding: 0px 14px; }"
            "QPushButton:hover { background-color: #1B1130; border-color: #7B1FA2; color: #FFFFFF; }"
            "QPushButton:pressed { background-color: #141824; }"
        );
        m_addFoldersBtn->setToolTip("Select and Scan an Entire Directory for Audio Tracks");

        m_importM3UBtn = new QPushButton("  Import Playlist", card);
        m_importM3UBtn->setObjectName("AddMusicBtn");
        m_importM3UBtn->setIcon(QIcon(":/resources/icons/plus.png"));
        m_importM3UBtn->setIconSize(QSize(14, 14));
        m_importM3UBtn->setCursor(Qt::PointingHandCursor);
        m_importM3UBtn->setFixedHeight(36);
        m_importM3UBtn->setStyleSheet(
            "QPushButton { background-color: #1A2030; color: #E1E4EB; font-size: 13px;"
            "  font-weight: 600; border: 1px solid #28324A; border-radius: 8px; padding: 0px 14px; }"
            "QPushButton:hover { background-color: #1B1130; border-color: #7B1FA2; color: #FFFFFF; }"
            "QPushButton:pressed { background-color: #141824; }"
        );
        m_importM3UBtn->setToolTip("Import an M3U/M3U8 playlist file.");

        m_exportM3UBtn = new QPushButton("  Export Playlist", card);
        m_exportM3UBtn->setObjectName("AddMusicBtn");
        m_exportM3UBtn->setIcon(QIcon(":/resources/icons/folders.png"));
        m_exportM3UBtn->setIconSize(QSize(14, 14));
        m_exportM3UBtn->setCursor(Qt::PointingHandCursor);
        m_exportM3UBtn->setFixedHeight(36);
        m_exportM3UBtn->setStyleSheet(
            "QPushButton { background-color: #1A2030; color: #E1E4EB; font-size: 13px;"
            "  font-weight: 600; border: 1px solid #28324A; border-radius: 8px; padding: 0px 14px; }"
            "QPushButton:hover { background-color: #1B1130; border-color: #7B1FA2; color: #FFFFFF; }"
            "QPushButton:pressed { background-color: #141824; }"
        );
        m_exportM3UBtn->setToolTip("Export one of your playlists to an M3U8 file.");

        btnGrid->addWidget(m_addSongsBtn, 0, 0);
        btnGrid->addWidget(m_addFoldersBtn, 0, 1);
        btnGrid->addWidget(m_importM3UBtn, 1, 0);
        btnGrid->addWidget(m_exportM3UBtn, 1, 1);
        cl->addLayout(btnGrid);

        connect(m_addSongsBtn,    &QPushButton::clicked, this, &SettingsPageWidget::addSongsRequested);
        connect(m_addFoldersBtn,  &QPushButton::clicked, this, &SettingsPageWidget::addFoldersRequested);
        connect(m_importM3UBtn,   &QPushButton::clicked, this, &SettingsPageWidget::importM3URequested);
        connect(m_exportM3UBtn,   &QPushButton::clicked, this, &SettingsPageWidget::exportM3URequested);

        leftCol->addWidget(card);
    }

    // Card 3: Library Folders (Left)
    {
        auto* card = new QFrame(scrollContent);
        card->setObjectName("SettingsCard");
        card->setFrameShape(QFrame::NoFrame);
        card->setStyleSheet(cardStyle);
        auto* cl = new QVBoxLayout(card);
        cl->setContentsMargins(22, 18, 22, 22);
        cl->setSpacing(10);

        auto* hdr = new QLabel("LIBRARY FOLDERS", card);
        hdr->setStyleSheet("font-size: 11px; font-weight: 700; color: #10B981; letter-spacing: 1px; border: none; background: transparent;");
        cl->addWidget(hdr);
        auto* sep = new QFrame(card); sep->setFrameShape(QFrame::HLine);
        sep->setStyleSheet(sepStyle); cl->addWidget(sep);

        auto* desc = new QLabel("Directories synchronized with your PlayTune library:", card);
        desc->setStyleSheet("font-size: 13px; color: #8E95AD; border: none; background: transparent;");
        cl->addWidget(desc);

        m_foldersListWidget = new QListWidget(card);
        m_foldersListWidget->setMinimumHeight(140);
        m_foldersListWidget->setFrameShape(QFrame::NoFrame);
        m_foldersListWidget->setStyleSheet(
            "QListWidget { background-color: #0B0E17; border: 1px solid #1E2436;"
            "  border-radius: 8px; color: #E1E4EB; font-size: 12px; padding: 4px; }"
            "QListWidget::item { padding: 0px; border-radius: 6px; margin-bottom: 2px; }"
            "QListWidget::item:alternate { background-color: #101420; }"
            "QListWidget::item:hover { background-color: #1A2033; }"
        );
        m_foldersListWidget->setAlternatingRowColors(true);
        cl->addWidget(m_foldersListWidget, 1);

        leftCol->addWidget(card, 1);
    }

    // ─── RIGHT COLUMN ─────────────────────────────────────────────────────

    // Card 4: Playback & Audio Processing (Right)
    {
        auto* card = new QFrame(scrollContent);
        card->setObjectName("SettingsCard");
        card->setFrameShape(QFrame::NoFrame);
        card->setStyleSheet(cardStyle);
        auto* cl = new QVBoxLayout(card);
        cl->setContentsMargins(22, 18, 22, 22);
        cl->setSpacing(14);

        auto* hdr = new QLabel("PLAYBACK & AUDIO PROCESSING", card);
        hdr->setStyleSheet("font-size: 11px; font-weight: 700; color: #7B1FA2; letter-spacing: 1px; border: none; background: transparent;");
        cl->addWidget(hdr);
        auto* sep = new QFrame(card); sep->setFrameShape(QFrame::HLine);
        sep->setStyleSheet(sepStyle); cl->addWidget(sep);

        m_backendCombo = new QComboBox(card);
        m_backendCombo->setObjectName("BackendComboBox");
        m_backendCombo->addItem("Auto (Default / Shared Mode)", 0);
#if defined(Q_OS_LINUX) || defined(Q_OS_UNIX)
        m_backendCombo->addItem("Direct ALSA (Exclusive Hardware)", 1);
#elif defined(Q_OS_WIN) || defined(Q_OS_WIN32)
        m_backendCombo->addItem("WASAPI Exclusive Mode", 2);
        m_backendCombo->addItem("ASIO Driver Mode", 3);
#elif defined(Q_OS_MAC) || defined(Q_OS_MACOS)
        m_backendCombo->addItem("CoreAudio Hog Mode (macOS)", 4);
#else
        m_backendCombo->addItem("Direct ALSA (Exclusive Hardware)", 1);
        m_backendCombo->addItem("WASAPI Exclusive Mode", 2);
        m_backendCombo->addItem("ASIO Driver Mode", 3);
        m_backendCombo->addItem("CoreAudio Hog Mode (macOS)", 4);
#endif
        m_backendCombo->setFixedSize(220, 34);
        m_backendCombo->setStyleSheet(
            "QComboBox { background-color: #1A2030; color: #FFFFFF; border: 1px solid #28324A;"
            "  border-radius: 8px; padding: 4px 10px; font-size: 12px; font-weight: 500; }"
            "QComboBox:hover { border-color: #7B1FA2; }"
            "QComboBox::drop-down { border: none; width: 24px; }"
            "QComboBox QAbstractItemView { background-color: #1A2030; color: #FFFFFF;"
            "  selection-background-color: #7B1FA2; selection-color: #FFFFFF; border: 1px solid #28324A; border-radius: 6px; padding: 4px; }"
        );
        m_backendCombo->setToolTip("Select Audio Output Driver Backend (Exclusive Bit-Perfect or Shared Mode)");
        cl->addLayout(createSettingRow(card, "Audio Output Driver", "Choose driver API (exclusive modes bypass OS mixer).", m_backendCombo));

        auto* divB = new QFrame(card); divB->setFrameShape(QFrame::HLine);
        divB->setStyleSheet(sepStyle); cl->addWidget(divB);

        m_deviceCombo = new QComboBox(card);
        m_deviceCombo->setObjectName("DeviceComboBox");
        m_deviceCombo->addItem("Default / Automatic");
        m_deviceCombo->setFixedSize(220, 34);
        m_deviceCombo->setStyleSheet(
            "QComboBox { background-color: #1A2030; color: #FFFFFF; border: 1px solid #28324A;"
            "  border-radius: 8px; padding: 4px 10px; font-size: 12px; font-weight: 500; }"
            "QComboBox:hover { border-color: #7B1FA2; }"
            "QComboBox::drop-down { border: none; width: 24px; }"
            "QComboBox QAbstractItemView { background-color: #1A2030; color: #FFFFFF;"
            "  selection-background-color: #7B1FA2; selection-color: #FFFFFF; border: 1px solid #28324A; border-radius: 6px; padding: 4px; }"
        );
        m_deviceCombo->setToolTip("Select Specific DAC or Audio Device");
        cl->addLayout(createSettingRow(card, "Target Audio Device", "Select hardware DAC or sound card interface.", m_deviceCombo));

        auto* divD = new QFrame(card); divD->setFrameShape(QFrame::HLine);
        divD->setStyleSheet(sepStyle); cl->addWidget(divD);

        m_crossfadeToggle = new ToggleSwitch(card);
        m_crossfadeToggle->setToolTip("Toggle 3-Second Crossfade Between Track Transitions");
        cl->addLayout(createSettingRow(card, "Crossfade Transition", "Smoothly blend audio between tracks.", m_crossfadeToggle));

        auto* div1 = new QFrame(card); div1->setFrameShape(QFrame::HLine);
        div1->setStyleSheet(sepStyle); cl->addWidget(div1);

        m_normalizeToggle = new ToggleSwitch(card);
        m_normalizeToggle->setToolTip("Normalize Volume Levels Across Different Audio Tracks");
        cl->addLayout(createSettingRow(card, "Audio Normalization", "Balance volume levels across tracks.", m_normalizeToggle));

        auto* div2 = new QFrame(card); div2->setFrameShape(QFrame::HLine);
        div2->setStyleSheet(sepStyle); cl->addWidget(div2);

        m_gaplessToggle = new ToggleSwitch(card);
        m_gaplessToggle->setToolTip("Enable Zero-Latency Gapless Playback Between Consecutive Tracks");
        cl->addLayout(createSettingRow(card, "Gapless Playback Mode", "Eliminate delays between consecutive tracks.", m_gaplessToggle));

        auto* div2b = new QFrame(card); div2b->setFrameShape(QFrame::HLine);
        div2b->setStyleSheet(sepStyle); cl->addWidget(div2b);

        m_crossfadeDurationSpin = new QSpinBox(card);
        m_crossfadeDurationSpin->setRange(500, 12000);
        m_crossfadeDurationSpin->setSingleStep(500);
        m_crossfadeDurationSpin->setValue(3000);
        m_crossfadeDurationSpin->setSuffix(" ms");
        m_crossfadeDurationSpin->setFixedSize(110, 34);
        m_crossfadeDurationSpin->setStyleSheet(
            "QSpinBox { background-color: #1A2030; color: #FFFFFF; border: 1px solid #28324A; "
            "border-radius: 8px; padding: 4px 8px; padding-right: 24px; font-size: 12px; }"
            "QSpinBox::up-button { subcontrol-origin: border; subcontrol-position: top right; width: 22px; height: 14px; border-left: 1px solid #28324A; border-bottom: 1px solid #28324A; border-top-right-radius: 7px; background-color: #242A3E; }"
            "QSpinBox::up-button:hover { background-color: #7B1FA2; }"
            "QSpinBox::down-button { subcontrol-origin: border; subcontrol-position: bottom right; width: 22px; height: 14px; border-left: 1px solid #28324A; border-bottom-right-radius: 7px; background-color: #242A3E; }"
            "QSpinBox::down-button:hover { background-color: #7B1FA2; }");
        m_crossfadeDurationSpin->setToolTip("Crossfade duration in milliseconds (only used when Crossfade is ON).");
        cl->addLayout(createSettingRow(card, "Crossfade Duration", "Duration in ms (when Crossfade is ON).", m_crossfadeDurationSpin));

        auto* div2c = new QFrame(card); div2c->setFrameShape(QFrame::HLine);
        div2c->setStyleSheet(sepStyle); cl->addWidget(div2c);

        m_cursorFollowToggle = new ToggleSwitch(card);
        m_cursorFollowToggle->setToolTip("Auto-scroll the songs table to follow the currently playing track.");
        cl->addLayout(createSettingRow(card, "Cursor Follows Playback", "Auto-scroll list to active playing song.", m_cursorFollowToggle));

        auto* div2d = new QFrame(card); div2d->setFrameShape(QFrame::HLine);
        div2d->setStyleSheet(sepStyle); cl->addWidget(div2d);

        m_notificationsToggle = new ToggleSwitch(card);
        m_notificationsToggle->setToolTip("Show desktop notifications when the track changes.");
        cl->addLayout(createSettingRow(card, "Desktop Notifications", "Display popup notification on track change.", m_notificationsToggle));

        auto* div2e = new QFrame(card); div2e->setFrameShape(QFrame::HLine);
        div2e->setStyleSheet(sepStyle); cl->addWidget(div2e);

        m_trayToggle = new ToggleSwitch(card);
        m_trayToggle->setToolTip("Show PlayTune in the system tray for background playback.");
        cl->addLayout(createSettingRow(card, "System Tray Icon", "Show system tray icon for background control.", m_trayToggle));

        auto* div2f = new QFrame(card); div2f->setFrameShape(QFrame::HLine);
        div2f->setStyleSheet(sepStyle); cl->addWidget(div2f);

        m_minimizeToTrayToggle = new ToggleSwitch(card);
        m_minimizeToTrayToggle->setToolTip("Hide to tray when closing the window instead of quitting.");
        cl->addLayout(createSettingRow(card, "Minimize to Tray on Close", "Hide to tray on close instead of quitting.", m_minimizeToTrayToggle));

        connect(m_crossfadeToggle, &ToggleSwitch::toggled, this, [this](bool c) { m_crossfadeEnabled = c; saveSettings(); emit crossfadeToggled(c); });
        connect(m_normalizeToggle, &ToggleSwitch::toggled, this, [this](bool c) { m_normalizeEnabled = c; saveSettings(); emit normalizeToggled(c); });
        connect(m_gaplessToggle,   &ToggleSwitch::toggled, this, [this](bool c) { m_gaplessEnabled   = c; saveSettings(); emit gaplessToggled(c); });
        connect(m_cursorFollowToggle, &ToggleSwitch::toggled, this, [this](bool c) {
            m_cursorFollows = c;
            QSettings s;
            s.setValue("cursor_follows_playback", c);
            saveSettings();
            emit cursorFollowsToggled(c);
        });
        connect(m_notificationsToggle, &ToggleSwitch::toggled, this, [this](bool c) {
            m_notificationsEnabled = c;
            saveSettings();
            emit notificationsToggled(c);
        });
        connect(m_trayToggle, &ToggleSwitch::toggled, this, [this](bool c) {
            m_trayEnabled = c;
            saveSettings();
            emit trayToggled(c);
        });
        connect(m_minimizeToTrayToggle, &ToggleSwitch::toggled, this, [this](bool c) {
            m_minimizeToTray = c;
            saveSettings();
            emit minimizeToTrayToggled(c);
        });
        connect(m_crossfadeDurationSpin, QOverload<int>::of(&QSpinBox::valueChanged), this,
                [this](int value) {
            saveSettings();
            emit crossfadeDurationChanged(value);
        });
        connect(m_backendCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this, [this](int) {
            if (m_backendCombo) {
                m_currentBackend = m_backendCombo->currentData().toInt();
                saveSettings();
                emit outputBackendChanged(m_currentBackend);
            }
        });
        connect(m_deviceCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this, [this](int) {
            if (m_deviceCombo && m_deviceCombo->currentIndex() >= 0) {
                m_currentDevice = m_deviceCombo->currentText();
                saveSettings();
                emit outputDeviceChanged(m_currentDevice);
            }
        });

        rightCol->addWidget(card);
    }

    // Card 5: Audio Analysis & ReplayGain (Right)
    {
        auto* card = new QFrame(scrollContent);
        card->setObjectName("SettingsCard");
        card->setFrameShape(QFrame::NoFrame);
        card->setStyleSheet(cardStyle);
        auto* cl = new QVBoxLayout(card);
        cl->setContentsMargins(22, 18, 22, 22);
        cl->setSpacing(14);

        auto* hdr = new QLabel("AUDIO ANALYSIS & REPLAYGAIN", card);
        hdr->setStyleSheet("font-size: 11px; font-weight: 700; color: #00E5FF; letter-spacing: 1px; border: none; background: transparent;");
        cl->addWidget(hdr);
        auto* sep = new QFrame(card); sep->setFrameShape(QFrame::HLine);
        sep->setStyleSheet(sepStyle); cl->addWidget(sep);

        auto* desc = new QLabel("Scan library using EBU R128 K-weighting for integrated LUFS and true peaks, writing ReplayGain 2.0 / R128 tags.", card);
        desc->setWordWrap(true);
        desc->setStyleSheet("font-size: 13px; color: #8E95AD; border: none; background: transparent;");
        cl->addWidget(desc);

        auto* btnLayout = new QHBoxLayout();
        auto* scanBtn = new QPushButton("Scan Library for ReplayGain...", card);
        m_loudnessScanBtn = scanBtn;  // store reference so Optimized Mode can disable it
        scanBtn->setFixedHeight(36);
        scanBtn->setStyleSheet(
            "QPushButton {"
            "   background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #FF2A7A, stop:1 #8B26B6);"
            "   border: 1px solid #FF2A7A; border-radius: 8px; color: #FFFFFF; font-weight: bold; padding: 0px 16px; font-size: 12px;"
            "}"
            "QPushButton:hover { background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #FF458D, stop:1 #9E3AC7); }"
        );
        connect(scanBtn, &QPushButton::clicked, this, [this]() {
            LoudnessScannerDialog dlg(QVector<int>(), this);
            dlg.exec();
        });
        btnLayout->addWidget(scanBtn);
        btnLayout->addStretch();
        cl->addLayout(btnLayout);

        rightCol->addWidget(card);
    }

    columnsLayout->addLayout(leftCol, 1);
    columnsLayout->addLayout(rightCol, 1);

    mainLayout->addLayout(columnsLayout);
    mainLayout->addStretch();
}

// ─── Folder list slot implementations ────────────────────────────
void SettingsPageWidget::clearFolderList() {
    if (m_foldersListWidget)
        m_foldersListWidget->clear();
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

    auto* textLabel = new QLabel(QString("\U0001F4C1 %1  \u2014  %2").arg(name, path), rowWidget);
    textLabel->setStyleSheet("color: #E1E4EB; font-size: 13px; font-weight: 500;");
    textLabel->setToolTip(path);

    auto* delBtn = new QPushButton(rowWidget);
    delBtn->setObjectName("DeleteFolderBtn");
    delBtn->setIcon(QIcon(":/resources/icons/close.png"));
    delBtn->setIconSize(QSize(16, 16));
    delBtn->setFixedSize(32, 32);
    delBtn->setCursor(Qt::PointingHandCursor);
    delBtn->setToolTip("Remove Folder and All Songs Inside");
    delBtn->setStyleSheet(
        "QPushButton { background-color: transparent; border: none; border-radius: 6px; padding: 0px; }"
        "QPushButton:hover { background-color: rgba(229, 57, 53, 0.8); }"
    );

    connect(delBtn, &QPushButton::clicked, this, [this, id]() {
        emit deleteFolderRequested(id);
    });

    rowLayout->addWidget(textLabel, 1);
    rowLayout->addWidget(delBtn, 0, Qt::AlignRight | Qt::AlignVCenter);

    m_foldersListWidget->addItem(item);
    m_foldersListWidget->setItemWidget(item, rowWidget);
}

void SettingsPageWidget::clearAudioDeviceList() {
    if (m_deviceCombo) {
        QSignalBlocker b(m_deviceCombo);
        m_deviceCombo->clear();
    }
}

void SettingsPageWidget::addAudioDeviceToList(const QString& name, bool isCurrent) {
    if (!m_deviceCombo) return;
    QSignalBlocker b(m_deviceCombo);
    m_deviceCombo->addItem(name);
    if (isCurrent) {
        m_deviceCombo->setCurrentIndex(m_deviceCombo->count() - 1);
        m_currentDevice = name;
    }
}

void SettingsPageWidget::loadSettings() {
    QSettings settings("PlayTune", "Settings");
    m_tooltipsEnabled = settings.value("tooltips", true).toBool();
    m_optimizedMode   = settings.value("optimized_mode", false).toBool();
    AppSettings::instance().setOptimizedMode(m_optimizedMode);
    m_crossfadeEnabled = settings.value("crossfade", false).toBool();
    m_normalizeEnabled = settings.value("normalize", false).toBool();
    m_gaplessEnabled = settings.value("gapless", true).toBool();
    m_cursorFollows = settings.value("cursor_follows_playback", false).toBool();
    m_notificationsEnabled = settings.value("notifications_enabled", true).toBool();
    m_trayEnabled = settings.value("tray_enabled", false).toBool();
    m_minimizeToTray = settings.value("minimize_to_tray", false).toBool();
    int crossfade_ms = settings.value("crossfade_duration_ms", 3000).toInt();
    m_currentTheme = settings.value("theme_text", ThemeManager::instance().currentTheme().name).toString();
    m_currentBackend = settings.value("audio_backend", 0).toInt();
    m_currentDevice = settings.value("audio_device", "Default / Automatic").toString();

    if (m_tooltipToggle) {
        m_tooltipToggle->setChecked(m_tooltipsEnabled);
        m_tooltipToggle->setEnabled(!m_optimizedMode);
        if (m_optimizedMode) {
            m_tooltipToggle->setToolTip("Tooltips hints are disabled in Optimized Mode");
        }
    }
    if (m_optimizedModeToggle)  m_optimizedModeToggle->setChecked(m_optimizedMode);
    if (m_loudnessScanBtn) {
        m_loudnessScanBtn->setEnabled(!m_optimizedMode);
        m_loudnessScanBtn->setToolTip(
            m_optimizedMode ? "Disabled in Optimized Mode" : "Scan Library for ReplayGain..."
        );
    }

    if (m_crossfadeToggle) m_crossfadeToggle->setChecked(m_crossfadeEnabled);
    if (m_normalizeToggle) m_normalizeToggle->setChecked(m_normalizeEnabled);
    if (m_gaplessToggle)   m_gaplessToggle->setChecked(m_gaplessEnabled);
    if (m_cursorFollowToggle) m_cursorFollowToggle->setChecked(m_cursorFollows);
    if (m_notificationsToggle) m_notificationsToggle->setChecked(m_notificationsEnabled);
    if (m_trayToggle) m_trayToggle->setChecked(m_trayEnabled);
    if (m_minimizeToTrayToggle) m_minimizeToTrayToggle->setChecked(m_minimizeToTray);
    if (m_crossfadeDurationSpin) {
        QSignalBlocker b(m_crossfadeDurationSpin);
        m_crossfadeDurationSpin->setValue(crossfade_ms);
    }
    if (m_themeCombo) {
        QString activeId = ThemeManager::instance().currentThemeId();
        int idx = m_themeCombo->findData(activeId);
        if (idx < 0) idx = m_themeCombo->findText(m_currentTheme);
        if (idx >= 0) {
            QSignalBlocker b(m_themeCombo);
            m_themeCombo->setCurrentIndex(idx);
        }
    }
    if (m_backendCombo) {
        int idx = m_backendCombo->findData(m_currentBackend);
        if (idx < 0) {
            idx = 0;
            m_currentBackend = 0;
        }
        QSignalBlocker b(m_backendCombo);
        m_backendCombo->setCurrentIndex(idx);
    }
}

void SettingsPageWidget::saveSettings() {
    QSettings settings("PlayTune", "Settings");
    settings.setValue("tooltips", m_tooltipsEnabled);
    settings.setValue("optimized_mode", m_optimizedMode);
    settings.setValue("crossfade", m_crossfadeEnabled);
    settings.setValue("normalize", m_normalizeEnabled);
    settings.setValue("gapless", m_gaplessEnabled);
    settings.setValue("cursor_follows_playback", m_cursorFollows);
    settings.setValue("notifications_enabled", m_notificationsEnabled);
    settings.setValue("tray_enabled", m_trayEnabled);
    settings.setValue("minimize_to_tray", m_minimizeToTray);
    if (m_crossfadeDurationSpin) {
        settings.setValue("crossfade_duration_ms", m_crossfadeDurationSpin->value());
    }
    settings.setValue("theme_text", m_currentTheme);
    settings.setValue("audio_backend", m_currentBackend);
    settings.setValue("audio_device", m_currentDevice);
}

void SettingsPageWidget::showEvent(QShowEvent* event) {
    QWidget::showEvent(event);
    if (m_tooltipToggle   && m_tooltipToggle->isChecked()   != m_tooltipsEnabled)  { QSignalBlocker b(m_tooltipToggle);   m_tooltipToggle->setChecked(m_tooltipsEnabled); }
    if (m_crossfadeToggle && m_crossfadeToggle->isChecked() != m_crossfadeEnabled) { QSignalBlocker b(m_crossfadeToggle); m_crossfadeToggle->setChecked(m_crossfadeEnabled); }
    if (m_normalizeToggle && m_normalizeToggle->isChecked() != m_normalizeEnabled) { QSignalBlocker b(m_normalizeToggle); m_normalizeToggle->setChecked(m_normalizeEnabled); }
    if (m_gaplessToggle   && m_gaplessToggle->isChecked()   != m_gaplessEnabled)   { QSignalBlocker b(m_gaplessToggle);   m_gaplessToggle->setChecked(m_gaplessEnabled); }
    if (m_cursorFollowToggle && m_cursorFollowToggle->isChecked() != m_cursorFollows) {
        QSignalBlocker b(m_cursorFollowToggle);
        m_cursorFollowToggle->setChecked(m_cursorFollows);
    }
    if (m_notificationsToggle && m_notificationsToggle->isChecked() != m_notificationsEnabled) {
        QSignalBlocker b(m_notificationsToggle);
        m_notificationsToggle->setChecked(m_notificationsEnabled);
    }
    if (m_trayToggle && m_trayToggle->isChecked() != m_trayEnabled) {
        QSignalBlocker b(m_trayToggle);
        m_trayToggle->setChecked(m_trayEnabled);
    }
    if (m_minimizeToTrayToggle && m_minimizeToTrayToggle->isChecked() != m_minimizeToTray) {
        QSignalBlocker b(m_minimizeToTrayToggle);
        m_minimizeToTrayToggle->setChecked(m_minimizeToTray);
    }
    if (m_themeCombo) {
        int idx = m_themeCombo->findText(m_currentTheme);
        if (idx >= 0 && m_themeCombo->currentIndex() != idx) { QSignalBlocker b(m_themeCombo); m_themeCombo->setCurrentIndex(idx); }
    }
    if (m_backendCombo) {
        int idx = m_backendCombo->findData(m_currentBackend);
        if (idx < 0) {
            idx = 0;
            m_currentBackend = 0;
        }
        if (m_backendCombo->currentIndex() != idx) { QSignalBlocker b(m_backendCombo); m_backendCombo->setCurrentIndex(idx); }
    }
    if (m_deviceCombo && !m_currentDevice.isEmpty()) {
        int idx = m_deviceCombo->findText(m_currentDevice);
        if (idx >= 0 && m_deviceCombo->currentIndex() != idx) { QSignalBlocker b(m_deviceCombo); m_deviceCombo->setCurrentIndex(idx); }
    }
}

bool SettingsPageWidget::isTooltipsEnabled() const {
    return m_tooltipsEnabled;
}

void SettingsPageWidget::updateThemeStyles(const ThemePalette& p) {
    setStyleSheet(QString("QWidget#SettingsPage { background-color: %1; }").arg(p.windowBg.name()));

    const QString cardStyle = QString(
        "QFrame#SettingsCard {"
        "  background-color: %1;"
        "  border: 1px solid %2;"
        "  border-radius: 14px;"
        "}"
    ).arg(p.cardBg.name(), p.cardBorder.name());

    const QString sepStyle = QString(
        "QFrame { background-color: %1; max-height: 1px; min-height: 1px; border: none; }"
    ).arg(p.cardBorder.name());

    const QString comboStyle = QString(
        "QComboBox { background-color: %1; color: %2; border: 1px solid %3;"
        "  border-radius: 8px; padding: 4px 12px; font-size: 12px; font-weight: 500; }"
        "QComboBox:hover { border-color: %4; }"
        "QComboBox::drop-down { border: none; width: 24px; }"
        "QComboBox QAbstractItemView { background-color: %1; color: %2;"
        "  selection-background-color: %5; selection-color: %6; border: 1px solid %3; border-radius: 6px; padding: 4px; outline: none; }"
        "QComboBox QAbstractItemView::item { padding: 6px 12px; border-radius: 4px; color: %2; background-color: transparent; }"
        "QComboBox QAbstractItemView::item:hover { background-color: %5; color: %2; }"
        "QComboBox QAbstractItemView::item:selected { background-color: %5; color: %6; font-weight: bold; }"
    ).arg(p.headerBg.name(), p.primaryText.name(), p.cardBorder.name(), p.primaryAccent.name(), p.itemHoverBg.name(), p.secondaryAccent.name());

    const QString spinStyle = QString(
        "QSpinBox { background-color: %1; color: %2; border: 1px solid %3; "
        "border-radius: 8px; padding: 4px 8px; padding-right: 24px; font-size: 12px; }"
        "QSpinBox::up-button { subcontrol-origin: border; subcontrol-position: top right; width: 22px; height: 14px; border-left: 1px solid %3; border-bottom: 1px solid %3; border-top-right-radius: 7px; background-color: %5; }"
        "QSpinBox::up-button:hover { background-color: %4; }"
        "QSpinBox::down-button { subcontrol-origin: border; subcontrol-position: bottom right; width: 22px; height: 14px; border-left: 1px solid %3; border-bottom-right-radius: 7px; background-color: %5; }"
        "QSpinBox::down-button:hover { background-color: %4; }"
    ).arg(p.headerBg.name(), p.primaryText.name(), p.cardBorder.name(), p.primaryAccent.name(), p.itemHoverBg.name());

    const QString primaryBtnStyle = QString(
        "QPushButton { background-color: %1; color: #FFFFFF; font-size: 13px;"
        "  font-weight: 600; border: none; border-radius: 8px; padding: 0px 14px; }"
        "QPushButton:hover { background-color: %2; }"
        "QPushButton:pressed { background-color: %1; }"
    ).arg(p.secondaryAccent.name(), p.primaryAccent.name());

    const QString secondaryBtnStyle = QString(
        "QPushButton { background-color: %1; color: %2; font-size: 13px;"
        "  font-weight: 600; border: 1px solid %3; border-radius: 8px; padding: 0px 14px; }"
        "QPushButton:hover { background-color: %4; border-color: %5; color: %6; }"
        "QPushButton:pressed { background-color: %1; }"
    ).arg(p.headerBg.name(), p.secondaryText.name(), p.cardBorder.name(), p.itemHoverBg.name(), p.primaryAccent.name(), p.primaryText.name());

    for (auto* card : findChildren<QFrame*>("SettingsCard")) {
        card->setStyleSheet(cardStyle);
    }
    for (auto* sep : findChildren<QFrame*>()) {
        if (sep->objectName() != "SettingsCard" && sep->frameShape() == QFrame::HLine) {
            sep->setStyleSheet(sepStyle);
        }
    }

    if (m_themeCombo) m_themeCombo->setStyleSheet(comboStyle);
    if (m_backendCombo) m_backendCombo->setStyleSheet(comboStyle);
    if (m_deviceCombo) m_deviceCombo->setStyleSheet(comboStyle);
    if (m_crossfadeDurationSpin) m_crossfadeDurationSpin->setStyleSheet(spinStyle);

    if (m_addSongsBtn) m_addSongsBtn->setStyleSheet(primaryBtnStyle);
    if (m_loudnessScanBtn) m_loudnessScanBtn->setStyleSheet(primaryBtnStyle);

    if (m_addFoldersBtn) m_addFoldersBtn->setStyleSheet(secondaryBtnStyle);
    if (m_importM3UBtn) m_importM3UBtn->setStyleSheet(secondaryBtnStyle);
    if (m_exportM3UBtn) m_exportM3UBtn->setStyleSheet(secondaryBtnStyle);

    if (m_foldersListWidget) {
        m_foldersListWidget->setStyleSheet(QString(
            "QListWidget { background-color: %1; border: 1px solid %2;"
            "  border-radius: 8px; color: %3; font-size: 12px; padding: 4px; }"
            "QListWidget::item { padding: 0px; border-radius: 6px; margin-bottom: 2px; }"
            "QListWidget::item:alternate { background-color: %4; }"
            "QListWidget::item:hover { background-color: %5; }"
        ).arg(p.headerBg.name(), p.cardBorder.name(), p.primaryText.name(), p.cardBg.name(), p.itemHoverBg.name()));
    }

    for (auto* label : findChildren<QLabel*>()) {
        QString text = label->text();
        if (text.startsWith("APPEARANCE") || text.startsWith("PERFORMANCE") || text.startsWith("ADD MUSIC") ||
            text.startsWith("LIBRARY FOLDERS") || text.startsWith("PLAYBACK") || text.startsWith("AUDIO ANALYSIS")) {
            label->setStyleSheet(QString("font-size: 11px; font-weight: 700; color: %1; letter-spacing: 1px; border: none; background: transparent;").arg(p.secondaryAccent.name()));
        } else if (text == "Settings & Library Management") {
            label->setStyleSheet(QString("font-size: 24px; font-weight: 800; color: %1; letter-spacing: -0.5px; border: none; background: transparent;").arg(p.primaryText.name()));
        } else if (text.contains("Configure your audio") || text.contains("Directories synchronized") || text.contains("Import audio tracks") || text.contains("Scan library using")) {
            label->setStyleSheet(QString("font-size: 13px; color: %1; border: none; background: transparent;").arg(p.mutedText.name()));
        } else if (!label->parentWidget() || label->parentWidget()->objectName() == "SettingsCard") {
            if (label->font().weight() >= QFont::Bold || label->styleSheet().contains("font-weight: 600")) {
                label->setStyleSheet(QString("font-size: 14px; font-weight: 600; color: %1; border: none; background: transparent;").arg(p.primaryText.name()));
            } else if (label->styleSheet().contains("font-size: 12px")) {
                label->setStyleSheet(QString("font-size: 12px; color: %1; border: none; background: transparent;").arg(p.mutedText.name()));
            }
        }
    }
}
