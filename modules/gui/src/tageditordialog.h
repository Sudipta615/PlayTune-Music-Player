#ifndef TAGEDITORDIALOG_H
#define TAGEDITORDIALOG_H

#include <QDialog>
#include <QLineEdit>
#include <QSpinBox>
#include <QLabel>
#include <QPushButton>
#include <QString>
#include <QPoint>
#include <QMouseEvent>

struct TagEditorTrackData {
    int track_id;
    QString title;
    QString artist;
    QString album;
    QString album_artist;
    QString genre;
    unsigned int year;
    unsigned int track_number;
    unsigned int disc_number;
    QString cover_path;
};

class TagEditorDialog : public QDialog {
    Q_OBJECT
public:
    explicit TagEditorDialog(const TagEditorTrackData& data, QWidget* parent = nullptr);
    ~TagEditorDialog() override = default;

protected:
    void mousePressEvent(QMouseEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;

private slots:
    void onChangeCoverClicked();
    void onRemoveCoverClicked();
    void onSaveClicked();

private:
    void setupUi();
    void updateCoverPreview();

    TagEditorTrackData m_originalData;
    QString m_currentCoverPath; // Can be empty if removed, or pointing to a new file

    // UI elements
    QLabel* m_coverPreview = nullptr;
    QLineEdit* m_titleEdit = nullptr;
    QLineEdit* m_artistEdit = nullptr;
    QLineEdit* m_albumEdit = nullptr;
    QLineEdit* m_albumArtistEdit = nullptr;
    QLineEdit* m_genreEdit = nullptr;
    QSpinBox* m_yearSpin = nullptr;
    QSpinBox* m_trackSpin = nullptr;
    QSpinBox* m_discSpin = nullptr;

    QPoint m_dragPosition;
};

#endif // TAGEDITORDIALOG_H
