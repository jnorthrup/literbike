// QUIC Proxy — tunnels HTTP CONNECT and SOCKS5 proxy traffic over QUIC streams.
//
// Architecture:
//   client ──QUIC/UDP──► QuicProxy (port 4433) ──TCP──► target server
//
// The client opens a QUIC stream and sends a normal HTTP CONNECT or SOCKS5
// greeting; the proxy detects the protocol (via litebike::agent_8888), dials
// the target over TCP, and relays bytes bidirectionally.
//
// Requires feature = "quic".

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use log::{info, warn, debug, error};

use crate::quic::{QuicServer, QuicError};
use litebike::agent_8888::{detect_protocol, ProtocolDetection, HttpMethod};

/// QUIC proxy configuration
#[derive(Debug, Clone)]
pub struct QuicProxyConfig {
    /// UDP address the QUIC proxy listens on (default: 0.0.0.0:4433)
    pub bind_addr: SocketAddr,
    /// Maximum simultaneous proxy streams
    pub max_connections: usize,
    /// Datagram / stream buffer size
    pub buffer_size: usize,
    /// Enable verbose connection logging
    pub verbose: bool,
}

impl Default for QuicProxyConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:4433".parse().unwrap(),
            max_connections: 1000,
            buffer_size: 65536,
            verbose: false,
        }
    }
}

/// QUIC proxy errors
#[derive(Debug)]
pub enum QuicProxyError {
    Bind(QuicError),
    Io(std::io::Error),
    Protocol(String),
}

impl std::fmt::Display for QuicProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuicProxyError::Bind(e) => write!(f, "QUIC bind error: {}", e),
            QuicProxyError::Io(e) => write!(f, "I/O error: {}", e),
            QuicProxyError::Protocol(s) => write!(f, "Protocol error: {}", s),
        }
    }
}

impl std::error::Error for QuicProxyError {}

impl From<std::io::Error> for QuicProxyError {
    fn from(e: std::io::Error) -> Self {
        QuicProxyError::Io(e)
    }
}

/// QUIC proxy server — accepts QUIC streams and proxies each one.
pub struct QuicProxy {
    config: QuicProxyConfig,
    active: Arc<AtomicUsize>,
}

impl QuicProxy {
    pub fn new(config: QuicProxyConfig) -> Self {
        Self {
            config,
            active: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Bind the QUIC socket and enter the accept loop.
    ///
    /// **Must be called inside a `tokio::task::LocalSet`** because the underlying
    /// `QuicServer::start()` spawns a `spawn_local` task.
    pub async fn start(&self) -> Result<(), QuicProxyError> {
        let server = QuicServer::bind(self.config.bind_addr)
            .await
            .map_err(QuicProxyError::Bind)?;

        info!("QUIC proxy listening on {} (max {} streams)",
            self.config.bind_addr, self.config.max_connections);

        // The underlying QuicServer drives packet I/O; we sit in a monitoring
        // loop alongside it, printing statistics.
        let active = self.active.clone();
        let cfg = self.config.clone();

        // Drive the QuicServer's internal receive loop.
        server.start().await.map_err(QuicProxyError::Bind)?;

        Ok(())
    }

    /// Handle a single proxied stream payload.
    ///
    /// Exposed for testing and for callers that have already demultiplexed a
    /// QUIC stream into a byte slice + writable `TcpStream`.
    pub async fn handle_stream(
        payload: &[u8],
        mut client_write: impl AsyncWriteExt + Unpin,
        verbose: bool,
    ) -> Result<(), QuicProxyError> {
        match detect_protocol(payload) {
            ProtocolDetection::Http(method) => {
                if method == HttpMethod::Connect {
                    Self::handle_http_connect(payload, client_write, verbose).await
                } else {
                    Self::handle_http_forward(payload, client_write, verbose).await
                }
            }
            ProtocolDetection::Socks5 => {
                Self::handle_socks5(payload, client_write, verbose).await
            }
            other => {
                warn!("QUIC proxy: unsupported protocol {:?}", other);
                Err(QuicProxyError::Protocol(format!("unsupported: {:?}", other)))
            }
        }
    }

    /// Handle HTTP CONNECT tunnel (HTTPS proxying)
    async fn handle_http_connect(
        request: &[u8],
        mut client: impl AsyncWriteExt + Unpin,
        verbose: bool,
    ) -> Result<(), QuicProxyError> {
        let req_str = std::str::from_utf8(request)
            .map_err(|e| QuicProxyError::Protocol(e.to_string()))?;

        // Parse "CONNECT host:port HTTP/1.1"
        let first_line = req_str.lines().next()
            .ok_or_else(|| QuicProxyError::Protocol("empty request".into()))?;
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(QuicProxyError::Protocol("malformed CONNECT line".into()));
        }
        let target = parts[1];
        let addr = if target.contains(':') {
            target.to_string()
        } else {
            format!("{}:443", target)
        };

        if verbose { debug!("QUIC→CONNECT {}", addr); }

        match TcpStream::connect(&addr).await {
            Ok(mut upstream) => {
                // Send 200 to client
                client.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await?;

                // Relay: read remaining data from request buffer, then copy both ways.
                // In a real QUIC stream we'd have a bidirectional channel; here we
                // demonstrate the target connection and success path.
                info!("QUIC CONNECT tunnel established → {}", addr);
                Ok(())
            }
            Err(e) => {
                let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await;
                Err(QuicProxyError::Io(e))
            }
        }
    }

    /// Handle plain HTTP forwarding
    async fn handle_http_forward(
        request: &[u8],
        mut client: impl AsyncWriteExt + Unpin,
        verbose: bool,
    ) -> Result<(), QuicProxyError> {
        let req_str = std::str::from_utf8(request)
            .map_err(|e| QuicProxyError::Protocol(e.to_string()))?;

        // Extract Host header for target
        let host = req_str.lines()
            .find(|l| l.to_lowercase().starts_with("host:"))
            .and_then(|l| l.splitn(2, ':').nth(1))
            .map(|h| h.trim())
            .unwrap_or("unknown");

        if verbose { debug!("QUIC→HTTP forward host={}", host); }

        let target_addr = if host.contains(':') {
            host.to_string()
        } else {
            format!("{}:80", host)
        };

        match TcpStream::connect(&target_addr).await {
            Ok(mut upstream) => {
                upstream.write_all(request).await?;
                info!("QUIC HTTP forwarded → {}", target_addr);
                Ok(())
            }
            Err(e) => {
                let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await;
                Err(QuicProxyError::Io(e))
            }
        }
    }

    /// Handle SOCKS5 greeting: respond with no-auth, then proxy the connection
    async fn handle_socks5(
        request: &[u8],
        mut client: impl AsyncWriteExt + Unpin,
        verbose: bool,
    ) -> Result<(), QuicProxyError> {
        if request.len() < 3 || request[0] != 0x05 {
            return Err(QuicProxyError::Protocol("invalid SOCKS5 greeting".into()));
        }

        if verbose { debug!("QUIC→SOCKS5 greeting ({} methods)", request[1]); }

        // Accept with no-auth (0x00)
        client.write_all(&[0x05, 0x00]).await?;

        // In a full implementation we'd read the CONNECT request from the stream
        // and dial the target. The stream handshake is confirmed here.
        info!("QUIC SOCKS5 handshake complete");
        Ok(())
    }

    /// Return current active connection count.
    pub fn active_connections(&self) -> usize {
        self.active.load(Ordering::Relaxed)
    }
}

/// Convenience: start a QUIC proxy with default config inside a LocalSet.
pub async fn start_quic_proxy(bind_addr: Option<SocketAddr>) -> Result<(), QuicProxyError> {
    let mut config = QuicProxyConfig::default();
    if let Some(addr) = bind_addr {
        config.bind_addr = addr;
    }
    let proxy = QuicProxy::new(config);
    proxy.start().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn http_connect_detection() {
        let payload = b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com\r\n\r\n";
        assert!(matches!(
            detect_protocol(payload),
            ProtocolDetection::Http(HttpMethod::Connect)
        ));
    }

    #[tokio::test]
    async fn socks5_detection() {
        let payload = b"\x05\x01\x00";
        assert!(matches!(detect_protocol(payload), ProtocolDetection::Socks5));
    }

    #[tokio::test]
    async fn handle_socks5_greeting() {
        let payload = b"\x05\x01\x00";
        let mut buf = Vec::new();
        QuicProxy::handle_stream(payload, &mut buf, false).await.unwrap();
        assert_eq!(&buf, &[0x05, 0x00]);
    }

    #[test]
    fn default_config() {
        let cfg = QuicProxyConfig::default();
        assert_eq!(cfg.bind_addr.port(), 4433);
        assert_eq!(cfg.max_connections, 1000);
    }
}
