//! Media key receiver and listener management
//!
//! Contains [`MediaKeyReceiver`] for polling media key actions and
//! the start/stop listener methods on [`PlatformIntegration`].

use std::sync::mpsc::Receiver;

use crate::{types::MediaKeyAction, PlatformIntegration};

/// Standalone receiver for media key actions.
pub struct MediaKeyReceiver {
    rx: Receiver<MediaKeyAction>,
}

impl MediaKeyReceiver {
    /// Create a new MediaKeyReceiver wrapping the given channel receiver.
    pub(crate) fn new(rx: Receiver<MediaKeyAction>) -> Self {
        Self { rx }
    }

    /// Try to receive a media key action (non-blocking)
    pub fn try_recv(&self) -> Option<MediaKeyAction> {
        self.rx.try_recv().ok()
    }

    /// Try to receive a media key action with a timeout
    pub fn recv_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<MediaKeyAction, std::sync::mpsc::RecvTimeoutError> {
        self.rx.recv_timeout(timeout)
    }

    /// Receive a media key action (blocking)
    pub fn recv(&self) -> Result<MediaKeyAction, std::sync::mpsc::RecvError> {
        self.rx.recv()
    }
}

impl PlatformIntegration {
    /// Start listening for media keys.
    /// - MPRIS D-Bus on Linux
    /// - MPRemoteCommandCenter on macOS
    /// - SystemMediaTransportControls on Windows
    pub fn start_media_key_listener(&mut self) -> Result<(), crate::types::PlatformError> {
        if self.media_controls.is_some() {
            log::debug!("Media key listener already active — start call is a no-op");
            return Ok(());
        }

        match crate::media_control::CrossPlatformMediaControls::new(self.action_tx.clone()) {
            Ok(controls) => {
                self.media_controls = Some(controls);
                log::info!("Re-created cross-platform media controls handle on start");
            }
            Err(e) => {
                log::warn!(
                    "Failed to (re)create cross-platform media controls on start: {}. \
                     Media keys will not be forwarded until next start attempt.",
                    e
                );
            }
        }

        // Cross-platform media controls (souvlaki) are already initialized
        // in PlatformIntegration::new() (or just re-created above). They
        // work on all platforms.
        if self.media_controls.as_ref().is_some_and(|c| c.is_available()) {
            log::info!("Media key listener active via cross-platform controls (souvlaki)");
        } else {
            log::warn!(
                "Cross-platform media controls not available. \
                 Media keys will not be forwarded. Use keyboard shortcuts instead."
            );
        }

        // On Linux, also start the MPRIS D-Bus service for advanced
        // property reporting (Metadata, CanGoNext, etc.) if registered.
        #[cfg(target_os = "linux")]
        {
            if self.mpris_registered {
                log::info!("MPRIS D-Bus service active for advanced property reporting");
            }
        }

        log::info!("Media key listener started");
        Ok(())
    }

    /// Stop listening for media keys.
    pub fn stop_media_key_listener(&mut self) {
        self.media_controls = None;

        #[cfg(target_os = "linux")]
        {
            // The D-Bus thread's event loop detects RecvTimeoutError::Disconnected
            // and breaks out of the loop, allowing the thread to exit.
            self.mpris_notify_tx = None;
            self.mpris_registered = false;
            self.mpris_state = None;
        }

        log::info!(
            "Media key listener stopped (media_controls handle dropped; \
             MPRIS registration cleared so it can be re-registered on next start)"
        );
    }
}
