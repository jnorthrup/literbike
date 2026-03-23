//! Literbike - Unified Protocol River with CCEK SDK
//!
//! Architecture: Protocol tributaries flow through CCEK channels into ENDGAME.
//!
//! ## CCEK SDK Structure
//!
//! ```text
//! Protocol Tributaries (compile-time bound)
//!        │
//!        ▼
//! ┌─────────────────────────────────────┐
//! │     CCEK Context (CcekContext)     │
//! │  Compile-time service bindings       │
//! │                                     │
//! │  [HtxVerifier] → HTX tributary      │
//! │  [QuicEngine] → QUIC tributary      │
//! │  [NioReactor] → NIO tributary      │
//! └─────────────────────────────────────┘
//!        │
//!        ▼
//! ┌─────────────────────────────────────┐
//! │         ENDGAME Densification         │
//! │    (io_uring / syscall bypass)       │
//! └─────────────────────────────────────┘
//! ```

// HTXKE - Kotlin kotlinx-coroutines translation (legacy)
pub mod htxke;

// Stubs for missing modules
pub mod core_types;
pub mod indexed;

// Protocol implementations
#[cfg(feature = "quic")]
pub mod quic;

#[cfg(feature = "htx")]
pub mod htx;

// Unified modules (re-exported from CCEK)
pub mod adapters;
pub mod cas_gateway;
pub mod cas_storage;
pub mod cas_backends;
pub mod channel;
pub mod dht;
pub mod reactor;
pub mod rbcursive;
pub mod modelmux;
pub mod protocol;
pub mod rbcurse;
pub mod uring;
pub mod simd;
pub mod config;
pub mod types;
pub mod model_serving_taxonomy;
pub mod provider_facade_models;
pub mod env_facade_parity;
pub mod http;
pub mod io_substrate;

// CCEK re-exports (code lives in src/ccek/)
pub use ccek_api_translation as api_translation;
pub use ccek_keymux as keymux;

// Userspace kernel emulation (inlined)
#[cfg(feature = "userspace-nio")]
pub mod userspace_nio_module;

#[cfg(feature = "userspace-kernel")]
pub mod userspace_kernel;

#[cfg(feature = "userspace-network")]
pub mod userspace_network;

// Concurrency (CCEK-based, no tokio)
pub mod concurrency;

#[cfg(feature = "git2")]
pub mod git_sync;

#[cfg(feature = "pijul-session")]
pub mod session;

#[cfg(feature = "warp")]
pub mod tethering_bypass;

#[cfg(feature = "quic")]
pub mod posix_sockets;

#[cfg(feature = "quic")]
pub mod host_trust;

#[cfg(feature = "quic")]
pub mod tcp_fingerprint;

#[cfg(feature = "quic")]
pub mod upnp_aggressive;

#[cfg(feature = "quic")]
pub mod radios;

#[cfg(feature = "quic")]
pub mod raw_telnet;

#[cfg(feature = "quic")]
pub mod ssh_tools;

#[cfg(feature = "quic")]
pub mod tls_fingerprint;

#[cfg(feature = "quic")]
pub mod universal_listener;

#[cfg(feature = "quic")]
pub mod packet_fragment;

#[cfg(feature = "quic")]
pub mod protocol_registry;

#[cfg(feature = "quic")]
pub mod traffic_mirror;

#[cfg(all(feature = "quic", feature = "tensor"))]
pub mod wam_engine;

#[cfg(feature = "sctp")]
pub mod sctp;

#[cfg(test)]
mod tests {
    #[test]
    fn htxke_smoke() {
        use crate::htxke::*;
        
        let ctx = CcekContext::new();
        #[cfg(feature = "htx")]
        let ctx = htx_verifier(ctx);
        #[cfg(feature = "quic")]
        let ctx = quic_engine(ctx);
        #[cfg(feature = "userspace-nio")]
        let ctx = nio_reactor(ctx);
        
        let _ = ctx;
    }
}
