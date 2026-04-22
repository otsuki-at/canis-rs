use crate::event:: FileEvent;

pub trait Observer: Send + Sync {
    fn update(&self, event: &FileEvent);
}

pub trait Subject {
    fn attach(&mut self, observer: Box<dyn Observer>);
    fn notify(&self, event: &FileEvent);
}
