use serde::Deserialize;
use crate::error::{Result, WatcherError};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone)]
pub struct BasicSettings {
    pub watcher: Option<String>,
    pub targets: Option<Vec<String>>,
    pub ignore: Option<Vec<String>>,
    pub logfile: Option<String>,
    pub hashdir: Option<PathBuf>,
    pub token: Option<String>,
    pub repo: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub basic_settings: BasicSettings,
}

impl Config {
    pub fn from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(WatcherError::ConfigError(
                format!("Configuration file not found: {}", path.display())
            ));
        }

        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;

        Ok(config)
    }

    pub fn from_xdg() -> Result<Self> {
        use directories_next::ProjectDirs;

        let proj_dirs = ProjectDirs::from("", "", "canis")
            .ok_or_else(|| WatcherError::ConfigError(
                "Failed to retrieve XDG configuration directory".to_string()
            ))?;

        let config_path = proj_dirs.config_dir().join("config.toml");

        if !config_path.exists() {
            return Err(WatcherError::ConfigError(
                format!("Configuration file not found: {}", config_path.display())
            ));
        }

        let content = std::fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&content)?;

        Ok(config)
    }
}
