#ifndef PLAYLISTCREATEDIALOG_H
#define PLAYLISTCREATEDIALOG_H

#include <QDialog>
#include <QLineEdit>
#include <QPushButton>
#include <QLabel>
#include <QFrame>
#include "apptheme.h"

/// Modal dialog for creating or renaming a playlist. Single text field
/// for the playlist name. Emits `nameSubmitted(QString)` when the user
/// clicks OK; the caller is responsible for invoking the bridge callback.
class PlaylistCreateDialog : public QDialog {
    Q_OBJECT
public:
    enum class Mode { Create, Rename };
    explicit PlaylistCreateDialog(Mode mode, const QString& existingName = QString(),
                                  QWidget* parent = nullptr);

    QString playlistName() const;

signals:
    void nameSubmitted(const QString& name);

private:
    void setupUi();
    void updateThemeStyles(const ThemePalette& p);

    Mode m_mode;
    QFrame* m_cardFrame = nullptr;
    QLabel* m_titleLabel = nullptr;
    QLabel* m_hintLabel = nullptr;
    QLineEdit* m_nameEdit = nullptr;
    QPushButton* m_cancelBtn = nullptr;
    QPushButton* m_okBtn = nullptr;
};

#endif // PLAYLISTCREATEDIALOG_H
