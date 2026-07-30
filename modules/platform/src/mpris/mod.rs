//! MPRIS D-Bus service implementation (Linux only)
//!
//! Exposes org.mpris.MediaPlayer2 and org.mpris.MediaPlayer2.Player
//! interfaces on the session bus so that desktop panels and media
//! player applets can control PlayTune.
//!
//! `PropertiesChanged` signals (C2 fix), uses `parking_lot::Mutex`
//! for poison-resistant state access (C5 fix), and the `play()` and
//! `pause()` MPRIS methods now send dedicated `MediaKeyAction::Play`
//! and `MediaKeyAction::Pause` actions instead of `PlayPause` (C3 fix).
//! The `Quit()` method now sends `MediaKeyAction::Quit` (C4 fix).

mod dbus;
mod signals;

use std::sync::{
    mpsc::{Receiver, SyncSender},
    Arc,
};

use parking_lot::Mutex;

use crate::types::{MediaKeyAction, MprisPlaybackStatus, MprisPropertyChanged, MprisTrackInfo};

/// Shared MPRIS state that can be updated from the main thread
/// and read from the D-Bus service thread.
#[derive(Debug, Clone)]
pub struct MprisState {
    pub playback_status: MprisPlaybackStatus,
    pub track_info: MprisTrackInfo,
    pub volume: f32,
    pub identity: String,
    pub desktop_entry: String,
    /// Whether shuffle is enabled
    pub shuffle: bool,
    /// Loop status string: must be one of "None", "Track", "Playlist"
    pub loop_status: String,
    /// Playback rate (must be > 0)
    pub rate: f32,
    /// Playback position in microseconds
    pub position_microseconds: i64,
    pub can_go_next: bool,
    pub can_go_previous: bool,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_seek: bool,
}

impl Default for MprisState {
    fn default() -> Self {
        Self {
            playback_status: MprisPlaybackStatus::Stopped,
            track_info: MprisTrackInfo::default(),
            volume: 1.0,
            identity: "PlayTune".to_string(),
            desktop_entry: "playtune".to_string(),
            shuffle: false,
            loop_status: "None".to_string(),
            rate: 1.0,
            position_microseconds: 0,
            can_go_next: true,
            can_go_previous: true,
            can_play: true,
            can_pause: true,
            can_seek: true,
        }
    }
}

/// MPRIS D-Bus service handle
pub struct MprisService {
    identity: String,
    action_tx: SyncSender<MediaKeyAction>,
    /// Shared MPRIS state, constructed once in `new()`.
    state: Arc<Mutex<MprisState>>,
}

impl MprisService {
    pub fn new(identity: &str, action_tx: SyncSender<MediaKeyAction>) -> Self {
        let state = Arc::new(Mutex::new(MprisState {
            identity: identity.to_string(),
            desktop_entry: identity.to_lowercase(),
            ..MprisState::default()
        }));
        Self { identity: identity.to_string(), action_tx, state }
    }

    /// Create the shared MprisState Arc.
    pub fn state(&self) -> Arc<Mutex<MprisState>> {
        Arc::clone(&self.state)
    }

    /// Attempt to register the MPRIS service on D-Bus.
    ///
    /// This spawns a background thread that owns the D-Bus connection.
    /// The thread exits when the notification channel is disconnected
    /// (i.e., when `PlatformIntegration` drops the sender).
    pub fn start(
        &self,
        state: Arc<Mutex<MprisState>>,
        notify_rx: Receiver<MprisPropertyChanged>,
    ) -> Result<(), String> {
        let identity = self.identity.clone();
        let action_tx = self.action_tx.clone();

        std::thread::Builder::new()
            .name("playtune-mpris-dbus".to_string())
            .spawn(move || match dbus::run_dbus_server(&identity, &action_tx, &state, &notify_rx) {
                Ok(()) => log::info!("MPRIS D-Bus service stopped"),
                Err(e) => log::warn!("MPRIS D-Bus service error: {}", e),
            })
            .map_err(|e| format!("Failed to spawn MPRIS thread: {}", e))?;

        Ok(())
    }
}
