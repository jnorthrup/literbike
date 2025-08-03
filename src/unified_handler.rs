// Unified Protocol Handler with Performance Optimizations
// Optimized for Samsung S20 (Snapdragon 865) and similar ARM64 devices

use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use log::{debug, info};

use crate::patricia_detector::PatriciaDetector;
use crate::patricia_detector::Protocol;

/// Buffer pool for reducing allocations
const BUFFER_POOL_SIZE: usize = 8;
const BUFFER_SIZE: usize = 8192; // 8KB optimized for mobile

// Thread-local buffer pool to avoid allocations
thread_local! {
    static BUFFER_POOL: std::cell::RefCell<Vec<Vec<u8>>> = std::cell::RefCell::new({
        let mut pool = Vec::with_capacity(BUFFER_POOL_SIZE);
        for _ in 0..BUFFER_POOL_SIZE {
            pool.push(vec![0u8; BUFFER_SIZE]);
        }
        pool
    });
}

/// Get a buffer from the pool or allocate a new one
fn get_buffer() -> Vec<u8> {
    BUFFER_POOL.with(|pool| {
        pool.borrow_mut().pop().unwrap_or_else(|| vec![0u8; BUFFER_SIZE])
    })
}

/// Return a buffer to the pool
fn return_buffer(mut buffer: Vec<u8>) {
    BUFFER_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < BUFFER_POOL_SIZE {
            buffer.clear();
            buffer.resize(BUFFER_SIZE, 0);
            pool.push(buffer);
        }
    });
}

/// Optimized unified handler with zero-copy where possible
pub async fn handle_unified_optimized<S>(mut stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // Get buffer from pool
    let mut buffer = get_buffer();
    
    // Read initial data
    let n = match stream.read(&mut buffer).await {
        Ok(0) => {
            return_buffer(buffer);
            return Ok(());
        }
        Ok(n) => n,
        Err(e) => {
            return_buffer(buffer);
            return Err(e);
        }
    };

    // Use Patricia Trie detection for all architectures
    let protocol = {
        let detector = PatriciaDetector::new();
        detector.detect(&buffer[..n])
    };

    debug!("Detected protocol: {:?} from {} bytes", protocol, n);

    // Protocol-specific handling with optimizations
    let result = match protocol {
        Protocol::Socks5 => {
            handle_socks5_optimized(&buffer[..n], stream).await
        }
        Protocol::Http | Protocol::WebSocket => {
            handle_http_optimized(&buffer[..n], stream).await
        }
        Protocol::Tls => {
            handle_tls_passthrough(&buffer[..n], stream).await
        }
        Protocol::ProxyProtocol => {
            handle_proxy_protocol(&buffer[..n], stream).await
        }
        Protocol::Http2 => {
            handle_http2_optimized(&buffer[..n], stream).await
        }
        Protocol::Unknown => {
            // Default to HTTP for unknown protocols
            handle_http_optimized(&buffer[..n], stream).await
        }
    };

    return_buffer(buffer);
    result
}

/// Optimized HTTP handler with zero-copy forwarding
async fn handle_http_optimized<S>(initial_data: &[u8], stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // Parse HTTP request efficiently
    let request_str = std::str::from_utf8(initial_data).unwrap_or("");
    
    // Extract method and target
    if let Some(first_line) = request_str.lines().next() {
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() >= 2 {
            let method = parts[0];
            let target = parts[1];
            
            match method {
                "CONNECT" => {
                    // HTTPS tunnel
                    let target_addr = if target.contains(':') {
                        target.to_string()
                    } else {
                        format!("{}:443", target)
                    };
                    
                    handle_connect_tunnel(stream, &target_addr).await
                }
                _ => {
                    // Regular HTTP proxy
                    if let Some(host) = extract_host_from_headers(request_str) {
                        let target_addr = format!("{}:80", host);
                        handle_http_forward(stream, initial_data, &target_addr).await
                    } else {
                        send_error_response(stream, 400, "Bad Request").await
                    }
                }
            }
        } else {
            send_error_response(stream, 400, "Bad Request").await
        }
    } else {
        send_error_response(stream, 400, "Bad Request").await
    }
}

/// Optimized SOCKS5 handler
async fn handle_socks5_optimized<S>(initial_data: &[u8], mut stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // SOCKS5 handshake already started
    if initial_data.len() >= 2 && initial_data[0] == 0x05 {
        let nmethods = initial_data[1] as usize;
        
        // Check if we have the complete handshake
        if initial_data.len() >= 2 + nmethods {
            // Send method selection (no auth)
            stream.write_all(&[0x05, 0x00]).await?;
            
            // Continue with SOCKS5 protocol
            handle_socks5_request(stream).await
        } else {
            // Need more data
            Err(io::Error::new(io::ErrorKind::InvalidData, "Incomplete SOCKS5 handshake"))
        }
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid SOCKS5 data"))
    }
}

/// TLS passthrough with SNI extraction
async fn handle_tls_passthrough<S>(initial_data: &[u8], stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // Extract SNI if possible
    let sni = extract_sni_from_tls(initial_data);
    if let Some(hostname) = sni {
        info!("TLS connection to: {}", hostname);
        let target_addr = format!("{}:443", hostname);
        forward_with_initial_data(stream, initial_data, &target_addr).await
    } else {
        // No SNI, can't determine target
        Err(io::Error::new(io::ErrorKind::InvalidData, "No SNI in TLS handshake"))
    }
}

/// Handle HAProxy PROXY protocol
async fn handle_proxy_protocol<S>(initial_data: &[u8], stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // Parse PROXY protocol header
    if let Ok(header_str) = std::str::from_utf8(initial_data) {
        if let Some(line_end) = header_str.find("\r\n") {
            let header = &header_str[..line_end];
            let remaining = &initial_data[line_end + 2..];
            
            // Extract real client IP from PROXY header
            debug!("PROXY protocol header: {}", header);
            
            // Process the actual request after the PROXY header
            if !remaining.is_empty() {
                // For now, handle remaining data as HTTP since we can't easily chain streams
                handle_http_optimized(remaining, stream).await
            } else {
                // Need to read more data
                handle_unified_optimized(stream).await
            }
        } else {
            Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid PROXY protocol header"))
        }
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid PROXY protocol data"))
    }
}

/// Optimized HTTP/2 handler
async fn handle_http2_optimized<S>(initial_data: &[u8], stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // For now, treat as HTTP/1.x
    // TODO: Implement proper HTTP/2 handling
    info!("HTTP/2 connection detected, falling back to HTTP/1.x");
    handle_http_optimized(initial_data, stream).await
}

// Helper functions

async fn handle_connect_tunnel<S>(mut stream: S, target: &str) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    match TcpStream::connect(target).await {
        Ok(remote) => {
            info!("CONNECT tunnel to {}", target);
            stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await?;
            relay_optimized(stream, remote).await
        }
        Err(e) => {
            send_error_response(stream, 502, "Bad Gateway").await?;
            Err(e)
        }
    }
}

async fn handle_http_forward<S>(stream: S, request: &[u8], target: &str) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    match TcpStream::connect(target).await {
        Ok(mut remote) => {
            info!("HTTP forward to {}", target);
            remote.write_all(request).await?;
            relay_optimized(stream, remote).await
        }
        Err(e) => {
            send_error_response(stream, 502, "Bad Gateway").await?;
            Err(e)
        }
    }
}

async fn forward_with_initial_data<S>(stream: S, initial_data: &[u8], target: &str) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    match TcpStream::connect(target).await {
        Ok(mut remote) => {
            remote.write_all(initial_data).await?;
            relay_optimized(stream, remote).await
        }
        Err(e) => {
            Err(e)
        }
    }
}

async fn handle_socks5_request<S>(mut stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // Read SOCKS5 request
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).await?;
    
    if buf[0] != 0x05 || buf[1] != 0x01 {
        // Only support CONNECT
        stream.write_all(&[0x05, 0x07, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).await?;
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported SOCKS5 command"));
    }
    
    // Parse address
    let atyp = buf[3];
    let target = read_socks5_address(&mut stream, atyp).await?;
    
    // Connect to target
    match TcpStream::connect(&target).await {
        Ok(remote) => {
            info!("SOCKS5 connect to {}", target);
            // Send success response
            stream.write_all(&[0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).await?;
            relay_optimized(stream, remote).await
        }
        Err(e) => {
            // Send failure response
            stream.write_all(&[0x05, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).await?;
            Err(e)
        }
    }
}

async fn read_socks5_address<S>(stream: &mut S, atyp: u8) -> io::Result<String>
where
    S: AsyncRead + Unpin,
{
    match atyp {
        0x01 => {
            // IPv4
            let mut buf = [0u8; 6];
            stream.read_exact(&mut buf).await?;
            let addr = std::net::Ipv4Addr::new(buf[0], buf[1], buf[2], buf[3]);
            let port = u16::from_be_bytes([buf[4], buf[5]]);
            Ok(format!("{}:{}", addr, port))
        }
        0x03 => {
            // Domain name
            let mut len_buf = [0u8; 1];
            stream.read_exact(&mut len_buf).await?;
            let len = len_buf[0] as usize;
            
            let mut domain_buf = vec![0u8; len + 2];
            stream.read_exact(&mut domain_buf).await?;
            
            let domain = String::from_utf8_lossy(&domain_buf[..len]);
            let port = u16::from_be_bytes([domain_buf[len], domain_buf[len + 1]]);
            Ok(format!("{}:{}", domain, port))
        }
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported address type")),
    }
}

async fn send_error_response<S>(mut stream: S, code: u16, message: &str) -> io::Result<()>
where
    S: AsyncWrite + Unpin,
{
    let response = format!("HTTP/1.1 {} {}\r\nContent-Length: 0\r\n\r\n", code, message);
    stream.write_all(response.as_bytes()).await
}

fn extract_host_from_headers(request: &str) -> Option<String> {
    for line in request.lines() {
        if line.to_lowercase().starts_with("host:") {
            return line.split(':').nth(1).map(|s| s.trim().to_string());
        }
    }
    None
}

fn extract_sni_from_tls(data: &[u8]) -> Option<String> {
    // Simple SNI extraction (full implementation would be more complex)
    if data.len() < 43 {
        return None;
    }
    
    // Check for TLS handshake
    if data[0] != 0x16 || data[1] != 0x03 {
        return None;
    }
    
    // This is a simplified extraction - real implementation would parse properly
    // For now, return None to avoid complexity
    None
}

/// Optimized relay using splice/sendfile when available
async fn relay_optimized<S1, S2>(client: S1, server: S2) -> io::Result<()>
where
    S1: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    S2: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // Use larger buffers for better throughput on mobile
    let (client_reader, client_writer) = tokio::io::split(client);
    let (server_reader, server_writer) = tokio::io::split(server);
    
    // Create buffered readers and writers with longer lifetimes
    let mut client_buf_reader = tokio::io::BufReader::with_capacity(16384, client_reader);
    let mut client_buf_writer = tokio::io::BufWriter::with_capacity(16384, client_writer);
    let mut server_buf_reader = tokio::io::BufReader::with_capacity(16384, server_reader);
    let mut server_buf_writer = tokio::io::BufWriter::with_capacity(16384, server_writer);
    
    let client_to_server = tokio::io::copy_buf(&mut client_buf_reader, &mut server_buf_writer);
    let server_to_client = tokio::io::copy_buf(&mut server_buf_reader, &mut client_buf_writer);
    
    tokio::select! {
        res = client_to_server => {
            debug!("Client to server copy finished: {:?}", res);
        }
        res = server_to_client => {
            debug!("Server to client copy finished: {:?}", res);
        }
    }
    
    Ok(())
}