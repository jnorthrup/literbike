//! SCTP Stream - ordered data channels
//!
//! This module CANNOT see association or chunk.

use ccek_core::{Element, Key};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicU64, Ordering};

/// StreamKey - SCTP stream manager
pub struct StreamKey;

impl StreamKey {
    pub const FACTORY: fn() -> StreamElement = || StreamElement::new();
}

impl Key for StreamKey {
    type Element = StreamElement;
    const FACTORY: fn() -> Self::Element = StreamKey::FACTORY;
}

/// StreamElement - manages SCTP streams
pub struct StreamElement {
    pub next_ssn: AtomicU64,
    pub max_streams: u16,
}

impl StreamElement {
    pub fn new() -> Self {
        Self {
            next_ssn: AtomicU64::new(0),
            max_streams: 65535,
        }
    }

    /// Allocate next stream sequence number
    pub fn next_ssn(&self) -> u16 {
        let ssn = self.next_ssn.fetch_add(1, Ordering::Relaxed);
        ssn as u16
    }
}

impl Element for StreamElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<StreamKey>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// SCTP stream
pub struct Stream {
    pub ssn: u16,
    pub send_seq: u32,
    pub recv_seq: u32,
    pub send_buffer: Vec<u8>,
    pub recv_buffer: Vec<u8>,
}

impl Stream {
    pub fn new(ssn: u16) -> Self {
        Self {
            ssn,
            send_seq: 0,
            recv_seq: 0,
            send_buffer: Vec::new(),
            recv_buffer: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_factory() {
        let elem = StreamKey::FACTORY();
        assert_eq!(elem.max_streams, 65535);
    }

    #[test]
    fn test_ssn_allocation() {
        let elem = StreamElement::new();
        assert_eq!(elem.next_ssn(), 0);
        assert_eq!(elem.next_ssn(), 1);
        assert_eq!(elem.next_ssn(), 2);
    }
}
