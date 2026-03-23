//! QUIC Stream - bidirectional byte streams
//!
//! This module CANNOT see crypto or connection.
//! It only knows about itself and ccek-core.

use ccek_core::{Context, Element, Key};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicU64, Ordering};

/// StreamKey - stream multiplexer
pub struct StreamKey;

impl StreamKey {
    pub const FACTORY: fn() -> StreamElement = || StreamElement::new();
}

impl Key for StreamKey {
    type Element = StreamElement;
    const FACTORY: fn() -> Self::Element = StreamKey::FACTORY;
}

/// StreamElement - manages QUIC streams
pub struct StreamElement {
    pub next_stream_id: AtomicU64,
    pub max_streams: u64,
    pub open_streams: u64,
}

impl StreamElement {
    pub fn new() -> Self {
        Self {
            next_stream_id: AtomicU64::new(0),
            max_streams: 100,
            open_streams: 0,
        }
    }

    /// Open a new bidirectional stream
    pub fn open_stream(&self) -> u64 {
        let id = self.next_stream_id.fetch_add(4, Ordering::Relaxed);
        id
    }

    /// Get current stream count
    pub fn stream_count(&self) -> u64 {
        self.open_streams
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

/// Individual stream state
pub struct Stream {
    pub id: u64,
    pub send_offset: u64,
    pub recv_offset: u64,
    pub send_data: Vec<u8>,
    pub recv_data: Vec<u8>,
    pub fin_sent: bool,
    pub fin_recv: bool,
}

impl Stream {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            send_offset: 0,
            recv_offset: 0,
            send_data: Vec::new(),
            recv_data: Vec::new(),
            fin_sent: false,
            fin_recv: false,
        }
    }

    pub fn write(&mut self, data: &[u8]) {
        self.send_data.extend_from_slice(data);
    }

    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        let len = buf.len().min(self.recv_data.len());
        buf[..len].copy_from_slice(&self.recv_data[..len]);
        self.recv_data.drain(..len);
        len
    }
}

/// Stream state
#[derive(Debug, Clone, Copy)]
pub enum StreamState {
    Idle,
    Open,
    HalfClosedLocal,
    HalfClosedRemote,
    Closed,
    Reset,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_factory() {
        let elem = StreamKey::FACTORY();
        assert_eq!(elem.max_streams, 100);
    }

    #[test]
    fn test_open_stream() {
        let elem = StreamElement::new();
        let id1 = elem.open_stream();
        let id2 = elem.open_stream();
        assert_eq!(id1, 0);
        assert_eq!(id2, 4);
    }
}
