use std::fs::{self, OpenOptions};
use std::io::{Write, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::error::Result;

pub trait Logger: Send + Sync {
    fn log(&self, message: &str);
    fn find_latest_hash(&self, path: &str) -> Result<Option<String>>;
}

pub struct FileLogger {
    file: Arc<Mutex<std::fs::File>>,
    path: PathBuf,
}

impl FileLogger {
    pub fn new(path: &str) -> Result<Self> {
        let path_buf = PathBuf::from(path);

        // ディレクトリが存在しない場合は作成
        if let Some(parent) = path_buf.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            path: path_buf,
        })
    }

    /// ファイルを逆順に読み取り、指定されたパスに一致する最新のハッシュ値を返す
    /// 大容量ファイルに対応するため、バッファリングしながら逆順に読み取る
    fn find_hash_reverse(&self, target_path: &str) -> Result<Option<String>> {
        let mut file = std::fs::File::open(&self.path)?;

        // ファイルサイズを取得
        let file_size = file.metadata()?.len();
        if file_size == 0 {
            return Ok(None);
        }

        const BUFFER_SIZE: usize = 8192; // 8KB単位で読み取り
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut leftover = Vec::new();
        let mut pos = file_size;

        loop {
            // 読み取る位置とサイズを計算
            let read_size = if pos >= BUFFER_SIZE as u64 {
                BUFFER_SIZE
            } else {
                pos as usize
            };

            if read_size == 0 {
                break;
            }

            pos -= read_size as u64;
            file.seek(SeekFrom::Start(pos))?;
            file.read_exact(&mut buffer[..read_size])?;

            // 現在のバッファと前回の残りを結合
            let mut chunk = buffer[..read_size].to_vec();
            chunk.extend_from_slice(&leftover);
            leftover.clear();

            // 改行で分割（逆順）
            let text = String::from_utf8_lossy(&chunk);
            let mut lines: Vec<&str> = text.split('\n').collect();

            // 最後の要素（最も古い行の一部）は次の反復に持ち越す
            if pos > 0 && !lines.is_empty() {
                leftover = lines[0].as_bytes().to_vec();
                lines.remove(0);
            }

            // 逆順（新しい行から古い行へ）に検索
            for line in lines.iter().rev() {
                if line.trim().is_empty() {
                    continue;
                }

                // CSV形式: 時刻,ハッシュ値,ファイルパス
                let parts: Vec<&str> = line.splitn(3, ',').collect();
                if parts.len() >= 3 {
                    let log_path = parts[2].trim();
                    if log_path == target_path {
                        return Ok(Some(parts[1].to_string()));
                    }
                }
            }

            if pos == 0 {
                break;
            }
        }

        // 最後の残りをチェック
        if !leftover.is_empty() {
            let line = String::from_utf8_lossy(&leftover);
            let parts: Vec<&str> = line.splitn(3, ',').collect();
            if parts.len() >= 3 {
                let log_path = parts[2].trim();
                if log_path == target_path {
                    return Ok(Some(parts[1].to_string()));
                }
            }
        }

        Ok(None)
    }
}

impl Logger for FileLogger {
    fn log(&self, message: &str) {
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "{}", message);
        }
    }

    fn find_latest_hash(&self, path: &str) -> Result<Option<String>> {
        self.find_hash_reverse(path)
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
