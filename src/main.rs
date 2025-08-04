extern crate env_logger;
pub mod detection_orchestrator;
// LiteBike Proxy - Universal Protocol Detection Proxy
// Copyright (c) 2025 jnorthrup
// 
// Licensed under AGPL-3.0-or-later
// Commercial licensing available - contact copyright holder
//
// Comprehensive proxy with taxonomical abstractions and protocol detection
// Supports DoH, UPnP, Bonjour, and extensible protocol handling

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str;
use std::time::Duration;
use std::io;
use std::sync::Arc;

use env_logger::Env;
use log::{debug, error, info, warn};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
 // External DoH resolver removed to minimize cargo surface; system resolver is used instead

use crate::patricia_detector::{PatriciaDetector, Protocol};
use crate::protocol_handlers::{HttpHandler, Socks5Handler, TlsHandler};
use crate::protocol_registry::ProtocolHandler;
use crate::universal_listener::PrefixedStream;
use crate::libc_socket_tune::{accept_with_options, TcpTuningOptions};

mod types;
mod abstractions;
// mod doh; // DoH functionality is now in protocol_handlers
#[cfg(feature = "upnp")]
mod upnp;
#[cfg(feature = "auto-discovery")]
mod bonjour;
#[cfg(feature = "auto-discovery")]
mod pac;
// Temporarily disabled problematic modules for testing
// mod extended_protocols;
// mod advanced_protocols;
// mod fuzzer;
// mod advanced_fuzzer;
// mod violent_fuzzer;
// mod stubs;
mod universal_listener;
#[cfg(feature = "auto-discovery")]
mod auto_discovery;
mod patricia_detector;
mod unified_handler;
mod protocol_registry;
mod protocol_handlers;
mod simple_routing;
mod libc_socket_tune;
mod libc_listener;
mod config;

// --- Configuration ---
const HTTP_PORT: u16 = 8080;
const SOCKS_PORT: u16 = 1080;
const UNIVERSAL_PORT: u16 = 8888;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const UPNP_LEASE_DURATION: u32 = 3600; // 1 hour


/// Universal connection handler with Patricia Trie protocol detection
async fn handle_universal_connection(stream: TcpStream) -> io::Result<()> {
    // PrefixedStream in this codebase expects an explicit prefix Vec<u8>
    let mut prefixed_stream = PrefixedStream::new(stream, Vec::new());
    
    // Peek at the first few bytes to detect protocol from the inner stream
    let mut buffer = [0u8; 64];
    let n = prefixed_stream.inner.peek(&mut buffer).await?;
    
    if n == 0 {
        return Ok(());
    }
    
    let data = &buffer[..n];
    
    let detector = PatriciaDetector::new();
    let (protocol, bytes_consumed) = detector.detect_with_length(data);

    // Preload the bytes that were used for detection into the prefix buffer
    prefixed_stream = PrefixedStream::new(prefixed_stream.inner, data[..bytes_consumed].to_vec());

    match protocol {
        Protocol::Http => {
            debug!("Universal port: HTTP detected");
            // Check for special HTTP-based protocols
            #[cfg(feature = "auto-discovery")]
            if pac::is_pac_request(std::str::from_utf8(data).unwrap_or("")).await {
                info!("Routing to PAC handler");
                return pac::handle_pac_request(prefixed_stream).await;
            }
            #[cfg(feature = "auto-discovery")]
            if bonjour::is_bonjour_request(std::str::from_utf8(data).unwrap_or("")).await {
                info!("Routing to Bonjour handler");
                return bonjour::handle_bonjour(prefixed_stream).await;
            }
            #[cfg(feature = "upnp")]
            if upnp::is_upnp_request(std::str::from_utf8(data).unwrap_or("")).await {
                info!("Routing to UPnP handler");
                return upnp::handle_upnp_request(prefixed_stream).await;
            }
            
            HttpHandler::new().handle(prefixed_stream).await
        },
        Protocol::Socks5 => {
            debug!("Universal port: SOCKS5 detected");
            Socks5Handler::new().handle(prefixed_stream).await
        },
        Protocol::Tls => {
            debug!("Universal port: TLS detected");
            TlsHandler::new().handle(prefixed_stream).await
        },
        Protocol::ProxyProtocol => {
            debug!("Universal port: PROXY protocol detected - not yet handled");
            Err(io::Error::new(io::ErrorKind::Other, "PROXY protocol not yet handled"))
        },
        Protocol::Http2 => {
            debug!("Universal port: HTTP/2 detected - not yet handled");
            Err(io::Error::new(io::ErrorKind::Other, "HTTP/2 not yet handled"))
        },
        Protocol::WebSocket => {
            debug!("Universal port: WebSocket detected - not yet handled");
            Err(io::Error::new(io::ErrorKind::Other, "WebSocket not yet handled"))
        },
        Protocol::Unknown => {
            debug!("Universal port: Unknown protocol, attempting HTTP as fallback");
            HttpHandler::new().handle(prefixed_stream).await
        }
    }
}

#[tokio::main]
async fn main() {
    // Load runtime configuration from environment
    let cfg = config::Config::from_env();

    // Initialize logging using configured level
    let env = Env::default().default_filter_or(&cfg.log_level);
    env_logger::Builder::from_env(env).init();

    // Apply EGRESS_* env side effects for handlers (keeps current handler API unchanged)
    cfg.apply_env_side_effects();

    // Build primary route from config overrides
    let primary = simple_routing::RouteConfig {
        interface: cfg.interface.clone(),
        port: cfg.bind_port,
        bind_addr: cfg.bind_addr,
        protocols: vec!["all".to_string()],
    };

    // Universal listener using simple routing with swlan0 fallback
    let router = simple_routing::SimpleRouter::with_primary(primary);
    let (universal_listener, active_config) = router
        .bind_with_fallback()
        .await
        .expect("Failed to bind universal listener");

    info!(
        "Universal proxy listening on {} (interface: {}) with protocols: {:?}",
        active_config.socket_addr(),
        active_config.interface,
        simple_routing::get_supported_protocols(&active_config)
    );

    if active_config.interface == "lo" {
        warn!("Using fallback configuration - primary interface 'swlan0' was not available");
    }

    let tcp_tuning = active_config.tcp_tuning.clone();
    
    loop {
        if let Ok((stream, addr)) = accept_with_options(&universal_listener, &tcp_tuning).await {
            debug!("New connection from {} with TCP tuning applied", addr);
            tokio::spawn(async move {
                if let Err(e) = handle_universal_connection(stream).await {
                    debug!("Universal handler error from {}: {}", addr, e);
                }
            });
        }
    }
}

// --- Tests ---
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::ReadBuf;


    #[tokio::test]
    async fn test_http_detection() {
        let detector = PatriciaDetector::new();
        
        assert!(matches!(detector.detect(b"GET / HTTP/1.1\r\n"), Protocol::Http));
        assert!(matches!(detector.detect(b"POST /api"), Protocol::Http));
        assert!(matches!(detector.detect(b"CONNECT example.com:443"), Protocol::Http));
    }

    #[tokio::test]
    async fn test_socks5_detection() {
        let detector = PatriciaDetector::new();
        
        assert!(matches!(detector.detect(&[0x05, 0x01, 0x00]), Protocol::Socks5));
    }

    #[tokio::test]
    async fn test_tls_detection() {
        let detector = PatriciaDetector::new();
        
        assert!(matches!(detector.detect(&[0x16, 0x03, 0x01]), Protocol::Tls));
        assert!(matches!(detector.detect(&[0x16, 0x03, 0x03]), Protocol::Tls));
    }
}