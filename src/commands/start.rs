use std::sync::Arc;
use anyhow::Result;
use std::fs::File;
use std::path::PathBuf;
#[cfg(unix)]
use daemonize::Daemonize;

use crate::config::Config;
use crate::logger::{self, Logger, FileLogger};
use crate::watcher;
use crate::adapter;
use crate::processor;
use crate::observer::Subject;
use crate::error::WatcherError;
use crate::cli::StartArgs;

pub fn start(args: StartArgs) -> Result<()>{
    let config = if args.is_complete() {
        None
    } else if let Some(config_path) = args.config {
        Some(Config::from_file(&config_path)?)
    } else {
        Some(Config::from_xdg()?)
    };

    let settings = config.as_ref().map(|c| &c.basic_settings);

    let watcher_system = args.watcher
        .filter(|w| !w.is_empty())
        .or_else(|| settings.and_then(|s| s.watcher.clone()))
        .ok_or_else(|| WatcherError::ConfigError(
            "Failed to determine watcher".to_string()
        ))?;

    let targets = args.targets
        .filter(|t| !t.is_empty() && t.iter().all(|s| !s.is_empty()))
        .or_else(|| settings.and_then(|s| s.targets.clone()))
        .ok_or_else(|| WatcherError::ConfigError(
            "targets not specified".to_string()
        ))?;

    let logfile_path = args.logfile
        .filter(|l| !l.is_empty())
        .or_else(|| settings.and_then(|s| s.logfile.clone()))
        .or_else(|| {
            logger::get_default_log_path("canis")
                .map(|p| p.to_string_lossy().into_owned())
        })
        .ok_or_else(|| WatcherError::ConfigError(
            "Failed to determine log file path".to_string()
        ))?;

    let logger: Arc<dyn Logger> = Arc::new(FileLogger::new(
        &logfile_path
    )?);

    let user_ignore_paths = args.ignore
        .filter(|t| !t.is_empty() && t.iter().all(|s| !s.is_empty()))
        .or_else(|| settings.and_then(|s| s.ignore.clone()))
        .ok_or_else(|| WatcherError::ConfigError(
            "ignore paths not specified".to_string()
        ))?;

    let ignore_paths: Vec<String> = std::iter::once(logfile_path.clone())
        .chain(user_ignore_paths)
        .collect();

    println!("===Configuration===");
    println!("watcher system: {}", watcher_system);
    println!("targets: {}", targets.join(", "));
    println!("ignore_paths: {}", ignore_paths.join(", "));
    println!("logfile: {}\n", logfile_path);

    #[cfg(unix)]
    if args.daemon {
        println!("\nRunning in the background");
        daemonize()?;
    }

    #[cfg(not(unix))]
    if args.daemon {
        anyhow::bail!(
            "Daemon mode is supported only on Unix-like systems\n"
        );
    }
    let mut watcher = watcher::FileWatcher::new(&watcher_system)?;

    let processor_level = match watcher_system.as_str() {
        "notify" => 1,
        #[cfg(all(feature = "fuse", any(target_os = "linux", target_os = "macos")))]
        "fuse"   => 3,
        _        => 1,
    };

    // 中間層: EventAdapterを作成
    let mut adapter = adapter::EventAdapter::new(
        processor_level,
    );

    // 処理部: ProcessorObserverを作成
    let processor = processor::ProcessorObserver::new(
        Arc::clone(&logger)
    );

    // 中間層にプロセッサーをObserverとして登録 (オブザーバパターン第2段階)
    adapter.attach(Box::new(processor));

    // 監視部にアダプターをObserverとして登録 (オブザーバパターン第1段階)
    watcher.attach(Box::new(adapter));

    watcher.start_watching(&targets, &ignore_paths)?;

    Ok(())
}

#[cfg(unix)]
fn daemonize() -> Result<()> {
    use directories_next::ProjectDirs;

    let proj_dirs = ProjectDirs::from("", "", "canis")
        .ok_or_else(|| anyhow::anyhow!("Failed to retrieve XDG configuration directory"))?;

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
