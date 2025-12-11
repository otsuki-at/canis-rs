use std::sync::Arc;
use crate::event::CanonicalEvent;
use crate::logger::Logger;
use crate::observer::Observer;
use crate::strategy::processor::ProcessorStrategy;

/// ProcessorObserver: ProcessorStrategyをObserverインターフェースでラップ
pub struct ProcessorObserver {
    strategy: Box<dyn ProcessorStrategy>,
}

impl ProcessorObserver {
    /// 処理レベルから戦略を選択してProcessorObserverを作成
    pub fn new(level: u8, logger: Arc<dyn Logger>) -> Self {
        let strategy = crate::strategy::processor::create_strategy(level, logger);

        println!("処理戦略: {}\n", strategy.description());

        Self { strategy }
    }
}

impl Observer for ProcessorObserver {
    fn update(&self, event: &CanonicalEvent) {
        self.strategy.process(event);
    }
}

