// Hybrid QUIC engine — combines standard and feature-gated crypto paths
// Exports: QuicEngineHybrid, QuicState, QuicStats

use super::quic_crypto::QuicCryptoProvider;
use super::quic_engine::{QuicEngine, Role};
use super::quic_protocol::{ConnectionState, QuicConnectionState};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::UdpSocket;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuicState {
    Initial,
    Handshaking,
    Connected,
    Closed,
}

impl From<ConnectionState> for QuicState {
    fn from(cs: ConnectionState) -> Self {
        match cs {
            ConnectionState::Handshaking => QuicState::Handshaking,
            ConnectionState::Connected   => QuicState::Connected,
            ConnectionState::Closed      => QuicState::Closed,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct QuicStats {
    pub packets_sent:     u64,
    pub packets_received: u64,
    pub bytes_sent:       u64,
    pub bytes_received:   u64,
    pub streams_opened:   u64,
    pub streams_closed:   u64,
}

/// Wraps QuicEngine with stats tracking and optional feature-gated crypto
pub struct QuicEngineHybrid {
    engine: Arc<QuicEngine>,
    packets_sent:     Arc<AtomicU64>,
    packets_received: Arc<AtomicU64>,
    bytes_sent:       Arc<AtomicU64>,
    bytes_received:   Arc<AtomicU64>,
    streams_opened:   Arc<AtomicU64>,
    streams_closed:   Arc<AtomicU64>,
    crypto_enabled:   bool,
}

impl QuicEngineHybrid {
    pub fn new(
        role: Role,
        state: QuicConnectionState,
        socket: Arc<UdpSocket>,
        remote_addr: SocketAddr,
        private_key: Vec<u8>,
    ) -> Self {
        Self {
            engine: Arc::new(QuicEngine::new(role, state, socket, remote_addr, private_key)),
            packets_sent:     Arc::new(AtomicU64::new(0)),
            packets_received: Arc::new(AtomicU64::new(0)),
            bytes_sent:       Arc::new(AtomicU64::new(0)),
            bytes_received:   Arc::new(AtomicU64::new(0)),
            streams_opened:   Arc::new(AtomicU64::new(0)),
            streams_closed:   Arc::new(AtomicU64::new(0)),
            crypto_enabled:   false,
        }
    }

    pub fn new_with_crypto(
        role: Role,
        state: QuicConnectionState,
        socket: Arc<UdpSocket>,
        remote_addr: SocketAddr,
        private_key: Vec<u8>,
        crypto_provider: Arc<dyn QuicCryptoProvider>,
    ) -> Self {
        Self {
            engine: Arc::new(QuicEngine::new_with_crypto_provider(
                role, state, socket, remote_addr, private_key, crypto_provider,
            )),
            packets_sent:     Arc::new(AtomicU64::new(0)),
            packets_received: Arc::new(AtomicU64::new(0)),
            bytes_sent:       Arc::new(AtomicU64::new(0)),
            bytes_received:   Arc::new(AtomicU64::new(0)),
            streams_opened:   Arc::new(AtomicU64::new(0)),
            streams_closed:   Arc::new(AtomicU64::new(0)),
            crypto_enabled:   cfg!(feature = "quic-crypto"),
        }
    }

    pub fn engine(&self) -> Arc<QuicEngine> {
        self.engine.clone()
    }

    pub fn crypto_enabled(&self) -> bool {
        self.crypto_enabled
    }

    pub fn record_sent(&self, bytes: u64) {
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_received(&self, bytes: u64) {
        self.packets_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_stream_opened(&self) { self.streams_opened.fetch_add(1, Ordering::Relaxed); }
    pub fn record_stream_closed(&self) { self.streams_closed.fetch_add(1, Ordering::Relaxed); }

    pub fn stats(&self) -> QuicStats {
        QuicStats {
            packets_sent:     self.packets_sent.load(Ordering::Relaxed),
            packets_received: self.packets_received.load(Ordering::Relaxed),
            bytes_sent:       self.bytes_sent.load(Ordering::Relaxed),
            bytes_received:   self.bytes_received.load(Ordering::Relaxed),
            streams_opened:   self.streams_opened.load(Ordering::Relaxed),
            streams_closed:   self.streams_closed.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quic_state_from_connection_state() {
        assert_eq!(QuicState::from(ConnectionState::Connected),   QuicState::Connected);
        assert_eq!(QuicState::from(ConnectionState::Handshaking), QuicState::Handshaking);
        assert_eq!(QuicState::from(ConnectionState::Closed),      QuicState::Closed);
    }

    #[test]
    fn stats_accumulate() {
        let stats = QuicStats { packets_sent: 5, bytes_sent: 1024, ..Default::default() };
        assert_eq!(stats.packets_sent, 5);
        assert_eq!(stats.bytes_sent, 1024);
    }
}
