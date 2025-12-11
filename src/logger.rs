use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::error::Result;

pub trait Logger: Send + Sync {
    fn log(&self, message: &str);
}

pub struct FileLogger {
    file: Arc<Mutex<std::fs::File>>,
}

impl FileLogger {
    pub fn new(path: &str) -> Result<Self> {
        // ディレクトリが存在しない場合は作成
        if let Some(parent) = std::path::Path::new(path).parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        Ok(Self {
            file: Arc::new(Mutex::new(file)),
        })
    }
}

impl Logger for FileLogger {
    fn log(&self, message: &str) {
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "{}", message);
        }
    }
}

/// XDG Base Directory に従ってデフォルトのログファイルパスを取得
/// 1. 設定ファイルで指定されていればそれを使用（呼び出し側で判断）
/// 2. なければ XDG data directory 配下にログファイルを作成
pub fn get_default_log_path(app_name: &str) -> Option<PathBuf> {
    directories_next::ProjectDirs::from("", "", app_name)
        .map(|proj_dirs| {
            let data_dir = proj_dirs.data_dir();
            data_dir.join("canis.log")
        })
}
