// LiteBike Proxy - Simplified Universal Protocol Detection Proxy
// Copyright (c) 2025 jnorthrup
// 
// Licensed under AGPL-3.0-or-later
// Simple proxy implementation for testing basic functionality

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str;
use std::time::Duration;
use std::io;

use env_logger::Env;
use log::{debug, error, info, warn};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

mod types;
mod simple_routing;

// --- Configuration ---
const HTTP_PORT: u16 = 8080;
const SOCKS_PORT: u16 = 1080;
const UNIVERSAL_PORT: u16 = 8888;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Connects to a target address
async fn connect_to_target(target: &str) -> io::Result<TcpStream> {
    let (host, port_str) = match target.rsplit_once(':') {
        Some((host, port)) => (host, port),
        None => return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid target format, missing port")),
    };
    
    let port: u16 = port_str.parse().map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid port number"))?;

    // Try parsing as IP first
    let ip_addr = if let Ok(ip) = host.parse::<IpAddr>() {
        ip
    } else {
        // Fallback to system DNS
        let addresses: Vec<SocketAddr> = tokio::net::lookup_host(format!("{}:80", host))
            .await?
            .collect();
        
        addresses.first()
            .map(|addr| addr.ip())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No IP address found"))?
    };

    let socket_addr = SocketAddr::new(ip_addr, port);

    debug!("Connecting to {} -> {}", target, socket_addr);
    match timeout(CONNECT_TIMEOUT, TcpStream::connect(socket_addr)).await {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(io::Error::new(io::ErrorKind::TimedOut, "Connection timed out")),
    }
}

/// Relays data between two streams efficiently.
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

// --- HTTP Handler ---
async fn handle_http<S>(mut stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut buffer = [0u8; 4096];
    let n = stream.read(&mut buffer).await?;
    if n == 0 { return Ok(()); }

    let request_str = str::from_utf8(&buffer[..n]).unwrap_or("");
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
                    remote.write_all(&buffer[..n]).await?;
                    relay_streams(stream, remote).await?;
                }
                Err(e) => error!("Failed to connect to {}: {}", target, e),
            }
        }
    }
    Ok(())
}

// --- SOCKS5 Handler ---
async fn handle_socks5<S>(mut stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
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

/// Simple universal connection handler with protocol detection
async fn handle_universal_connection(mut stream: TcpStream) -> io::Result<()> {
    // Peek at the first few bytes to detect protocol
    let mut buffer = [0u8; 64];
    let n = stream.peek(&mut buffer).await?;
    
    if n == 0 {
        return Ok(());
    }
    
    let data = &buffer[..n];
    
    // Simple protocol detection chain
    if data.len() >= 2 && data[0] == 0x05 {
        // SOCKS5 detected
        debug!("Universal port: SOCKS5 detected");
        handle_socks5(stream).await
    } else if let Ok(text) = std::str::from_utf8(data) {
        // Text-based protocols
        if text.starts_with("GET ") || text.starts_with("POST ") || 
           text.starts_with("PUT ") || text.starts_with("DELETE ") ||
           text.starts_with("HEAD ") || text.starts_with("OPTIONS ") ||
           text.starts_with("CONNECT ") || text.starts_with("PATCH ") {
            
            debug!("Universal port: HTTP detected");
            
            // Check for special HTTP-based protocols
            if text.contains("/proxy.pac") {
                debug!("Universal port: PAC request detected (serving simple response)");
                serve_simple_pac(stream).await
            } else if text.contains("/wpad.dat") {
                debug!("Universal port: WPAD request detected (serving simple response)");
                serve_simple_pac(stream).await
            } else {
                // Regular HTTP request
                handle_http(stream).await
            }
        } else {
            // Unknown text-based protocol, try HTTP
            debug!("Universal port: Unknown text protocol, attempting HTTP");
            handle_http(stream).await
        }
    } else if data.len() >= 3 && data[0] == 0x16 && data[1] == 0x03 {
        // TLS handshake detected
        debug!("Universal port: TLS detected - connection will be closed");
        // For now, just close TLS connections as we can't proxy them without termination
        Ok(())
    } else {
        // Unknown binary protocol
        debug!("Universal port: Unknown binary protocol");
        Err(io::Error::new(io::ErrorKind::InvalidData, "Unknown protocol"))
    }
}

/// Serve a simple PAC/WPAD response
async fn serve_simple_pac(mut stream: TcpStream) -> io::Result<()> {
    let pac_content = r#"function FindProxyForURL(url, host) {
    // Simple PAC configuration
    if (host == "localhost" || 
        host == "127.0.0.1" || 
        isInNet(host, "127.0.0.0", "255.0.0.0")) {
        return "DIRECT";
    }
    
    // Private network bypass
    if (isInNet(host, "10.0.0.0", "255.0.0.0") ||
        isInNet(host, "172.16.0.0", "255.240.0.0") ||
        isInNet(host, "192.168.0.0", "255.255.0.0")) {
        return "DIRECT";
    }
    
    // Default proxy chain with fallback
    return "PROXY 127.0.0.1:8080; SOCKS5 127.0.0.1:1080; DIRECT";
}"#;

    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: application/x-ns-proxy-autoconfig\r\n\
         Content-Length: {}\r\n\
         Cache-Control: max-age=3600\r\n\
         \r\n\
         {}",
        pac_content.len(),
        pac_content
    );
    
    info!("Serving simple PAC file to client");
    stream.write_all(response.as_bytes()).await
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let bind_ip: IpAddr = std::env::var("BIND_IP")
        .unwrap_or_else(|_| "0.0.0.0".to_string())
        .parse()
        .expect("Invalid BIND_IP address");

    // Start HTTP proxy listener
    let http_listener = TcpListener::bind(SocketAddr::new(bind_ip, HTTP_PORT)).await.expect("Failed to bind HTTP");
    info!("HTTP proxy listening on {}", http_listener.local_addr().unwrap_or_else(|_| SocketAddr::new(bind_ip, HTTP_PORT)));
    tokio::spawn(async move {
        loop {
            if let Ok((stream, _)) = http_listener.accept().await {
                tokio::spawn(async move {
                    if let Err(e) = handle_http(stream).await {
                        debug!("HTTP handler error: {}", e);
                    }
                });
            }
        }
    });

    // Start SOCKS5 proxy listener
    let socks_listener = TcpListener::bind(SocketAddr::new(bind_ip, SOCKS_PORT)).await.expect("Failed to bind SOCKS5");
    info!("SOCKS5 proxy listening on {}", socks_listener.local_addr().unwrap_or_else(|_| SocketAddr::new(bind_ip, SOCKS_PORT)));
    tokio::spawn(async move {
        loop {
            if let Ok((stream, _)) = socks_listener.accept().await {
                tokio::spawn(async move {
                    if let Err(e) = handle_socks5(stream).await {
                        debug!("SOCKS5 handler error: {}", e);
                    }
                });
            }
        }
    });

    // Universal listener using simple routing with swlan0 fallback
    let router = simple_routing::SimpleRouter::new();
    let (universal_listener, active_config) = router.bind_with_fallback().await.expect("Failed to bind universal listener");
    
    info!("Universal proxy listening on {} (interface: {}) with protocols: {:?}", 
          active_config.socket_addr(), 
          active_config.interface,
          simple_routing::get_supported_protocols(&active_config));
    
    if active_config.interface == "lo" {
        warn!("Using fallback configuration - primary interface 'swlan0' was not available");
    }
    
    loop {
        if let Ok((stream, addr)) = universal_listener.accept().await {
            debug!("New connection from {}", addr);
            tokio::spawn(async move {
                // Simple protocol detection and routing
                if let Err(e) = handle_universal_connection(stream).await {
                    debug!("Universal handler error from {}: {}", addr, e);
                }
            });
        }
    }
}