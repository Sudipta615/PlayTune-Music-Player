#ifndef APPSETTINGS_H
#define APPSETTINGS_H

/// Process-wide singleton for fast, lock-free reads of global UI settings.
///
/// Components like loadThumbnail() and MediaGridCard::setContent() need to
/// know whether Optimized Mode is active without taking a Qt signal parameter.
/// Rather than passing a bool through every call-site, a single singleton flag
/// lets any helper query the current mode cheaply.
///
/// Thread-safety: Only written from the GUI thread (SettingsPageWidget toggle).
/// Only read from the GUI thread (all callers are widget code). No mutex needed.
class AppSettings {
public:
    static AppSettings& instance() {
        static AppSettings inst;
        return inst;
    }

    bool isOptimizedMode() const { return m_optimizedMode; }
    void setOptimizedMode(bool enabled) { m_optimizedMode = enabled; }

    bool isMoodColumnEnabled() const { return m_moodColumnEnabled; }
    void setMoodColumnEnabled(bool enabled) { m_moodColumnEnabled = enabled; }

    bool isCursorFollowsPlayback() const { return m_cursorFollowsPlayback; }
    void setCursorFollowsPlayback(bool enabled) { m_cursorFollowsPlayback = enabled; }

private:
    AppSettings() = default;
    bool m_optimizedMode = false;
    bool m_moodColumnEnabled = true;
    bool m_cursorFollowsPlayback = false;
};

#endif // APPSETTINGS_H
