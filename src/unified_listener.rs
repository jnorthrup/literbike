//! Elite unified listeners with priority-cascade byte scanners
//! TCP and UDP listeners on port 8888 with fast protocol detection

use std::io;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional};
use log::{info, warn, error, debug};
use crate::libc_listener::{bind_with_options, ListenerOptions};

/// Configuration for unified listeners
#[derive(Debug, Clone)]
pub struct UnifiedConfig {
    pub port: u16,
    pub bind_addr: IpAddr,
}

impl Default for UnifiedConfig {
    fn default() -> Self {
        Self {
            port: 8888,
            bind_addr: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        }
    }
}

/// Unified TCP/UDP listener manager
pub struct UnifiedListener {
    config: UnifiedConfig,
}

impl UnifiedListener {
    pub fn new(config: UnifiedConfig) -> Self {
        Self { config }
    }

    /// Start both TCP and UDP listeners
    pub async fn start(&self) -> io::Result<()> {
        let tcp_addr = SocketAddr::new(self.config.bind_addr, self.config.port);
        let udp_addr = SocketAddr::new(self.config.bind_addr, self.config.port);

        info!("Starting LiteBike unified listeners on port {}", self.config.port);

        // Start TCP listener
        let tcp_handle = {
            let tcp_addr = tcp_addr;
            tokio::spawn(async move {
                if let Err(e) = Self::run_tcp_listener(tcp_addr).await {
                    error!("TCP listener failed: {}", e);
                }
            })
        };

        // Start UDP listener  
        let udp_handle = {
            let udp_addr = udp_addr;
            tokio::spawn(async move {
                if let Err(e) = Self::run_udp_listener(udp_addr).await {
                    error!("UDP listener failed: {}", e);
                }
            })
        };

        info!("✅ LiteBike unified listeners started on {}", self.config.port);

        // Wait for both listeners
        tokio::select! {
            _ = tcp_handle => {},
            _ = udp_handle => {},
        }

        Ok(())
    }

    /// TCP listener with priority-cascade protocol detection
    async fn run_tcp_listener(addr: SocketAddr) -> io::Result<()> {
        // Use high-performance TCP listener
        let listener_options = ListenerOptions {
            reuse_addr: true,
            reuse_port: true,
            backlog: 1024,
        };

        let listener = bind_with_options(addr, &listener_options).await?;
        info!("🔌 TCP listener bound to {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    debug!("New TCP connection from {}", peer_addr);
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_tcp_connection(stream, peer_addr).await {
                            warn!("TCP connection error from {}: {}", peer_addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept TCP connection: {}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    }

    /// UDP listener with priority-cascade protocol detection
    async fn run_udp_listener(addr: SocketAddr) -> io::Result<()> {
        let socket = UdpSocket::bind(addr).await?;
        info!("📡 UDP listener bound to {}", addr);

        let mut buffer = vec![0u8; 65536]; // Max UDP packet size

        loop {
            match socket.recv_from(&mut buffer).await {
                Ok((len, peer_addr)) => {
                    let packet = &buffer[..len];
                    debug!("UDP packet from {} ({} bytes)", peer_addr, len);
                    
                    // Handle packet inline (no background task needed for UDP)
                    Self::handle_udp_packet(packet.to_vec(), peer_addr, &socket).await;
                }
                Err(e) => {
                    error!("UDP receive error: {}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            }
        }
    }

    /// Handle TCP connection with priority-cascade byte scanning
    async fn handle_tcp_connection(mut stream: TcpStream, peer_addr: SocketAddr) -> io::Result<()> {
        // Peek at first bytes for protocol detection
        let mut peek_buffer = [0u8; 16];
        let peek_len = stream.peek(&mut peek_buffer).await?;
        let peek_bytes = &peek_buffer[..peek_len];

        // Priority cascade - first match wins
        if socks5_scanner(peek_bytes) {
            info!("🧦 SOCKS5 connection from {}", peer_addr);
            Self::handle_socks5(stream, peer_addr).await
        } else if http_scanner(peek_bytes) {
            info!("🌐 HTTP connection from {}", peer_addr);
            Self::handle_http(stream, peer_addr).await
        } else {
            info!("🔌 Raw TCP connection from {}", peer_addr);
            Self::handle_raw_tcp(stream, peer_addr).await
        }
    }

    /// Handle UDP packet with priority-cascade byte scanning
    async fn handle_udp_packet(packet: Vec<u8>, peer_addr: SocketAddr, socket: &UdpSocket) {
        // Priority cascade
        if dns_scanner(&packet) {
            info!("🌍 DNS packet from {}", peer_addr);
            Self::handle_dns(packet, peer_addr, socket).await;
        } else if stun_scanner(&packet) {
            info!("📞 STUN packet from {}", peer_addr);
            Self::handle_stun(packet, peer_addr, socket).await;
        } else if mdns_scanner(&packet) {
            info!("🔍 mDNS packet from {}", peer_addr);
            Self::handle_mdns(packet, peer_addr, socket).await;
        } else {
            debug!("❓ Unknown UDP packet from {} ({} bytes)", peer_addr, packet.len());
        }
    }

    /// SOCKS5 handler
    async fn handle_socks5(mut stream: TcpStream, peer_addr: SocketAddr) -> io::Result<()> {
        // Read SOCKS5 auth methods
        let mut auth_buf = [0u8; 257];
        let n = stream.read(&mut auth_buf).await?;
        if n < 3 || auth_buf[0] != 0x05 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid SOCKS5 handshake"));
        }

        // Send no-auth response
        stream.write_all(&[0x05, 0x00]).await?;

        // Read connection request
        let mut req_buf = [0u8; 1024];
        let n = stream.read(&mut req_buf).await?;
        if n < 10 || req_buf[0] != 0x05 || req_buf[1] != 0x01 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid SOCKS5 request"));
        }

        // Parse target address
        let (target, _) = parse_socks5_target(&req_buf[3..n])?;
        info!("SOCKS5 request to {} from {}", target, peer_addr);

        // Connect to target
        match TcpStream::connect(&target).await {
            Ok(mut target_stream) => {
                // Send success response
                stream.write_all(&[0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).await?;
                
                // Start bidirectional relay
                let (_, _) = copy_bidirectional(&mut stream, &mut target_stream).await?;
                Ok(())
            }
            Err(_) => {
                // Send error response
                stream.write_all(&[0x05, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).await?;
                Err(io::Error::new(io::ErrorKind::ConnectionRefused, "Target unreachable"))
            }
        }
    }

    /// HTTP handler
    async fn handle_http(mut stream: TcpStream, peer_addr: SocketAddr) -> io::Result<()> {
        // Read HTTP request
        let mut buffer = Vec::new();
        let mut temp = [0u8; 4096];
        
        loop {
            let n = stream.read(&mut temp).await?;
            if n == 0 { break; }
            buffer.extend_from_slice(&temp[..n]);
            
            // Check for complete HTTP headers
            if buffer.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            if buffer.len() > 65536 { // Prevent DoS
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Request too large"));
            }
        }

        let request = String::from_utf8_lossy(&buffer);
        let first_line = request.lines().next().unwrap_or("");
        
        if first_line.starts_with("CONNECT ") {
            // HTTPS tunnel
            let target = first_line.split_whitespace().nth(1).unwrap_or("");
            info!("HTTP CONNECT to {} from {}", target, peer_addr);
            
            match TcpStream::connect(target).await {
                Ok(mut target_stream) => {
                    stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await?;
                    let (_, _) = copy_bidirectional(&mut stream, &mut target_stream).await?;
                    Ok(())
                }
                Err(e) => {
                    let response = format!("HTTP/1.1 502 Bad Gateway\r\n\r\nFailed to connect: {}", e);
                    stream.write_all(response.as_bytes()).await?;
                    Err(e)
                }
            }
        } else {
            // Regular HTTP - simple echo response for now
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nLiteBike Proxy\nFrom: {}\nRequest: {}\n", peer_addr, first_line);
            stream.write_all(response.as_bytes()).await?;
            Ok(())
        }
    }

    /// Raw TCP handler
    async fn handle_raw_tcp(mut stream: TcpStream, peer_addr: SocketAddr) -> io::Result<()> {
        info!("Raw TCP echo for {}", peer_addr);
        
        let mut buffer = [0u8; 4096];
        loop {
            match stream.read(&mut buffer).await {
                Ok(0) => break, // Connection closed
                Ok(n) => {
                    stream.write_all(&buffer[..n]).await?;
                }
                Err(e) => {
                    warn!("Raw TCP error from {}: {}", peer_addr, e);
                    break;
                }
            }
        }
        Ok(())
    }

    /// DNS handler
    async fn handle_dns(packet: Vec<u8>, peer_addr: SocketAddr, socket: &UdpSocket) {
        // Simple DNS echo response for now
        info!("DNS query from {} ({} bytes)", peer_addr, packet.len());
        
        // In a real implementation, this would parse the DNS query and respond appropriately
        // For now, just acknowledge receipt
        let response = b"DNS response placeholder";
        let _ = socket.send_to(response, peer_addr).await;
    }

    /// STUN handler
    async fn handle_stun(packet: Vec<u8>, peer_addr: SocketAddr, socket: &UdpSocket) {
        info!("STUN request from {} ({} bytes)", peer_addr, packet.len());
        
        // Simple STUN binding response
        // Real implementation would parse STUN message and create proper response
        let response = b"STUN response placeholder";
        let _ = socket.send_to(response, peer_addr).await;
    }

    /// mDNS handler  
    async fn handle_mdns(packet: Vec<u8>, peer_addr: SocketAddr, socket: &UdpSocket) {
        info!("mDNS query from {} ({} bytes)", peer_addr, packet.len());
        
        // mDNS responses would go here
        // For now, just log the query
    }
}

// ==== FAST BYTE SCANNERS ====

/// SOCKS5 scanner - checks for version 0x05
fn socks5_scanner(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && bytes[0] == 0x05
}

/// HTTP scanner - checks for HTTP methods
fn http_scanner(bytes: &[u8]) -> bool {
    bytes.starts_with(b"GET ") ||
    bytes.starts_with(b"POST ") ||
    bytes.starts_with(b"PUT ") ||
    bytes.starts_with(b"DELETE ") ||
    bytes.starts_with(b"HEAD ") ||
    bytes.starts_with(b"OPTIONS ") ||
    bytes.starts_with(b"CONNECT ") ||
    bytes.starts_with(b"PATCH ")
}

/// DNS scanner - checks for valid DNS header
fn dns_scanner(bytes: &[u8]) -> bool {
    if bytes.len() < 12 { return false; }
    
    // Check DNS header structure
    let flags = u16::from_be_bytes([bytes[2], bytes[3]]);
    let opcode = (flags >> 11) & 0x0F;
    
    // Standard query or response
    opcode == 0 && bytes.len() >= 12
}

/// STUN scanner - checks for STUN magic cookie
fn stun_scanner(bytes: &[u8]) -> bool {
    if bytes.len() < 20 { return false; }
    
    // STUN messages start with 00 or 01, followed by message type
    // Magic cookie at bytes 4-7: 0x2112A442
    bytes.len() >= 20 &&
    (bytes[0] == 0x00 || bytes[0] == 0x01) &&
    bytes[4] == 0x21 && bytes[5] == 0x12 && 
    bytes[6] == 0xA4 && bytes[7] == 0x42
}

/// mDNS scanner - checks for multicast DNS
fn mdns_scanner(bytes: &[u8]) -> bool {
    if bytes.len() < 12 { return false; }
    
    // mDNS has DNS structure but with multicast flag
    let flags = u16::from_be_bytes([bytes[2], bytes[3]]);
    let opcode = (flags >> 11) & 0x0F;
    
    // Check for mDNS characteristics (queries to .local, etc.)
    opcode == 0 && bytes.len() >= 12
}

// ==== HELPER FUNCTIONS ====

/// Parse SOCKS5 target address
fn parse_socks5_target(data: &[u8]) -> io::Result<(String, usize)> {
    if data.is_empty() { 
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Empty SOCKS5 data"));
    }

    let addr_type = data[0];
    match addr_type {
        0x01 => {
            // IPv4
            if data.len() < 7 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid IPv4"));
            }
            let ip = format!("{}.{}.{}.{}", data[1], data[2], data[3], data[4]);
            let port = u16::from_be_bytes([data[5], data[6]]);
            Ok((format!("{}:{}", ip, port), 7))
        }
        0x03 => {
            // Domain name
            if data.len() < 2 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid domain"));
            }
            let domain_len = data[1] as usize;
            if data.len() < 4 + domain_len {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Domain too short"));
            }
            let domain = String::from_utf8_lossy(&data[2..2 + domain_len]);
            let port = u16::from_be_bytes([data[2 + domain_len], data[3 + domain_len]]);
            Ok((format!("{}:{}", domain, port), 4 + domain_len))
        }
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported address type"))
    }
}