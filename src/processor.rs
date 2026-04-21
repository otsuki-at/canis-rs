use std::sync::Mutex;
use crate::event::CanonicalEvent;
use crate::observer::Observer;
use crate::error::{Result, WatcherError};
use crate::db::{EventRepository, Digest, Operation};

/// ProcessorObserver: ProcessorStrategyをObserverインターフェースでラップ
pub struct ProcessorObserver {
    db: Mutex<EventRepository>
}

impl ProcessorObserver {
    /// 処理レベルから戦略を選択してProcessorObserverを作成
    pub fn new(db: EventRepository) -> Self {
        Self { db: Mutex::new(db) }
    }

    fn compute_hash(&self, path: &std::path::Path) -> Result<String> {
        use sha2::{Digest, Sha256};
        use hex::encode;

        let data = std::fs::read(path).map_err(|e| {
            WatcherError::HashError(format!("Failed to read file {}: {}", path.display(), e))
        })?;

        let mut hasher = Sha256::new();
        hasher.update(&data);
        Ok(encode(hasher.finalize()))
    }

    fn process(&self, event: &CanonicalEvent) {
        let (digest, operation) = match event {
            CanonicalEvent::Create { path, time } | CanonicalEvent::Modify { path, time } => {
                match self.compute_hash(path) {
                    Ok(hash) => (
                        Digest{
                            filepath:  path.display().to_string(),
                            hash: hash,
                        },
                        Operation{
                            timestamp: time.clone(),
                            operation: event.operation_type(),
                            filepath: path.display().to_string(),
                            src_path: None,
                            pid: None,
                            ppid: None,
                        }
                    ),
                    Err(e) => {
                        eprintln!("Unexpected error while processing: {} - {}", path.display(), e);
                        return;
                    }
                }
            }

            CanonicalEvent::Move { src, dst, time } =>{
                let src_str = src.display().to_string();
                match self.db.lock().unwrap().latest_hash_by_path(&src_str){
                    Ok(Some(hash)) => (
                        Digest{
                            filepath:  dst.display().to_string(),
                            hash: hash,
                        },
                        Operation{
                            timestamp: time.clone(),
                            operation: event.operation_type(),
                            filepath: dst.display().to_string(),
                            src_path: Some(src_str),
                            pid: None,
                            ppid: None,
                        }
                    ),
                    Ok(None) => {
                        // 変更元のファイルパスが見つからなかった場合
                        match self.compute_hash(dst) {
                            Ok(hash) => (
                                Digest{
                                    filepath:  dst.display().to_string(),
                                    hash: hash,
                                },
                                Operation{
                                    timestamp: time.clone(),
                                    operation: event.operation_type(),
                                    filepath: dst.display().to_string(),
                                    src_path: Some(src_str),
                                    pid: None,
                                    ppid: None,
                                }
                            ),
                            Err(e) => {
                                eprintln!("Unexpected error while processing: {} - {}", dst.display(), e);
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Unexpected error while processing: {} - {}", dst.display(), e);
                        return;
                    }
                }
            }

            _ => return,
        };

        let digest_id = self.db.lock().unwrap().insert_digest(&digest).unwrap();
        let _ = self.db.lock().unwrap().insert_operation(&digest_id, &operation);
    }
}

impl Observer for ProcessorObserver {
    fn update(&self, event: &CanonicalEvent) {
        self.process(event);
    }
}

