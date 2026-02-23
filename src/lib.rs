#[cfg(feature = "quic")]
pub mod quic;

#[cfg(feature = "couchdb")]
pub mod couchdb;

#[cfg(feature = "gates")]
pub mod gates;

#[cfg(feature = "ipfs")]
pub mod ipfs_integration;

// Core modules always available
pub mod adapters;
pub mod channel;
pub mod reactor;
pub mod rbcursive;
pub mod syscall_net;

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

#[cfg(feature = "tensor")]
pub mod wam_engine;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        #[cfg(feature = "quic")]
        let _ = quic::QuicServer::bind;
    }
}
