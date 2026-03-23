//! HTTP-HTX Handler - Protocol handlers
//!
//! This module CANNOT see matcher, listener, reactor, timer.

use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct HandlerKey;

impl HandlerKey {
    pub const FACTORY: fn() -> HandlerElement = HandlerElement::new;
}

pub struct HandlerElement {
    http_count: AtomicU64,
    http2_count: AtomicU64,
    http3_count: AtomicU64,
    unknown_count: AtomicU64,
}

impl HandlerElement {
    pub fn new() -> Self {
        Self {
            http_count: AtomicU64::new(0),
            http2_count: AtomicU64::new(0),
            http3_count: AtomicU64::new(0),
            unknown_count: AtomicU64::new(0),
        }
    }

    pub fn handle_http1(&self) {
        self.http_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn handle_http2(&self) {
        self.http2_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn handle_http3(&self) {
        self.http3_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn handle_unknown(&self) {
        self.unknown_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn stats(&self) -> HandlerStats {
        HandlerStats {
            http1: self.http_count.load(Ordering::Relaxed),
            http2: self.http2_count.load(Ordering::Relaxed),
            http3: self.http3_count.load(Ordering::Relaxed),
            unknown: self.unknown_count.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HandlerStats {
    pub http1: u64,
    pub http2: u64,
    pub http3: u64,
    pub unknown: u64,
}

impl HandlerStats {
    pub fn total(&self) -> u64 {
        self.http1 + self.http2 + self.http3 + self.unknown
    }
}

impl Element for HandlerElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<HandlerKey>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for HandlerKey {
    type Element = HandlerElement;
    const FACTORY: fn() -> Self::Element = HandlerElement::new;
}

pub trait ProtocolHandler: Send + Sync {
    fn protocol(&self) -> &'static str;
    fn handle(&self, data: &[u8]) -> HandlerResult;
}

#[derive(Debug)]
pub enum HandlerResult {
    Handled(usize),
    NeedMoreData,
    Error(&'static str),
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
    fn test_handler_factory() {
        let elem = HandlerKey::FACTORY();
        let stats = elem.stats();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn test_handler_dispatch() {
        let elem = HandlerElement::new();
        elem.handle_http1();
        elem.handle_http2();
        elem.handle_http3();

        let stats = elem.stats();
        assert_eq!(stats.http1, 1);
        assert_eq!(stats.http2, 1);
        assert_eq!(stats.http3, 1);
        assert_eq!(stats.total(), 3);
    }
}
