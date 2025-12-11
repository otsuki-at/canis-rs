use crate::event::CanonicalEvent;

pub trait Observer: Send + Sync {
    fn update(&self, event: &CanonicalEvent);
}

pub trait Subject {
    fn attach(&mut self, observer: Box<dyn Observer>);
    fn notify(&self, event: &CanonicalEvent);
}
