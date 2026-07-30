#include "lrcparser.h"
#include <QRegularExpression>
#include <QStringList>
#include <algorithm>

QVector<LrcLine> LrcParser::parse(const QString& lrcText) {
    QVector<LrcLine> result;
    if (lrcText.trimmed().isEmpty()) return result;

    // Matches [mm:ss.xx] or [mm:ss:xx] or [mm:ss]
    static const QRegularExpression timeRegex(QStringLiteral("\\[(\\d{1,3}):(\\d{2})(?:[\\.:](\\d{1,3}))?\\]"));

    QStringList lines = lrcText.split(QLatin1Char('\n'));
    for (const QString& rawLine : lines) {
        QString line = rawLine.trimmed();
        if (line.isEmpty()) continue;

        // Skip metadata tags like [ti:Title], [ar:Artist], [al:Album], [offset:0]
        if (line.startsWith(QLatin1String("[ti:")) || line.startsWith(QLatin1String("[ar:")) ||
            line.startsWith(QLatin1String("[al:")) || line.startsWith(QLatin1String("[offset:")) ||
            line.startsWith(QLatin1String("[by:")) || line.startsWith(QLatin1String("[re:")) ||
            line.startsWith(QLatin1String("[ve:"))) {
            continue;
        }

        QRegularExpressionMatchIterator it = timeRegex.globalMatch(line);
        if (!it.hasNext()) continue;

        // Extract all timestamps on this line
        QVector<double> lineTimestamps;
        int lastMatchEnd = 0;
        while (it.hasNext()) {
            QRegularExpressionMatch match = it.next();
            int mins = match.captured(1).toInt();
            int secs = match.captured(2).toInt();
            double frac = 0.0;
            if (!match.captured(3).isEmpty()) {
                QString fracStr = match.captured(3);
                if (fracStr.length() == 1) frac = fracStr.toInt() * 0.1;
                else if (fracStr.length() == 2) frac = fracStr.toInt() * 0.01;
                else if (fracStr.length() == 3) frac = fracStr.toInt() * 0.001;
            }
            double totalSecs = mins * 60.0 + secs + frac;
            lineTimestamps.append(totalSecs);
            lastMatchEnd = match.capturedEnd();
        }

        QString text = line.mid(lastMatchEnd).trimmed();
        for (double ts : lineTimestamps) {
            result.append({ts, text});
        }
    }

    // Sort chronologically
    std::sort(result.begin(), result.end(), [](const LrcLine& a, const LrcLine& b) {
        return a.timestampSeconds < b.timestampSeconds;
    });

    return result;
}

int LrcParser::findActiveLineIndex(const QVector<LrcLine>& lines, double elapsedSeconds) {
    if (lines.isEmpty()) return -1;
    if (elapsedSeconds < lines.first().timestampSeconds) return -1;

    // Find the last line whose timestamp <= elapsedSeconds
    int activeIdx = -1;
    for (int i = 0; i < lines.size(); ++i) {
        if (elapsedSeconds >= lines[i].timestampSeconds) {
            activeIdx = i;
        } else {
            break;
        }
    }
    return activeIdx;
}
