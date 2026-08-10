// DSP configuration structs for the PlayTune audio engine.
use crate::enums::{
    AudioBackend, CrossfeedProfile, FilterType, LoudnessMode, PerformanceMode, ResamplerQuality,
};

// ---- EQ ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EqBandConfig {
    #[serde(default = "default_filter_type")]
    pub filter_type: FilterType,
    #[serde(default = "default_eq_frequency")]
    pub frequency: f32,
    #[serde(default)]
    pub gain_db: f32,
    #[serde(default = "default_eq_q")]
    pub q: f32,
    #[serde(default = "default_eq_enabled")]
    pub enabled: bool,
}

fn default_filter_type() -> FilterType {
    FilterType::Peaking
}
fn default_eq_frequency() -> f32 {
    1000.0
}
fn default_eq_q() -> f32 {
    1.0
}
fn default_eq_enabled() -> bool {
    true
}

impl Default for EqBandConfig {
    fn default() -> Self {
        Self {
            filter_type: FilterType::Peaking,
            frequency: 1000.0,
            gain_db: 0.0,
            q: 1.0,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EqConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub preamp_db: f32,
    #[serde(default)]
    pub post_gain_db: f32,
    #[serde(default)]
    pub headroom_db: f32,
    #[serde(default)]
    pub bands: Vec<EqBandConfig>,
}

impl Default for EqConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            preamp_db: 0.0,
            post_gain_db: 0.0,
            headroom_db: 0.0,
            bands: Vec::new(),
        }
    }
}

// ---- Loudness ---------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LoudnessConfig {
    #[serde(default)]
    pub mode: LoudnessMode,
    #[serde(default = "default_target_lufs")]
    pub target_lufs: f32,
    #[serde(default)]
    pub true_peak_guard: bool,
    #[serde(default = "default_true_peak_dbtp")]
    pub true_peak_dbtp: f32,
}

fn default_target_lufs() -> f32 {
    -14.0
}
fn default_true_peak_dbtp() -> f32 {
    -1.0
}

impl Default for LoudnessConfig {
    fn default() -> Self {
        Self {
            mode: LoudnessMode::Off,
            target_lufs: -14.0,
            true_peak_guard: false,
            true_peak_dbtp: -1.0,
        }
    }
}

// ---- Convolution ------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct ConvolutionConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub wet_mix: f32,
    #[serde(default)]
    pub ir_path: Option<String>,
}

// ---- Crossfeed --------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CrossfeedConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub profile: CrossfeedProfile,
    #[serde(default = "default_crossfeed_freq")]
    pub custom_freq: f32,
    #[serde(default = "default_crossfeed_q")]
    pub custom_q: f32,
    #[serde(default = "default_crossfeed_delay")]
    pub custom_delay_ms: f32,
}

fn default_crossfeed_freq() -> f32 {
    700.0
}
fn default_crossfeed_q() -> f32 {
    0.707
}
fn default_crossfeed_delay() -> f32 {
    0.3
}

impl Default for CrossfeedConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            profile: CrossfeedProfile::Bauer,
            custom_freq: 700.0,
            custom_q: 0.707,
            custom_delay_ms: 0.3,
        }
    }
}

// ---- Multiband Compressor --------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CompressorBandConfig {
    #[serde(default)]
    pub threshold_db: f32,
    #[serde(default = "default_compressor_ratio")]
    pub ratio: f32,
    #[serde(default = "default_compressor_attack")]
    pub attack_ms: f32,
    #[serde(default = "default_compressor_release")]
    pub release_ms: f32,
    #[serde(default)]
    pub makeup_gain_db: f32,
}

fn default_compressor_ratio() -> f32 {
    1.0
}
fn default_compressor_attack() -> f32 {
    10.0
}
fn default_compressor_release() -> f32 {
    100.0
}

impl Default for CompressorBandConfig {
    fn default() -> Self {
        Self {
            threshold_db: 0.0,
            ratio: 1.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            makeup_gain_db: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct MultibandCompressorConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub low_band: CompressorBandConfig,
    #[serde(default)]
    pub mid_band: CompressorBandConfig,
    #[serde(default)]
    pub high_band: CompressorBandConfig,
}

// ---- Stereo Enhancer -------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct StereoEnhancerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_stereo_width")]
    pub width: f32,
}

fn default_stereo_width() -> f32 {
    1.0
}

// ---- Limiter ---------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LimiterConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_lookahead_ms")]
    pub lookahead_ms: f32,
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_ceiling_db")]
    pub ceiling_db: f32,
    #[serde(default = "default_soft_clip")]
    pub soft_clip: bool,
}

fn default_lookahead_ms() -> f32 {
    5.0
}
fn default_attack_ms() -> f32 {
    1.0
}
fn default_release_ms() -> f32 {
    50.0
}
fn default_ceiling_db() -> f32 {
    -0.1
}
fn default_soft_clip() -> bool {
    true
}

impl Default for LimiterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            lookahead_ms: 5.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            ceiling_db: -0.1,
            soft_clip: true,
        }
    }
}

// ---- Crossfade -------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CrossfadeConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_crossfade_duration")]
    pub duration_ms: u32,
}

fn default_crossfade_duration() -> u32 {
    3000
}

impl Default for CrossfadeConfig {
    fn default() -> Self {
        Self { enabled: false, duration_ms: 3000 }
    }
}

// ---- Engine Config (aggregates all DSP sub-configs) ------------------

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EngineConfig {
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default)]
    pub output_backend: AudioBackend,
    #[serde(default)]
    pub output_device: Option<String>,
    #[serde(default)]
    pub resampler_quality: ResamplerQuality,
    #[serde(default)]
    pub performance_mode: PerformanceMode,
    #[serde(default = "default_dither_enabled")]
    pub dither_enabled: bool,
    #[serde(default = "default_volume_fade_ms")]
    pub volume_fade_ms: u32,
    #[serde(default = "default_seek_fade_ms")]
    pub seek_fade_ms: u32,
    #[serde(default)]
    pub eq: EqConfig,
    #[serde(default)]
    pub loudness: LoudnessConfig,
    #[serde(default)]
    pub crossfeed: CrossfeedConfig,
    #[serde(default)]
    pub multiband_compressor: MultibandCompressorConfig,
    #[serde(default = "default_stereo_enhancer")]
    pub stereo_enhancer: StereoEnhancerConfig,
    #[serde(default)]
    pub limiter: LimiterConfig,
    #[serde(default)]
    pub crossfade: CrossfadeConfig,
    #[serde(default)]
    pub convolution: ConvolutionConfig,
}

fn default_sample_rate() -> u32 {
    44100
}
fn default_dither_enabled() -> bool {
    true
}
fn default_volume_fade_ms() -> u32 {
    10
}
fn default_seek_fade_ms() -> u32 {
    30
}
fn default_stereo_enhancer() -> StereoEnhancerConfig {
    StereoEnhancerConfig { enabled: false, width: 1.0 }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            output_backend: AudioBackend::Auto,
            output_device: None,
            resampler_quality: ResamplerQuality::Balanced,
            performance_mode: PerformanceMode::Normal,
            dither_enabled: true,
            volume_fade_ms: 10,
            seek_fade_ms: 30,
            eq: EqConfig::default(),
            loudness: LoudnessConfig::default(),
            crossfeed: CrossfeedConfig::default(),
            multiband_compressor: MultibandCompressorConfig::default(),
            stereo_enhancer: StereoEnhancerConfig::default(),
            limiter: LimiterConfig::default(),
            crossfade: CrossfadeConfig::default(),
            convolution: ConvolutionConfig::default(),
        }
    }
}

impl EngineConfig {
    pub fn sample_rate(&self) -> SampleRateWrapper {
        SampleRateWrapper(self.sample_rate)
    }
}

pub struct SampleRateWrapper(pub u32);
