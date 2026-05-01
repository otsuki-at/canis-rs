use std::sync::Mutex;
use chrono::{Utc, TimeZone};
use url::Url;
#[cfg(feature = "fuse")]
use std::path::{Path, PathBuf};


use crate::event::{CanonicalEvent, FileEvent};
use crate::observer::Observer;
use crate::error::{Result, WatcherError};
use crate::db::{EventRepository, Digest, Operation, Process};

/// ProcessorObserver: ProcessorStrategyをObserverインターフェースでラップ
pub struct ProcessorObserver {
    db: Mutex<EventRepository>,
    #[cfg(feature = "fuse")]
    target: Url,
    #[cfg(feature = "fuse")]
    target_depth: usize,
}

impl ProcessorObserver {
    /// 処理レベルから戦略を選択してProcessorObserverを作成
    pub fn new(db: EventRepository, target: Option<&str>) -> Self {
        #[cfg(feature = "fuse")]
            let target = Url::from_file_path(target.unwrap()).expect("Invalid file path");

        Self {
            db: Mutex::new(db),
            #[cfg(feature = "fuse")]
            target_depth: Path::new(target.path()).components().count(),
            #[cfg(feature = "fuse")]
            target,
        }
    }

    #[cfg(feature = "fuse")]
    fn replace_watch_path(&self, uri: &Url) -> Url {
        let relative: PathBuf = Path::new(uri.path())
            .components()
            .skip(self.target_depth)
            .collect();

        let new_path = Path::new(self.target.path()).join(&relative);
        Url::from_file_path(&new_path).expect("Failed to convert path to URL")
    }

    fn compute_hash(&self, uri: &Url) -> Result<String> {
        use sha2::{Digest, Sha256};
        use hex::encode;

        let path = match uri.scheme() {
            "file" => uri.to_file_path()
                .map_err(|_| WatcherError::UriFailed(uri.to_string()))?,
            scheme => return Err(WatcherError::UnsupportedSystem(
                format!("Unsupported scheme for hash computation: {}", scheme)
            )),
        };

        let data = std::fs::read(&path).map_err(|e| {
            WatcherError::HashError(format!("Failed to read file {}: {}", path.display(), e))
        })?;

        let mut hasher = Sha256::new();
        hasher.update(&data);
        Ok(encode(hasher.finalize()))
    }

    fn process(&self, file_event: &FileEvent) {
        let (digest, operation) = match &file_event.event {
            CanonicalEvent::Create { uri, time } | CanonicalEvent::Modify { uri, time } => {
                match self.compute_hash(uri) {
                    Ok(hash) => {
                        #[cfg(feature = "fuse")]
                        let store_uri = self.replace_watch_path(uri);
                        #[cfg(not(feature = "fuse"))]
                        let store_uri = uri;

                        (
                            Digest{
                                uri:  store_uri.clone(),
                                hash: hash,
                            },
                            Operation{
                                timestamp: time.clone(),
                                operation: file_event.event.operation_type(),
                                uri: store_uri.clone(),
                                src_path: None,
                            }
                        )
                    },
                    Err(e) => {
                        eprintln!("Unexpected error while processing: {} - {}", uri.to_string(), e);
                        return;
                    }
                }
            }

            CanonicalEvent::Move { src, dst, time } =>{
                #[cfg(feature = "fuse")]
                let src_uri = self.replace_watch_path(src);
                #[cfg(not(feature = "fuse"))]
                let src_uri = src;

                #[cfg(feature = "fuse")]
                let dst_uri = self.replace_watch_path(dst);
                #[cfg(not(feature = "fuse"))]
                let dst_uri = dst;

                let src_str = src.to_string();
                match self.db.lock().unwrap().latest_hash_by_path(&src_str){
                    Ok(Some(hash)) => (
                        Digest{
                            uri:  dst_uri.clone(),
                            hash: hash,
                        },
                        Operation{
                            timestamp: time.clone(),
                            operation: file_event.event.operation_type(),
                            uri: dst_uri.clone(),
                            src_path: Some(src_uri.clone()),
                        }
                    ),
                    Ok(None) => {
                        // 変更元のファイルパスが DB から見つからなかった場合
                        match self.compute_hash(dst) {
                            Ok(hash) => (
                                Digest{
                                    uri:  dst_uri.clone(),
                                    hash: hash,
                                },
                                Operation{
                                    timestamp: time.clone(),
                                    operation: file_event.event.operation_type(),
                                    uri: dst_uri.clone(),
                                    src_path: Some(src_uri.clone()),
                                }
                            ),
                            Err(e) => {
                                eprintln!("Unexpected error while processing: {} - {}", src.to_string(), e);
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Unexpected error while processing: {} - {}", src.to_string(), e);
                        return;
                    }
                }
            }

            _ => return,
        };

        let digest_id = self.db.lock().unwrap().insert_digest(&digest).unwrap();
        let operation_id = self.db.lock().unwrap().insert_operation(&digest_id, &operation).unwrap();

        if let Some(process_info) = &file_event.process_info{
            let starttime = Utc
                .timestamp_opt(process_info.start_time as i64, 0)
                .single()
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default();

            let process = Process {
                starttime,
                pid: process_info.pid,
                ppid: process_info.ppid,
                exe: process_info.exe.clone(),
                cmd: process_info.cmd.clone()
            };

            // let _ = self.db.lock().unwrap().insert_process(&operation_id, &process);
            if let Err(e) = self.db.lock().unwrap().insert_process(&operation_id, &process) {
                eprintln!("insert_process error: {:?}", e);
            }
        }
    }
}

impl Observer for ProcessorObserver {
    fn update(&self, event: &FileEvent) {
        self.process(event);
    }
}

