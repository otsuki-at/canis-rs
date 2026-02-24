use serde::Deserialize;
use crate::error::{Result, WatcherError};
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct BasicSettings {
    pub watcher: String,
    pub targets: Vec<String>,
    pub logfile: Option<String>,
    pub hashdir: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub basic_settings: BasicSettings,
}

impl Config {
    pub fn from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(WatcherError::ConfigError(
                format!("設定ファイルが見つかりません: {}", path.display())
            ));
        }

        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;

        config.validate()?;

        Ok(config)
    }

    pub fn from_xdg() -> Result<Self> {
        use directories_next::ProjectDirs;

        let proj_dirs = ProjectDirs::from("", "", "canis")
            .ok_or_else(|| WatcherError::ConfigError(
                "XDG設定ディレクトリを取得できませんでした".to_string()
            ))?;

        let config_path = proj_dirs.config_dir().join("config.toml");

        if !config_path.exists() {
            return Err(WatcherError::ConfigError(
                format!("設定ファイルが見つかりません: {}", config_path.display())
            ));
        }

        let content = std::fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&content)?;

        config.validate()?;

        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.basic_settings.targets.is_empty() {
            return Err(WatcherError::ConfigError(
                "監視パスが指定されていません".to_string()
            ));
        }

        Ok(())
    }
}
