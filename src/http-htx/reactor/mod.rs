//! HTTP-HTX Reactor - event loop
//!
//! This module CANNOT see matcher, timer, handler.

use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub struct ReactorKey;

impl ReactorKey {
    pub const FACTORY: fn() -> ReactorElement = ReactorElement::new;
    pub const DEFAULT_TIMEOUT_MS: u64 = 100;
}

pub struct ReactorElement {
    pub running: AtomicBool,
    pub select_calls: AtomicU64,
    pub events_dispatched: AtomicU64,
    pub timeout_ms: u64,
}

impl ReactorElement {
    pub fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            select_calls: AtomicU64::new(0),
            events_dispatched: AtomicU64::new(0),
            timeout_ms: ReactorKey::DEFAULT_TIMEOUT_MS,
        }
    }

    pub fn start(&self) {
        self.running.store(true, Ordering::Relaxed);
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn tick(&self) {
        self.select_calls.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dispatch(&self) {
        self.events_dispatched.fetch_add(1, Ordering::Relaxed);
    }

    pub fn select_calls(&self) -> u64 {
        self.select_calls.load(Ordering::Relaxed)
    }
}

impl Element for ReactorElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<ReactorKey>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for ReactorKey {
    type Element = ReactorElement;
    const FACTORY: fn() -> Self::Element = ReactorElement::new;
}

#[derive(Debug, Clone, Copy)]
pub struct ReadyEvent {
    pub fd: i32,
    pub readable: bool,
    pub writable: bool,
    pub error: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct InterestSet {
    pub read: bool,
    pub write: bool,
    pub error: bool,
}

impl InterestSet {
    pub fn read() -> Self {
        Self {
            read: true,
            ..Default::default()
        }
    }

    pub fn write() -> Self {
        Self {
            write: true,
            ..Default::default()
        }
    }

    pub fn read_write() -> Self {
        Self {
            read: true,
            write: true,
            ..Default::default()
        }
    }
}

pub trait Element: Send + Sync + 'static {
    fn key_type(&self) -> TypeId;
    fn as_any(&self) -> &dyn Any;
}

pub trait Key: 'static {
    type Element: Element;
    const FACTORY: fn() -> Self::Element;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reactor_factory() {
        let elem = ReactorKey::FACTORY();
        assert!(!elem.is_running());
        assert_eq!(elem.select_calls(), 0);
    }

    #[test]
    fn test_reactor_lifecycle() {
        let elem = ReactorElement::new();
        elem.start();
        assert!(elem.is_running());
        elem.tick();
        elem.dispatch();
        assert_eq!(elem.select_calls(), 1);
        elem.stop();
        assert!(!elem.is_running());
    }
}
