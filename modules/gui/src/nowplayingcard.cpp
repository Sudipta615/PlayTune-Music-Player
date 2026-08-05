#include "nowplayingcard.h"
#include "appsettings.h"
#include <QHBoxLayout>

#include <QVBoxLayout>
#include <QPainter>
#include <QPainterPath>
#include <QIcon>
#include <QStyle>
#include <QDebug>
#include <QImage>
#include <algorithm>
#include <cmath>
#include <QResizeEvent>
#include <QGraphicsDropShadowEffect>

// Helper to round pixmaps
static QPixmap getRoundedPixmap(const QPixmap& src, int size, int radius) {
    if (src.isNull()) return src;
    QPixmap target(size, size);
    target.fill(Qt::transparent);
    QPainter painter(&target);
    painter.setRenderHint(QPainter::Antialiasing, true);
    painter.setRenderHint(QPainter::SmoothPixmapTransform, true);
    QPainterPath path;
    path.addRoundedRect(0, 0, size, size, radius, radius);
    painter.setClipPath(path);

    QPixmap expanded = src.scaled(size, size, Qt::KeepAspectRatioByExpanding, Qt::SmoothTransformation);
    int ex = (size - expanded.width()) / 2;
    int ey = (size - expanded.height()) / 2;
    painter.drawPixmap(ex, ey, expanded);

    return target;
}

NowPlayingCard::NowPlayingCard(QWidget* parent) : QFrame(parent) {
    setObjectName("NowPlayingCard");
    m_coverPixmap = getDefaultAlbumArt();
    setupUi();

    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this](const ThemePalette& p) {
<<<<<<< HEAD
=======
        applyLabelStyles(p);
>>>>>>> mulberry-calendula
        if (!m_hasCustomCover) {
            m_coverPixmap = getDefaultAlbumArt();
            updateCoverPixmap();
            applyCardStyle(p.cardBgGradStart, p.cardBgGradEnd, p.cardBorder);
        }
<<<<<<< HEAD
        // Update icon tinting & hover for NowPlayingCard white buttons
        if (m_editTagsBtn) {
            m_editTagsBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/more.png", QColor("#FFFFFF")));
            m_editTagsBtn->setStyleSheet(QString(
                "QPushButton { border: none; background: transparent; }"
                "QPushButton:hover { background-color: %1; border-radius: 8px; }"
            ).arg(p.primaryAccent.name()));
        }
=======
>>>>>>> mulberry-calendula
    });
}

void NowPlayingCard::setupUi() {
    auto* mainLayout = new QHBoxLayout(this);
    mainLayout->setContentsMargins(20, 20, 20, 20);
    mainLayout->setSpacing(20);

    // 1. Album Art Label (Left)
    m_coverLabel = new QLabel(this);
    m_coverLabel->setFixedSize(140, 140);

    auto* shadow = new QGraphicsDropShadowEffect(m_coverLabel);
    shadow->setBlurRadius(24);
    shadow->setColor(QColor(0, 0, 0, 160));
    shadow->setOffset(0, 6);
    m_coverLabel->setGraphicsEffect(shadow);

    QPixmap defaultCover = getDefaultAlbumArt();
    m_coverLabel->setPixmap(getRoundedPixmap(defaultCover, 140, 16));
    mainLayout->addWidget(m_coverLabel);

    // 2. Info and controls container (Right)
    m_rightLayout = new QVBoxLayout();
    m_rightLayout->setSpacing(8);
    m_rightLayout->setContentsMargins(0, 0, 0, 0);
    auto* rightLayout = m_rightLayout;

    // Now Playing Header & Title Line
    auto* topInfoLayout = new QHBoxLayout();
    topInfoLayout->setSpacing(5);

    auto* infoVLayout = new QVBoxLayout();
    infoVLayout->setSpacing(2);

    auto* nowPlayingLabel = new QLabel("Now Playing", this);
    m_nowPlayingLabel = nowPlayingLabel;

    m_titleLabel = new QLabel("No Track Playing", this);
    m_titleLabel->setObjectName("NowPlayingTitle");
    m_titleLabel->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);

    infoVLayout->addWidget(nowPlayingLabel);
    infoVLayout->addWidget(m_titleLabel);

    topInfoLayout->addLayout(infoVLayout, 1);

    auto* editTagsBtn = new QPushButton(this);
    m_editTagsBtn = editTagsBtn;
    editTagsBtn->setObjectName("IconButton");
    editTagsBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/more.png", QColor("#FFFFFF")));
    editTagsBtn->setIconSize(QSize(18, 18));
    editTagsBtn->setFixedSize(32, 32);
<<<<<<< HEAD
    editTagsBtn->setStyleSheet(QString(
        "QPushButton { border: none; background: transparent; }"
        "QPushButton:hover { background-color: %1; border-radius: 8px; }"
    ).arg(ThemeManager::instance().currentTheme().primaryAccent.name()));
=======
>>>>>>> mulberry-calendula
    editTagsBtn->setToolTip("Edit Track Metadata Tags...");
    connect(editTagsBtn, &QPushButton::clicked, this, &NowPlayingCard::editTagsClicked);
    topInfoLayout->addWidget(editTagsBtn, 0, Qt::AlignTop | Qt::AlignRight);

    rightLayout->addLayout(topInfoLayout);

    // Artist and Album Labels
    auto* artistAlbumLayout = new QHBoxLayout();
    artistAlbumLayout->setSpacing(10);
    
    m_artistLabel = new QLabel("Select a track from library", this);
    m_artistLabel->setObjectName("NowPlayingArtist");
    
    m_albumLabel = new QLabel("", this);
    m_albumLabel->setObjectName("NowPlayingAlbum");

    artistAlbumLayout->addWidget(m_artistLabel);
    artistAlbumLayout->addWidget(m_albumLabel);
    artistAlbumLayout->addStretch();
    rightLayout->addLayout(artistAlbumLayout);

    // 3. Waveform Seekbar Visualizer (Horizontal margins aligned precisely with m_seekBar)
    auto* visualizerLayout = new QHBoxLayout();
    visualizerLayout->setContentsMargins(45, 0, 45, 0);
    m_visualizer = new WaveformVisualizer(this);
    m_visualizer->setToolTip("Real-time Audio Waveform & Playback Progress");
    visualizerLayout->addWidget(m_visualizer);
    rightLayout->addLayout(visualizerLayout);

    // 4. Seek Slider layout
    auto* seekLayout = new QHBoxLayout();
    seekLayout->setSpacing(10);

    m_timeElapsed = new QLabel("0:00", this);
    m_timeElapsed->setObjectName("TimeLabel");
    m_timeElapsed->setFixedWidth(46);
    m_timeElapsed->setAlignment(Qt::AlignCenter);
    m_timeElapsed->setToolTip("Elapsed Playback Time");

    auto* shadowElapsed = new QGraphicsDropShadowEffect(m_timeElapsed);
    shadowElapsed->setBlurRadius(8);
    shadowElapsed->setColor(QColor(0, 0, 0, 180));
    shadowElapsed->setOffset(0, 2);
    m_timeElapsed->setGraphicsEffect(shadowElapsed);

    m_seekBar = new ClickableSlider(Qt::Horizontal, this);
    m_seekBar->setRange(0, 1000);
    m_seekBar->setValue(0);
    m_seekBar->setToolTip("Seek Playback Position (Click or Drag / Shift+← / Shift+→)");

    m_timeTotal = new QLabel("0:00", this);
    m_timeTotal->setObjectName("TimeLabel");
    m_timeTotal->setFixedWidth(46);
    m_timeTotal->setAlignment(Qt::AlignCenter);
    m_timeTotal->setToolTip("Total Track Duration");

    auto* shadowTotal = new QGraphicsDropShadowEffect(m_timeTotal);
    shadowTotal->setBlurRadius(8);
    shadowTotal->setColor(QColor(0, 0, 0, 180));
    shadowTotal->setOffset(0, 2);
    m_timeTotal->setGraphicsEffect(shadowTotal);

    seekLayout->addWidget(m_timeElapsed);
    seekLayout->addWidget(m_seekBar);
    seekLayout->addWidget(m_timeTotal);
    rightLayout->addLayout(seekLayout);

    // Connect waveform visualizer click gesture
    connect(m_visualizer, &WaveformVisualizer::seekRequested, this, [this](double ratio) {
        if (m_totalDuration > 0.0) {
            m_seekBar->setValue(static_cast<int>(ratio * m_seekBar->maximum()));
            m_timeElapsed->setText(formatTime(ratio * m_totalDuration));
            emit seekRequested(ratio * m_totalDuration);
        }
    });

    // Connect slider gestures
    connect(m_seekBar, &QSlider::sliderPressed, this, [this]() {
        m_isSliderPressed = true;
    });
    connect(m_seekBar, &QSlider::sliderReleased, this, [this]() {
        m_isSliderPressed = false;
        if (m_totalDuration > 0.0) {
            double ratio = (double)m_seekBar->value() / m_seekBar->maximum();
            emit seekRequested(ratio * m_totalDuration);
        }
    });
    connect(m_seekBar, &QSlider::sliderMoved, this, [this](int val) {
        if (m_totalDuration > 0.0) {
            double ratio = (double)val / m_seekBar->maximum();
            m_timeElapsed->setText(formatTime(ratio * m_totalDuration));
            if (m_visualizer) {
                m_visualizer->setPlaybackProgress(ratio);
            }
        }
    });

    // 5. Playback buttons Layout
    m_controlsLayout = new QHBoxLayout();
    m_controlsLayout->setSpacing(15);
    m_controlsLayout->setContentsMargins(0, 5, 0, 0);
    auto* controlsLayout = m_controlsLayout;

    // Repeat button
    m_repeatBtn = new QPushButton(this);
    m_repeatBtn->setObjectName("MediaControlBtn");
    m_repeatBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/repeat.png", QColor("#FFFFFF")));
    m_repeatBtn->setIconSize(QSize(22, 22));
    m_repeatBtn->setFixedSize(38, 38);
    m_repeatBtn->setCheckable(true);
    m_repeatBtn->setToolTip("Repeat: OFF (R)");
    connect(m_repeatBtn, &QPushButton::toggled, this, [this](bool checked) {
        m_repeatBtn->setToolTip(checked ? "Repeat: ON (R)" : "Repeat: OFF (R)");
    });

    // Prev button
    m_prevBtn = new QPushButton(this);
    m_prevBtn->setObjectName("MediaControlBtn");
    m_prevBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/prev.png", QColor("#FFFFFF")));
    m_prevBtn->setIconSize(QSize(26, 26));
    m_prevBtn->setFixedSize(38, 38);
    m_prevBtn->setToolTip("Previous Track (← / Ctrl+←)");

    // Play/Pause button
    m_playPauseBtn = new QPushButton(this);
    m_playPauseBtn->setObjectName("PlayPauseBtn");
    m_playPauseBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/pause.png", QColor("#FFFFFF")));
    m_playPauseBtn->setIconSize(QSize(28, 28));
    m_playPauseBtn->setFixedSize(50, 50);
    m_playPauseBtn->setToolTip("Pause Audio (Space)");

    // Next button
    m_nextBtn = new QPushButton(this);
    m_nextBtn->setObjectName("MediaControlBtn");
    m_nextBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/next.png", QColor("#FFFFFF")));
    m_nextBtn->setIconSize(QSize(26, 26));
    m_nextBtn->setFixedSize(38, 38);
    m_nextBtn->setToolTip("Next Track (→ / Ctrl+→)");

    // Shuffle button
    m_shuffleBtn = new QPushButton(this);
    m_shuffleBtn->setObjectName("MediaControlBtn");
    m_shuffleBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/shuffle.png", QColor("#FFFFFF")));
    m_shuffleBtn->setIconSize(QSize(22, 22));
    m_shuffleBtn->setFixedSize(38, 38);
    m_shuffleBtn->setCheckable(true);
    m_shuffleBtn->setToolTip("Shuffle: OFF (S)");
    connect(m_shuffleBtn, &QPushButton::toggled, this, [this](bool checked) {
        m_shuffleBtn->setToolTip(checked ? "Shuffle: ON (S)" : "Shuffle: OFF (S)");
    });

    // EQ button (Equalizer)
    m_eqBtn = new QPushButton(this);
    m_eqBtn->setObjectName("MediaControlBtn");
    m_eqBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/equalizer.png", QColor("#FFFFFF")));
    m_eqBtn->setIconSize(QSize(22, 22));
    m_eqBtn->setFixedSize(38, 38);
    m_eqBtn->setToolTip("Open DSP Equalizer & Resampler Window (E)");

    // Sleep timer button.
    m_sleepTimerBtn = new QPushButton(this);
    m_sleepTimerBtn->setObjectName("MediaControlBtn");
    m_sleepTimerBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/recently_played.png", QColor("#FFFFFF")));
    m_sleepTimerBtn->setIconSize(QSize(22, 22));
    m_sleepTimerBtn->setFixedSize(38, 38);
    m_sleepTimerBtn->setToolTip("Sleep Timer");
    m_sleepTimerBtn->setCheckable(false);
    m_sleepTimerLabel = new QLabel("", this);
    m_sleepTimerLabel->setStyleSheet("color: #FFC53D; font-size: 11px; padding: 0 4px;");
    m_sleepTimerLabel->setFixedWidth(48);
    m_sleepTimerLabel->setAlignment(Qt::AlignCenter);
    m_sleepTimerLabel->setVisible(false);
    connect(m_sleepTimerBtn, &QPushButton::clicked, this, &NowPlayingCard::sleepTimerClicked);

    // Reconnect themeChanged for media buttons
    connect(&ThemeManager::instance(), &ThemeManager::themeChanged, this, [this](const ThemePalette&) {
        if (m_repeatBtn) m_repeatBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/repeat.png", QColor("#FFFFFF")));
        if (m_prevBtn)   m_prevBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/prev.png", QColor("#FFFFFF")));
        if (m_nextBtn)   m_nextBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/next.png", QColor("#FFFFFF")));
        if (m_shuffleBtn) m_shuffleBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/shuffle.png", QColor("#FFFFFF")));
        if (m_eqBtn)     m_eqBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/equalizer.png", QColor("#FFFFFF")));
        if (m_sleepTimerBtn) m_sleepTimerBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/recently_played.png", QColor("#FFFFFF")));
    });

    controlsLayout->addStretch();
    controlsLayout->addWidget(m_repeatBtn);
    controlsLayout->addWidget(m_prevBtn);
    controlsLayout->addWidget(m_playPauseBtn);
    controlsLayout->addWidget(m_nextBtn);
    controlsLayout->addWidget(m_shuffleBtn);
    controlsLayout->addStretch();
    controlsLayout->addWidget(m_sleepTimerBtn);
    controlsLayout->addWidget(m_sleepTimerLabel);
    controlsLayout->addWidget(m_eqBtn);

    m_rightLayout->addLayout(m_controlsLayout);

    mainLayout->addLayout(m_rightLayout, 1);

    // Connect button clicks
    connect(m_playPauseBtn, &QPushButton::clicked, this, &NowPlayingCard::playPauseClicked);
    connect(m_prevBtn, &QPushButton::clicked, this, &NowPlayingCard::prevClicked);
    connect(m_nextBtn, &QPushButton::clicked, this, &NowPlayingCard::nextClicked);
    connect(m_eqBtn, &QPushButton::clicked, this, &NowPlayingCard::eqClicked);
    connect(m_repeatBtn, &QPushButton::clicked, this, &NowPlayingCard::repeatClicked);
    connect(m_shuffleBtn, &QPushButton::clicked, this, &NowPlayingCard::shuffleClicked);

    // Initially animate the waveform
    m_visualizer->setPlaying(true);

    // Apply theme-aware label styling (labels no longer rely on the global
    // stylesheet, whose hard-coded colors were unreadable on light themes).
    applyLabelStyles(ThemeManager::instance().currentTheme());
}

void NowPlayingCard::setTrackInfo(const QString& title, const QString& artist, const QString& album, const QString& coverPath) {
    m_titleLabel->setText(title.isEmpty() ? "Unknown Title" : title);
    m_artistLabel->setText(artist.isEmpty() ? "Unknown Artist" : artist);
    if (album.trimmed().isEmpty()) {
        m_albumLabel->setText("");
        m_albumLabel->setVisible(false);
    } else {
        m_albumLabel->setText(album);
        m_albumLabel->setVisible(true);
    }


    QPixmap cover;
    if (!coverPath.isEmpty() && cover.load(coverPath)) {
        m_coverPixmap = cover;
        m_hasCustomCover = true;
    } else {
        m_coverPixmap = getDefaultAlbumArt();
        m_hasCustomCover = false;
    }
    updateCoverPixmap();

    QColor c1, c2, borderColor;
    if (m_hasCustomCover) {
        // Dynamic vibrant gradient from album cover with visible, colorful results.
        QImage img = m_coverPixmap.toImage().scaled(32, 32, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
        c1 = QColor("#151624");
        c2 = QColor("#0F111D");
        borderColor = QColor("#23283E");
        if (!img.isNull()) {
            QVector<QColor> vibrantColors;
            int stepX = qMax(1, img.width() / 8);
            int stepY = qMax(1, img.height() / 8);
            for (int y = 0; y < img.height(); y += stepY) {
                for (int x = 0; x < img.width(); x += stepX) {
                    QColor c = img.pixelColor(x, y);
                    int s = c.hslSaturation();
                    int l = c.lightness();
                    if (s > 40 && l > 20 && l < 220) {
                        vibrantColors.append(c);
                    }
                }
            }
            if (!vibrantColors.isEmpty()) {
                std::sort(vibrantColors.begin(), vibrantColors.end(), [](const QColor& a, const QColor& b) {
                    return a.hslSaturation() > b.hslSaturation();
                });
                c1 = vibrantColors.first();
                bool foundSecond = false;
                for (const QColor& c : vibrantColors) {
                    if (std::abs(c.hslHue() - c1.hslHue()) > 40) {
                        c2 = c;
                        foundSecond = true;
                        break;
                    }
                }
                if (!foundSecond) {
                    int h, s, l;
                    c1.getHsl(&h, &s, &l);
                    c2.setHsl((h + 50) % 360, qMax(50, s - 20), qBound(15, l - 8, 30));
                }
                int h1, s1, l1;
                c1.getHsl(&h1, &s1, &l1);
                c1.setHsl(h1, qBound(50, s1, 140), qBound(18, l1, 35));
                borderColor.setHsl(h1, qBound(60, s1, 160), qBound(25, l1 + 12, 50));
            }
        }
    } else {
        const auto& p = ThemeManager::instance().currentTheme();
        c1 = p.cardBgGradStart;
        c2 = p.cardBgGradEnd;
        borderColor = p.cardBorder;
    }

    // Apply gradient colors: bypass the 400 ms animation in Optimized Mode (no QVariantAnimation cost).
    if (m_optimizedMode)
        applyCardStyle(c1, c2, borderColor);
    else
        animateToColors(c1, c2, borderColor);
}


void NowPlayingCard::setOptimizedMode(bool enabled) {
    m_optimizedMode = enabled;

    // 1. Spectrum Visualizer: stop FFT ticks entirely
    if (m_visualizer) {
        m_visualizer->setVisible(!enabled);
        if (enabled)
            m_visualizer->setPlaying(false);  // stops internal animation timer
        else
            m_visualizer->setPlaying(m_isPlaying);
    }

    // 2. Drop shadows: remove them in Optimized Mode to eliminate GPU compositing cost.
    //    Qt deletes the old effect when a new one (or nullptr) is set.
    if (m_coverLabel) {
        if (enabled) {
            m_coverLabel->setGraphicsEffect(nullptr);
        } else {
            auto* shadow = new QGraphicsDropShadowEffect(m_coverLabel);
            shadow->setBlurRadius(24);
            shadow->setColor(QColor(0, 0, 0, 160));
            shadow->setOffset(0, 6);
            m_coverLabel->setGraphicsEffect(shadow);
        }
    }
    if (m_timeElapsed) {
        if (enabled) {
            m_timeElapsed->setGraphicsEffect(nullptr);
        } else {
            auto* sh = new QGraphicsDropShadowEffect(m_timeElapsed);
            sh->setBlurRadius(8); sh->setColor(QColor(0, 0, 0, 180)); sh->setOffset(0, 2);
            m_timeElapsed->setGraphicsEffect(sh);
        }
    }
    if (m_timeTotal) {
        if (enabled) {
            m_timeTotal->setGraphicsEffect(nullptr);
        } else {
            auto* sh = new QGraphicsDropShadowEffect(m_timeTotal);
            sh->setBlurRadius(8); sh->setColor(QColor(0, 0, 0, 180)); sh->setOffset(0, 2);
            m_timeTotal->setGraphicsEffect(sh);
        }
    }
}

void NowPlayingCard::applyLabelStyles(const ThemePalette& p) {
    if (m_nowPlayingLabel) {
        m_nowPlayingLabel->setStyleSheet(QString(
            "color: %1; font-size: 11px; text-transform: uppercase; font-weight: bold;"
        ).arg(p.mutedText.name()));
    }
    if (m_titleLabel) {
        m_titleLabel->setStyleSheet(QString(
            "font-size: 22px; font-weight: bold; color: %1; background: transparent; border: none;"
        ).arg(p.primaryText.name()));
    }

    // Pill backgrounds behind the artist/album/time chips. Translucent so
    // they read well over both the dark and the light Now-Playing gradient.
    const QColor pillBg     = p.isLight ? QColor(15, 23, 42, 26) : QColor(10, 12, 20, 150);
    const QColor pillBorder = p.isLight ? QColor(15, 23, 42, 46) : QColor(255, 255, 255, 55);

    const QString chipStyle = QString(
        "font-size: 12px; font-weight: 500; color: %1;"
        "background-color: %2; border: 1px solid %3;"
        "border-radius: 6px; padding: 3px 8px;"
    ).arg(p.secondaryText.name())
     .arg(pillBg.name(QColor::HexArgb))
     .arg(pillBorder.name(QColor::HexArgb));
    if (m_artistLabel) m_artistLabel->setStyleSheet(chipStyle);
    if (m_albumLabel)  m_albumLabel->setStyleSheet(chipStyle);

    const QString timeStyle = QString(
        "font-size: 11px; font-weight: 600; color: %1;"
        "background-color: %2; border: 1px solid %3;"
        "border-radius: 6px; padding: 3px 6px;"
    ).arg(p.primaryText.name())
     .arg(pillBg.name(QColor::HexArgb))
     .arg(pillBorder.name(QColor::HexArgb));
    if (m_timeElapsed) m_timeElapsed->setStyleSheet(timeStyle);
    if (m_timeTotal)   m_timeTotal->setStyleSheet(timeStyle);

    if (m_editTagsBtn) {
        m_editTagsBtn->setStyleSheet(QString(
            "QPushButton { border: none; background: transparent; }"
            "QPushButton:hover { background-color: %1; border-radius: 6px; }"
        ).arg(p.itemHoverBg.name()));
    }
}

void NowPlayingCard::applyCardStyle(const QColor& c1, const QColor& c2, const QColor& border) {
    m_currentC1 = c1;
    m_currentC2 = c2;
    m_currentBorderColor = border;
    setStyleSheet(QString(
        "QFrame#NowPlayingCard {"
        "  background: qlineargradient(x1:0, y1:0, x2:1, y2:1, stop:0 %1, stop:1 %2);"
        "  border: 1px solid %3;"
        "  border-radius: 18px;"
        "}"
    ).arg(c1.name()).arg(c2.name()).arg(border.name()));
}

void NowPlayingCard::animateToColors(const QColor& targetC1, const QColor& targetC2, const QColor& targetBorder) {
    if (m_colorAnimation) {
        m_colorAnimation->stop();
        delete m_colorAnimation.data();
        m_colorAnimation = nullptr;
    }

    QColor startC1 = m_currentC1;
    QColor startC2 = m_currentC2;
    QColor startBorder = m_currentBorderColor;

    auto* anim = new QVariantAnimation(this);
    m_colorAnimation = anim;
    anim->setDuration(400);
    anim->setEasingCurve(QEasingCurve::OutCubic);
    anim->setStartValue(0.0);
    anim->setEndValue(1.0);

    connect(anim, &QVariantAnimation::valueChanged, this, [this, startC1, startC2, startBorder, targetC1, targetC2, targetBorder](const QVariant& value) {
        double progress = value.toDouble();
        auto interpolateColor = [](const QColor& from, const QColor& to, double t) -> QColor {
            int r = qBound(0, static_cast<int>(from.red() + t * (to.red() - from.red())), 255);
            int g = qBound(0, static_cast<int>(from.green() + t * (to.green() - from.green())), 255);
            int b = qBound(0, static_cast<int>(from.blue() + t * (to.blue() - from.blue())), 255);
            return QColor(r, g, b);
        };
        QColor currC1 = interpolateColor(startC1, targetC1, progress);
        QColor currC2 = interpolateColor(startC2, targetC2, progress);
        QColor currBorder = interpolateColor(startBorder, targetBorder, progress);
        applyCardStyle(currC1, currC2, currBorder);
    });

    anim->start(QAbstractAnimation::KeepWhenStopped);
}

void NowPlayingCard::setPlayState(bool playing) {
    m_isPlaying = playing;
    const auto& p = ThemeManager::instance().currentTheme();
    if (playing) {
        m_playPauseBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/pause.png", QColor("#FFFFFF")));
        m_playPauseBtn->setToolTip("Pause Audio (Space)");
        m_visualizer->setPlaying(true);
    } else {
        m_playPauseBtn->setIcon(ThemeManager::tintedIcon(":/resources/icons/play.png", QColor("#FFFFFF")));
        m_playPauseBtn->setToolTip("Play Audio (Space)");
        m_visualizer->setPlaying(false);
    }
}

void NowPlayingCard::setPlaybackProgress(double elapsed, double total) {
    m_totalDuration = total;
    m_elapsedSeconds = elapsed;
    if (!m_isSliderPressed) {
        if (total > 0.0) {
            double ratio = elapsed / total;
            m_seekBar->setValue(static_cast<int>(ratio * m_seekBar->maximum()));
            m_visualizer->setPlaybackProgress(ratio);
        } else {
            m_seekBar->setValue(0);
            m_visualizer->setPlaybackProgress(0.0);
        }
    }
    m_timeElapsed->setText(formatTime(elapsed));
    m_timeTotal->setText(formatTime(total));
}

void NowPlayingCard::updateVisualizer(const QVector<float>& data) {
    if (m_optimizedMode || AppSettings::instance().isOptimizedMode()) {
        if (m_visualizer && m_visualizer->isVisible()) {
            m_visualizer->setVisible(false);
            m_visualizer->setPlaying(false);
        }
        return;
    }
    if (m_visualizer) {
        m_visualizer->updateBuffer(data);
    }
}


QString NowPlayingCard::formatTime(double seconds) {
    if (std::isnan(seconds) || std::isinf(seconds) || seconds < 0) {
        return "0:00";
    }
    int totalSecs = static_cast<int>(seconds + 0.5);
    int hours = totalSecs / 3600;
    int mins = (totalSecs % 3600) / 60;
    int secs = totalSecs % 60;
    if (hours > 0) {
        return QString("%1:%2:%3")
            .arg(hours)
            .arg(mins, 2, 10, QChar('0'))
            .arg(secs, 2, 10, QChar('0'));
    }
    return QString("%1:%2").arg(mins).arg(secs, 2, 10, QChar('0'));
}

void NowPlayingCard::updateCoverPixmap() {
    if (!m_coverPixmap.isNull()) {
        int radius = qMax(6, m_coverSize * 16 / 140);
        m_coverLabel->setPixmap(getRoundedPixmap(m_coverPixmap, m_coverSize, radius));
    }
}

void NowPlayingCard::resizeEvent(QResizeEvent* event) {
    QFrame::resizeEvent(event);
    int w = width();
    int newCoverSize = 140;
    int margin = 20;
    int spacing = 20;
    int rightSpacing = 8;
    int btnSpacing = 15;
    bool showVisualizer = true;

    if (w < 450) {
        newCoverSize = 80;
        margin = 10;
        spacing = 10;
        rightSpacing = 4;
        btnSpacing = 8;
        showVisualizer = false;
    } else if (w < 600) {
        newCoverSize = 110;
        margin = 12;
        spacing = 12;
        rightSpacing = 6;
        btnSpacing = 10;
        showVisualizer = true;
    } else {
        newCoverSize = 140;
        margin = 20;
        spacing = 20;
        rightSpacing = 8;
        btnSpacing = 15;
        showVisualizer = true;
    }

    if (newCoverSize != m_coverSize) {
        m_coverSize = newCoverSize;
        m_coverLabel->setFixedSize(m_coverSize, m_coverSize);
        updateCoverPixmap();
    }

    if (auto* mainLayout = qobject_cast<QHBoxLayout*>(layout())) {
        mainLayout->setContentsMargins(margin, margin, margin, margin);
        mainLayout->setSpacing(spacing);
    }

    if (m_rightLayout) {
        m_rightLayout->setSpacing(rightSpacing);
    }

    if (m_controlsLayout) {
        m_controlsLayout->setSpacing(btnSpacing);
    }

    if (m_visualizer) {
        bool opt = m_optimizedMode || AppSettings::instance().isOptimizedMode();
        m_visualizer->setVisible(!opt && showVisualizer);
        if (opt) m_visualizer->setPlaying(false);
    }
}


void NowPlayingCard::setSpeedLabel(double speed) {
    Q_UNUSED(speed);
}

void NowPlayingCard::setSleepTimerRemaining(int seconds_remaining) {
    m_sleepTimerRemaining = seconds_remaining;
    if (!m_sleepTimerLabel) return;
    if (seconds_remaining <= 0) {
        m_sleepTimerLabel->setVisible(false);
        m_sleepTimerLabel->setText("");
    } else {
        int m = seconds_remaining / 60;
        int s = seconds_remaining % 60;
        m_sleepTimerLabel->setText(QString("⏰ %1:%2")
                                       .arg(m, 2, 10, QChar('0'))
                                       .arg(s, 2, 10, QChar('0')));
        m_sleepTimerLabel->setVisible(true);
    }
}

bool NowPlayingCard::eventFilter(QObject* watched, QEvent* event) {
    Q_UNUSED(watched);
    Q_UNUSED(event);
    return QFrame::eventFilter(watched, event);
}
