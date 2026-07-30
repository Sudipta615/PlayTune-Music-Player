//! Cross-platform media key and transport control handling
//!
//! - **Linux**: Uses MPRIS D-Bus (via souvlaki's MPRIS backend)
//! - **macOS**: Uses MPRemoteCommandCenter (via souvlaki's macOS backend)
//! - **Windows**: Uses SystemMediaTransportControls (via souvlaki's Windows backend)

use std::sync::mpsc::SyncSender;

#[cfg(not(target_os = "linux"))]
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};

use crate::types::{MediaKeyAction, MprisPlaybackStatus};

/// Wrapper around `souvlaki::MediaControls` that translates platform
/// media key events into `MediaKeyAction` values sent through the
/// application's action channel.
pub struct CrossPlatformMediaControls {
    /// The souvlaki MediaControls handle. None if initialization failed
    /// (e.g., no D-Bus on Linux, no window handle on some platforms).
    #[cfg(not(target_os = "linux"))]
    controls: Option<MediaControls>,
    current_status: MprisPlaybackStatus,
    last_volume: f32,
    last_position: std::time::Duration,
}

impl CrossPlatformMediaControls {
    /// Create a new cross-platform media controls instance.
    ///
    /// On Linux, this creates an MPRIS D-Bus service. On macOS, it
    /// registers with MPRemoteCommandCenter. On Windows, it registers
    /// with SystemMediaTransportControls.
    ///
    /// If platform initialization fails (e.g., no D-Bus daemon on Linux),
    /// the controls will be None and media key events will not be forwarded.
    /// The application should fall back to keyboard shortcuts in this case.
    pub fn new(action_tx: SyncSender<MediaKeyAction>) -> Result<Self, String> {
        #[cfg(target_os = "windows")]
        let hwnd = {
            #[link(name = "user32")]
            extern "system" {
                fn GetForegroundWindow() -> *mut std::ffi::c_void;
                fn GetDesktopWindow() -> *mut std::ffi::c_void;
            }
            #[link(name = "kernel32")]
            extern "system" {
                fn GetConsoleWindow() -> *mut std::ffi::c_void;
            }
            unsafe {
                let console = GetConsoleWindow();
                if !console.is_null() {
                    Some(console)
                } else {
                    let fg = GetForegroundWindow();
                    if !fg.is_null() {
                        Some(fg)
                    } else {
                        log::warn!(
                            "CrossPlatformMediaControls: no console or \
                             foreground window available; falling back to \
                             GetDesktopWindow. SMTC events may not route \
                             correctly. Pass a real HWND for proper \
                             integration. (F#54)"
                        );
                        Some(GetDesktopWindow())
                    }
                }
            }
        };
        #[cfg(all(not(target_os = "windows"), not(target_os = "linux")))]
        let hwnd: Option<*mut std::ffi::c_void> = None;

        #[cfg(target_os = "linux")]
        {
            // Suppress unused-variable warning for action_tx on Linux: it
            // IS used by macOS/Windows branches but not on Linux.
            let _ = action_tx;
            log::debug!(
                "Linux: skipping souvlaki init; the custom mpris module \
                 handles both media-key events and D-Bus property reporting."
            );
            Ok(Self {
                current_status: MprisPlaybackStatus::Stopped,
                last_volume: 1.0,
                last_position: std::time::Duration::ZERO,
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            let config = PlatformConfig { dbus_name: "playtune", display_name: "PlayTune", hwnd };

            let controls = match MediaControls::new(config) {
                Ok(mut ctrl) => {
                    // Attach the event handler that translates souvlaki events
                    // into our MediaKeyAction type.
                    let tx = action_tx.clone();
                    ctrl.attach(move |event: MediaControlEvent| {
                        Self::handle_event(event, &tx);
                    })
                    .map_err(|e| format!("Failed to attach media control handler: {:?}", e))?;
                    Some(ctrl)
                }
                Err(e) => {
                    log::warn!(
                        "Failed to initialize cross-platform media controls: {:?}. \
                         Media key events will not be forwarded. \
                         On Linux, ensure a D-Bus session is available. \
                         On macOS/Windows, this should not fail.",
                        e
                    );
                    None
                }
            };

            Ok(Self {
                controls,
                current_status: MprisPlaybackStatus::Stopped,
                last_volume: 1.0,
                last_position: std::time::Duration::ZERO,
            })
        }
    }

    /// Translate a souvlaki MediaControlEvent into our MediaKeyAction
    /// and send it through the action channel.
    #[cfg(not(target_os = "linux"))]
    fn handle_event(event: MediaControlEvent, tx: &SyncSender<MediaKeyAction>) {
        let action = match event {
            MediaControlEvent::Play => MediaKeyAction::Play,
            MediaControlEvent::Pause => MediaKeyAction::Pause,
            MediaControlEvent::Toggle => MediaKeyAction::PlayPause,
            MediaControlEvent::Next => MediaKeyAction::Next,
            MediaControlEvent::Previous => MediaKeyAction::Previous,
            MediaControlEvent::Stop => MediaKeyAction::Stop,
            MediaControlEvent::Seek(direction) => {
                let amount = match direction {
                    SeekDirection::Forward => 5_000_000,
                    SeekDirection::Backward => -5_000_000,
                };
                MediaKeyAction::Seek(amount)
            }
            MediaControlEvent::SeekBy(direction, duration) => {
                let sign = match direction {
                    SeekDirection::Forward => 1,
                    SeekDirection::Backward => -1,
                };
                MediaKeyAction::Seek(sign * duration.as_micros() as i64)
            }
            MediaControlEvent::Raise => MediaKeyAction::Raise,
            MediaControlEvent::Quit => MediaKeyAction::Quit,
            MediaControlEvent::SetVolume(volume) => MediaKeyAction::SetVolume(volume as f32),
            MediaControlEvent::SetPosition(position) => {
                let pos_us = i64::try_from(position.0.as_micros()).unwrap_or(i64::MAX);
                MediaKeyAction::SetPosition {
                    track_id: "/org/mpris/MediaPlayer2/TrackList/NoTrack".to_string(),
                    position_us: pos_us,
                }
            }
            MediaControlEvent::OpenUri(uri) => MediaKeyAction::OpenUri(uri),
        };

        if let Err(e) = tx.send(action) {
            log::warn!("Failed to send media key action: {}", e);
        }
    }

    /// Update the playback status shown in the OS media controls.
    pub fn set_playback_status(&mut self, status: MprisPlaybackStatus) {
        self.current_status = status;
        #[cfg(not(target_os = "linux"))]
        if let Some(ref mut ctrl) = self.controls {
            let playback = match status {
                MprisPlaybackStatus::Playing => MediaPlayback::Playing { progress: None },
                MprisPlaybackStatus::Paused => MediaPlayback::Paused { progress: None },
                MprisPlaybackStatus::Stopped => MediaPlayback::Stopped,
            };
            if let Err(e) = ctrl.set_playback(playback) {
                log::warn!("Failed to update media playback status: {:?}", e);
            }
        }
    }

    /// Update the track metadata shown in the OS media controls.
    pub fn set_metadata(
        &mut self,
        title: Option<&str>,
        artist: Option<&str>,
        album: Option<&str>,
        duration: Option<std::time::Duration>,
        art_url: Option<&str>,
    ) {
        #[cfg(not(target_os = "linux"))]
        if let Some(ref mut ctrl) = self.controls {
            let metadata = MediaMetadata { title, artist, album, cover_url: art_url, duration };
            if let Err(e) = ctrl.set_metadata(metadata) {
                log::warn!("Failed to update media metadata: {:?}", e);
            }
        }
        #[cfg(target_os = "linux")]
        {
            let _ = (title, artist, album, duration, art_url);
        }
    }

    /// Update the current playback position.
    pub fn set_position(&mut self, position: std::time::Duration) {
        self.last_position = position;
        #[cfg(not(target_os = "linux"))]
        if let Some(ref mut ctrl) = self.controls {
            let playback = match self.current_status {
                MprisPlaybackStatus::Playing => {
                    MediaPlayback::Playing { progress: Some(MediaPosition(position)) }
                }
                MprisPlaybackStatus::Paused => {
                    MediaPlayback::Paused { progress: Some(MediaPosition(position)) }
                }
                MprisPlaybackStatus::Stopped => MediaPlayback::Stopped,
            };
            if let Err(e) = ctrl.set_playback(playback) {
                log::warn!("Failed to update media position: {:?}", e);
            }
        }
    }

    /// Update the volume shown in the OS media controls.
    pub fn set_volume(&mut self, volume: f32) {
        let v = if volume.is_nan() { 0.0 } else { volume.clamp(0.0, 1.0) };
        // Cache so callers can query the last-set volume via get_volume().
        self.last_volume = v;
        log::trace!(
            "CrossPlatformMediaControls::set_volume({}) cached (souvlaki 0.7.x no-op on macOS/Windows)",
            self.last_volume
        );
    }

    pub fn get_volume(&self) -> f32 {
        self.last_volume
    }

    /// Check if media controls are available on this platform.
    pub fn is_available(&self) -> bool {
        #[cfg(not(target_os = "linux"))]
        {
            self.controls.is_some()
        }
        #[cfg(target_os = "linux")]
        {
            false
        }
    }
}
