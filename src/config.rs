use serde::Deserialize;
use crate::error::{Result, WatcherError};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub watcher_system: String,
    pub processor_level: u8,
    pub watch_paths: Vec<String>,
    pub log_file: Option<String>,
}

impl Config {
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
        if self.watch_paths.is_empty() {
            return Err(WatcherError::ConfigError(
                "監視パスが指定されていません".to_string()
            ));
        }

        if self.processor_level != 1 && self.processor_level != 3 {
            return Err(WatcherError::ConfigError(
                format!("未対応の処理レベル: L{}", self.processor_level)
            ));
        }

        Ok(())
    }
}
