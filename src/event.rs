use url::Url;

use crate::db::{OperationType, Process};

#[derive(Clone)]
pub struct FileEvent {
    pub event: CanonicalEvent,
    pub process_info: Option<ProcessInfo>,
}

#[derive(Debug, Clone)]
pub enum CanonicalEvent {
    Create { uri: Url, time: String },
    Modify { uri: Url, time: String },
    Move { src: Url, dst: Url, time: String },
    Write { uri: Url, content: Vec<u8>, time: String },
    Open { uri: Url, time: String },
    Append { uri: Url, time: String },
}

#[derive(Clone)]
pub struct ProcessInfo {
    pub start_time:  u64,
    pub pid:        i32,
    pub ppid:       i32,
    pub exe:        String,
    pub cmd:        String,
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
