use std::sync::Arc;
use crate::event::CanonicalEvent;
use crate::logger::Logger;
use crate::observer::Observer;
use crate::error::{Result, WatcherError};

/// ProcessorObserver: ProcessorStrategyをObserverインターフェースでラップ
pub struct ProcessorObserver {
    logger: Arc<dyn Logger>,
}

impl ProcessorObserver {
    /// 処理レベルから戦略を選択してProcessorObserverを作成
    pub fn new(logger: Arc<dyn Logger>) -> Self {
        Self { logger }
    }

    fn compute_hash(&self, path: &std::path::Path) -> Result<String> {
        use sha2::{Digest, Sha256};
        use hex::encode;

        let data = std::fs::read(path).map_err(|e| {
            WatcherError::HashError(format!("ファイル読み込みエラー {}: {}", path.display(), e))
        })?;

        let mut hasher = Sha256::new();
        hasher.update(&data);
        Ok(encode(hasher.finalize()))
    }

    fn process(&self, event: &CanonicalEvent) {
        let log_msg = match event {
            CanonicalEvent::Create { path, time } | CanonicalEvent::Modify { path, time } => {
                match self.compute_hash(path) {
                    Ok(hash) => format!("{},{},{}", time, hash, path.display()),
                    Err(WatcherError::HashError(e)) => {
                        eprintln!("⚠️  {}", e);
                        return;
                    }
                    Err(e) => {
                        eprintln!("⚠️  予期しないエラー: {} - {}", path.display(), e);
                        return;
                    }
                }
            }

            CanonicalEvent::Move { src, dst, time } => {
                let src_str = src.display().to_string();
                match self.logger.find_latest_hash(&src_str) {
                    Ok(Some(hash)) => {
                        // 変更元のファイルパスが見つかった場合
                        format!("{},{},{}", time, hash, dst.display())
                    }
                    Ok(None) => {
                        // 変更元のファイルパスが見つからなかった場合
                        match self.compute_hash(dst) {
                            Ok(hash) => format!("{},{},{}", time, hash, dst.display()),
                            Err(WatcherError::HashError(e)) => {
                                eprintln!("⚠️  {}", e);
                                return;
                            }
                            Err(e) => {
                                eprintln!("⚠️  予期しないエラー: {} - {}", dst.display(), e);
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        // ログ検索エラー
                        eprintln!("⚠️  ログ検索エラー: {} - {}", src.display(), e);
                        return;
                    }
                }
            }
            _ => return,
        };

        self.logger.log(&log_msg);
    }
}

impl Observer for ProcessorObserver {
    fn update(&self, event: &CanonicalEvent) {
        self.process(event);
    }
}

