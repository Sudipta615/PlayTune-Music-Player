#include "equalizerwindow.h"
#include "apptheme.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QGridLayout>
#include <QFrame>
#include <QMouseEvent>
#include <QIcon>
#include <QDebug>
#include <QWindow>
#include <QSettings>
#include <QTimer>

EqualizerWindow::EqualizerWindow(QWidget* parent) : QWidget(parent) {
    setObjectName("EqualizerWindow");
    setWindowIcon(QIcon(":/resources/icons/playtune_logo.png"));
    setWindowFlags(Qt::Widget | Qt::FramelessWindowHint);
    setAttribute(Qt::WA_TranslucentBackground, false);
    setAttribute(Qt::WA_StyledBackground, true);

    // Set a fixed premium dark theme size
    resize(780, 560);
    setMinimumSize(650, 480);

    m_saveTimer = new QTimer(this);
    m_saveTimer->setSingleShot(true);
    m_saveTimer->setInterval(300);
    connect(m_saveTimer, &QTimer::timeout, this, &EqualizerWindow::saveSettings);

    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this](const ThemePalette& p) {
        applyTheme(p);
        update();
    });

    setupUi();
    applyTheme(ThemeManager::instance().currentTheme());
    loadSettings();
    hide();
}

void EqualizerWindow::setupUi() {
    auto* mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(20, 20, 20, 20);
    mainLayout->setSpacing(15);

    // 1. Frameless Drag Header Bar
    auto* headerLayout = new QHBoxLayout();
    headerLayout->setContentsMargins(5, 0, 5, 5);
    headerLayout->setSpacing(12);

    auto* eqIcon = new QLabel(this);
    eqIcon->setPixmap(QIcon(":/resources/icons/equalizer.png").pixmap(18, 18));
    eqIcon->setAttribute(Qt::WA_TransparentForMouseEvents);
    
    auto* eqTitle = new QLabel("Equalizer", this);
    m_eqTitleLabel = eqTitle;
    eqTitle->setObjectName("EqTitleLabel");
    eqTitle->setAttribute(Qt::WA_TransparentForMouseEvents);

    m_enableToggle = new ToggleSwitch(this);
    m_enableToggle->setChecked(false);

    headerLayout->addWidget(eqIcon);
    headerLayout->addWidget(eqTitle);
    headerLayout->addWidget(m_enableToggle);
    headerLayout->addStretch();

    // More Options & Close buttons
    auto* moreBtn = new QPushButton(this);
    moreBtn->setObjectName("IconButton");
    moreBtn->setIcon(QIcon(":/resources/icons/more.png"));
    moreBtn->setIconSize(QSize(16, 16));
    moreBtn->setFixedSize(28, 28);
    moreBtn->setStyleSheet("QPushButton { border: none; background: transparent; } QPushButton:hover { background-color: #1B1130; }");
    moreBtn->setToolTip("Equalizer Advanced Options");

    auto* closeBtn = new QPushButton(this);
    closeBtn->setObjectName("IconButton");
    closeBtn->setIcon(QIcon(":/resources/icons/close.png"));
    closeBtn->setIconSize(QSize(16, 16));
    closeBtn->setFixedSize(28, 28);
    closeBtn->setStyleSheet("QPushButton { border: none; background: transparent; } QPushButton:hover { background-color: #3D1022; }");
    closeBtn->setToolTip("Close Equalizer Window (E)");

    headerLayout->addWidget(moreBtn);
    headerLayout->addWidget(closeBtn);
    mainLayout->addLayout(headerLayout);

    connect(closeBtn, &QPushButton::clicked, this, &QWidget::close);

    // 2. Content wrapper for disabling/graying out all EQ controls
    m_eqContentWidget = new QWidget(this);
    auto* contentLayout = new QVBoxLayout(m_eqContentWidget);
    contentLayout->setContentsMargins(0, 0, 0, 0);
    contentLayout->setSpacing(15);

    // A. Segmented Tab Selection Bar
    auto* tabBarLayout = new QHBoxLayout();
    tabBarLayout->setSpacing(0);
    tabBarLayout->setContentsMargins(0, 5, 0, 5);

    m_tab10BandsBtn = new QPushButton("10 Bands", m_eqContentWidget);
    m_tabControlsBtn = new QPushButton("Controls", m_eqContentWidget);
    m_tabAdvancedBtn = new QPushButton("Advanced", m_eqContentWidget);

    m_tab10BandsBtn->setCheckable(true);
    m_tabControlsBtn->setCheckable(true);
    m_tabAdvancedBtn->setCheckable(true);
    m_tab10BandsBtn->setChecked(true);

    m_tab10BandsBtn->setToolTip("10-Band Graphic Equalizer Curve & Presets");
    m_tabControlsBtn->setToolTip("Parametric Controls (Bass, Treble, Stereo Width, Balance)");
    m_tabAdvancedBtn->setToolTip("Advanced Parametric EQ Bands & DSP Resampler Mode");

    QString tabBtnStyle =
        "QPushButton {"
        "   font-size: 13px; font-weight: bold; color: #9FA6B2;"
        "   background-color: #181B28; border: 1px solid #242A3D;"
        "   padding: 10px 20px; outline: none;"
        "}"
        "QPushButton:hover { background-color: #232736; color: #FFFFFF; }"
        "QPushButton:checked {"
        "   background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #FF2A7A, stop:1 #8B26B6);"
        "   color: #FFFFFF; border: 1px solid #FF2A7A;"
        "}"
        "QPushButton:disabled {"
        "   color: #4C5264;"
        "   background-color: #14172A; border: 1px solid #1B2034;"
        "}"
        "QPushButton:disabled:checked {"
        "   color: #6B7080;"
        "   background-color: #1E2235; border: 1px solid #2A2F48;"
        "}";

    m_tab10BandsBtn->setStyleSheet(tabBtnStyle + "QPushButton { border-top-left-radius: 6px; border-bottom-left-radius: 6px; border-right: none; } QPushButton:checked { border-top-left-radius: 6px; border-bottom-left-radius: 6px; border: 1px solid #FF2A7A; } QPushButton:disabled:checked { border-top-left-radius: 6px; border-bottom-left-radius: 6px; border: 1px solid #2A2F48; }");
    m_tabControlsBtn->setStyleSheet(tabBtnStyle + "QPushButton { border-radius: 0px; border-right: none; } QPushButton:checked { border-radius: 0px; border: 1px solid #FF2A7A; } QPushButton:disabled:checked { border-radius: 0px; border: 1px solid #2A2F48; }");
    m_tabAdvancedBtn->setStyleSheet(tabBtnStyle + "QPushButton { border-top-right-radius: 6px; border-bottom-right-radius: 6px; } QPushButton:checked { border-top-right-radius: 6px; border-bottom-right-radius: 6px; border: 1px solid #FF2A7A; } QPushButton:disabled:checked { border-top-right-radius: 6px; border-bottom-right-radius: 6px; border: 1px solid #2A2F48; }");

    m_tabGroup = new QButtonGroup(m_eqContentWidget);
    m_tabGroup->setExclusive(true);
    m_tabGroup->addButton(m_tab10BandsBtn, 0);
    m_tabGroup->addButton(m_tabControlsBtn, 1);
    m_tabGroup->addButton(m_tabAdvancedBtn, 2);

    tabBarLayout->addWidget(m_tab10BandsBtn, 1);
    tabBarLayout->addWidget(m_tabControlsBtn, 1);
    tabBarLayout->addWidget(m_tabAdvancedBtn, 1);
    contentLayout->addLayout(tabBarLayout);

    // B. Stacked Widget for Tab Content
    m_stackedWidget = new QStackedWidget(m_eqContentWidget);

    // --- Page 0: "10 Bands" ---
    auto* page10Bands = new QWidget(m_stackedWidget);
    auto* page0Layout = new QVBoxLayout(page10Bands);
    page0Layout->setContentsMargins(0, 5, 0, 5);
    page0Layout->setSpacing(15);

    // 10-band spline graphic curve
    m_curveWidget = new EqualizerCurveWidget(page10Bands);
    page0Layout->addWidget(m_curveWidget, 1); // Stretches to fill vertical space

    connect(m_curveWidget, &EqualizerCurveWidget::bandChanged, this, [this](int idx, double db) {
        emit bandChanged(idx, db);
        if (m_presetGroup && m_presetGroup->checkedId() != 7) {
            selectPresetButton(7);
            emit presetSelected(7);
        }
        scheduleSave();
    });

    // Preset buttons row
    auto* presetLayout = new QHBoxLayout();
    presetLayout->setSpacing(8);
    presetLayout->setContentsMargins(0, 5, 0, 5);

    m_presetGroup = new QButtonGroup(page10Bands);
    m_presetGroup->setExclusive(true);

    QStringList presets = {"Flat", "Pop", "Rock", "Jazz", "Classical", "Electronic", "Hip Hop", "Custom"};
    for (int i = 0; i < presets.size(); ++i) {
        auto* btn = new QPushButton(presets[i], page10Bands);
        btn->setObjectName("PresetBtn");
        btn->setCheckable(true);
        if (i == 7) btn->setChecked(true); // Custom is active by default
        btn->setToolTip("Apply " + presets[i] + " Equalizer Preset");
        btn->setStyleSheet(
            "QPushButton { font-size: 11px; font-weight: bold; color: #C4C8D4; background-color: #1A1D2C; border: 1px solid #2E344A; border-radius: 6px; padding: 6px 12px; }"
            "QPushButton:hover { background-color: #1B1130; color: #FFFFFF; }"
            "QPushButton:checked { background-color: #7B1FA2; border-color: #FF2A7A; color: #FFFFFF; }"
            "QPushButton:disabled { color: #4C5264; background-color: #14172A; border: 1px solid #1B2034; }"
            "QPushButton:disabled:checked { color: #6B7080; background-color: #1E2235; border: 1px solid #2A2F48; }"
        );
        presetLayout->addWidget(btn);
        m_presetGroup->addButton(btn, i);
        m_presetBtns.append(btn);
    }
    page0Layout->addLayout(presetLayout);

    connect(m_presetGroup, &QButtonGroup::idClicked, this, [this](int id) {
        emit presetSelected(id);
        setPresetGains(id);
        saveSettings();
    });

    m_stackedWidget->addWidget(page10Bands); // index 0

    // --- Page 1: "Controls" ---
    auto* pageControls = new QWidget(m_stackedWidget);
    auto* page1Layout = new QVBoxLayout(pageControls);
    page1Layout->setContentsMargins(0, 5, 0, 5);
    page1Layout->setSpacing(15);

    // Parametric Sliders (2x2 Grid)
    auto* gridLayout = new QGridLayout();
    gridLayout->setSpacing(15);
    gridLayout->setContentsMargins(0, 0, 0, 0);

    // Bass Slider
    QWidget* bassCard = createParamSliderCard("Bass", -12.0, 12.0, 0.0, "dB", 0);
    gridLayout->addWidget(bassCard, 0, 0);

    // Treble Slider
    QWidget* trebleCard = createParamSliderCard("Treble", -12.0, 12.0, 0.0, "dB", 1);
    gridLayout->addWidget(trebleCard, 0, 1);

    // Stereo Width Slider
    QWidget* stereoCard = createParamSliderCard("Stereo Width", 0.0, 200.0, 100.0, "%", 2);
    gridLayout->addWidget(stereoCard, 1, 0);

    // Balance Slider
    QWidget* balanceCard = createParamSliderCard("Balance", -1.0, 1.0, 0.0, "", 3);
    gridLayout->addWidget(balanceCard, 1, 1);

    page1Layout->addLayout(gridLayout);
    page1Layout->addStretch(1);

    m_stackedWidget->addWidget(pageControls); // index 1

    // Initialize 10 advanced bands state
    double defaultFreqs[10] = {31.5, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0};
    m_advBands.clear();
    for (int i = 0; i < 10; ++i) {
        AdvBandState st;
        st.freqHz = defaultFreqs[i];
        st.gainDb = 0.0;
        st.q = 1.0;
        st.enabled = true;
        if (i == 0) st.filterType = 0; // LowShelf
        else if (i == 9) st.filterType = 2; // HighShelf
        else st.filterType = 1; // Peaking
        m_advBands.append(st);
    }

    // --- Page 2: "Advanced" ---
    auto* pageAdvanced = new QWidget(m_stackedWidget);
    auto* page2Layout = new QVBoxLayout(pageAdvanced);
    page2Layout->setContentsMargins(0, 5, 0, 5);
    page2Layout->setSpacing(12);

    // Card 1: DSP Engine Resampler & Oversampling Mode
    auto* resamplerCard = new QFrame(pageAdvanced);
    m_resamplerCard = resamplerCard;
    resamplerCard->setStyleSheet("QFrame { background-color: #131622; border: 1px solid #282E43; border-radius: 8px; }");
    auto* resamplerLayout = new QVBoxLayout(resamplerCard);
    resamplerLayout->setContentsMargins(15, 12, 15, 12);
    resamplerLayout->setSpacing(8);

    auto* resamplerHeader = new QLabel("DSP Resampler & Oversampling Engine Mode", resamplerCard);
    resamplerHeader->setObjectName("EqSectionHeaderLabel");
    resamplerLayout->addWidget(resamplerHeader);

    auto* resamplerBtnsLayout = new QHBoxLayout();
    resamplerBtnsLayout->setSpacing(8);
    m_resamplerGroup = new QButtonGroup(pageAdvanced);
    m_resamplerGroup->setExclusive(true);

    QStringList resamplerModes = {"Fast (Low CPU)", "Balanced", "High Quality (4x)", "Ultra HD"};
    for (int i = 0; i < resamplerModes.size(); ++i) {
        auto* btn = new QPushButton(resamplerModes[i], resamplerCard);
        btn->setCheckable(true);
        if (i == 1) btn->setChecked(true);
        btn->setStyleSheet(
            "QPushButton { font-size: 11px; font-weight: bold; color: #C4C8D4; background-color: #1A1D2C; border: 1px solid #2E344A; border-radius: 6px; padding: 6px 12px; }"
            "QPushButton:hover { background-color: #1B1130; color: #FFFFFF; }"
            "QPushButton:checked { background-color: #7B1FA2; border-color: #FF2A7A; color: #FFFFFF; }"
            "QPushButton:disabled { color: #4C5264; background-color: #14172A; border: 1px solid #1B2034; }"
            "QPushButton:disabled:checked { color: #6B7080; background-color: #1E2235; border: 1px solid #2A2F48; }"
        );
        btn->setToolTip("DSP Resampler Quality: " + resamplerModes[i]);
        resamplerBtnsLayout->addWidget(btn);
        m_resamplerGroup->addButton(btn, i);
        m_resamplerBtns.append(btn);
    }
    resamplerLayout->addLayout(resamplerBtnsLayout);
    page2Layout->addWidget(resamplerCard);

    connect(m_resamplerGroup, &QButtonGroup::idClicked, this, [this](int id) {
        emit resamplerQualityChanged(id);
        saveSettings();
    });

    // Card 2: Parametric Band Selector (10 pill buttons)
    auto* bandSelectorLayout = new QHBoxLayout();
    bandSelectorLayout->setSpacing(6);
    m_advBandGroup = new QButtonGroup(pageAdvanced);
    m_advBandGroup->setExclusive(true);

    QStringList bandLabels = {"31Hz", "63Hz", "125Hz", "250Hz", "500Hz", "1kHz", "2kHz", "4kHz", "8kHz", "16kHz"};
    for (int i = 0; i < 10; ++i) {
        auto* btn = new QPushButton(bandLabels[i], pageAdvanced);
        btn->setCheckable(true);
        if (i == 0) btn->setChecked(true);
        btn->setStyleSheet(
            "QPushButton { font-size: 11px; font-weight: bold; color: #A0A5B4; background-color: #161926; border: 1px solid #262B3E; border-radius: 5px; padding: 5px 4px; }"
            "QPushButton:hover { background-color: #1B1130; color: #FFFFFF; }"
            "QPushButton:checked { background-color: #7B1FA2; border-color: #FF2A7A; color: #FFFFFF; }"
            "QPushButton:disabled { color: #4C5264; background-color: #14172A; border: 1px solid #1B2034; }"
            "QPushButton:disabled:checked { color: #6B7080; background-color: #1E2235; border: 1px solid #2A2F48; }"
        );
        btn->setToolTip("Configure Parametric Filter for " + bandLabels[i] + " Frequency Band");
        bandSelectorLayout->addWidget(btn);
        m_advBandGroup->addButton(btn, i);
        m_advBandBtns.append(btn);
    }
    page2Layout->addLayout(bandSelectorLayout);

    connect(m_advBandGroup, &QButtonGroup::idClicked, this, [this](int id) {
        updateAdvancedBandControls(id);
        saveSettings();
    });

    // Card 3: Band Parameters Editor
    auto* bandEditorCard = new QFrame(pageAdvanced);
    m_bandEditorCard = bandEditorCard;
    bandEditorCard->setStyleSheet("QFrame { background-color: #131622; border: 1px solid #282E43; border-radius: 8px; }");
    auto* bandEditorLayout = new QGridLayout(bandEditorCard);
    bandEditorLayout->setContentsMargins(15, 15, 15, 15);
    bandEditorLayout->setSpacing(12);

    // Row 0: Filter Type & Band Enable Toggle
    auto* typeLabel = new QLabel("Filter Type:", bandEditorCard);
    typeLabel->setObjectName("EqHeaderLabel");
    m_advFilterTypeCombo = new QComboBox(bandEditorCard);
    m_advFilterTypeCombo->addItems({"Low Shelf", "Peaking", "High Shelf", "Low Pass", "High Pass", "Bandpass", "Notch"});
    m_advFilterTypeCombo->setStyleSheet(
        "QComboBox { background-color: #1C2030; color: #FFFFFF; border: 1px solid #30374E; border-radius: 5px; padding: 4px 8px; font-size: 12px; }"
        "QComboBox::drop-down { border: none; }"
        "QComboBox QAbstractItemView { background-color: #1C2030; color: #FFFFFF; selection-background-color: #7B1FA2; }"
    );
    m_advFilterTypeCombo->setToolTip("Select Parametric Filter Shape (Low Shelf, Peaking, High Shelf, Pass/Notch)");

    auto* toggleContainer = new QWidget(bandEditorCard);
    auto* toggleLayout = new QHBoxLayout(toggleContainer);
    toggleLayout->setContentsMargins(0, 0, 0, 0);
    auto* enableLabel = new QLabel("Active:", bandEditorCard);
    enableLabel->setObjectName("EqHeaderLabel");
    m_advBandToggle = new ToggleSwitch(bandEditorCard);
    m_advBandToggle->setChecked(true);
    m_advBandToggle->setToolTip("Activate / Deactivate Selected Frequency Band");
    toggleLayout->addWidget(enableLabel);
    toggleLayout->addWidget(m_advBandToggle);
    toggleLayout->addStretch();

    bandEditorLayout->addWidget(typeLabel, 0, 0);
    bandEditorLayout->addWidget(m_advFilterTypeCombo, 0, 1);
    bandEditorLayout->addWidget(toggleContainer, 0, 2);

    // Row 1: Center Frequency
    auto* freqLabel = new QLabel("Center Freq:", bandEditorCard);
    freqLabel->setObjectName("EqHeaderLabel");
    m_advFreqSlider = new QSlider(Qt::Horizontal, bandEditorCard);
    m_advFreqSlider->setRange(20, 20000);
    m_advFreqSlider->setValue(1000);
    m_advFreqSlider->setToolTip("Adjust Center Frequency (Hz)");
    const QString doubleSpinStyle =
        "QDoubleSpinBox { background-color: #1C2030; color: #FFFFFF; border: 1px solid #30374E; border-radius: 5px; padding: 4px; padding-right: 22px; font-size: 12px; }"
        "QDoubleSpinBox::up-button { subcontrol-origin: border; subcontrol-position: top right; width: 20px; height: 12px; border-left: 1px solid #30374E; border-bottom: 1px solid #30374E; border-top-right-radius: 5px; background-color: #262D42; }"
        "QDoubleSpinBox::up-button:hover { background-color: #7B1FA2; }"
        "QDoubleSpinBox::down-button { subcontrol-origin: border; subcontrol-position: bottom right; width: 20px; height: 12px; border-left: 1px solid #30374E; border-bottom-right-radius: 5px; background-color: #262D42; }"
        "QDoubleSpinBox::down-button:hover { background-color: #7B1FA2; }";

    m_advFreqSpin = new QDoubleSpinBox(bandEditorCard);
    m_advFreqSpin->setRange(20.0, 20000.0);
    m_advFreqSpin->setDecimals(1);
    m_advFreqSpin->setValue(1000.0);
    m_advFreqSpin->setSuffix(" Hz");
    m_advFreqSpin->setStyleSheet(doubleSpinStyle);
    m_advFreqSpin->setToolTip("Adjust Center Frequency (Hz)");

    bandEditorLayout->addWidget(freqLabel, 1, 0);
    bandEditorLayout->addWidget(m_advFreqSlider, 1, 1);
    bandEditorLayout->addWidget(m_advFreqSpin, 1, 2);

    // Row 2: Quality Factor (Q)
    auto* qLabel = new QLabel("Q Factor:", bandEditorCard);
    qLabel->setObjectName("EqHeaderLabel");
    m_advQSlider = new QSlider(Qt::Horizontal, bandEditorCard);
    m_advQSlider->setRange(1, 240); // 0.1 to 24.0 (value / 10.0)
    m_advQSlider->setValue(10);
    m_advQSlider->setToolTip("Adjust Quality Factor / Bandwidth (Q Factor)");
    m_advQSpin = new QDoubleSpinBox(bandEditorCard);
    m_advQSpin->setRange(0.1, 24.0);
    m_advQSpin->setSingleStep(0.1);
    m_advQSpin->setDecimals(2);
    m_advQSpin->setValue(1.00);
    m_advQSpin->setStyleSheet(doubleSpinStyle);
    m_advQSpin->setToolTip("Adjust Quality Factor / Bandwidth (Q Factor)");

    bandEditorLayout->addWidget(qLabel, 2, 0);
    bandEditorLayout->addWidget(m_advQSlider, 2, 1);
    bandEditorLayout->addWidget(m_advQSpin, 2, 2);

    // Row 3: Band Gain (dB)
    auto* gainLabel = new QLabel("Band Gain:", bandEditorCard);
    gainLabel->setObjectName("EqHeaderLabel");
    m_advGainSlider = new QSlider(Qt::Horizontal, bandEditorCard);
    m_advGainSlider->setRange(-120, 120); // -12.0 to +12.0 dB
    m_advGainSlider->setValue(0);
    m_advGainSlider->setToolTip("Adjust Band Gain (-12.0 dB to +12.0 dB)");
    m_advGainLabel = new QLabel("0.0 dB", bandEditorCard);
    m_advGainLabel->setObjectName("EqHeaderLabel");
    m_advGainLabel->setAlignment(Qt::AlignCenter);

    bandEditorLayout->addWidget(gainLabel, 3, 0);
    bandEditorLayout->addWidget(m_advGainSlider, 3, 1);
    bandEditorLayout->addWidget(m_advGainLabel, 3, 2);

    page2Layout->addWidget(bandEditorCard);
    page2Layout->addStretch(1);

    m_stackedWidget->addWidget(pageAdvanced); // index 2

    // Sync UI changes between Slider and SpinBox / Label on Advanced page
    connect(m_advFreqSlider, &QSlider::valueChanged, this, [this](int val) {
        if (!m_advFreqSpin) return;
        QSignalBlocker blocker(m_advFreqSpin);
        m_advFreqSpin->setValue(val);
        m_advBands[m_currentAdvBandIdx].freqHz = val;
        AdvBandState& b = m_advBands[m_currentAdvBandIdx];
        emit advancedBandChanged(m_currentAdvBandIdx, b.freqHz, b.enabled ? b.gainDb : 0.0, b.q, b.filterType);
        scheduleSave();
    });
    connect(m_advFreqSpin, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [this](double val) {
        if (!m_advFreqSlider) return;
        QSignalBlocker blocker(m_advFreqSlider);
        m_advFreqSlider->setValue(qRound(val));
        m_advBands[m_currentAdvBandIdx].freqHz = val;
        AdvBandState& b = m_advBands[m_currentAdvBandIdx];
        emit advancedBandChanged(m_currentAdvBandIdx, b.freqHz, b.enabled ? b.gainDb : 0.0, b.q, b.filterType);
        scheduleSave();
    });

    connect(m_advQSlider, &QSlider::valueChanged, this, [this](int val) {
        if (!m_advQSpin) return;
        double realVal = val / 10.0;
        QSignalBlocker blocker(m_advQSpin);
        m_advQSpin->setValue(realVal);
        m_advBands[m_currentAdvBandIdx].q = realVal;
        AdvBandState& b = m_advBands[m_currentAdvBandIdx];
        emit advancedBandChanged(m_currentAdvBandIdx, b.freqHz, b.enabled ? b.gainDb : 0.0, b.q, b.filterType);
        scheduleSave();
    });
    connect(m_advQSpin, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [this](double val) {
        if (!m_advQSlider) return;
        QSignalBlocker blocker(m_advQSlider);
        m_advQSlider->setValue(qRound(val * 10.0));
        m_advBands[m_currentAdvBandIdx].q = val;
        AdvBandState& b = m_advBands[m_currentAdvBandIdx];
        emit advancedBandChanged(m_currentAdvBandIdx, b.freqHz, b.enabled ? b.gainDb : 0.0, b.q, b.filterType);
        scheduleSave();
    });

    connect(m_advGainSlider, &QSlider::valueChanged, this, [this](int val) {
        if (!m_advGainLabel) return;
        double db = val / 10.0;
        m_advGainLabel->setText(QString("%1%2 dB").arg(db >= 0 ? "+" : "").arg(db, 0, 'f', 1));
        m_advBands[m_currentAdvBandIdx].gainDb = db;
        AdvBandState& b = m_advBands[m_currentAdvBandIdx];
        emit advancedBandChanged(m_currentAdvBandIdx, b.freqHz, b.enabled ? b.gainDb : 0.0, b.q, b.filterType);
        scheduleSave();
    });

    connect(m_advFilterTypeCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this, [this](int idx) {
        m_advBands[m_currentAdvBandIdx].filterType = idx;
        AdvBandState& b = m_advBands[m_currentAdvBandIdx];
        emit advancedBandChanged(m_currentAdvBandIdx, b.freqHz, b.enabled ? b.gainDb : 0.0, b.q, b.filterType);
        scheduleSave();
    });

    connect(m_advBandToggle, &ToggleSwitch::toggled, this, [this](bool checked) {
        m_advBands[m_currentAdvBandIdx].enabled = checked;
        AdvBandState& b = m_advBands[m_currentAdvBandIdx];
        emit advancedBandChanged(m_currentAdvBandIdx, b.freqHz, checked ? b.gainDb : 0.0, b.q, b.filterType);
        scheduleSave();
    });

    updateAdvancedBandControls(0);

    contentLayout->addWidget(m_stackedWidget, 1);

    connect(m_tabGroup, &QButtonGroup::idClicked, this, [this](int id) {
        m_stackedWidget->setCurrentIndex(id);
        saveSettings();
    });

    // C. Preamp Slider & Reset Button (Bottom Row - Global across both tabs)
    auto* bottomLayout = new QHBoxLayout();
    bottomLayout->setSpacing(20);
    bottomLayout->setContentsMargins(5, 5, 5, 0);

    // Preamp block
    auto* preampVLayout = new QVBoxLayout();
    preampVLayout->setSpacing(2);
    
    auto* preampHeader = new QLabel("Preamp", m_eqContentWidget);
    m_preampHeaderLabel = preampHeader;
    preampHeader->setObjectName("EqHeaderLabel");
    preampVLayout->addWidget(preampHeader);

    auto* preampSliderLayout = new QHBoxLayout();
    preampSliderLayout->setSpacing(10);

    auto* preampMinLabel = new QLabel("-12dB", m_eqContentWidget);
    m_preampMinLabel = preampMinLabel;
    preampMinLabel->setObjectName("EqSubLabel");

    m_preampSlider = new QSlider(Qt::Horizontal, m_eqContentWidget);
    m_preampSlider->setRange(-120, 120); // -12.0dB to +12.0dB
    m_preampSlider->setValue(0);
    m_preampSlider->setToolTip("Master Preamp Gain (+0.0 dB)");

    auto* preampMaxLabel = new QLabel("+12dB", m_eqContentWidget);
    m_preampMaxLabel = preampMaxLabel;
    preampMaxLabel->setObjectName("EqSubLabel");

    preampSliderLayout->addWidget(preampMinLabel);
    preampSliderLayout->addWidget(m_preampSlider);
    preampSliderLayout->addWidget(preampMaxLabel);
    preampVLayout->addLayout(preampSliderLayout);

    m_preampValueLabel = new QLabel("0.0 dB", m_eqContentWidget);
    m_preampValueLabel->setObjectName("EqValueLabel");
    m_preampValueLabel->setAlignment(Qt::AlignCenter);
    preampVLayout->addWidget(m_preampValueLabel);

    bottomLayout->addLayout(preampVLayout, 1);

    // Reset button next to Preamp
    m_resetBtn = new QPushButton("Reset", m_eqContentWidget);
    m_resetBtn->setObjectName("ResetBtn");
    m_resetBtn->setIcon(QIcon(":/resources/icons/reset.png"));
    m_resetBtn->setIconSize(QSize(14, 14));
    m_resetBtn->setFixedSize(90, 36);
    m_resetBtn->setToolTip("Reset Active Tab Settings to Default");

    bottomLayout->addWidget(m_resetBtn, 0, Qt::AlignBottom);
    contentLayout->addLayout(bottomLayout);

    mainLayout->addWidget(m_eqContentWidget, 1);

    // Sync initial toggle state
    m_eqContentWidget->setEnabled(m_enableToggle->isChecked());

    // Connect toggle switch to gray out content panel
    connect(m_enableToggle, &ToggleSwitch::toggled, this, [this](bool checked) {
        m_eqContentWidget->setEnabled(checked);
        emit eqToggled(checked);
        saveSettings();
    });

    // Preamp Slider connection
    connect(m_preampSlider, &QSlider::valueChanged, this, [this](int val) {
        double db = val / 10.0;
        QString text = QString("%1%2 dB").arg(db >= 0 ? "+" : "").arg(db, 0, 'f', 1);
        m_preampValueLabel->setText(text);
        m_preampSlider->setToolTip("Master Preamp Gain (" + text + ")");
        emit sliderParamChanged(4, db);
        scheduleSave();
    });

    connect(m_resetBtn, &QPushButton::clicked, this, [this]() {
        auto resetPreamp = [this]() {
            if (m_preampSlider) {
                QSignalBlocker blocker(m_preampSlider);
                m_preampSlider->setValue(0);
                emit sliderParamChanged(4, 0.0);
            }
            if (m_preampValueLabel) {
                m_preampValueLabel->setText("+0.0 dB");
            }
            if (m_preampSlider) {
                m_preampSlider->setToolTip("Master Preamp Gain (+0.0 dB)");
            }
        };

        if (m_stackedWidget && m_stackedWidget->currentIndex() == 0) {
            // Tab 1 (10 Bands) Active: Reset 10 EQ bands, presets, AND preamp.
            emit resetEqClicked();
            QVector<double> flat(10, 0.0);
            m_curveWidget->setGains(flat);
            for (int i = 0; i < 10; ++i) {
                emit bandChanged(i, 0.0);
            }
            selectPresetButton(0);
            emit presetSelected(0);
            resetPreamp();
        } else if (m_stackedWidget && m_stackedWidget->currentIndex() == 1) {
            auto resetParamSlider = [this](QSlider* s, int val, int paramIdx, double realVal) {
                if (!s) return;
                QSignalBlocker blocker(s);
                s->setValue(val);
                emit sliderParamChanged(paramIdx, realVal);
            };
            resetParamSlider(m_bassSlider, 0, 0, 0.0);
            resetParamSlider(m_trebleSlider, 0, 1, 0.0);
            resetParamSlider(m_stereoSlider, 100, 2, 100.0);
            resetParamSlider(m_balanceSlider, 0, 3, 0.0);
            resetParamSlider(m_preampSlider, 0, 4, 0.0);

            if (m_bassValueLabel) m_bassValueLabel->setText("+0.0dB");
            if (m_trebleValueLabel) m_trebleValueLabel->setText("+0.0dB");
            if (m_stereoValueLabel) m_stereoValueLabel->setText("100%");
            if (m_balanceValueLabel) m_balanceValueLabel->setText("+0.00");
            if (m_preampValueLabel) m_preampValueLabel->setText("+0.0 dB");
        } else if (m_stackedWidget && m_stackedWidget->currentIndex() == 2) {
            // Tab 3 (Advanced) Active: Reset Advanced Parametric Bands,
            // Resampler Quality, AND preamp.
            double defaultFreqs[10] = {31.5, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0};
            for (int i = 0; i < 10 && i < m_advBands.size(); ++i) {
                m_advBands[i].freqHz = defaultFreqs[i];
                m_advBands[i].gainDb = 0.0;
                m_advBands[i].q = 1.0;
                m_advBands[i].enabled = true;
                if (i == 0) m_advBands[i].filterType = 0;
                else if (i == 9) m_advBands[i].filterType = 2;
                else m_advBands[i].filterType = 1;

                const AdvBandState& b = m_advBands[i];
                emit advancedBandChanged(i, b.freqHz, b.gainDb, b.q, b.filterType);
            }
            if (m_resamplerGroup) {
                if (auto* btn = m_resamplerGroup->button(1)) {
                    btn->setChecked(true);
                }
                emit resamplerQualityChanged(1);
            }
            updateAdvancedBandControls(m_currentAdvBandIdx);
            resetPreamp();
        }
        saveSettings();
    });
}

void EqualizerWindow::emitInitialState() {
    emit eqToggled(m_enableToggle->isChecked());
    
    // Emit 10 bands gains
    QVector<double> gains = m_curveWidget->getGains();
    for (int i = 0; i < gains.size(); ++i) {
        emit bandChanged(i, gains[i]);
    }

    // Emit preset selection
    int presetIdx = m_presetGroup->checkedId();
    if (presetIdx >= 0) {
        emit presetSelected(presetIdx);
    }

    // Emit preamp
    emit sliderParamChanged(4, m_preampSlider->value() / 10.0);

    // Emit controls
    emit sliderParamChanged(0, m_bassSlider ? m_bassSlider->value() / 10.0 : 0.0);
    emit sliderParamChanged(1, m_trebleSlider ? m_trebleSlider->value() / 10.0 : 0.0);
    emit sliderParamChanged(2, m_stereoSlider ? m_stereoSlider->value() : 100.0);
    emit sliderParamChanged(3, m_balanceSlider ? m_balanceSlider->value() / 100.0 : 0.0);

    // Emit resampler quality
    if (m_resamplerGroup) {
        emit resamplerQualityChanged(m_resamplerGroup->checkedId());
    }

    // Emit advanced bands
    for (int i = 0; i < m_advBands.size(); ++i) {
        const AdvBandState& b = m_advBands[i];
        emit advancedBandChanged(i, b.freqHz, b.enabled ? b.gainDb : 0.0, b.q, b.filterType);
    }
}

QWidget* EqualizerWindow::createParamSliderCard(const QString& name, double minVal, double maxVal, double defaultVal, const QString& unit, int paramIdx) {
    auto* card = new QFrame(m_eqContentWidget);
    card->setObjectName("EqControlPanel");
    
    auto* cardLayout = new QVBoxLayout(card);
    cardLayout->setContentsMargins(15, 10, 15, 10);
    cardLayout->setSpacing(2);

    // Card Header (Name + Info)
    auto* headerLayout = new QHBoxLayout();
    
    auto* labelName = new QLabel(name, card);
    labelName->setObjectName("EqHeaderLabel");

    auto* infoBtn = new QPushButton(card);
    infoBtn->setIcon(QIcon(":/resources/icons/info.png"));
    infoBtn->setIconSize(QSize(12, 12));
    infoBtn->setFixedSize(16, 16);
    infoBtn->setStyleSheet("QPushButton { border: none; background: transparent; } QPushButton:hover { background-color: #1B1E28; }");
    infoBtn->setToolTip("Information about " + name + " parameter adjustment (" + QString::number(minVal) + unit + " to " + QString::number(maxVal) + unit + ")");

    headerLayout->addWidget(labelName);
    headerLayout->addWidget(infoBtn);
    headerLayout->addStretch();
    cardLayout->addLayout(headerLayout);

    // Slider Row
    auto* sliderLayout = new QHBoxLayout();
    sliderLayout->setSpacing(8);

    auto* minLabel = new QLabel(QString::number(minVal) + unit, card);
    minLabel->setObjectName("EqSubLabel");

    auto* slider = new QSlider(Qt::Horizontal, card);
    
    // Scale mapping based on value ranges
    if (minVal == -12.0) {
        slider->setRange(-120, 120); // -12.0 to +12.0 dB
        slider->setValue(static_cast<int>(defaultVal * 10));
        m_bassSlider = (paramIdx == 0) ? slider : m_bassSlider;
        m_trebleSlider = (paramIdx == 1) ? slider : m_trebleSlider;
    } else if (minVal == 0.0) {
        slider->setRange(0, 200);    // 0% to 200%
        slider->setValue(static_cast<int>(defaultVal));
        m_stereoSlider = slider;
    } else {
        slider->setRange(-100, 100);  // -1.00 to +1.00
        slider->setValue(static_cast<int>(defaultVal * 100));
        m_balanceSlider = slider;
    }

    auto* maxLabel = new QLabel(QString("%1%2%3").arg(maxVal > 0 && unit == "dB" ? "+" : "").arg(maxVal).arg(unit), card);
    maxLabel->setObjectName("EqSubLabel");

    sliderLayout->addWidget(minLabel);
    sliderLayout->addWidget(slider);
    sliderLayout->addWidget(maxLabel);
    cardLayout->addLayout(sliderLayout);

    // Value display label below
    auto* valLabel = new QLabel(card);
    valLabel->setObjectName("EqValueLabel");
    valLabel->setAlignment(Qt::AlignCenter);
    cardLayout->addWidget(valLabel);

    auto updateValLabel = [valLabel, minVal, unit, slider, name](double val) {
        QString text;
        if (minVal == -12.0) {
            text = QString("%1%2dB").arg(val >= 0 ? "+" : "").arg(val, 0, 'f', 1);
        } else if (minVal == 0.0) {
            text = QString("%1%").arg(val, 0, 'f', 0);
        } else {
            text = QString("%1%2").arg(val >= 0 ? "+" : "").arg(val, 0, 'f', 2);
        }
        valLabel->setText(text);
        if (slider) slider->setToolTip("Adjust " + name + ": " + text);
    };
    updateValLabel(defaultVal);

    if (paramIdx == 0) m_bassValueLabel = valLabel;
    else if (paramIdx == 1) m_trebleValueLabel = valLabel;
    else if (paramIdx == 2) m_stereoValueLabel = valLabel;
    else if (paramIdx == 3) m_balanceValueLabel = valLabel;

    connect(slider, &QSlider::valueChanged, this, [this, paramIdx, minVal, updateValLabel](int val) {
        double realVal = val;
        if (minVal == -12.0) realVal = val / 10.0;
        else if (minVal == -1.0) realVal = val / 100.0;
        
        updateValLabel(realVal);
        emit sliderParamChanged(paramIdx, realVal);
        scheduleSave();
    });

    return card;
}

void EqualizerWindow::selectPresetButton(int presetIdx) {
    if (presetIdx >= 0 && presetIdx < m_presetBtns.size()) {
        m_presetBtns[presetIdx]->setChecked(true);
    }
}

void EqualizerWindow::setPresetGains(int presetIdx) {
    QVector<double> gains(10, 0.0);
    switch (presetIdx) {
        case 0: // Flat
            gains.fill(0.0);
            break;
        case 1: // Pop
            gains = {-1.0, 1.5, 2.5, 3.0, 1.0, -1.0, -1.5, -1.5, -1.0, -1.0};
            break;
        case 2: // Rock
            gains = {4.0, 3.0, -2.0, -4.0, -1.5, 1.0, 3.0, 4.5, 5.0, 5.0};
            break;
        case 3: // Jazz
            gains = {3.0, 2.0, 1.0, 1.5, -1.0, -1.0, 0.0, 1.5, 2.5, 3.0};
            break;
        case 4: // Classical
            gains = {3.5, 2.5, 2.0, 1.5, -1.0, -1.0, -0.5, 1.0, 2.0, 2.5};
            break;
        case 5: // Electronic
            gains = {4.5, 3.5, 1.0, 0.0, -2.0, 1.5, 1.0, 1.0, 3.5, 4.0};
            break;
        case 6: // Hip Hop
            gains = {5.0, 4.0, 1.5, 2.5, -1.0, -1.5, 0.5, -0.5, 2.0, 3.0};
            break;
        default: // Custom
            return; // Don't alter gains
    }
    m_curveWidget->setGains(gains);
    for (int i = 0; i < 10; ++i) {
        emit bandChanged(i, gains[i]);
    }
}

void EqualizerWindow::setEqGains(const QVector<double>& gains) {
    m_curveWidget->setGains(gains);
}

void EqualizerWindow::setEqEnabled(bool enabled) {
    QSignalBlocker block(m_enableToggle);
    m_enableToggle->setChecked(enabled);
    m_eqContentWidget->setEnabled(enabled);
    // Do NOT re-emit eqToggled — the caller already knows the new state.
}

void EqualizerWindow::updateAdvancedBandControls(int bandIdx) {
    if (bandIdx < 0 || bandIdx >= m_advBands.size()) return;
    m_currentAdvBandIdx = bandIdx;
    const AdvBandState& b = m_advBands[bandIdx];

    if (m_advFilterTypeCombo) {
        QSignalBlocker blocker(m_advFilterTypeCombo);
        m_advFilterTypeCombo->setCurrentIndex(b.filterType);
    }
    if (m_advFreqSlider && m_advFreqSpin) {
        QSignalBlocker blocker1(m_advFreqSlider);
        QSignalBlocker blocker2(m_advFreqSpin);
        m_advFreqSlider->setValue(qRound(b.freqHz));
        m_advFreqSpin->setValue(b.freqHz);
    }
    if (m_advQSlider && m_advQSpin) {
        QSignalBlocker blocker1(m_advQSlider);
        QSignalBlocker blocker2(m_advQSpin);
        m_advQSlider->setValue(qRound(b.q * 10.0));
        m_advQSpin->setValue(b.q);
    }
    if (m_advGainSlider && m_advGainLabel) {
        QSignalBlocker blocker(m_advGainSlider);
        m_advGainSlider->setValue(qRound(b.gainDb * 10.0));
        m_advGainLabel->setText(QString("%1%2 dB").arg(b.gainDb >= 0 ? "+" : "").arg(b.gainDb, 0, 'f', 1));
    }
    if (m_advBandToggle) {
        QSignalBlocker blocker(m_advBandToggle);
        m_advBandToggle->setChecked(b.enabled);
    }
}

// Restyle every Equalizer control from the active palette. Called once at
// construction and on every ThemeManager::themeChanged, so the floating EQ
// window follows the Light/Dark/colored themes instead of staying pinned to
// the hard-coded purple look.
void EqualizerWindow::applyTheme(const ThemePalette& p) {
    const QString primaryTextC  = p.primaryText.name();
    const QString secondaryTextC = p.secondaryText.name();
    const QString mutedTextC    = p.mutedText.name();
    const QString accentC       = p.primaryAccent.name();
    const QString accent2C      = p.secondaryAccent.name();
    const QString surfaceC      = p.headerBg.name();      // control / button surface
    const QString cardBgC       = p.cardBg.name();
    const QString cardBorderC   = p.cardBorder.name();
    const QString hoverC        = p.itemHoverBg.name();

    // --- Segmented tab buttons (10 Bands / Controls / Advanced) ---
    const QString tabBase = QString(
        "QPushButton { font-size: 13px; font-weight: bold; color: %1; background-color: %2;"
        " border: 1px solid %3; padding: 10px 20px; outline: none; }"
        "QPushButton:hover { background-color: %4; color: %5; }"
        "QPushButton:checked { background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 %6, stop:1 %7);"
        " color: #FFFFFF; border: 1px solid %6; }"
        "QPushButton:disabled { color: %8; background-color: %2; border: 1px solid %3; }"
        "QPushButton:disabled:checked { color: %8; background-color: %4; border: 1px solid %3; }"
    ).arg(secondaryTextC, surfaceC, cardBorderC, hoverC, primaryTextC, accent2C, accentC, mutedTextC);

    if (m_tab10BandsBtn) {
        m_tab10BandsBtn->setStyleSheet(tabBase +
            "QPushButton { border-top-left-radius: 6px; border-bottom-left-radius: 6px; border-right: none; }"
            "QPushButton:checked { border-top-left-radius: 6px; border-bottom-left-radius: 6px; border: 1px solid " + accent2C + "; }"
            "QPushButton:disabled:checked { border-top-left-radius: 6px; border-bottom-left-radius: 6px; border: 1px solid " + cardBorderC + "; }");
    }
    if (m_tabControlsBtn) {
        m_tabControlsBtn->setStyleSheet(tabBase +
            "QPushButton { border-radius: 0px; border-right: none; }"
            "QPushButton:checked { border-radius: 0px; border: 1px solid " + accent2C + "; }"
            "QPushButton:disabled:checked { border-radius: 0px; border: 1px solid " + cardBorderC + "; }");
    }
    if (m_tabAdvancedBtn) {
        m_tabAdvancedBtn->setStyleSheet(tabBase +
            "QPushButton { border-top-right-radius: 6px; border-bottom-right-radius: 6px; }"
            "QPushButton:checked { border-top-right-radius: 6px; border-bottom-right-radius: 6px; border: 1px solid " + accent2C + "; }"
            "QPushButton:disabled:checked { border-top-right-radius: 6px; border-bottom-right-radius: 6px; border: 1px solid " + cardBorderC + "; }");
    }

    // --- Pill buttons (presets, resampler modes, advanced bands) ---
    const QString pillStyle = QString(
        "QPushButton { font-size: 11px; font-weight: bold; color: %1; background-color: %2;"
        " border: 1px solid %3; border-radius: 6px; padding: 6px 12px; }"
        "QPushButton:hover { background-color: %4; color: %5; }"
        "QPushButton:checked { background-color: %6; border-color: %7; color: #FFFFFF; }"
        "QPushButton:disabled { color: %8; background-color: %2; border: 1px solid %3; }"
        "QPushButton:disabled:checked { color: %8; background-color: %4; border: 1px solid %3; }"
    ).arg(secondaryTextC, surfaceC, cardBorderC, hoverC, primaryTextC, accentC, accent2C, mutedTextC);
    for (QPushButton* b : m_presetBtns)    if (b) b->setStyleSheet(pillStyle);
    for (QPushButton* b : m_resamplerBtns) if (b) b->setStyleSheet(pillStyle);
    for (QPushButton* b : m_advBandBtns)   if (b) b->setStyleSheet(pillStyle);

    // --- Section cards (Resampler + Band editor) ---
    const QString cardStyle = QString(
        "QFrame { background-color: %1; border: 1px solid %2; border-radius: 8px; }"
    ).arg(cardBgC, cardBorderC);
    if (m_resamplerCard) m_resamplerCard->setStyleSheet(cardStyle);
    if (m_bandEditorCard) m_bandEditorCard->setStyleSheet(cardStyle);

    // --- Filter type combo + advance frequency/Q spin boxes ---
    const QString comboStyle = QString(
        "QComboBox { background-color: %1; color: %2; border: 1px solid %3; border-radius: 5px;"
        " padding: 4px 8px; font-size: 12px; }"
        "QComboBox::drop-down { border: none; }"
        "QComboBox QAbstractItemView { background-color: %1; color: %2;"
        " selection-background-color: %4; border: 1px solid %3; outline: none; }"
    ).arg(surfaceC, primaryTextC, cardBorderC, accentC);
    if (m_advFilterTypeCombo) m_advFilterTypeCombo->setStyleSheet(comboStyle);

    const QString spinStyle = QString(
        "QDoubleSpinBox { background-color: %1; color: %2; border: 1px solid %3; border-radius: 5px;"
        " padding: 4px; padding-right: 22px; font-size: 12px; }"
        "QDoubleSpinBox::up-button { subcontrol-origin: border; subcontrol-position: top right;"
        " width: 20px; height: 12px; border-left: 1px solid %3; border-bottom: 1px solid %3;"
        " border-top-right-radius: 5px; background-color: %5; }"
        "QDoubleSpinBox::up-button:hover { background-color: %4; }"
        "QDoubleSpinBox::down-button { subcontrol-origin: border; subcontrol-position: bottom right;"
        " width: 20px; height: 12px; border-left: 1px solid %3; border-bottom-right-radius: 5px;"
        " background-color: %5; }"
        "QDoubleSpinBox::down-button:hover { background-color: %4; }"
    ).arg(surfaceC, primaryTextC, cardBorderC, accentC, hoverC);
    if (m_advFreqSpin) m_advFreqSpin->setStyleSheet(spinStyle);
    if (m_advQSpin)    m_advQSpin->setStyleSheet(spinStyle);

    // --- Labels, tagged by objectName role in setupUi() ---
    const QString titleLabelStyle   = QString("font-size: 18px; font-weight: bold; color: %1;").arg(primaryTextC);
    const QString sectionLabelStyle = QString("font-size: 13px; font-weight: bold; color: %1; border: none;").arg(primaryTextC);
    const QString headerLabelStyle  = QString("font-size: 12px; font-weight: bold; color: %1; border: none;").arg(primaryTextC);
    const QString subLabelStyle     = QString("font-size: 10px; color: %1; border: none;").arg(mutedTextC);

    for (QLabel* l : findChildren<QLabel*>()) {
        const QString on = l->objectName();
        if (on == "EqTitleLabel") {
            l->setStyleSheet(titleLabelStyle);
        } else if (on == "EqSectionHeaderLabel") {
            l->setStyleSheet(sectionLabelStyle);
        } else if (on == "EqHeaderLabel") {
            l->setStyleSheet(headerLabelStyle);
        } else if (on == "EqValueLabel" || on == "EqSubLabel") {
            l->setStyleSheet(subLabelStyle);
        }
    }
}

// Window Dragging & Corner Resizing handlers
void EqualizerWindow::mousePressEvent(QMouseEvent* event) {
    if (event->button() == Qt::LeftButton) {
        QPoint pos = event->pos();
        if (pos.x() > width() - 15 && pos.y() > height() - 15) {
            m_isResizing = true;
            m_resizeStartSize = size();
            m_resizeStartPos = mapToParent(event->pos());
            event->accept();
        } else if (pos.y() < 60) { // Drag zone
            m_dragPosition = event->pos();
            event->accept();
        }
    }
}

void EqualizerWindow::mouseMoveEvent(QMouseEvent* event) {
    if (m_isResizing) {
        QPoint delta = mapToParent(event->pos()) - m_resizeStartPos;
        int newW = m_resizeStartSize.width() + delta.x();
        int newH = m_resizeStartSize.height() + delta.y();
        if (QWidget* parentWin = parentWidget()) {
            int maxW = parentWin->width() - x();
            int maxH = parentWin->height() - y();
            newW = qBound(650, newW, qMax(650, maxW));
            newH = qBound(480, newH, qMax(480, maxH));
        } else {
            newW = qMax(650, newW);
            newH = qMax(480, newH);
        }
        resize(newW, newH);
        event->accept();
    } else if (event->buttons() & Qt::LeftButton && !m_dragPosition.isNull()) {
        QPoint newPos = mapToParent(event->pos()) - m_dragPosition;
        if (QWidget* parentWin = parentWidget()) {
            QRect parentRect = parentWin->rect();
            int minX = 0;
            int minY = 0;
            int maxX = parentRect.width() - width();
            int maxY = parentRect.height() - height();

            newPos.setX(qBound(minX, newPos.x(), qMax(minX, maxX)));
            newPos.setY(qBound(minY, newPos.y(), qMax(minY, maxY)));
        }
        move(newPos);
        event->accept();
    }
}

void EqualizerWindow::mouseReleaseEvent(QMouseEvent* event) {
    Q_UNUSED(event);
    m_isResizing = false;
    m_dragPosition = QPoint();
    saveSettings();
}

void EqualizerWindow::scheduleSave() {
    if (m_saveTimer) {
        if (!m_saveTimer->isActive()) {
            m_saveTimer->start();
        }
    } else {
        saveSettings();
    }
}

void EqualizerWindow::saveSettings() {
    QSettings s("PlayTune", "Settings");
    s.setValue("eq_enabled", m_enableToggle->isChecked());
    s.setValue("eq_tab_index", m_stackedWidget->currentIndex());
    
    QVector<double> gains = m_curveWidget->getGains();
    QVariantList gainsList;
    for (double g : gains) gainsList.append(g);
    s.setValue("eq_gains", gainsList);

    s.setValue("eq_preset", m_presetGroup->checkedId());

    if (m_preampSlider) {
        s.setValue("eq_preamp", m_preampSlider->value() / 10.0);
    }

    if (m_bassSlider) s.setValue("eq_param_bass", m_bassSlider->value() / 10.0);
    if (m_trebleSlider) s.setValue("eq_param_treble", m_trebleSlider->value() / 10.0);
    if (m_stereoSlider) s.setValue("eq_param_stereo", m_stereoSlider->value());
    if (m_balanceSlider) s.setValue("eq_param_balance", m_balanceSlider->value() / 100.0);

    if (m_resamplerGroup) s.setValue("eq_resampler_quality", m_resamplerGroup->checkedId());

    s.setValue("eq_current_adv_band", m_currentAdvBandIdx);

    for (int i = 0; i < m_advBands.size(); ++i) {
        s.setValue(QString("eq_adv_band_%1_type").arg(i), m_advBands[i].filterType);
        s.setValue(QString("eq_adv_band_%1_freq").arg(i), m_advBands[i].freqHz);
        s.setValue(QString("eq_adv_band_%1_gain").arg(i), m_advBands[i].gainDb);
        s.setValue(QString("eq_adv_band_%1_q").arg(i), m_advBands[i].q);
        s.setValue(QString("eq_adv_band_%1_enabled").arg(i), m_advBands[i].enabled);
    }

    s.setValue("eq_geometry_pos", pos());
    s.setValue("eq_geometry_size", size());
    s.setValue("eq_geometry_saved", true);
}

void EqualizerWindow::loadSettings() {
    QSettings s("PlayTune", "Settings");
    
    // 1. EQ Toggle (off by default!)
    bool enabled = s.value("eq_enabled", false).toBool();
    setEqEnabled(enabled);

    // 2. Tab index
    int tabIndex = s.value("eq_tab_index", 0).toInt();
    if (m_tabGroup && m_tabGroup->button(tabIndex)) {
        QSignalBlocker blocker(m_tabGroup);
        m_tabGroup->button(tabIndex)->setChecked(true);
        m_stackedWidget->setCurrentIndex(tabIndex);
    }

    // 3. Preamp
    double preamp = s.value("eq_preamp", 0.0).toDouble();
    if (m_preampSlider) {
        QSignalBlocker blocker(m_preampSlider);
        m_preampSlider->setValue(qRound(preamp * 10.0));
        QString text = QString("%1%2 dB").arg(preamp >= 0 ? "+" : "").arg(preamp, 0, 'f', 1);
        m_preampValueLabel->setText(text);
        m_preampSlider->setToolTip("Master Preamp Gain (" + text + ")");
    }

    // 4. Controls
    double bass = s.value("eq_param_bass", 0.0).toDouble();
    if (m_bassSlider) {
        QSignalBlocker blocker(m_bassSlider);
        m_bassSlider->setValue(qRound(bass * 10.0));
        m_bassValueLabel->setText(QString("%1%2dB").arg(bass >= 0 ? "+" : "").arg(bass, 0, 'f', 1));
        m_bassSlider->setToolTip("Adjust Bass: " + m_bassValueLabel->text());
    }
    double treble = s.value("eq_param_treble", 0.0).toDouble();
    if (m_trebleSlider) {
        QSignalBlocker blocker(m_trebleSlider);
        m_trebleSlider->setValue(qRound(treble * 10.0));
        m_trebleValueLabel->setText(QString("%1%2dB").arg(treble >= 0 ? "+" : "").arg(treble, 0, 'f', 1));
        m_trebleSlider->setToolTip("Adjust Treble: " + m_trebleValueLabel->text());
    }
    double stereo = s.value("eq_param_stereo", 100.0).toDouble();
    if (m_stereoSlider) {
        QSignalBlocker blocker(m_stereoSlider);
        m_stereoSlider->setValue(qRound(stereo));
        m_stereoValueLabel->setText(QString("%1%").arg(stereo, 0, 'f', 0));
        m_stereoSlider->setToolTip("Adjust Stereo Width: " + m_stereoValueLabel->text());
    }
    double balance = s.value("eq_param_balance", 0.0).toDouble();
    if (m_balanceSlider) {
        QSignalBlocker blocker(m_balanceSlider);
        m_balanceSlider->setValue(qRound(balance * 100.0));
        m_balanceValueLabel->setText(QString("%1%2").arg(balance >= 0 ? "+" : "").arg(balance, 0, 'f', 2));
        m_balanceSlider->setToolTip("Adjust Balance: " + m_balanceValueLabel->text());
    }

    // 5. 10 bands gains & preset
    QVariantList defaultGains;
    for (int i = 0; i < 10; ++i) defaultGains.append(0.0);
    QVariantList gainsVal = s.value("eq_gains", defaultGains).toList();
    QVector<double> gains;
    for (const QVariant& v : gainsVal) gains.append(v.toDouble());
    if (gains.size() == 10) {
        m_curveWidget->setGains(gains);
    }
    
    int presetIdx = s.value("eq_preset", 7).toInt(); // default to Custom
    if (m_presetGroup) {
        QSignalBlocker blocker(m_presetGroup);
        selectPresetButton(presetIdx);
    }

    // 6. Resampler Quality
    int resIdx = s.value("eq_resampler_quality", 1).toInt();
    if (m_resamplerGroup && m_resamplerGroup->button(resIdx)) {
        QSignalBlocker blocker(m_resamplerGroup);
        m_resamplerGroup->button(resIdx)->setChecked(true);
    }

    // 7. Advanced bands
    double defaultFreqs[10] = {31.5, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0};
    for (int i = 0; i < 10; ++i) {
        int filterType = (i == 0) ? 0 : ((i == 9) ? 2 : 1);
        m_advBands[i].filterType = s.value(QString("eq_adv_band_%1_type").arg(i), filterType).toInt();
        m_advBands[i].freqHz = s.value(QString("eq_adv_band_%1_freq").arg(i), defaultFreqs[i]).toDouble();
        m_advBands[i].gainDb = s.value(QString("eq_adv_band_%1_gain").arg(i), 0.0).toDouble();
        m_advBands[i].q = s.value(QString("eq_adv_band_%1_q").arg(i), 1.0).toDouble();
        m_advBands[i].enabled = s.value(QString("eq_adv_band_%1_enabled").arg(i), true).toBool();
    }
    
    int curAdvBand = s.value("eq_current_adv_band", 0).toInt();
    if (m_advBandGroup && m_advBandGroup->button(curAdvBand)) {
        QSignalBlocker blocker(m_advBandGroup);
        m_advBandGroup->button(curAdvBand)->setChecked(true);
    }
    m_currentAdvBandIdx = curAdvBand;
    updateAdvancedBandControls(curAdvBand);

    // 8. Geometry
    if (s.value("eq_geometry_saved", false).toBool()) {
        QPoint savedPos = s.value("eq_geometry_pos").toPoint();
        QSize savedSize = s.value("eq_geometry_size").toSize();
        if (QWidget* p = parentWidget()) {
            savedSize.setWidth(qBound(650, savedSize.width(), p->width()));
            savedSize.setHeight(qBound(480, savedSize.height(), p->height()));
            int maxX = p->width() - savedSize.width();
            int maxY = p->height() - savedSize.height();
            savedPos.setX(qBound(0, savedPos.x(), qMax(0, maxX)));
            savedPos.setY(qBound(0, savedPos.y(), qMax(0, maxY)));
        }
        resize(savedSize);
        move(savedPos);
    } else {
        // Center relative to parent
        QTimer::singleShot(0, this, [this]() {
            if (QWidget* p = parentWidget()) {
                move(p->rect().center() - rect().center());
            }
        });
    }
}

void EqualizerWindow::closeEvent(QCloseEvent* event) {
    saveSettings();
    QWidget::closeEvent(event);
}

void EqualizerWindow::hideEvent(QHideEvent* event) {
    saveSettings();
    QWidget::hideEvent(event);
}


