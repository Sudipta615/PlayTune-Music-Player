#ifndef LRCPARSER_H
#define LRCPARSER_H

#include <QString>
#include <QVector>

struct LrcLine {
    double timestampSeconds;
    QString text;
};

class LrcParser {
public:
    static QVector<LrcLine> parse(const QString& lrcText);
    static int findActiveLineIndex(const QVector<LrcLine>& lines, double elapsedSeconds);
};

#endif // LRCPARSER_H
