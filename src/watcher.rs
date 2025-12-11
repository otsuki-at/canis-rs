use crate::error::{Result, WatcherError};
use crate::event::CanonicalEvent;
use crate::observer::{Observer, Subject};
use crate::strategy::watcher::{WatcherStrategy, WatcherCapabilities};

pub struct FileWatcher {
    strategy: Box<dyn WatcherStrategy>,
    observers: Vec<Box<dyn Observer>>,
}

impl FileWatcher {
    pub fn new(system_name: &str) -> Result<Self> {
        let strategy = crate::strategy::watcher::create_watcher_strategy(system_name)?;
        let capabilities = strategy.capabilities();

        println!("監視システム: {})",
                 system_name);
        println!("複数パス対応: {}",
                 if capabilities.supports_multiple_paths { "はい" } else { "いいえ" });
        println!("監視戦略: {}\n", strategy.description());

        Ok(Self {
            strategy,
            observers: Vec::new(),
        })
    }

    pub fn capabilities(&self) -> WatcherCapabilities {
        self.strategy.capabilities()
    }

    pub fn start_watching(mut self, paths: &[String]) -> Result<()> {
        let capabilities = self.strategy.capabilities();

        // 複数パス対応チェック
        if paths.len() > 1 && !capabilities.supports_multiple_paths {
            return Err(WatcherError::ConfigError(
                format!("{} は複数パスの監視をサポートしていません。指定されたパス数: {}",
                    capabilities.system_name, paths.len())
            ));
        }

        let observers = std::sync::Arc::new(self.observers);

        self.strategy.start_watching(paths, Box::new(move |event| {
            for observer in observers.iter() {
                observer.update(&event);
            }
        }))
    }
}

impl Subject for FileWatcher {
    fn attach(&mut self, observer: Box<dyn Observer>) {
        self.observers.push(observer);
    }

    fn notify(&self, event: &CanonicalEvent) {
        for observer in &self.observers {
            observer.update(event);
        }
    }
}
