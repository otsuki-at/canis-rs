use std::path::PathBuf;

use crate::db::{OperationType};

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
    pub fn operation_type(&self) -> OperationType {
        match self {
            CanonicalEvent::Create { .. }  => OperationType::Create,
            CanonicalEvent::Modify { .. }  => OperationType::Modify,
            CanonicalEvent::Move { .. }    => OperationType::Move,
            CanonicalEvent::Write { .. }   => OperationType::Write,
            CanonicalEvent::Open { .. }    => OperationType::Open,
            CanonicalEvent::Append { .. }  => OperationType::Append,
        }
    }
}
