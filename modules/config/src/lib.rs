// Re-export the enums at the crate root for direct access (`config::FilterType`,
// `config::CrossfeedProfile`, etc.). The previous `types::enums` shim module
// re-exported `crate::*` which made the path self-referential and polluted the
// public API. Callers should use the root path or `config::prelude::*`.
//
// Backwards-compatibility shim: keep `config::types::enums::Foo` working for
// any external code (and for `buffer.rs` which references
// `config::types::enums::CrossfeedProfile`) by re-exporting the specific
// enum types instead of the entire crate.
pub mod types {
    pub mod enums {
        pub use crate::{
            AudioBackend, CrossfadeConfig, CrossfeedProfile, FilterType, LoudnessMode,
            PerformanceMode, ResamplerQuality,
        };
    }
}

use std::path::{Path, PathBuf};

/// Library scanner configuration.
///
/// Holds the list of directories the scanner walks and any future
/// per-library options (exclusion globs, max depth, ...). Lives in the
/// `config` crate so that both `library` and the root binary can construct
/// it without circular dependencies.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct LibraryConfig {
    /// Directories the scanner walks recursively for audio files.
    #[serde(default)]
    pub watch_dirs: Vec<std::path::PathBuf>,
    /// Optional maximum recursion depth (`None` = unlimited).
    #[serde(default)]
    pub max_depth: Option<usize>,
    /// Glob patterns to exclude (e.g. `["**/Podcasts/**"]`).
    #[serde(default)]
    pub exclude_globs: Vec<String>,
}

impl LibraryConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Convenience builder: add a watch directory.
    pub fn with_watch_dir(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.watch_dirs.push(dir.into());
        self
    }

    /// Resolve the on-disk config path: platform-specific data dir /
    /// "playtune" / "config.json".
    pub fn config_path() -> Option<PathBuf> {
        let base = dirs::data_local_dir()?;
        Some(base.join("playtune").join("config.json"))
    }

    /// Load from the default platform config path. Returns `Ok(None)` if the
    /// file does not exist (so callers can fall back to defaults without
    /// treating first-launch as an error).
    pub fn load() -> Result<Option<Self>, ConfigError> {
        Self::load_from(&Self::config_path().ok_or(ConfigError::NoDataDir)?)
    }

    /// Load from an explicit path.
    pub fn load_from(path: &Path) -> Result<Option<Self>, ConfigError> {
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(path).map_err(ConfigError::Io)?;
        if bytes.is_empty() {
            return Ok(None);
        }
        let cfg: Self = serde_json::from_slice(&bytes).map_err(ConfigError::Json)?;
        Ok(Some(cfg))
    }

    /// Save to the default platform config path, atomically. The write goes
    /// to a sibling `.tmp` file first, then `rename` replaces the target.
    /// This prevents concurrent writers (e.g. two PlayTune instances, or a
    /// settings UI thread racing a background scan thread) from corrupting
    /// the file by interleaving partial writes.
    pub fn save(&self) -> Result<(), ConfigError> {
        Self::save_to(&Self::config_path().ok_or(ConfigError::NoDataDir)?, self)
    }

    /// Save to an explicit path, atomically.
    pub fn save_to(path: &Path, cfg: &Self) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ConfigError::Io)?;
        }
        let tmp_path = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(cfg).map_err(ConfigError::Json)?;
        {
            let file = std::fs::File::create(&tmp_path).map_err(ConfigError::Io)?;
            use std::io::Write;
            let mut writer = std::io::BufWriter::new(&file);
            writer.write_all(&bytes).map_err(ConfigError::Io)?;
            writer.flush().map_err(ConfigError::Io)?;
            drop(writer);
            file.sync_all().map_err(ConfigError::Io)?;
        }
        std::fs::rename(&tmp_path, path).map_err(ConfigError::Io)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub library: LibraryConfig,
    #[serde(default)]
    pub engine: EngineConfig,
}

impl ConfigFile {
    /// Current config schema version. Bump this whenever a breaking
    /// schema change is made (a removed field, a renamed field, a type
    /// change). Loaders can compare this to the file's version to decide
    /// whether to migrate.
    pub const CURRENT_VERSION: u32 = 1;

    pub fn load() -> Result<Option<Self>, ConfigError> {
        Self::load_from(&Self::config_path().ok_or(ConfigError::NoDataDir)?)
    }

    pub fn load_from(path: &Path) -> Result<Option<Self>, ConfigError> {
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(path).map_err(ConfigError::Io)?;
        if bytes.is_empty() {
            return Ok(None);
        }
        let cfg: Self = serde_json::from_slice(&bytes).map_err(ConfigError::Json)?;
        if cfg.version > Self::CURRENT_VERSION {
            log::warn!(
                "Config file version {} is newer than CURRENT_VERSION {}; \
                 some fields may have been dropped during deserialization.",
                cfg.version,
                Self::CURRENT_VERSION
            );
        }
        Ok(Some(cfg))
    }

    pub fn config_path() -> Option<PathBuf> {
        let base = dirs::data_local_dir()?;
        Some(base.join("playtune").join("config.json"))
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        Self::save_to(&Self::config_path().ok_or(ConfigError::NoDataDir)?, self)
    }

    pub fn save_to(path: &Path, cfg: &Self) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ConfigError::Io)?;
        }
        let tmp_path = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(cfg).map_err(ConfigError::Json)?;
        {
            let file = std::fs::File::create(&tmp_path).map_err(ConfigError::Io)?;
            use std::io::Write;
            let mut writer = std::io::BufWriter::new(&file);
            writer.write_all(&bytes).map_err(ConfigError::Io)?;
            writer.flush().map_err(ConfigError::Io)?;
            drop(writer);
            file.sync_all().map_err(ConfigError::Io)?;
        }
        std::fs::rename(&tmp_path, path).map_err(ConfigError::Io)?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    #[error("JSON error: {0}")]
    Json(serde_json::Error),
    #[error("no local data directory available on this platform")]
    NoDataDir,
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

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct ConvolutionConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub wet_mix: f32,
    #[serde(default)]
    pub ir_path: Option<String>,
}

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
