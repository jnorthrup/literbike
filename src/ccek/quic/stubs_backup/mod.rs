//! QUIC Assembly - multiplexed transport protocol
//!
//! Hierarchical structure (matches Kotlin CCEK):
//! ```text
//! QuicKey
//!   ├── QuicElement    (base)
//!   ├── CryptoKey       (feature: crypto)
//!   │     └── CryptoElement
//!   ├── StreamKey      (feature: stream)
//!   │     └── StreamElement
//!   └── ConnectionKey (feature: connection)
//!         └── ConnectionElement
//! ```
//!
//! Code reuse via shared ccek-core.

pub mod connection;
pub mod crypto;
pub mod stream;

use ccek_core::{Context, Element, Key};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// QuicKey - QUIC protocol root
pub struct QuicKey;

impl QuicKey {
    pub const VERSION: u32 = 0x00000001;
    pub const MAX_PACKET_SIZE: usize = 1350;
    pub const MIN_PACKET_SIZE: usize = 1200;
}

impl Key for QuicKey {
    type Element = QuicElement;
    const FACTORY: fn() -> Self::Element = || QuicElement::new();
}

/// QuicElement - base QUIC state
pub struct QuicElement {
    pub version: u32,
    pub connections: AtomicU32,
    pub streams: AtomicU32,
    pub packets_sent: AtomicU64,
    pub packets_recv: AtomicU64,
}

impl QuicElement {
    pub fn new() -> Self {
        Self {
            version: QuicKey::VERSION,
            connections: AtomicU32::new(0),
            streams: AtomicU32::new(0),
            packets_sent: AtomicU64::new(0),
            packets_recv: AtomicU64::new(0),
        }
    }

    pub fn connections(&self) -> u32 {
        self.connections.load(Ordering::Relaxed)
    }

    pub fn increment_connections(&self) {
        self.connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn streams(&self) -> u32 {
        self.streams.load(Ordering::Relaxed)
    }

    pub fn increment_streams(&self) {
        self.streams.fetch_add(1, Ordering::Relaxed);
    }
}

impl Element for QuicElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<QuicKey>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// QUIC packet types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuicPacketType {
    Initial,
    Handshake,
    ZeroRTT,
    OneRTT,
}

/// QUIC connection ID
#[derive(Debug, Clone)]
pub struct ConnectionId(pub Vec<u8>);

impl ConnectionId {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quic_key_factory() {
        let elem = QuicKey::FACTORY();
        assert_eq!(elem.version, 0x00000001);
    }

    #[test]
    fn test_quic_context() {
        let ctx = Context::new().plus(QuicKey::FACTORY());
        let elem = ctx.get::<QuicKey>().unwrap();
        let e = elem.as_any().downcast_ref::<QuicElement>().unwrap();
        assert_eq!(e.version, QuicKey::VERSION);
    }
}
