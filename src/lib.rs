pub mod adapters;
pub mod channel;
pub mod quic;
pub mod reactor;
pub mod rbcursive;
pub mod syscall_net;
pub mod git_sync;
pub mod tethering_bypass;
pub mod knox_proxy;
pub mod posix_sockets;
pub mod host_trust;
pub mod tcp_fingerprint;
pub mod upnp_aggressive;
pub mod config;
pub mod types;
pub mod gates;
pub mod radios;
pub mod raw_telnet;
pub mod ssh_tools;
pub mod tls_fingerprint;
pub mod universal_listener;
pub mod packet_fragment;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        // Simple smoke test to confirm crate builds and modules link
        let _ = adapters::ssh::ssh_adapter_name();
    }
}
