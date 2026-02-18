use std::sync::Arc;
use anyhow::Result;
use std::fs::File;
#[cfg(target_os = "linux")]
use daemonize::Daemonize;

use crate::config::Config;
use crate::logger::{self, Logger, FileLogger};
use crate::watcher;
use crate::adapter;
use crate::processor;
use crate::observer::Subject;
use crate::error;
use crate::cli::StartArgs;

pub fn start(args: StartArgs) -> Result<()>{
    println!("=== ファイルアクセス監視システム ===\n");

    let config = if let Some(config_path) = args.config {
        // --config が指定された場合
        Config::from_file(&config_path)?
    } else {
        // --config が指定されなかった場合
        Config::from_xdg()?
    };

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

    #[cfg(target_os = "linux")]
    if args.daemon {
        println!("\nバックグラウンドで実行します...");
        daemonize()?;
    }

    #[cfg(not(unix))]
    if args.daemon {
        anyhow::bail!(
            "デーモン化は Unix/Linux/macOS でのみサポートされています。\n"
        );
    }
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

#[cfg(target_os = "linux")]
fn daemonize() -> Result<()> {
    use directories_next::ProjectDirs;

    let proj_dirs = ProjectDirs::from("", "", "canis")
        .ok_or_else(|| anyhow::anyhow!("XDG ディレクトリを取得できませんでした"))?;

    let data_dir = proj_dirs.data_dir();
    std::fs::create_dir_all(data_dir)?;

    let stdout = File::create(data_dir.join("canis.out"))?;
    let stderr = File::create(data_dir.join("canis.err"))?;
    let pid_file = data_dir.join("canis.pid");

    Daemonize::new()
        .pid_file(pid_file)
        .chown_pid_file(true)
        .working_directory("/")
        .stdout(stdout)
        .stderr(stderr)
        .start()?;

    Ok(())
}
