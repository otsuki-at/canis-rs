use std::fmt;

#[derive(Debug)]
pub enum WatcherError {
    NotifyError(notify::Error),
    ConfigError(String),
    IoError(std::io::Error),
    UnsupportedSystem(String),
    HashError(String),
    DatabaseError(String),
    CanonicalizeFailed(std::io::Error),
    UriFailed(String),
    Other(String),
}

impl fmt::Display for WatcherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotifyError(e) => write!(f, "Watcher error: {}", e),
            Self::ConfigError(s) => write!(f, "Configuration error: {}", s),
            Self::IoError(e) => write!(f, "I/O error: {}", e),
            Self::UnsupportedSystem(s) => write!(f, "Unsupported system: {}", s),
            Self::HashError(s) => write!(f, "Hash computation error: {}", s),
            Self::DatabaseError(s) => write!(f, "Database error: {}", s),
            Self::CanonicalizeFailed(e) => write!(f, "Path canonicalize failed: {}", e),
            Self::UriFailed(p)          => write!(f, "URI conversion failed: {}", p),
            Self::Other(s) => write!(f, "Error: {}", s),
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
        Self::ConfigError(format!("TOML parse error: {}", err))
    }
}

// anyhow::Error からの変換を追加
impl From<anyhow::Error> for WatcherError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err.to_string())
    }
}

impl From<rusqlite::Error> for WatcherError {
    fn from(err: rusqlite::Error) -> Self {
        Self::DatabaseError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, WatcherError>;
