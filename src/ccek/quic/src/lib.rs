//! ccek-quic — Real QUIC implementation for the CCEK subsystem.
//!
//! This crate contains the full QUIC stack relocated from `src/quic/`.
//! External dependencies on the main literbike crate are stubbed in `compat`
//! when compiled standalone, and can be wired to real implementations via
//! feature flags.

#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unreachable_code)]
#![allow(clippy::needless_return)]

// Compatibility layer for types from the main literbike crate
pub mod compat;

// Core QUIC modules (no external deps beyond std + common crates)
pub mod quic_ccek;
pub mod quic_ccek_types;
pub mod quic_config;
pub mod quic_crypto;
pub mod quic_engine;
pub mod quic_error;
pub mod quic_failure_log;
pub mod quic_protocol;
pub mod quic_request_factory;
pub mod quic_server;
pub mod quic_session_cache;
pub mod quic_stream;

// Engine variants — quic_engine_full has deep literbike deps, gate it
#[cfg(feature = "literbike-full")]
pub mod quic_engine_full;

// Hybrid engine uses cas_storage
pub mod quic_engine_hybrid;

// HTX protocol modules — these depend heavily on uring/wam/htx from the main crate
// Gate behind literbike-full until the compat stubs are complete
#[cfg(feature = "literbike-full")]
pub mod wam;
#[cfg(feature = "literbike-full")]
pub mod bedrock;
#[cfg(feature = "literbike-full")]
pub mod engine;
#[cfg(feature = "literbike-full")]
pub mod congruence;

// TLS module (requires tls-quic feature)
#[cfg(feature = "tls-quic")]
pub mod tls;
#[cfg(feature = "tls-quic")]
pub mod tls_ccek;
#[cfg(feature = "tls-quic")]
pub mod tls_crypto;

// WAM module requires tensor feature
#[cfg(feature = "tensor")]
pub mod quic_wam;

// Re-exports (matching what src/quic/mod.rs exported)
#[cfg(feature = "quic-crypto")]
pub use quic_crypto::FeatureGatedCryptoProvider;
pub use quic_crypto::{HandshakePhase, NoopQuicCryptoProvider, QuicCryptoProvider};
pub use quic_engine::{QuicEngine, QuicEngineDiagnosticsSnapshot};
pub use quic_engine_hybrid::{QuicEngineHybrid, QuicState, QuicStats};
pub use quic_error::QuicError;
pub use quic_protocol::{
    AckFrame, ConnectionId, ConnectionState, CryptoFrame, QuicConnectionState, QuicFrame,
    QuicHeader, QuicPacket, QuicPacketType, QuicProtocol, QuicStreamState, StreamFrame,
    StreamState, TransportParameters,
};
pub use quic_engine::Role;
pub use quic_stream::QuicStream;
pub use quic_server::QuicServer;

// TLS exports (feature-gated)
#[cfg(feature = "tls-quic")]
pub use tls::TlsTerminator;
