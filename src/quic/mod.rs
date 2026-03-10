pub mod quic_ccek;
#[cfg(feature = "tls-quic")]
pub mod tls_ccek;
pub mod quic_ccek_types;
pub mod quic_config;
pub mod quic_crypto;
pub mod quic_engine;
pub mod quic_engine_hybrid;
pub mod quic_error;
pub mod quic_failure_log;
pub mod quic_protocol;
pub mod quic_request_factory;
pub mod quic_server;
pub mod quic_session_cache;
pub mod quic_stream;

// TLS module (requires tls-quic feature)
#[cfg(feature = "tls-quic")]
pub mod tls;
#[cfg(feature = "tls-quic")]
pub mod tls_crypto;

// WAM module requires tensor feature
#[cfg(feature = "tensor")]
pub mod quic_wam;

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
