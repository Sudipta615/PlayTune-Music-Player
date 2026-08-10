#[derive(Debug, Clone, PartialEq)]
pub enum EngineCommand {
    Play,
    Pause,
    Stop,
    /// Seek to position in seconds. Must be finite and >= 0; invalid values are ignored.
    Seek(f32),
    SetVolume(f32),
    SetSpeed(f32),
    NextTrack,
    PrevTrack,
    LoadTrack(u64),
    Shutdown,
    SetOutputBackend(config::AudioBackend),
    SetOutputDevice(Option<String>),
    SetEqEnabled(bool),
    SetEqBand {
        index: usize,
        frequency: f32,
        gain_db: f32,
        q: f32,
        enabled: bool,
    },
    SetEqBandParams {
        index: usize,
        frequency: f32,
        gain_db: f32,
        q: f32,
        filter_type: crate::dsp::equalizer::EqFilterType,
        enabled: bool,
    },
    SetResamplerQuality(config::types::enums::ResamplerQuality),
    SetBassShelf(f32),
    SetTrebleShelf(f32),
    SetPreamp(f32),
    SetStereoWidth(f32),
    SetBalance(f32),
    SetDitherEnabled(bool),
    SetMidsideEq(bool),
    SetCrossfeedEnabled(bool),
    SetCrossfeedProfile(config::types::enums::CrossfeedProfile),
    SetCrossfeedCustomParams {
        frequency_hz: f32,
        q: f32,
        delay_ms: f32,
        mix_db: f32,
    },
    SetCompressorEnabled(bool),
    SetCompressorBandParams {
        band: usize, // 0=Low, 1=Mid, 2=High
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_gain_db: f32,
    },
    /// Set shuffle on/off (used by MPRIS integration to propagate shuffle state to the engine)
    SetShuffle(bool),
    /// Set loop status: "None", "Track", "Playlist" (MPRIS-style)
    SetLoopStatus(String),
    /// Open a URI for playback (file:// URIs only)
    OpenUri(String),
    /// Prepare the next track for crossfading by pre-opening its decoder.
    PrepareNextTrack(std::path::PathBuf),
    /// Request stream recovery after a device disconnection or error.
    RecoverStream,
    /// Automatically triggered stream recovery from the background monitor thread.
    AutoRecoverStream,
}
