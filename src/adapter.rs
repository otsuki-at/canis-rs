use crate::event::CanonicalEvent;
use crate::observer::{Observer, Subject};
use crate::strategy::conversion::ConversionStrategy;

pub struct EventAdapter {
    strategy: Box<dyn ConversionStrategy>,
    observers: Vec<Box<dyn Observer>>,
}

impl EventAdapter {
    pub fn new(processor_level: u8) -> Self {
        let strategy = crate::strategy::conversion::create_strategy(processor_level);

        println!("変換戦略: {}\n", strategy.description());

        Self {
            strategy,
            observers: Vec::new(),
        }
    }
}

impl Observer for EventAdapter {
    fn update(&self, event: &CanonicalEvent) {
        if let Some(converted) = self.strategy.convert(event.clone()) {
            self.notify(&converted);
        }
    }
}

impl Subject for EventAdapter {
    fn attach(&mut self, observer: Box<dyn Observer>) {
        self.observers.push(observer);
    }

    fn notify(&self, event: &CanonicalEvent) {
        for observer in &self.observers {
            observer.update(event);
        }
    }
}
