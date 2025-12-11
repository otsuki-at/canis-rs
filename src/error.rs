use std::fmt;

#[derive(Debug)]
pub enum WatcherError {
    NotifyError(notify::Error),
    ConfigError(String),
    IoError(std::io::Error),
    UnsupportedSystem(String),
    HashError(String),
}

impl fmt::Display for WatcherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotifyError(e) => write!(f, "監視エラー: {}", e),
            Self::ConfigError(s) => write!(f, "設定エラー: {}", s),
            Self::IoError(e) => write!(f, "I/Oエラー: {}", e),
            Self::UnsupportedSystem(s) => write!(f, "未対応システム: {}", s),
            Self::HashError(s) => write!(f, "ハッシュ計算エラー: {}", s),
        }
    }
}

impl std::error::Error for WatcherError {}

impl From<notify::Error> for WatcherError {
    fn from(err: notify::Error) -> Self {
        Self::NotifyError(err)
    }
}

impl From<std::io::Error> for WatcherError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}

impl From<toml::de::Error> for WatcherError {
    fn from(err: toml::de::Error) -> Self {
        Self::ConfigError(format!("TOML解析エラー: {}", err))
    }
}

pub type Result<T> = std::result::Result<T, WatcherError>;
