#ifndef LOUDNESSSCANNERDIALOG_H
#define LOUDNESSSCANNERDIALOG_H

#include <QDialog>
#include <QProgressBar>
#include <QLabel>
#include <QTableWidget>
#include <QPushButton>
#include <QVector>
#include <QMouseEvent>
#include "gui_bridge.h"
#include "apptheme.h"

struct ScannedTrackItem {
    int track_id;
    QString title;
    float lufs;
    float peak;
    float rg_gain_db;
    float r128_gain_db;
};

class LoudnessScannerDialog : public QDialog {
    Q_OBJECT
public:
    /// Construct the scanner dialog and immediately start scanning `track_ids` (`count == 0` scans entire library).
    explicit LoudnessScannerDialog(const QVector<int>& trackIds, QWidget* parent = nullptr);
    ~LoudnessScannerDialog() override;

protected:
    void mousePressEvent(QMouseEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void closeEvent(QCloseEvent* event) override;

private slots:
    void onScanProgress(int current, int total, const QString& current_file);
    void onTrackResult(int track_id, float lufs, float peak, float rg_gain_db, float r128_gain_db);
    void onScanFinished(bool success, const QString& error_msg);
    void onCancelOrCloseClicked();
    void onWriteTagsClicked();

private:
    void setupUi();
    void updateThemeStyles(const ThemePalette& p);

    QVector<int> m_targetTrackIds;
    QVector<ScannedTrackItem> m_results;
    bool m_scanning = true;

    QLabel* m_statusLabel = nullptr;
    QProgressBar* m_progressBar = nullptr;
    QTableWidget* m_resultsTable = nullptr;
    QPushButton* m_cancelCloseBtn = nullptr;
    QPushButton* m_writeBtn = nullptr;

    QPoint m_dragPosition;
};

#endif // LOUDNESSSCANNERDIALOG_H
