use crate::event::CanonicalEvent;
use crate::observer::{Observer, Subject};

pub struct EventAdapter {
    strategy: Box<dyn ConverterStrategy>,
    observers: Vec<Box<dyn Observer>>,
}

impl EventAdapter {
    pub fn new(processor_level: u8) -> Self {
        let strategy = create_strategy(processor_level);

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

pub trait ConverterStrategy: Send + Sync {
    fn convert(&self, event: CanonicalEvent) -> Option<CanonicalEvent>;
    fn description(&self) -> String;
}

/// L1レベルまでの処理戦略
/// L1以下 → そのまま通す
/// L2以上 → L1にダウングレード
pub struct DownToL1Strategy;

impl ConverterStrategy for DownToL1Strategy {
    fn convert(&self, event: CanonicalEvent) -> Option<CanonicalEvent> {
        match event {
            // L3イベント → L1に変換
            CanonicalEvent::Write { path, time, .. } |
            CanonicalEvent::Append { path, time } => {
                Some(CanonicalEvent::Modify { path, time })
            }
            CanonicalEvent::Open { .. } => None,

            // L2イベント → L1に変換
            CanonicalEvent::Move { dst, time, .. } => {
                Some(CanonicalEvent::Create { path: dst, time })
            }

            // L1イベント → そのまま通す
            CanonicalEvent::Create { .. } |
            CanonicalEvent::Modify { .. } => Some(event),
        }
    }

    fn description(&self) -> String {
        "L1処理部: L2以上のイベントをL1に正規化".to_string()
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
            CanonicalEvent::Write { path, time, .. } |
            CanonicalEvent::Append { path, time } => {
                Some(CanonicalEvent::Modify { path, time })
            }
            CanonicalEvent::Open { .. } => None,

            // L1/L2イベント → そのまま通す
            CanonicalEvent::Create { .. }
            | CanonicalEvent::Modify { .. }
            | CanonicalEvent::Move { .. } => Some(event),
        }
    }

    fn description(&self) -> String {
        "L2処理部: L3以上のイベントをL2に正規化".to_string()
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

    fn description(&self) -> String {
        "L3処理部: すべてのイベントをそのまま処理".to_string()
    }
}

/// 処理部のレベルに応じた変換戦略を生成する
pub fn create_strategy(processor_level: u8) -> Box<dyn ConverterStrategy> {
    match processor_level {
        1 => Box::new(DownToL1Strategy),
        2 => Box::new(DownToL2Strategy),
        3 => Box::new(DownToL3Strategy),
        _ => panic!("未対応の処理部レベル: P{}", processor_level),
    }
}
