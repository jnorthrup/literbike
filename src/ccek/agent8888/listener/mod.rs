//! Agent8888 Listener - TCP socket listener
//!
//! This module CANNOT see matcher, reactor, timer, handler.
//! It only knows about itself and the core traits.

use crate::core::{Element, Key};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicU32, Ordering};

/// ListenerKey - manages TCP listener socket
pub struct ListenerKey {
    pub bind_addr: String,
}

impl ListenerKey {
    pub const FACTORY: fn() -> ListenerElement = || ListenerElement::new("0.0.0.0:8888");

    pub fn new(bind_addr: &str) -> Self {
        Self {
            bind_addr: bind_addr.to_string(),
        }
    }
}

/// ListenerElement - TCP listener state
pub struct ListenerElement {
    pub bind_addr: String,
    pub fd: i32,
    pub accepted_connections: AtomicU32,
    // Backlog size
    pub backlog: u32,
}

impl ListenerElement {
    pub fn new(bind_addr: &str) -> Self {
        Self {
            bind_addr: bind_addr.to_string(),
            fd: -1,
            accepted_connections: AtomicU32::new(0),
            backlog: 128,
        }
    }

    pub fn increment_accepted(&self) {
        self.accepted_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn accepted(&self) -> u32 {
        self.accepted_connections.load(Ordering::Relaxed)
    }
}

impl Element for ListenerElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<ListenerKey>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for ListenerKey {
    type Element = ListenerElement;
    const FACTORY: fn() -> Self::Element = || ListenerElement::new("0.0.0.0:8888");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_listener_factory() {
        let elem = ListenerKey::FACTORY();
        assert_eq!(elem.bind_addr, "0.0.0.0:8888");
        assert_eq!(elem.fd, -1);
        assert_eq!(elem.backlog, 128);
    }

    #[test]
    fn test_listener_accept_count() {
        let elem = ListenerElement::new("127.0.0.1:9999");
        elem.increment_accepted();
        elem.increment_accepted();
        assert_eq!(elem.accepted(), 2);
    }
}
