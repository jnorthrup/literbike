//! Protocol Handlers - Per-protocol connection handlers
//!
//! This module CANNOT see matcher, listener, reactor, timer.
//! It only knows about itself, protocol types, and the core traits.

use crate::core::{Element, Key};
use crate::protocol::{HttpMethod, ProtocolDetection};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicU64, Ordering};

/// HandlerKey - manages protocol-specific handlers
pub struct HandlerKey;

impl HandlerKey {
    pub const FACTORY: fn() -> HandlerElement = HandlerElement::new;
}

/// HandlerElement - protocol handler registry
pub struct HandlerElement {
    http_count: AtomicU64,
    socks5_count: AtomicU64,
    websocket_count: AtomicU64,
    upnp_count: AtomicU64,
    unknown_count: AtomicU64,
}

impl HandlerElement {
    pub fn new() -> Self {
        Self {
            http_count: AtomicU64::new(0),
            socks5_count: AtomicU64::new(0),
            websocket_count: AtomicU64::new(0),
            upnp_count: AtomicU64::new(0),
            unknown_count: AtomicU64::new(0),
        }
    }

    pub fn handle(&self, protocol: &ProtocolDetection) {
        match protocol {
            ProtocolDetection::Http(_) => self.http(),
            ProtocolDetection::Socks5 => self.socks5(),
            ProtocolDetection::WebSocket => self.websocket(),
            ProtocolDetection::Upnp => self.upnp(),
            _ => self.unknown(),
        }
    }

    fn http(&self) {
        self.http_count.fetch_add(1, Ordering::Relaxed);
    }

    fn socks5(&self) {
        self.socks5_count.fetch_add(1, Ordering::Relaxed);
    }

    fn websocket(&self) {
        self.websocket_count.fetch_add(1, Ordering::Relaxed);
    }

    fn upnp(&self) {
        self.upnp_count.fetch_add(1, Ordering::Relaxed);
    }

    fn unknown(&self) {
        self.unknown_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn stats(&self) -> HandlerStats {
        HandlerStats {
            http: self.http_count.load(Ordering::Relaxed),
            socks5: self.socks5_count.load(Ordering::Relaxed),
            websocket: self.websocket_count.load(Ordering::Relaxed),
            upnp: self.upnp_count.load(Ordering::Relaxed),
            unknown: self.unknown_count.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HandlerStats {
    pub http: u64,
    pub socks5: u64,
    pub websocket: u64,
    pub upnp: u64,
    pub unknown: u64,
}

impl HandlerStats {
    pub fn total(&self) -> u64 {
        self.http + self.socks5 + self.websocket + self.upnp + self.unknown
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

/// Protocol handler trait
pub trait ProtocolHandler: Send + Sync {
    fn protocol(&self) -> ProtocolDetection;
    fn handle(&self, data: &[u8]) -> HandlerResult;
}

#[derive(Debug)]
pub enum HandlerResult {
    Handled(usize),
    NeedMoreData,
    Error(&'static str),
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

        elem.handle(&ProtocolDetection::Http(HttpMethod::Get));
        elem.handle(&ProtocolDetection::Socks5);
        elem.handle(&ProtocolDetection::WebSocket);

        let stats = elem.stats();
        assert_eq!(stats.http, 1);
        assert_eq!(stats.socks5, 1);
        assert_eq!(stats.websocket, 1);
        assert_eq!(stats.total(), 3);
    }
}
