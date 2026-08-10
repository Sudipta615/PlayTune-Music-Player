pub const DEFAULT_SAMPLE_RATE: u32 = 44100;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
    Buffering,
}

#[derive(Debug, Clone)]
pub struct PlaybackInfo {
    pub state: PlaybackState,
    pub position_secs: f32,
    pub duration_secs: f32,
    pub volume: f32,
    pub speed: f32,
    pub track_id: Option<u64>,
    pub sample_rate: u32,
    pub cpu_usage_pct: f32,
    /// Number of audio dropouts / CPU overloads detected
    pub cpu_overloads: u32,
    /// Whether the resampler has been disabled due to creation or rebuild failures.
    pub resampler_disabled: bool,
    /// Whether the convolution engine's loaded IR has a stale frequency
    /// mapping due to a sample rate change and needs to be reloaded.
    pub convolution_ir_needs_reload: bool,
    /// Latest fatal engine error that requires UI intervention or playback halt.
    pub engine_error: Option<String>,
}

impl Default for PlaybackInfo {
    fn default() -> Self {
        Self {
            state: PlaybackState::Stopped,
            position_secs: 0.0,
            duration_secs: 0.0,
            volume: 0.75,
            speed: 1.0,
            track_id: None,
            sample_rate: DEFAULT_SAMPLE_RATE,
            cpu_usage_pct: 0.0,
            cpu_overloads: 0,
            resampler_disabled: false,
            convolution_ir_needs_reload: false,
            engine_error: None,
        }
    }
}
