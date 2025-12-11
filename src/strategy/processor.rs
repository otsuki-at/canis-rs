use std::sync::Arc;
use crate::event::CanonicalEvent;
use crate::logger::Logger;
use crate::error::{Result, WatcherError};

pub trait ProcessorStrategy: Send + Sync {
    fn process(&self, event: &CanonicalEvent);
    #[allow(dead_code)]  // デバッグ/ログ用
    fn description(&self) -> String;
    #[allow(dead_code)]  // メタデータ用
    fn level(&self) -> u8;
}

/// L1レベル（基本）処理戦略: Create/Modifyイベントのハッシュとログ出力
pub struct L1ProcessorStrategy {
    logger: Arc<dyn Logger>,
}

impl L1ProcessorStrategy {
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
}

impl ProcessorStrategy for L1ProcessorStrategy {
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
            _ => return,
        };

        self.logger.log(&log_msg);
    }

    fn description(&self) -> String {
        "L1: 基本処理（ハッシュ値計算 + ログ出力）".to_string()
    }

    fn level(&self) -> u8 {
        1
    }
}

/// L3レベル（詳細）処理戦略: すべてのイベントを詳細表示
pub struct L3ProcessorStrategy {
    logger: Arc<dyn Logger>,
}

impl L3ProcessorStrategy {
    pub fn new(logger: Arc<dyn Logger>) -> Self {
        Self { logger }
    }
}

impl ProcessorStrategy for L3ProcessorStrategy {
    fn process(&self, event: &CanonicalEvent) {
        match event {
            CanonicalEvent::Create { path, time } => {
                println!("✨ Create {} ({})", path.display(), time);
                self.logger.log(&format!("CREATE {} time={}", path.display(), time));
            }
            CanonicalEvent::Modify { path, time } => {
                println!("📝 Modify {} ({})", path.display(), time);
                self.logger.log(&format!("MODIFY {} time={}", path.display(), time));
            }
            CanonicalEvent::Move { src, dst, time } => {
                println!("📦 Move {} → {} ({})", src.display(), dst.display(), time);
                self.logger.log(&format!("MOVE {} -> {} time={}", src.display(), dst.display(), time));
            }
            CanonicalEvent::Write { path, time, content } => {
                let size = content.len();
                println!("✏️ Write {} ({}) [{}バイト]", path.display(), time, size);

                if size > 0 {
                    let preview = String::from_utf8_lossy(&content[..size.min(100)]);
                    println!("   内容: {}", preview.trim());
                }

                self.logger.log(&format!("WRITE {} time={} size={}", path.display(), time, size));
            }
            CanonicalEvent::Open { path, time, pid } => {
                println!("📂 Open {} PID={:?} ({})", path.display(), pid, time);
                self.logger.log(&format!("OPEN {} pid={:?} time={}", path.display(), pid, time));
            }
            CanonicalEvent::Append { path, time } => {
                println!("📄 Append {} ({})", path.display(), time);
                self.logger.log(&format!("APPEND {} time={}", path.display(), time));
            }
        }
    }

    fn description(&self) -> String {
        "L3: 詳細処理（すべてのイベントを詳細表示）".to_string()
    }

    fn level(&self) -> u8 {
        3
    }
}

pub fn create_strategy(level: u8, logger: Arc<dyn Logger>) -> Box<dyn ProcessorStrategy> {
    match level {
        1 => Box::new(L1ProcessorStrategy::new(logger)),
        3 => Box::new(L3ProcessorStrategy::new(logger)),
        _ => panic!("未対応の処理レベル: L{}", level),
    }
}




