#ifndef SLEEPTIMERDIALOG_H
#define SLEEPTIMERDIALOG_H

#include <QDialog>
#include <QPushButton>
#include <QSpinBox>
#include <QLabel>

/// A simple dialog for setting a sleep timer. Offers preset buttons
/// (15/30/45/60/90 min) plus a custom spin box (1–120 minutes). The
/// "Cancel Timer" button stops any active timer.
class SleepTimerDialog : public QDialog {
    Q_OBJECT
public:
    explicit SleepTimerDialog(QWidget* parent = nullptr);

signals:
    /// Emitted when the user picks a duration. `minutes`=0 means "cancel".
    void durationSelected(int minutes);

private:
    void setupUi();
    void pickPreset(int minutes);
    void pickCustom();
    void cancelTimer();

    QSpinBox* m_customSpin = nullptr;
    QLabel* m_hintLabel = nullptr;
};

#endif // SLEEPTIMERDIALOG_H
