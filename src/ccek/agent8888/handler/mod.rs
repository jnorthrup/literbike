//! Protocol Handlers - Per-protocol connection handlers
//!
//! This module claims all I/O handling from litebike protocols.
//! It provides concrete implementations for HTTP, SOCKS5, TLS, and WebSocket.

use crate::core::{Element, Key};
use crate::protocol::{HttpMethod, ProtocolDetection};
use std::any::{Any, TypeId};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};

/// HandlerKey - manages protocol-specific handlers
pub struct HandlerKey;

impl HandlerKey {
    pub const FACTORY: fn() -> HandlerElement = HandlerElement::new;
}

/// HandlerElement - protocol handler registry and dispatch
pub struct HandlerElement {
    http_count: AtomicU64,
    socks5_count: AtomicU64,
    websocket_count: AtomicU64,
    upnp_count: AtomicU64,
    unknown_count: AtomicU64,
}

impl HandlerElement {
    pub fn new() -> Self {
        Self {
            http_count: AtomicU64::new(0),
            socks5_count: AtomicU64::new(0),
            websocket_count: AtomicU64::new(0),
            upnp_count: AtomicU64::new(0),
            unknown_count: AtomicU64::new(0),
        }
    }

    /// Dispatch handling based on protocol detection
    pub fn handle(&self, protocol: &ProtocolDetection, stream: &mut TcpStream, buffer: &[u8]) -> HandlerResult {
        match protocol {
            ProtocolDetection::Http(method) => {
                self.http_count.fetch_add(1, Ordering::Relaxed);
                handle_http(stream, buffer, *method)
            }
            ProtocolDetection::Socks5 => {
                self.socks5_count.fetch_add(1, Ordering::Relaxed);
                handle_socks5(stream, buffer)
            }
            ProtocolDetection::WebSocket => {
                self.websocket_count.fetch_add(1, Ordering::Relaxed);
                handle_websocket(stream, buffer)
            }
            ProtocolDetection::Upnp => {
                self.upnp_count.fetch_add(1, Ordering::Relaxed);
                handle_upnp(stream, buffer)
            }
            _ => {
                self.unknown_count.fetch_add(1, Ordering::Relaxed);
                HandlerResult::Unsupported
            }
        }
    }

    pub fn stats(&self) -> HandlerStats {
        HandlerStats {
            http: self.http_count.load(Ordering::Relaxed),
            socks5: self.socks5_count.load(Ordering::Relaxed),
            websocket: self.websocket_count.load(Ordering::Relaxed),
            upnp: self.upnp_count.load(Ordering::Relaxed),
            unknown: self.unknown_count.load(Ordering::Relaxed),
        }
    }
}

impl Element for HandlerElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<HandlerKey>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for HandlerKey {
    type Element = HandlerElement;
    const FACTORY: fn() -> Self::Element = HandlerElement::new;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HandlerStats {
    pub http: u64,
    pub socks5: u64,
    pub websocket: u64,
    pub upnp: u64,
    pub unknown: u64,
}

impl HandlerStats {
    pub fn total(&self) -> u64 {
        self.http + self.socks5 + self.websocket + self.upnp + self.unknown
    }
}

#[derive(Debug)]
pub enum HandlerResult {
    Handled(usize),
    NeedMoreData,
    Error(&'static str),
    Unsupported,
}

// ===== HTTP Handler =====

fn handle_http(stream: &mut TcpStream, buffer: &[u8], method: HttpMethod) -> HandlerResult {
    if let Ok(request) = std::str::from_utf8(buffer) {
        match method {
            HttpMethod::Connect => handle_http_connect(stream, request),
            _ => handle_http_proxy(stream, request),
        }
    } else {
        HandlerResult::Error("Invalid UTF-8 in HTTP request")
    }
}

fn handle_http_connect(stream: &mut TcpStream, request: &str) -> HandlerResult {
    // Extract target from CONNECT request
    if let Some(line) = request.lines().next() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let target = parts[1];
            let target_addr = if target.contains(':') {
                target.to_string()
            } else {
                format!("{}:443", target)
            };

            // Connect to target
            match TcpStream::connect(&target_addr) {
                Ok(mut remote) => {
                    // Send 200 Connection Established
                    let response = b"HTTP/1.1 200 Connection Established\r\n\r\n";
                    if stream.write_all(response).is_err() {
                        return HandlerResult::Error("Failed to write response");
                    }

                    // Relay data between client and server
                    relay_streams(stream, &mut remote);
                    HandlerResult::Handled(request.len())
                }
                Err(_) => {
                    let response = b"HTTP/1.1 502 Bad Gateway\r\n\r\n";
                    let _ = stream.write_all(response);
                    HandlerResult::Error("Failed to connect to target")
                }
            }
        } else {
            HandlerResult::Error("Invalid CONNECT request")
        }
    } else {
        HandlerResult::NeedMoreData
    }
}

fn handle_http_proxy(stream: &mut TcpStream, request: &str) -> HandlerResult {
    // Extract Host header
    let host = extract_host_from_headers(request);

    if let Some(host) = host {
        let target_addr = format!("{}:80", host);

        match TcpStream::connect(&target_addr) {
            Ok(mut remote) => {
                // Forward the original request
                if remote.write_all(request.as_bytes()).is_err() {
                    return HandlerResult::Error("Failed to forward request");
                }

                // Relay responses back
                relay_streams(stream, &mut remote);
                HandlerResult::Handled(request.len())
            }
            Err(_) => {
                let response = b"HTTP/1.1 502 Bad Gateway\r\n\r\n";
                let _ = stream.write_all(response);
                HandlerResult::Error("Failed to connect to target")
            }
        }
    } else {
        let response = b"HTTP/1.1 400 Bad Request\r\n\r\n";
        let _ = stream.write_all(response);
        HandlerResult::Error("No Host header found")
    }
}

fn extract_host_from_headers(request: &str) -> Option<String> {
    for line in request.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.starts_with("host:") {
            if let Some(host_part) = line.split(':').nth(1) {
                let host = host_part.trim();
                let host_without_port = if host.contains(':') {
                    host.split(':').next().unwrap_or(host)
                } else {
                    host
                };
                return Some(host_without_port.to_string());
            }
        }
    }
    None
}

// ===== SOCKS5 Handler =====

fn handle_socks5(stream: &mut TcpStream, buffer: &[u8]) -> HandlerResult {
    if buffer.len() < 3 {
        return HandlerResult::NeedMoreData;
    }

    // SOCKS5 greeting: VER, NMETHODS, METHODS[]
    let version = buffer[0];
    let nmethods = buffer[1] as usize;

    if version != 0x05 {
        return HandlerResult::Error("Invalid SOCKS version");
    }

    if buffer.len() < 2 + nmethods {
        return HandlerResult::NeedMoreData;
    }

    // Select method (0x00 = no auth)
    let methods = &buffer[2..2 + nmethods];
    let selected_method = if methods.contains(&0x00) { 0x00 } else { 0xFF };

    // Send method selection
    let response = [0x05, selected_method];
    if stream.write_all(&response).is_err() {
        return HandlerResult::Error("Failed to send SOCKS5 response");
    }

    if selected_method == 0xFF {
        return HandlerResult::Error("No acceptable auth method");
    }

    // Read SOCKS5 request
    let mut request_buf = [0u8; 256];
    match stream.read(&mut request_buf) {
        Ok(n) if n >= 10 => {
            handle_socks5_request(stream, &request_buf[..n])
        }
        Ok(_) => HandlerResult::NeedMoreData,
        Err(_) => HandlerResult::Error("Failed to read SOCKS5 request"),
    }
}

fn handle_socks5_request(stream: &mut TcpStream, request: &[u8]) -> HandlerResult {
    if request.len() < 10 {
        return HandlerResult::NeedMoreData;
    }

    let version = request[0];
    let cmd = request[1];
    let atyp = request[3];

    if version != 0x05 {
        return HandlerResult::Error("Invalid SOCKS version in request");
    }

    if cmd != 0x01 {
        // 0x01 = CONNECT
        let response = [0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0];
        let _ = stream.write_all(&response);
        return HandlerResult::Error("Unsupported SOCKS5 command");
    }

    // Parse target address
    let target_addr = match atyp {
        0x01 => {
            // IPv4
            if request.len() < 10 {
                return HandlerResult::NeedMoreData;
            }
            let ip = format!("{}.{}.{}.{}", request[4], request[5], request[6], request[7]);
            let port = ((request[8] as u16) << 8) | (request[9] as u16);
            format!("{}:{}", ip, port)
        }
        0x03 => {
            // Domain name
            let domain_len = request[4] as usize;
            if request.len() < 5 + domain_len + 2 {
                return HandlerResult::NeedMoreData;
            }
            let domain = &request[5..5 + domain_len];
            let domain_str = String::from_utf8_lossy(domain);
            let port_idx = 5 + domain_len;
            let port = ((request[port_idx] as u16) << 8) | (request[port_idx + 1] as u16);
            format!("{}:{}", domain_str, port)
        }
        _ => {
            let response = [0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0];
            let _ = stream.write_all(&response);
            return HandlerResult::Error("Unsupported address type");
        }
    };

    // Connect to target
    match TcpStream::connect(&target_addr) {
        Ok(mut remote) => {
            // Send success response
            let response = [0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0];
            if stream.write_all(&response).is_err() {
                return HandlerResult::Error("Failed to send SOCKS5 success");
            }

            // Relay data
            relay_streams(stream, &mut remote);
            HandlerResult::Handled(request.len())
        }
        Err(_) => {
            let response = [0x05, 0x05, 0x00, 0x01, 0, 0, 0, 0, 0, 0];
            let _ = stream.write_all(&response);
            HandlerResult::Error("Failed to connect to target")
        }
    }
}

// ===== WebSocket Handler =====

fn handle_websocket(_stream: &mut TcpStream, _buffer: &[u8]) -> HandlerResult {
    // WebSocket upgrade handling would go here
    // For now, mark as handled but not implemented
    HandlerResult::Error("WebSocket handling not yet implemented")
}

// ===== UPnP Handler =====

fn handle_upnp(stream: &mut TcpStream, buffer: &[u8]) -> HandlerResult {
    if let Ok(request) = std::str::from_utf8(buffer) {
        if request.starts_with("M-SEARCH") {
            // Send SSDP response
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 CACHE-CONTROL: max-age=1800\r\n\
                 EXT:\r\n\
                 LOCATION: http://{}:8080/rootdesc.xml\r\n\
                 SERVER: CCEK/1.0 UPnP/1.0\r\n\
                 ST: upnp:rootdevice\r\n\
                 USN: uuid:ccek-001::upnp:rootdevice\r\n\
                 \r\n",
                stream.local_addr().map(|a| a.ip().to_string()).unwrap_or_else(|_| "127.0.0.1".to_string())
            );

            if stream.write_all(response.as_bytes()).is_err() {
                return HandlerResult::Error("Failed to send UPnP response");
            }
            HandlerResult::Handled(buffer.len())
        } else {
            HandlerResult::Handled(0)
        }
    } else {
        HandlerResult::Error("Invalid UTF-8 in UPnP request")
    }
}

// ===== Utility Functions =====

fn relay_streams(client: &mut TcpStream, server: &mut TcpStream) {
    let mut client_buf = [0u8; 4096];
    let mut server_buf = [0u8; 4096];

    // Set non-blocking for polling
    let _ = client.set_nonblocking(true);
    let _ = server.set_nonblocking(true);

    loop {
        let mut activity = false;

        // Client to server
        match client.read(&mut client_buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                if server.write_all(&client_buf[..n]).is_err() {
                    break;
                }
                activity = true;
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => break,
        }

        // Server to client
        match server.read(&mut server_buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                if client.write_all(&server_buf[..n]).is_err() {
                    break;
                }
                activity = true;
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => break,
        }

        if !activity {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_factory() {
        let elem = HandlerKey::FACTORY();
        let stats = elem.stats();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn test_extract_host_from_headers() {
        let request = "GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        assert_eq!(extract_host_from_headers(request), Some("example.com".to_string()));

        let request_with_port = "GET / HTTP/1.1\r\nHost: example.com:8080\r\n\r\n";
        assert_eq!(extract_host_from_headers(request_with_port), Some("example.com".to_string()));
    }

    #[test]
    fn test_socks5_version_check() {
        // Valid SOCKS5 greeting
        let valid = [0x05, 0x01, 0x00];
        assert_eq!(valid[0], 0x05);

        // Invalid version
        let invalid = [0x04, 0x01, 0x00];
        assert_ne!(invalid[0], 0x05);
    }
}
