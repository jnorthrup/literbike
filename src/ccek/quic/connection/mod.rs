//! QUIC Connection - connection state machine
//!
//! This module CANNOT see crypto or stream.
//! It only knows about itself and ccek-core.

use ccek_core::{Context, Element, Key};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicU32, Ordering};

/// ConnectionKey - QUIC connection state machine
pub struct ConnectionKey;

impl ConnectionKey {
    pub const FACTORY: fn() -> ConnectionElement = || ConnectionElement::new();
}

impl Key for ConnectionKey {
    type Element = ConnectionElement;
    const FACTORY: fn() -> Self::Element = ConnectionKey::FACTORY;
}

/// ConnectionElement - connection state
pub struct ConnectionElement {
    pub state: AtomicU32,
    pub packet_num: AtomicU32,
}

impl ConnectionElement {
    pub fn new() -> Self {
        Self {
            state: AtomicU32::new(ConnectionState::Initial as u32),
            packet_num: AtomicU32::new(0),
        }
    }

    pub fn state(&self) -> ConnectionState {
        let s = self.state.load(Ordering::Relaxed);
        ConnectionState::from_u32(s)
    }

    pub fn set_state(&self, state: ConnectionState) {
        self.state.store(state as u32, Ordering::Relaxed);
    }

    pub fn next_packet_num(&self) -> u32 {
        self.packet_num.fetch_add(1, Ordering::Relaxed)
    }
}

impl Element for ConnectionElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<ConnectionKey>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// QUIC connection states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Initial,
    Handshake,
    Established,
    Closing,
    Draining,
    Closed,
}

impl ConnectionState {
    pub fn from_u32(v: u32) -> Self {
        match v {
            0 => ConnectionState::Initial,
            1 => ConnectionState::Handshake,
            2 => ConnectionState::Established,
            3 => ConnectionState::Closing,
            4 => ConnectionState::Draining,
            5 => ConnectionState::Closed,
            _ => ConnectionState::Closed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_factory() {
        let elem = ConnectionKey::FACTORY();
        assert_eq!(elem.state(), ConnectionState::Initial);
    }

    #[test]
    fn test_connection_state_transitions() {
        let elem = ConnectionElement::new();

        elem.set_state(ConnectionState::Handshake);
        assert_eq!(elem.state(), ConnectionState::Handshake);

        elem.set_state(ConnectionState::Established);
        assert_eq!(elem.state(), ConnectionState::Established);

        elem.set_state(ConnectionState::Closing);
        assert_eq!(elem.state(), ConnectionState::Closing);
    }

    #[test]
    fn test_packet_numbering() {
        let elem = ConnectionElement::new();
        assert_eq!(elem.next_packet_num(), 0);
        assert_eq!(elem.next_packet_num(), 1);
        assert_eq!(elem.next_packet_num(), 2);
    }
}
