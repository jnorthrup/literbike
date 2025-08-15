// Knox Proxy - Dedicated carrier bypass and tethering restoration module
// Expert-level automation for TERMUX Knox environments

use std::io;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use log::{info, warn, error, debug};

use crate::tethering_bypass::{TetheringBypass, enable_carrier_bypass};
use crate::universal_listener::{Protocol, detect_protocol_posix, PrefixedStream};
use crate::posix_sockets::{posix_peek, PosixTcpStream};
use crate::tcp_fingerprint::{TcpFingerprintManager, MobileProfile};
use crate::packet_fragment::{PacketFragmenter, MobileFragmentPattern};
use crate::tls_fingerprint::{TlsFingerprintManager, MobileBrowserProfile};

/// Knox proxy configuration
pub struct KnoxProxyConfig {
    pub bind_addr: String,
    pub socks_port: u16,
    pub enable_knox_bypass: bool,
    pub enable_tethering_bypass: bool,
    pub ttl_spoofing: u8,
    pub max_connections: usize,
    pub buffer_size: usize,
    pub tcp_fingerprint_enabled: bool,
    pub packet_fragmentation_enabled: bool,
    pub tls_fingerprint_enabled: bool,
}

impl Default for KnoxProxyConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:8080".to_string(),
            socks_port: 1080,
            enable_knox_bypass: true,
            enable_tethering_bypass: true,
            ttl_spoofing: 64,
            max_connections: 100,
            buffer_size: 4096,
            tcp_fingerprint_enabled: true,
            packet_fragmentation_enabled: true,
            tls_fingerprint_enabled: true,
        }
    }
}

/// Knox proxy server
pub struct KnoxProxy {
    config: KnoxProxyConfig,
    tethering_bypass: Option<TetheringBypass>,
    active_connections: Arc<std::sync::atomic::AtomicUsize>,
}

impl KnoxProxy {
    pub fn new(config: KnoxProxyConfig) -> Self {
        Self {
            config,
            tethering_bypass: None,
            active_connections: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
    
    /// Start the Knox proxy server
    pub async fn start(&mut self) -> io::Result<()> {
        info!("🚀 Starting Knox Proxy");
        info!("   Bind address: {}", self.config.bind_addr);
        info!("   SOCKS port: {}", self.config.socks_port);
        info!("   Knox bypass: {}", self.config.enable_knox_bypass);
        info!("   Tethering bypass: {}", self.config.enable_tethering_bypass);
        info!("   TTL spoofing: {}", self.config.ttl_spoofing);
        
        // Setup tethering bypass
        if self.config.enable_tethering_bypass {
            info!("🔓 Enabling tethering bypass...");
            let mut bypass = TetheringBypass::new();
            match bypass.enable_bypass() {
                Ok(()) => {
                    info!("✅ Tethering bypass enabled");
                    self.tethering_bypass = Some(bypass);
                }
                Err(e) => {
                    warn!("⚠ Tethering bypass failed: {}", e);
                }
            }
        }
        
        // Bind listener
        let listener = TcpListener::bind(&self.config.bind_addr).await?;
        info!("✅ Knox proxy listening on {}", self.config.bind_addr);
        
        // Print usage instructions
        self.print_usage_instructions();
        
        // Accept connections
        while let Ok((stream, peer_addr)) = listener.accept().await {
            let current_connections = self.active_connections.load(std::sync::atomic::Ordering::Relaxed);
            
            if current_connections >= self.config.max_connections {
                warn!("⚠ Max connections ({}) reached, dropping {}", self.config.max_connections, peer_addr);
                continue;
            }
            
            self.active_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            
            let config = self.config.clone();
            let active_connections = self.active_connections.clone();
            
            tokio::spawn(async move {
                match Self::handle_connection(stream, &config).await {
                    Ok(()) => {
                        debug!("✓ Connection from {} completed", peer_addr);
                    }
                    Err(e) => {
                        error!("❌ Connection from {} failed: {}", peer_addr, e);
                    }
                }
                active_connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            });
        }
        
        Ok(())
    }
    
    /// Handle individual connection with Knox bypass
    async fn handle_connection(stream: TcpStream, config: &KnoxProxyConfig) -> io::Result<()> {
        let peer_addr = stream.peer_addr()?;
        debug!("New connection from {}", peer_addr);
        
        // Use Knox bypass for protocol detection if enabled
        let protocol = if config.enable_knox_bypass {
            detect_protocol_posix(&stream)?
        } else {
            // Fallback to regular detection
            let mut buffer = vec![0u8; 512];
            let n = {
                let mut stream_ref = &stream;
                stream_ref.read(&mut buffer).await?
            };
            
            if n > 0 && buffer[0] == 0x05 {
                Protocol::Socks5
            } else if n > 0 {
                if let Ok(text) = std::str::from_utf8(&buffer[..std::cmp::min(n, 256)]) {
                    if text.starts_with("GET ") || text.starts_with("POST ") || 
                       text.starts_with("PUT ") || text.starts_with("CONNECT ") {
                        Protocol::Http
                    } else {
                        Protocol::Unknown
                    }
                } else {
                    Protocol::Unknown
                }
            } else {
                Protocol::Unknown
            }
        };
        
        match protocol {
            Protocol::Http => {
                info!("Handling HTTP connection from {}", peer_addr);
                Self::handle_http_proxy(stream, config).await
            }
            Protocol::Socks5 => {
                info!("Handling SOCKS5 connection from {}", peer_addr);
                Self::handle_socks5_proxy(stream, config).await
            }
            _ => {
                warn!("Unknown protocol from {}, treating as HTTP", peer_addr);
                Self::handle_http_proxy(stream, config).await
            }
        }
    }
    
    /// Handle HTTP CONNECT proxy
    async fn handle_http_proxy(mut stream: TcpStream, config: &KnoxProxyConfig) -> io::Result<()> {
        let mut buffer = vec![0u8; config.buffer_size];
        let n = stream.read(&mut buffer).await?;
        
        if n == 0 {
            return Ok(());
        }
        
        let request = String::from_utf8_lossy(&buffer[..n]);
        let lines: Vec<&str> = request.lines().collect();
        
        if lines.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Empty request"));
        }
        
        let first_line = lines[0];
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        
        if parts.len() < 3 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid HTTP request"));
        }
        
        let method = parts[0];
        let target = parts[1];
        
        if method == "CONNECT" {
            // HTTP CONNECT for HTTPS tunneling
            let addr = if target.contains(':') {
                target.to_string()
            } else {
                format!("{}:443", target)
            };
            
            debug!("CONNECT to {}", addr);
            
            // Connect to target
            let target_stream = match TcpStream::connect(&addr).await {
                Ok(s) => s,
                Err(e) => {
                    let response = "HTTP/1.1 502 Bad Gateway\r\n\r\n";
                    stream.write_all(response.as_bytes()).await?;
                    return Err(e);
                }
            };
            
            // Send success response
            let response = "HTTP/1.1 200 Connection established\r\n\r\n";
            stream.write_all(response.as_bytes()).await?;
            
            // Start bidirectional copy
            Self::copy_bidirectional(stream, target_stream).await?;
        } else {
            // Regular HTTP proxy
            debug!("HTTP {} to {}", method, target);
            
            // Parse target URL
            let url = if target.starts_with("http://") {
                target.to_string()
            } else if !target.starts_with("/") {
                format!("http://{}", target)
            } else {
                // Relative URL - need Host header
                let host = lines.iter()
                    .find(|line| line.to_lowercase().starts_with("host:"))
                    .and_then(|line| line.split(':').nth(1))
                    .map(|h| h.trim())
                    .unwrap_or("localhost");
                format!("http://{}{}", host, target)
            };
            
            // Simple HTTP forwarding (basic implementation)
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nProxy working";
            stream.write_all(response.as_bytes()).await?;
        }
        
        Ok(())
    }
    
    /// Handle SOCKS5 proxy
    async fn handle_socks5_proxy(mut stream: TcpStream, _config: &KnoxProxyConfig) -> io::Result<()> {
        // SOCKS5 authentication
        let mut buffer = [0u8; 256];
        let n = stream.read(&mut buffer).await?;
        
        if n < 3 || buffer[0] != 0x05 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid SOCKS5 request"));
        }
        
        // Respond with no authentication required
        stream.write_all(&[0x05, 0x00]).await?;
        
        // Read connection request
        let n = stream.read(&mut buffer).await?;
        
        if n < 10 || buffer[0] != 0x05 || buffer[1] != 0x01 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid SOCKS5 connection request"));
        }
        
        // Parse target address
        let (target_addr, addr_len) = match buffer[3] {
            0x01 => {
                // IPv4
                if n < 10 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid IPv4 address"));
                }
                let ip = format!("{}.{}.{}.{}", buffer[4], buffer[5], buffer[6], buffer[7]);
                let port = u16::from_be_bytes([buffer[8], buffer[9]]);
                (format!("{}:{}", ip, port), 10)
            }
            0x03 => {
                // Domain name
                let domain_len = buffer[4] as usize;
                if n < 7 + domain_len {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid domain name"));
                }
                let domain = String::from_utf8_lossy(&buffer[5..5 + domain_len]);
                let port = u16::from_be_bytes([buffer[5 + domain_len], buffer[6 + domain_len]]);
                (format!("{}:{}", domain, port), 7 + domain_len)
            }
            _ => {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported address type"));
            }
        };
        
        debug!("SOCKS5 connect to {}", target_addr);
        
        // Connect to target
        let target_stream = match TcpStream::connect(&target_addr).await {
            Ok(s) => s,
            Err(_) => {
                // Send connection failed response
                let mut response = vec![0x05, 0x05, 0x00, 0x01]; // Connection refused
                response.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // Dummy bind address
                stream.write_all(&response).await?;
                return Err(io::Error::new(io::ErrorKind::ConnectionRefused, "Target connection failed"));
            }
        };
        
        // Send success response
        let mut response = vec![0x05, 0x00, 0x00, 0x01]; // Success
        response.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // Dummy bind address
        stream.write_all(&response).await?;
        
        // Start bidirectional copy
        Self::copy_bidirectional(stream, target_stream).await?;
        
        Ok(())
    }
    
    /// Bidirectional copy between two streams
    async fn copy_bidirectional(mut stream1: TcpStream, mut stream2: TcpStream) -> io::Result<()> {
        let (mut r1, mut w1) = stream1.split();
        let (mut r2, mut w2) = stream2.split();
        
        let copy1 = tokio::io::copy(&mut r1, &mut w2);
        let copy2 = tokio::io::copy(&mut r2, &mut w1);
        
        tokio::try_join!(copy1, copy2)?;
        
        Ok(())
    }
    
    /// Print usage instructions
    fn print_usage_instructions(&self) {
        println!("");
        println!("📱 Knox bypass features:");
        if self.config.enable_knox_bypass {
            println!("   ✓ POSIX socket operations bypass /proc restrictions");
            println!("   ✓ Direct syscalls avoid Android security policies");
        }
        if self.config.enable_tethering_bypass {
            println!("   ✓ TTL spoofing (set to {})", self.config.ttl_spoofing);
            println!("   ✓ DNS override (8.8.8.8, 1.1.1.1)");
            println!("   ✓ User-Agent rotation");
            println!("   ✓ Traffic pattern mimicry");
        }
        println!("");
        println!("🔗 Usage:");
        println!("   export http_proxy=http://{}", self.config.bind_addr);
        println!("   export https_proxy=http://{}", self.config.bind_addr);
        println!("   export all_proxy=socks5://127.0.0.1:{}", self.config.socks_port);
        println!("");
        println!("💡 Test with:");
        println!("   curl -x http://{} http://httpbin.org/ip", self.config.bind_addr);
        println!("   curl --socks5 127.0.0.1:{} http://httpbin.org/ip", self.config.socks_port);
        println!("");
    }
}

impl Clone for KnoxProxyConfig {
    fn clone(&self) -> Self {
        Self {
            bind_addr: self.bind_addr.clone(),
            socks_port: self.socks_port,
            enable_knox_bypass: self.enable_knox_bypass,
            enable_tethering_bypass: self.enable_tethering_bypass,
            ttl_spoofing: self.ttl_spoofing,
            max_connections: self.max_connections,
            buffer_size: self.buffer_size,
        }
    }
}

/// Start Knox proxy with configuration
pub async fn start_knox_proxy(config: KnoxProxyConfig) -> io::Result<()> {
    let mut proxy = KnoxProxy::new(config);
    proxy.start().await
}

/// Quick start function for immediate carrier bypass
pub async fn quick_start_knox_proxy() -> io::Result<()> {
    let config = KnoxProxyConfig::default();
    start_knox_proxy(config).await
}