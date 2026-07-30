//! MPRIS PropertiesChanged D-Bus signal emission
//!
//! Contains the logic for emitting `org.freedesktop.DBus.Properties.PropertiesChanged`
//! signals when MPRIS properties are updated.
//!
//! state and includes them in the `changed_properties` dictionary
//! of the signal (C2 fix). This allows MPRIS clients to update
//! their display immediately without making additional Get() calls.
//!
//! `Connection::emit_signal()` with `zbus::message::Builder::signal()`.

use std::sync::Arc;

use parking_lot::Mutex;
use zbus::zvariant::Value;

use super::MprisState;
use crate::types::{MprisPlaybackStatus, MprisPropertyChanged};

/// Emit a PropertiesChanged signal using the zbus v4 message builder API.
fn send_properties_changed(
    conn: &zbus::blocking::Connection,
    path: &str,
    iface_name: &str,
    changed_props: std::collections::HashMap<&str, Value>,
    invalidated: Vec<&str>,
) -> Result<(), zbus::Error> {
    // TODO: migrate to Connection::emit_signal when zbus stabilizes
    // the replacement API. The deprecated Builder::signal works fine and
    // produces correct wire output, but each zbus release prints a
    // deprecation warning at compile time.
    #[allow(deprecated)] // zbus v4.4 deprecated Builder::signal; see F#60 above.
    let msg = zbus::message::Builder::signal(
        path,
        "org.freedesktop.DBus.Properties",
        "PropertiesChanged",
    )?
    .build(&(iface_name, changed_props, invalidated))?;
    conn.send(&msg)?;
    Ok(())
}

/// Emit a PropertiesChanged D-Bus signal for the given property.
pub(crate) fn emit_properties_changed(
    conn: &zbus::blocking::Connection,
    state: &Arc<Mutex<MprisState>>,
    changed: MprisPropertyChanged,
) -> Result<(), zbus::Error> {
    let iface_name = "org.mpris.MediaPlayer2.Player";
    let path = "/org/mpris/MediaPlayer2";

    match changed {
        MprisPropertyChanged::PlaybackStatus => {
            let status_str = {
                let s = state.lock();
                match s.playback_status {
                    MprisPlaybackStatus::Playing => "Playing",
                    MprisPlaybackStatus::Paused => "Paused",
                    MprisPlaybackStatus::Stopped => "Stopped",
                }
                .to_string()
            };
            let mut map = std::collections::HashMap::<&str, Value>::new();
            map.insert("PlaybackStatus", Value::Str(status_str.into()));
            send_properties_changed(conn, path, iface_name, map, vec![])?;
        }
        MprisPropertyChanged::TrackMetadata => {
            // Invalidate so clients re-query the full metadata dict.
            send_properties_changed(
                conn,
                path,
                iface_name,
                std::collections::HashMap::new(),
                vec!["Metadata"],
            )?;
        }
        MprisPropertyChanged::Volume => {
            let vol = state.lock().volume;
            let mut map = std::collections::HashMap::<&str, Value>::new();
            map.insert("Volume", Value::F64(vol as f64));
            send_properties_changed(conn, path, iface_name, map, vec![])?;
        }
        MprisPropertyChanged::Shuffle => {
            let shuffle = state.lock().shuffle;
            let mut map = std::collections::HashMap::<&str, Value>::new();
            map.insert("Shuffle", Value::Bool(shuffle));
            send_properties_changed(conn, path, iface_name, map, vec![])?;
        }
        MprisPropertyChanged::LoopStatus => {
            let loop_status = state.lock().loop_status.clone();
            let mut map = std::collections::HashMap::<&str, Value>::new();
            map.insert("LoopStatus", Value::Str(loop_status.into()));
            send_properties_changed(conn, path, iface_name, map, vec![])?;
        }
        MprisPropertyChanged::Rate => {
            let rate = state.lock().rate;
            let mut map = std::collections::HashMap::<&str, Value>::new();
            map.insert("Rate", Value::F64(rate as f64));
            send_properties_changed(conn, path, iface_name, map, vec![])?;
        }
        MprisPropertyChanged::PositionSeeked => {
            log::debug!(
                "PositionSeeked reached emit_properties_changed — \
                 should have been handled by the event loop's pending_seeked path"
            );
        }
    }

    Ok(())
}

///
/// The signal payload is (track_id: ObjectPath, position: int64 microseconds).
pub(crate) fn emit_seeked(
    conn: &zbus::blocking::Connection,
    state: &Arc<Mutex<MprisState>>,
) -> Result<(), zbus::Error> {
    let path = "/org/mpris/MediaPlayer2";
    let (track_id, position) = {
        let s = state.lock();
        let tid = s
            .track_info
            .track_id
            .clone()
            .unwrap_or_else(|| "/org/mpris/MediaPlayer2/TrackList/NoTrack".to_string());
        (tid, s.position_microseconds)
    };

    static NO_TRACK: std::sync::OnceLock<zbus::zvariant::ObjectPath<'static>> =
        std::sync::OnceLock::new();
    let no_track = NO_TRACK.get_or_init(|| {
        // SAFETY (compile-time): the literal "/org/mpris/MediaPlayer2/TrackList/NoTrack"
        // is a valid MPRIS object path (starts with '/', only [A-Za-z0-9_].
        // try_from can only fail on invalid paths, which this is not. We
        // use expect() HERE ONLY — inside the OnceLock initializer — because
        // a failure would be a programming bug, not a runtime condition.
        // This is the only .expect() remaining in this file, and it lives
        // behind a OnceLock so it runs at most once per process.
        zbus::zvariant::ObjectPath::try_from("/org/mpris/MediaPlayer2/TrackList/NoTrack")
            .expect("static NoTrack path is always valid (validated at first-use)")
    });

    // ObjectPath requires the path to start with '/'. If track_id was set
    // to a non-path string by mistake, fall back to NoTrack.
    let safe_track_id = if track_id.starts_with('/') {
        zbus::zvariant::ObjectPath::try_from(track_id.as_str()).unwrap_or_else(|_| no_track.clone())
    } else {
        no_track.clone()
    };

    #[allow(deprecated)]
    let msg = zbus::message::Builder::signal(path, "org.mpris.MediaPlayer2.Player", "Seeked")?
        .build(&(safe_track_id, position))?;
    conn.send(&msg)?;
    Ok(())
}
