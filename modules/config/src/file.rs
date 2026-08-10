use std::path::{Path, PathBuf};

use crate::{EngineConfig, LibraryConfig};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    #[error("JSON error: {0}")]
    Json(serde_json::Error),
    #[error("no local data directory available on this platform")]
    NoDataDir,
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
