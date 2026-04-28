use crate::event::{CanonicalEvent, FileEvent};
use crate::observer::{Observer, Subject};

pub struct EventAdapter {
    strategy: Box<dyn ConverterStrategy>,
    observers: Vec<Box<dyn Observer>>,
}

impl EventAdapter {
    pub fn new(processor_level: u8) -> Self {
        let strategy = create_strategy(processor_level);

        Self {
            strategy,
            observers: Vec::new(),
        }
    }
}

impl Observer for EventAdapter {
    fn update(&self, file_event: &FileEvent) {
        if let Some(converted) = self.strategy
            .convert(file_event.event.clone())
            .map(|event| FileEvent {
                event,
                process_info: file_event.process_info.clone(),
            })
        {
            self.notify(&converted);
        }
    }
}

impl Subject for EventAdapter {
    fn attach(&mut self, observer: Box<dyn Observer>) {
        self.observers.push(observer);
    }

    fn notify(&self, event: &FileEvent) {
        for observer in &self.observers {
            observer.update(event);
        }
    }
}

pub trait ConverterStrategy: Send + Sync {
    fn convert(&self, event: CanonicalEvent) -> Option<CanonicalEvent>;
}

/// L1レベルまでの処理戦略
/// L1以下 → そのまま通す
/// L2以上 → L1にダウングレード
pub struct DownToL1Strategy;

impl ConverterStrategy for DownToL1Strategy {
    fn convert(&self, event: CanonicalEvent) -> Option<CanonicalEvent> {
        match event {
            // L3イベント → L1に変換
            CanonicalEvent::Write { uri, time, .. } |
            CanonicalEvent::Append { uri, time } => {
                Some(CanonicalEvent::Modify { uri, time })
            }
            CanonicalEvent::Open { .. } => None,

            // L2イベント → L1に変換
            CanonicalEvent::Move { dst, time, .. } => {
                Some(CanonicalEvent::Create { uri: dst, time })
            }

            // L1イベント → そのまま通す
            CanonicalEvent::Create { .. } |
            CanonicalEvent::Modify { .. } => Some(event),
        }
    }
}

/// L2レベルまでの処理戦略
/// L2以下 → そのまま通す
/// L3以上 → L2にダウングレード
pub struct DownToL2Strategy;

impl ConverterStrategy for DownToL2Strategy {
    fn convert(&self, event: CanonicalEvent) -> Option<CanonicalEvent> {
        match event {
            // L3イベント → L2に変換
            CanonicalEvent::Write { uri, time, .. } |
            CanonicalEvent::Append { uri, time } => {
                Some(CanonicalEvent::Modify { uri, time })
            }
            CanonicalEvent::Open { .. } => None,

            // L1/L2イベント → そのまま通す
            CanonicalEvent::Create { .. }
            | CanonicalEvent::Modify { .. }
            | CanonicalEvent::Move { .. } => Some(event),
        }
    }
}

/// L3レベルまでの処理戦略
/// すべてのイベント → そのまま通す
pub struct DownToL3Strategy;

impl ConverterStrategy for DownToL3Strategy {
    fn convert(&self, event: CanonicalEvent) -> Option<CanonicalEvent> {
        // L3処理部はすべてのレベルをそのまま処理
        Some(event)
    }
}

/// 処理部のレベルに応じた変換戦略を生成する
pub fn create_strategy(processor_level: u8) -> Box<dyn ConverterStrategy> {
    match processor_level {
        1 => Box::new(DownToL1Strategy),
        2 => Box::new(DownToL2Strategy),
        3 => Box::new(DownToL3Strategy),
        _ => panic!("Unsupported processor level: P{}", processor_level),
    }
}
