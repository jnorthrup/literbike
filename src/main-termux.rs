// LiteBike Proxy - Termux Optimized Version
// Simplified proxy without complex dependencies for Android deployment

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str;
use std::time::Duration;
use std::io;

use env_logger::Env;
use log::{debug, error, info, warn};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

mod patricia_detector;
use patricia_detector::{PatriciaDetector, Protocol, quick_detect};
mod auto_discovery;
use auto_discovery::AutoDiscovery;
use litebike::note20_features::{Note20NetworkConfig, get_optimal_bind_address, configure_5g_proxy};

// --- Configuration ---
const HTTP_PORT: u16 = 8080;
const SOCKS_PORT: u16 = 1080;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Simple DNS resolution using system resolver
async fn resolve_host(host: &str) -> io::Result<IpAddr> {
    match host.parse::<IpAddr>() {
        Ok(ip) => Ok(ip),
        Err(_) => {
            // Use tokio's built-in DNS resolution
            let addresses: Vec<SocketAddr> = tokio::net::lookup_host(format!("{}:80", host))
                .await?
                .collect();
            
            addresses.first()
                .map(|addr| addr.ip())
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No IP address found"))
        }
    }
}

/// Connects to a target address with timeout
async fn connect_to_target(target: &str) -> io::Result<TcpStream> {
    let (host, port_str) = match target.rsplit_once(':') {
        Some((host, port)) => (host, port),
        None => return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid target format, missing port")),
    };
    
    let port: u16 = port_str.parse().map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid port number"))?;
    let ip_addr = resolve_host(host).await?;
    let socket_addr = SocketAddr::new(ip_addr, port);

    debug!("Connecting to {} -> {}", target, socket_addr);
    match timeout(CONNECT_TIMEOUT, TcpStream::connect(socket_addr)).await {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(io::Error::new(io::ErrorKind::TimedOut, "Connection timed out")),
    }
}

/// Relays data between two streams efficiently
async fn relay_streams<S1, S2>(mut client: S1, mut remote: S2) -> io::Result<()>
where
    S1: AsyncRead + AsyncWrite + Unpin,
    S2: AsyncRead + AsyncWrite + Unpin,
{
    let (mut client_reader, mut client_writer) = tokio::io::split(&mut client);
    let (mut remote_reader, mut remote_writer) = tokio::io::split(&mut remote);

    let client_to_remote = tokio::io::copy(&mut client_reader, &mut remote_writer);
    let remote_to_client = tokio::io::copy(&mut remote_reader, &mut client_writer);

    tokio::select! {
        res = client_to_remote => {
            if let Err(e) = res { debug!("Error copying client to remote: {}", e); }
        },
        res = remote_to_client => {
            if let Err(e) = res { debug!("Error copying remote to client: {}", e); }
        },
    }
    debug!("Relay streams finished.");
    Ok(())
}

// Global Patricia detector instance
lazy_static::lazy_static! {
    static ref PROTOCOL_DETECTOR: PatriciaDetector = PatriciaDetector::new();
}

// --- Universal Handler (HTTP + SOCKS5 + TLS) ---
async fn handle_universal<S>(mut stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut buffer = [0u8; 4096];
    let n = stream.read(&mut buffer).await?;
    if n == 0 { return Ok(()); }

    // Use Patricia trie for fast protocol detection
    let (protocol, matched_len) = PROTOCOL_DETECTOR.detect_with_length(&buffer[..n]);
    debug!("Patricia detector: {:?} (matched {} bytes of {})", protocol, matched_len, n);
    
    // Fallback to quick bitwise detection for edge cases
    let protocol = match protocol {
        Protocol::Unknown => {
            if let Some(p) = quick_detect(&buffer[..n]) {
                debug!("Quick detect fallback: {:?}", p);
                p
            } else {
                Protocol::Unknown
            }
        }
        p => p,
    };
    
    match protocol {
        Protocol::Socks5 => {
            // Handle as SOCKS5 with the pre-read buffer
            handle_socks5_with_buffer(&buffer[..n], stream).await
        },
        Protocol::Http => {
            let request_str = str::from_utf8(&buffer[..n]).unwrap_or("");
            handle_http_with_buffer(request_str, &buffer[..n], stream).await
        },
        Protocol::Tls => {
            // For TLS, we need to forward to a TLS-capable upstream
            handle_tls_with_buffer(&buffer[..n], stream).await
        },
        Protocol::WebSocket => {
            // WebSocket starts as HTTP and upgrades
            let request_str = str::from_utf8(&buffer[..n]).unwrap_or("");
            handle_http_with_buffer(request_str, &buffer[..n], stream).await
        },
        Protocol::ProxyProtocol => {
            // HAProxy PROXY protocol - handle as HTTP after stripping header
            let request_str = str::from_utf8(&buffer[..n]).unwrap_or("");
            handle_http_with_buffer(request_str, &buffer[..n], stream).await
        },
        Protocol::Http2 => {
            // HTTP/2 - handle as HTTP for now
            let request_str = str::from_utf8(&buffer[..n]).unwrap_or("");
            handle_http_with_buffer(request_str, &buffer[..n], stream).await
        },
        Protocol::Unknown => {
            debug!("Unknown protocol, treating as HTTP");
            let request_str = str::from_utf8(&buffer[..n]).unwrap_or("");
            handle_http_with_buffer(request_str, &buffer[..n], stream).await
        }
    }
}

// --- TLS Handler with SNI extraction ---
async fn handle_tls_with_buffer<S>(buffer: &[u8], stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // Extract SNI (Server Name Indication) from TLS Client Hello
    let sni_hostname = extract_sni_hostname(buffer);
    
    let target = match sni_hostname {
        Some(hostname) => {
            debug!("TLS SNI detected: {}", hostname);
            format!("{}:443", hostname)
        }
        None => {
            debug!("No SNI found, using default HTTPS port");
            "127.0.0.1:443".to_string()
        }
    };
    
    match connect_to_target(&target).await {
        Ok(mut remote) => {
            info!("TLS passthrough to {} established", target);
            // Write the already-read buffer first
            remote.write_all(buffer).await?;
            // Then relay the streams
            relay_streams(stream, remote).await?;
        }
        Err(e) => {
            error!("Failed to establish TLS passthrough to {}: {}", target, e);
            return Err(e);
        }
    }
    Ok(())
}

// Extract SNI hostname from TLS Client Hello
fn extract_sni_hostname(buffer: &[u8]) -> Option<String> {
    // Minimum size: 5 (record) + 4 (handshake) + 2 (client version) + 32 (random) = 43
    if buffer.len() < 43 {
        return None;
    }
    
    // Verify this is a TLS handshake (0x16) and Client Hello (0x01)
    if buffer[0] != 0x16 || buffer[5] != 0x01 {
        return None;
    }
    
    // Skip fixed-length fields to get to session ID
    let mut pos = 43;
    
    // Skip session ID
    if pos >= buffer.len() {
        return None;
    }
    let session_id_len = buffer[pos] as usize;
    pos += 1 + session_id_len;
    
    // Skip cipher suites
    if pos + 2 > buffer.len() {
        return None;
    }
    let cipher_suites_len = u16::from_be_bytes([buffer[pos], buffer[pos + 1]]) as usize;
    pos += 2 + cipher_suites_len;
    
    // Skip compression methods
    if pos >= buffer.len() {
        return None;
    }
    let compression_len = buffer[pos] as usize;
    pos += 1 + compression_len;
    
    // Extensions length
    if pos + 2 > buffer.len() {
        return None;
    }
    let extensions_len = u16::from_be_bytes([buffer[pos], buffer[pos + 1]]) as usize;
    pos += 2;
    
    let extensions_end = pos + extensions_len;
    if extensions_end > buffer.len() {
        return None;
    }
    
    // Parse extensions to find SNI (type 0x0000)
    while pos + 4 <= extensions_end {
        let ext_type = u16::from_be_bytes([buffer[pos], buffer[pos + 1]]);
        let ext_len = u16::from_be_bytes([buffer[pos + 2], buffer[pos + 3]]) as usize;
        pos += 4;
        
        if pos + ext_len > extensions_end {
            break;
        }
        
        if ext_type == 0x0000 {  // SNI extension
            // SNI format: list length (2) + type (1) + hostname length (2) + hostname
            if ext_len >= 5 && pos + 5 <= buffer.len() {
                let name_type = buffer[pos + 2];
                if name_type == 0x00 {  // host_name type
                    let name_len = u16::from_be_bytes([buffer[pos + 3], buffer[pos + 4]]) as usize;
                    let name_start = pos + 5;
                    if name_start + name_len <= buffer.len() {
                        return String::from_utf8(buffer[name_start..name_start + name_len].to_vec()).ok();
                    }
                }
            }
            break;
        }
        
        pos += ext_len;
    }
    
    None
}

// --- HTTP Handler with pre-read buffer ---
async fn handle_http_with_buffer<S>(request_str: &str, buffer: &[u8], mut stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    debug!("HTTP Request: {}", request_str);

    if request_str.starts_with("CONNECT ") {
        if let Some(host) = request_str.split_whitespace().nth(1) {
            let target = if host.contains(':') { host.to_string() } else { format!("{}:443", host) };
            match connect_to_target(&target).await {
                Ok(remote) => {
                    info!("HTTP CONNECT to {}", target);
                    stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await?;
                    relay_streams(stream, remote).await?;
                }
                Err(e) => {
                    error!("Failed to connect to {}: {}", target, e);
                    stream.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await?;
                }
            }
        }
    } else if let Some(host_line) = request_str.lines().find(|l| l.to_lowercase().starts_with("host:")) {
        if let Some(host) = host_line.split(':').nth(1).map(|s| s.trim()) {
            let target = format!("{}:80", host);
            match connect_to_target(&target).await {
                Ok(mut remote) => {
                    info!("HTTP GET to {}", target);
                    remote.write_all(buffer).await?;
                    relay_streams(stream, remote).await?;
                }
                Err(e) => error!("Failed to connect to {}: {}", target, e),
            }
        }
    }
    Ok(())
}

// --- SOCKS5 Handler with pre-read buffer ---
async fn handle_socks5_with_buffer<S>(buffer: &[u8], mut stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // For SOCKS5, if we have a pre-read buffer, we need to handle the case where
    // the entire handshake might be in the buffer already
    
    // 1. Parse handshake from buffer
    if buffer.len() < 3 { // Need at least version + nmethods + 1 method
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Incomplete SOCKS5 handshake"));
    }
    
    let version = buffer[0];
    let nmethods = buffer[1] as usize;
    
    if version != 5 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported SOCKS version"));
    }
    
    if buffer.len() < 2 + nmethods {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Incomplete SOCKS5 handshake"));
    }
    
    let methods = &buffer[2..2 + nmethods];
    if !methods.contains(&0) {
        stream.write_all(&[5, 0xFF]).await?; // No acceptable methods
        return Err(io::Error::new(io::ErrorKind::InvalidData, "No supported authentication methods"));
    }
    
    // Send handshake response
    stream.write_all(&[5, 0]).await?;
    
    // 2. Now wait for the actual CONNECT request from the stream
    // (the initial buffer only contained the handshake)
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).await?;
    
    if buf[0] != 5 || buf[1] != 1 { // VER, CONNECT
        stream.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]).await?; // Command not supported
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported SOCKS command"));
    }

    // 3. Address Parsing
    let atyp = buf[3];
    let target = read_socks_address(&mut stream, atyp).await?;

    // 4. Connect and Relay
    match connect_to_target(&target).await {
        Ok(remote) => {
            info!("SOCKS5 CONNECT to {}", target);
            let local_addr = remote.local_addr().unwrap_or(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0));
            let mut resp = vec![5, 0, 0]; // VER, REP(Success), RSV
            match local_addr {
                SocketAddr::V4(addr) => {
                    resp.push(1); resp.extend_from_slice(&addr.ip().octets()); resp.extend_from_slice(&addr.port().to_be_bytes());
                }
                SocketAddr::V6(addr) => {
                    resp.push(4); resp.extend_from_slice(&addr.ip().octets()); resp.extend_from_slice(&addr.port().to_be_bytes());
                }
            }
            stream.write_all(&resp).await?;
            relay_streams(stream, remote).await?;
        }
        Err(e) => {
            error!("SOCKS5: Failed to connect to {}: {}", target, e);
            stream.write_all(&[5, 1, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            return Err(e);
        }
    }
    Ok(())
}

// --- SOCKS5 Handler ---
async fn handle_socks5<S>(mut stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // 1. Handshake
    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).await?;
    if buf[0] != 5 { return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported SOCKS version")); }
    let nmethods = buf[1] as usize;
    let mut methods = vec![0u8; nmethods];
    stream.read_exact(&mut methods).await?;

    if !methods.contains(&0) {
        stream.write_all(&[5, 0xFF]).await?; // No acceptable methods
        return Err(io::Error::new(io::ErrorKind::InvalidData, "No supported authentication methods"));
    }
    stream.write_all(&[5, 0]).await?;

    // 2. Request
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).await?;
    if buf[0] != 5 || buf[1] != 1 { // VER, CONNECT
        stream.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]).await?; // Command not supported
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported SOCKS command"));
    }

    // 3. Address Parsing
    let atyp = buf[3];
    let target = read_socks_address(&mut stream, atyp).await?;

    // 4. Connect and Relay
    match connect_to_target(&target).await {
        Ok(remote) => {
            info!("SOCKS5 CONNECT to {}", target);
            let local_addr = remote.local_addr().unwrap_or(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0));
            let mut resp = vec![5, 0, 0]; // VER, REP(Success), RSV
            match local_addr {
                SocketAddr::V4(addr) => {
                    resp.push(1); resp.extend_from_slice(&addr.ip().octets()); resp.extend_from_slice(&addr.port().to_be_bytes());
                }
                SocketAddr::V6(addr) => {
                    resp.push(4); resp.extend_from_slice(&addr.ip().octets()); resp.extend_from_slice(&addr.port().to_be_bytes());
                }
            }
            stream.write_all(&resp).await?;
            relay_streams(stream, remote).await?;
        }
        Err(e) => {
            error!("SOCKS5: Failed to connect to {}: {}", target, e);
            stream.write_all(&[5, 1, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            return Err(e);
        }
    }
    Ok(())
}

async fn read_socks_address<S>(stream: &mut S, atyp: u8) -> io::Result<String>
where
    S: AsyncRead + Unpin,
{
    match atyp {
        1 => { // IPv4
            let mut buf = [0u8; 6];
            stream.read_exact(&mut buf).await?;
            let ip = Ipv4Addr::new(buf[0], buf[1], buf[2], buf[3]);
            let port = u16::from_be_bytes([buf[4], buf[5]]);
            Ok(format!("{}:{}", ip, port))
        }
        3 => { // Domain Name
            let mut len_buf = [0u8; 1];
            stream.read_exact(&mut len_buf).await?;
            let len = len_buf[0] as usize;
            let mut domain_buf = vec![0u8; len];
            stream.read_exact(&mut domain_buf).await?;
            let mut port_buf = [0u8; 2];
            stream.read_exact(&mut port_buf).await?;
            let domain = String::from_utf8_lossy(&domain_buf);
            let port = u16::from_be_bytes(port_buf);
            Ok(format!("{}:{}", domain, port))
        }
        4 => { // IPv6
            let mut buf = [0u8; 18];
            stream.read_exact(&mut buf).await?;
            let ip = std::net::Ipv6Addr::from([
                buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
                buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
            ]);
            let port = u16::from_be_bytes([buf[16], buf[17]]);
            Ok(format!("[{}]:{}", ip, port))
        }
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "Unsupported address type")),
    }
}

/// Gets the local IP address for binding
fn get_bind_address() -> IpAddr {
    // Check for Galaxy Note20 5G specific configuration
    if let Ok(device) = std::fs::read_to_string("/sys/devices/soc0/machine") {
        if device.contains("SM-N981") || device.contains("SM-N986") {
            info!("Detected Galaxy Note20 5G");
            let config = Note20NetworkConfig::default();
            if let Ok(addr) = get_optimal_bind_address(&config) {
                if let Ok(ip) = addr.parse::<IpAddr>() {
                    info!("Using Note20 5G optimized address: {}", ip);
                    let _ = configure_5g_proxy(&addr);
                    return ip;
                }
            }
        }
    }
    
    // Check environment variables for Termux-specific configuration
    if let Ok(bind_ip_str) = std::env::var("BIND_IP") {
        if let Ok(ip) = bind_ip_str.parse::<IpAddr>() {
            return ip;
        }
    }
    
    // Default to all interfaces for Termux
    IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    info!("🔥 LiteBike Proxy for Termux 🔥");
    
    let bind_ip = get_bind_address();
    info!("Binding to: {}", bind_ip);
    
    // Start auto-discovery services (PAC, WPAD, Bonjour, UPnP)
    if let IpAddr::V4(ipv4) = bind_ip {
        if !ipv4.is_loopback() {
            let hostname = hostname::get()
                .ok()
                .and_then(|h| h.to_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "litebike".to_string());
            
            let auto_discovery = AutoDiscovery::new(ipv4, hostname);
            if let Err(e) = auto_discovery.start().await {
                warn!("Auto-discovery failed to start: {}", e);
            }
        }
    }

    // Start Universal proxy (HTTP + SOCKS5 detection on 8080)
    let universal_listener = TcpListener::bind(SocketAddr::new(bind_ip, HTTP_PORT))
        .await
        .expect("Failed to bind universal port");
    info!("Universal proxy (HTTP+SOCKS5) listening on {}", universal_listener.local_addr().unwrap_or_else(|_| SocketAddr::new(bind_ip, HTTP_PORT)));
    
    tokio::spawn(async move {
        loop {
            if let Ok((stream, addr)) = universal_listener.accept().await {
                debug!("Universal connection from {}", addr);
                tokio::spawn(async move {
                    if let Err(e) = handle_universal(stream).await {
                        debug!("Universal handler error: {}", e);
                    }
                });
            }
        }
    });

    // Start SOCKS5 proxy
    let socks_listener = TcpListener::bind(SocketAddr::new(bind_ip, SOCKS_PORT))
        .await
        .expect("Failed to bind SOCKS5 port");
    info!("SOCKS5 proxy listening on {}", socks_listener.local_addr().unwrap_or_else(|_| SocketAddr::new(bind_ip, SOCKS_PORT)));
    
    info!("✅ LiteBike Termux proxy ready!");
    info!("  Universal proxy (HTTP/HTTPS/SOCKS5): {}:{}", bind_ip, HTTP_PORT);
    info!("  Dedicated SOCKS5 proxy: {}:{}", bind_ip, SOCKS_PORT);
    
    loop {
        if let Ok((stream, addr)) = socks_listener.accept().await {
            debug!("SOCKS5 connection from {}", addr);
            tokio::spawn(async move {
                if let Err(e) = handle_socks5(stream).await {
                    debug!("SOCKS5 handler error: {}", e);
                }
            });
        }
    }
}