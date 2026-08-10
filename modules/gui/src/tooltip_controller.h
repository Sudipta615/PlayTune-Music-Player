#ifndef TOOLTIPCONTROLLER_H
#define TOOLTIPCONTROLLER_H

#include <QObject>

class ToolTipController : public QObject {
    Q_OBJECT
public:
    explicit ToolTipController(QObject* parent = nullptr);
    ~ToolTipController() override = default;

    void setEnabled(bool enabled);
    bool isEnabled() const;

protected:
    bool eventFilter(QObject* obj, QEvent* event) override;

private:
    void showInstantToolTip(QWidget* target);
    bool m_enabled = true;
};

#endif // TOOLTIPCONTROLLER_H
