#[cfg(feature = "quic")]
pub mod quic;

#[cfg(feature = "couchdb")]
pub mod couchdb;

#[cfg(feature = "gates")]
pub mod gates;

#[cfg(feature = "ipfs")]
pub mod ipfs_integration;

#[cfg(feature = "curl-h2")]
pub mod curl_h2;

// Core modules always available
pub mod adapters;
pub mod cas_gateway;
pub mod cas_storage;
pub mod cas_backends;
pub mod channel;
pub mod dht;
pub mod reactor;
pub mod api_translation;
pub mod rbcursive;
pub use rbcursive::precompile::{PRECOMPILED_PATTERNS, PrecompiledPatterns};
pub mod model_mux;

// Keymux - unified model facade
pub mod keymux;

// ModelMux - model caching and selection
pub mod modelmux;

pub mod syscall_net;

// Structured concurrency (Kotlin coroutines pattern)
pub mod concurrency;

// legacy patterns module removed during betanet cleanup
// pub mod betanet_patterns;  // intentionally disabled

#[cfg(feature = "git2")]
pub mod git_sync;

#[cfg(feature = "warp")]
pub mod tethering_bypass;

#[cfg(feature = "warp")]
pub mod knox_proxy;

#[cfg(feature = "quic")]
pub mod posix_sockets;

#[cfg(feature = "quic")]
pub mod host_trust;

#[cfg(feature = "quic")]
pub mod tcp_fingerprint;

#[cfg(feature = "quic")]
pub mod upnp_aggressive;

pub mod config;
pub mod types;
pub mod model_serving_taxonomy;
pub mod provider_facade_models;
pub mod env_facade_parity;

pub mod http;

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

// WAM engine requires tensor feature
#[cfg(all(feature = "quic", feature = "tensor"))]
pub mod wam_engine;

// SCTP protocol support (KMPngSCTP integration)
#[cfg(feature = "sctp")]
pub mod sctp;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        #[cfg(feature = "quic")]
        let _ = quic::QuicServer::bind;
    }
}
