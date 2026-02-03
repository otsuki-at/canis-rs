use std::sync::Arc;

mod error;
mod config;
mod logger;
mod event;
mod observer;
mod adapter;
mod watcher;
mod processor;

use error::Result;
use config::Config;
use logger::{Logger, FileLogger};
use observer::Subject;

fn main() -> Result<()> {
    println!("=== ファイルアクセス監視システム ===\n");

    let config = Config::from_xdg()?;

    println!("\n--- 設定内容 ---");
    println!("監視システム: {}", config.watcher_system);
    println!("処理レベル: L{}", config.processor_level);
    println!("監視パス数: {}", config.watch_paths.len());

    let logger: Arc<dyn Logger> = match &config.log_file {
        Some(path) => {
            println!("ログファイル: {} (設定ファイルで指定)", path);
            Arc::new(FileLogger::new(path)?)
        },
        None => {
            // XDG に従ったデフォルトパスを取得
            let default_path = logger::get_default_log_path("canis")
                .ok_or_else(|| {
                    eprintln!("エラー: XDG データディレクトリを取得できませんでした");
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "XDG data directory not available"
                    )
                })?;

            let path_str = default_path.to_string_lossy();
            println!("ログファイル: {} (XDG デフォルト)", path_str);
            Arc::new(FileLogger::new(&path_str)?)
        }
    };

    let mut watcher = watcher::FileWatcher::new(&config.watcher_system)?;

    // 中間層: EventAdapterを作成
    let mut adapter = adapter::EventAdapter::new(
        config.processor_level,
    );

    // 処理部: ProcessorObserverを作成
    let processor = processor::ProcessorObserver::new(
        Arc::clone(&logger)
    );

    // 中間層にプロセッサーをObserverとして登録 (オブザーバパターン第2段階)
    adapter.attach(Box::new(processor));

    // 監視部にアダプターをObserverとして登録 (オブザーバパターン第1段階)
    watcher.attach(Box::new(adapter));

    watcher.start_watching(&config.watch_paths)?;

    Ok(())
}
