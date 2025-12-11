use notify::{Watcher, RecursiveMode, Event, EventKind, RecommendedWatcher};
use notify::event::ModifyKind;
use std::sync::mpsc::channel;
use chrono::{DateTime, Utc};
use crate::event::CanonicalEvent;
use crate::error::{Result, WatcherError};

#[derive(Debug, Clone)]
pub struct WatcherCapabilities {
    pub supports_multiple_paths: bool,
    pub system_name: String,
}

impl WatcherCapabilities {
    pub fn for_system(system_name: &str) -> Result<Self> {
        match system_name {
            "notify" => Ok(Self {
                supports_multiple_paths: true,
                system_name: "notify".to_string(),
            }),
            "fuse" => Ok(Self {
                supports_multiple_paths: false,
                system_name: "fuse".to_string(),
            }),
            _ => Err(WatcherError::UnsupportedSystem(
                format!("未対応の監視システム: {}", system_name)
            )),
        }
    }
}

pub trait WatcherStrategy: Send {
    fn start_watching(
        &mut self,
        paths: &[String],
        callback: Box<dyn Fn(CanonicalEvent) + Send>
    ) -> Result<()>;

    fn description(&self) -> String;
    fn capabilities(&self) -> WatcherCapabilities;
}

// Level 1: notify を使った標準監視
pub struct NotifyWatcherStrategy {
    capabilities: WatcherCapabilities,
}

impl NotifyWatcherStrategy {
    pub fn new() -> Self {
        Self {
            capabilities: WatcherCapabilities::for_system("notify").unwrap(),
        }
    }

    fn convert_to_canonical(&self, event: Event, timestamp: DateTime<Utc>) -> Vec<CanonicalEvent> {
        let mut events = Vec::new();
        let time = timestamp.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();

        match event.kind {
            EventKind::Create(_) => {
                for path in event.paths {
                    events.push(CanonicalEvent::Create {
                        path,
                        time: time.clone()
                    });
                }
            }
            EventKind::Modify(ModifyKind::Data(_)) => {
                for path in event.paths {
                    events.push(CanonicalEvent::Modify {
                        path,
                        time: time.clone()
                    });
                }
            }
            _ => {}
        }

        events
    }
}

impl WatcherStrategy for NotifyWatcherStrategy {
    fn start_watching(
        &mut self,
        paths: &[String],
        callback: Box<dyn Fn(CanonicalEvent) + Send>
    ) -> Result<()> {
        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let now = Utc::now();
                tx.send((res, now)).unwrap();
            },
            notify::Config::default(),
        )?;

        for path in paths {
            watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;
            println!("監視開始: {}", path);
        }

        println!("Ctrl+C で終了\n");

        for (res, time) in rx {
            match res {
                Ok(event) => {
                    let canonical_events = self.convert_to_canonical(event, time);
                    for canonical_event in canonical_events {
                        callback(canonical_event);
                    }
                }
                Err(e) => eprintln!("エラー: {:?}", e),
            }
        }

        Ok(())
    }

    fn description(&self) -> String {
        "notify: 標準ファイルシステム監視".to_string()
    }

    fn capabilities(&self) -> WatcherCapabilities {
        self.capabilities.clone()
    }
}

// Level 3: FUSE を使った詳細監視
pub struct FuseWatcherStrategy {
    capabilities: WatcherCapabilities,
}

impl FuseWatcherStrategy {
    pub fn new() -> Self {
        Self {
            capabilities: WatcherCapabilities::for_system("fuse").unwrap(),
        }
    }
}

impl WatcherStrategy for FuseWatcherStrategy {
    fn start_watching(
        &mut self,
        paths: &[String],
        _callback: Box<dyn Fn(CanonicalEvent) + Send>
    ) -> Result<()> {
        eprintln!("❌ エラー: FUSE監視(Level 3)は未実装です");
        eprintln!("   config.toml の watcher_system を \"notify\" に変更してください");

        if paths.len() > 1 {
            eprintln!("⚠️  注意: FUSEは複数パスの監視をサポートしていません");
            eprintln!("   指定されたパス: {:?}", paths);
        }

        Err(WatcherError::UnsupportedSystem("FUSE監視は未実装".to_string()))
    }

    fn description(&self) -> String {
        "FUSE: 詳細なファイルシステム監視(未実装)".to_string()
    }

    fn capabilities(&self) -> WatcherCapabilities {
        self.capabilities.clone()
    }
}

pub fn create_watcher_strategy(system_name: &str) -> Result<Box<dyn WatcherStrategy>> {
    match system_name {
        "notify" => Ok(Box::new(NotifyWatcherStrategy::new())),
        "fuse" => Ok(Box::new(FuseWatcherStrategy::new())),
        _ => Err(WatcherError::UnsupportedSystem(
            format!("未対応の監視システム: {}", system_name)
        )),
    }
}
