use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum CanonicalEvent {
    Create { path: PathBuf, time: String },
    Modify { path: PathBuf, time: String },
    Move { src: PathBuf, dst: PathBuf, time: String },
    Write { path: PathBuf, content: Vec<u8>, time: String },
    Open { path: PathBuf, pid: Option<u32>, time: String },
    Append { path: PathBuf, time: String },
}

impl CanonicalEvent {
    pub fn path(&self) -> &PathBuf {
        match self {
            Self::Create { path, .. } => path,
            Self::Modify { path, .. } => path,
            Self::Move { dst, .. } => dst,
            Self::Write { path, .. } => path,
            Self::Open { path, .. } => path,
            Self::Append { path, .. } => path,
        }
    }

    pub fn time(&self) -> &str {
        match self {
            Self::Create { time, .. } => time,
            Self::Modify { time, .. } => time,
            Self::Move { time, .. } => time,
            Self::Write { time, .. } => time,
            Self::Open { time, .. } => time,
            Self::Append { time, .. } => time,
        }
    }
}
