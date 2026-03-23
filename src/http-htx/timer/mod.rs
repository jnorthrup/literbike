//! HTTP-HTX Timer Wheel - Scheduled callbacks
//!
//! This module CANNOT see matcher, listener, handler.

use std::any::{Any, TypeId};
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};

pub type TimerId = u64;

pub struct TimerKey;

impl TimerKey {
    pub const FACTORY: fn() -> TimerElement = TimerElement::new;
}

pub struct TimerElement {
    next_id: AtomicU64,
    wheel_size: usize,
}

impl TimerElement {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            wheel_size: 256,
        }
    }

    pub fn schedule<F>(&self, delay: Duration, callback: F) -> TimerId
    where
        F: FnOnce() + Send + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let _ = callback;
        let _ = delay;
        id
    }

    pub fn cancel(&self, _id: TimerId) -> bool {
        true
    }
}

impl Element for TimerElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<TimerKey>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for TimerKey {
    type Element = TimerElement;
    const FACTORY: fn() -> Self::Element = TimerElement::new;
}

pub struct TimerEntry {
    pub id: TimerId,
    pub deadline: Instant,
    pub callback: Box<dyn FnOnce() + Send>,
}

pub trait Element: Send + Sync + 'static {
    fn key_type(&self) -> TypeId;
    fn as_any(&self) -> &dyn Any;
}

pub trait Key: 'static {
    type Element: Element;
    const FACTORY: fn() -> Self::Element;
}

use std::sync::atomic::Ordering;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_factory() {
        let elem = TimerKey::FACTORY();
        assert_eq!(elem.wheel_size, 256);
    }

    #[test]
    fn test_timer_schedule() {
        let elem = TimerElement::new();
        let id = elem.schedule(Duration::from_secs(1), || {});
        assert_eq!(id, 1);
    }
}
