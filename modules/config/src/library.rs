use std::path::{Path, PathBuf};

use crate::file::ConfigError;

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
