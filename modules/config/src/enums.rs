/// Audio backend selection enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum AudioBackend {
    #[default]
    Auto,
    Cpal,
    Asio,
    Dummy,
    ExclusiveAlsa,
    ExclusiveAsio,
    ExclusiveWasapi,
    ExclusiveCoreAudioHog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ResamplerQuality {
    #[default]
    Balanced,
    HighQuality,
    Fast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum LoudnessMode {
    #[default]
    Off,
    TrackReplayGain,
    AlbumReplayGain,
    EbuR128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum FilterType {
    #[default]
    Peaking,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
    Notch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum CrossfeedProfile {
    #[default]
    Bauer,
    ChuMoy,
    Jmeier,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum PerformanceMode {
    #[default]
    Normal,
    LowLatency,
    HighQuality,
    LowPower,
}
