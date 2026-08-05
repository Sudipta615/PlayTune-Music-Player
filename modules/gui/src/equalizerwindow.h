#ifndef EQUALIZERWINDOW_H
#define EQUALIZERWINDOW_H

#include <QWidget>
#include <QCloseEvent>
#include <QHideEvent>
#include <QSlider>
#include <QLabel>
#include <QPushButton>
#include <QButtonGroup>
#include <QStackedWidget>
#include <QComboBox>
#include <QDoubleSpinBox>
#include "custom_widgets.h"

class EqualizerWindow : public QWidget {
    Q_OBJECT
public:
    explicit EqualizerWindow(QWidget* parent = nullptr);
    ~EqualizerWindow() override = default;

    // Load gains from outside
    void setEqGains(const QVector<double>& gains);
    void setEqEnabled(bool enabled);

    void emitInitialState();
    void saveSettings();
    void scheduleSave();
    void loadSettings();

signals:
    void eqToggled(bool enabled);
    void bandChanged(int bandIdx, double gainDb);
    void presetSelected(int presetIdx);
    void resetEqClicked();
    void sliderParamChanged(int paramIdx, double value); // 0: Bass, 1: Treble, 2: Stereo Width, 3: Balance, 4: Preamp
    void advancedBandChanged(int bandIdx, double freqHz, double gainDb, double q, int filterType);
    void resamplerQualityChanged(int qualityIdx);

protected:
    void mousePressEvent(QMouseEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;
    void closeEvent(QCloseEvent* event) override;
    void hideEvent(QHideEvent* event) override;

private:
    void setupUi();
    QWidget* createParamSliderCard(const QString& name, double minVal, double maxVal, double defaultVal, const QString& unit, int paramIdx);
    void selectPresetButton(int presetIdx);
    void setPresetGains(int presetIdx);
    void updateAdvancedBandControls(int bandIdx);
    void applyTheme(const ThemePalette& p);

    // Theme-able widgets (labels get an objectName *role*; the theme handler
    // restyles them so the window follows Light/Dark/colored themes).
    QLabel* m_eqTitleLabel = nullptr;
    QLabel* m_preampHeaderLabel = nullptr;
    QLabel* m_preampMinLabel = nullptr;
    QLabel* m_preampMaxLabel = nullptr;
    QFrame* m_resamplerCard = nullptr;
    QFrame* m_bandEditorCard = nullptr;

    // Widgets
    ToggleSwitch* m_enableToggle = nullptr;
    EqualizerCurveWidget* m_curveWidget = nullptr;
    
    QButtonGroup* m_presetGroup = nullptr;
    QVector<QPushButton*> m_presetBtns;

    // Parametric Sliders
    QSlider* m_bassSlider = nullptr;
    QLabel* m_bassValueLabel = nullptr;
    
    QSlider* m_trebleSlider = nullptr;
    QLabel* m_trebleValueLabel = nullptr;
    
    QSlider* m_stereoSlider = nullptr;
    QLabel* m_stereoValueLabel = nullptr;
    
    QSlider* m_balanceSlider = nullptr;
    QLabel* m_balanceValueLabel = nullptr;

    QSlider* m_preampSlider = nullptr;
    QLabel* m_preampValueLabel = nullptr;

    QPushButton* m_resetBtn = nullptr;

    // Content container to enable/disable (gray out) when toggle clicked
    QWidget* m_eqContentWidget = nullptr;

    // Tabs & Stacked Widget
    QStackedWidget* m_stackedWidget = nullptr;
    QButtonGroup* m_tabGroup = nullptr;
    QPushButton* m_tab10BandsBtn = nullptr;
    QPushButton* m_tabControlsBtn = nullptr;
    QPushButton* m_tabAdvancedBtn = nullptr;

    // Advanced Tab State & Controls
    struct AdvBandState {
        int filterType = 1; // 0: LowShelf, 1: Peaking, 2: HighShelf, etc.
        double freqHz = 1000.0;
        double gainDb = 0.0;
        double q = 1.0;
        bool enabled = true;
    };
    QVector<AdvBandState> m_advBands;
    int m_currentAdvBandIdx = 0;

    QButtonGroup* m_resamplerGroup = nullptr;
    QVector<QPushButton*> m_resamplerBtns;

    QButtonGroup* m_advBandGroup = nullptr;
    QVector<QPushButton*> m_advBandBtns;

    QComboBox* m_advFilterTypeCombo = nullptr;
    QSlider* m_advFreqSlider = nullptr;
    QDoubleSpinBox* m_advFreqSpin = nullptr;
    QSlider* m_advQSlider = nullptr;
    QDoubleSpinBox* m_advQSpin = nullptr;
    QSlider* m_advGainSlider = nullptr;
    QLabel* m_advGainLabel = nullptr;
    ToggleSwitch* m_advBandToggle = nullptr;

    // Themed UI elements
    QLabel* m_eqIconLabel = nullptr;
    QPushButton* m_moreBtn = nullptr;
    QPushButton* m_closeBtn = nullptr;
    QLabel* m_resamplerHeader = nullptr;
    QLabel* m_typeLabel = nullptr;
    QLabel* m_enableLabel = nullptr;
    QLabel* m_freqLabel = nullptr;
    QLabel* m_qLabel = nullptr;
    QLabel* m_gainLabel = nullptr;
    QLabel* m_preampHeader = nullptr;
    QVector<QWidget*> m_paramCards;
    QVector<QLabel*> m_paramNameLabels;
    QVector<QLabel*> m_paramMinLabels;
    QVector<QLabel*> m_paramMaxLabels;
    QVector<QPushButton*> m_paramInfoBtns;

    void updateThemeStyles(const ThemePalette& p);

    // Window dragging & resizing state
    QPoint m_dragPosition;
    bool m_isResizing = false;
    QPoint m_resizeStartPos;
    QSize m_resizeStartSize;
    QTimer* m_saveTimer = nullptr;
};

#endif // EQUALIZERWINDOW_H
