use base64::Engine;
// Protocol Handlers implementing the registry interface
// Provides concrete implementations of detectors and handlers for all supported protocols

use std::io;
use std::net::{IpAddr, SocketAddr};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use log::{debug, info, warn};

#[cfg(target_os = "android")]
use libc;
#[cfg(any(target_os = "android", target_os = "linux"))]
use std::os::fd::{FromRawFd, IntoRawFd};


use crate::protocol_registry::{ProtocolDetector, ProtocolHandler, ProtocolDetectionResult, ProtocolFut};
// Removed async-trait to minimize dependencies; keep trait methods sync by returning boxed futures if needed
use crate::universal_listener::PrefixedStream;
#[cfg(feature = "auto-discovery")]
use crate::{pac, bonjour};
#[cfg(feature = "upnp")]
use crate::upnp;
#[cfg(feature = "doh")]
use hickory_resolver::TokioAsyncResolver;
#[cfg(feature = "doh")]
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
#[cfg(feature = "doh")]
use base64::Engine as _; // bring the trait into scope so STANDARD.decode(...) is available
use base64::engine::general_purpose;

/// Establish an outbound TCP connection with optional egress binding via libc syscalls.
/// Priority:
/// 1) If EGRESS_INTERFACE is set, attempt SO_BINDTODEVICE(iface)
/// 2) Else if EGRESS_BIND_IP is set, bind(2) to that local IP (port 0)
/// 3) Else connect normally
async fn connect_via_egress_sys(target: &str) -> io::Result<TcpStream> {
    // Parse "host:port"
    let (host, port_str) = target.rsplit_once(':')
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid target format, missing port"))?;
    let port: u16 = port_str.parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid port"))?;

    // Resolve with Tokio (async-friendly)
    let mut addrs = tokio::net::lookup_host(format!("{}:{}", host, port)).await?;
    let addr = addrs.next().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No address resolved"))?;

    // Environment controls
    let iface_opt = std::env::var("EGRESS_INTERFACE").ok();
    let bind_ip_opt = std::env::var("EGRESS_BIND_IP").ok().and_then(|s| s.parse::<IpAddr>().ok());

    // Non-Android/Linux fallback: use Tokio connect directly
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    {
        let _ = (iface_opt, bind_ip_opt);
        return TcpStream::connect(addr).await;
    }

    // Android/Linux path: libc socket + optional SO_BINDTODEVICE/bind + connect, then wrap fd into Tokio
    #[cfg(any(target_os = "android", target_os = "linux"))]
    unsafe {
        // Choose domain by addr family
        let (domain, sockaddr_storage, socklen) = match addr {
            SocketAddr::V4(v4) => {
                let mut sa: libc::sockaddr_in = std::mem::zeroed();
                sa.sin_family = libc::AF_INET as u16;
                sa.sin_port = u16::to_be(v4.port());
                sa.sin_addr = libc::in_addr { s_addr: u32::from_ne_bytes(v4.ip().octets()) };
                (libc::AF_INET, std::mem::transmute::<libc::sockaddr_in, libc::sockaddr_storage>(sa), std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t)
            }
            SocketAddr::V6(v6) => {
                let mut sa: libc::sockaddr_in6 = std::mem::zeroed();
                sa.sin6_family = libc::AF_INET6 as u16;
                sa.sin6_port = u16::to_be(v6.port());
                sa.sin6_addr = libc::in6_addr { s6_addr: v6.ip().octets() };
                sa.sin6_flowinfo = v6.flowinfo();
                sa.sin6_scope_id = v6.scope_id();
                (libc::AF_INET6, std::mem::transmute::<libc::sockaddr_in6, libc::sockaddr_storage>(sa), std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t)
            }
        };

        // Create non-blocking socket
        let fd = libc::socket(domain, libc::SOCK_STREAM | libc::SOCK_NONBLOCK, 0);
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        // Helper to ensure close on error
        struct FdGuard(i32);
        impl Drop for FdGuard {
            fn drop(&mut self) {
                if self.0 >= 0 {
                    unsafe { libc::close(self.0); }
                }
            }
        }
        let mut guard = FdGuard(fd);

        // Try SO_BINDTODEVICE if interface provided
        if let Some(iface) = iface_opt.as_ref() {
            let ifname = std::ffi::CString::new(iface.as_str()).unwrap_or_default();
            let ret = libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_BINDTODEVICE,
                                       ifname.as_ptr() as *const libc::c_void,
                                       ifname.as_bytes_with_nul().len() as libc::socklen_t);
            if ret != 0 {
                let e = io::Error::last_os_error();
                debug!("SO_BINDTODEVICE({}) failed: {}", iface, e);
                // proceed; fall back to IP bind if available
            } else {
                debug!("SO_BINDTODEVICE applied to interface {}", iface);
            }
        }

        // If no iface or iface failed, try bind to specific IP if provided and family matches
        if let Some(ip) = bind_ip_opt {
            let (bind_ok, bind_ret) = match (ip, addr) {
                (IpAddr::V4(ipv4), SocketAddr::V4(_)) => {
                    let mut sa: libc::sockaddr_in = std::mem::zeroed();
                    sa.sin_family = libc::AF_INET as u16;
                    sa.sin_port = 0u16.to_be(); // ephemeral
                    sa.sin_addr = libc::in_addr { s_addr: u32::from_ne_bytes(ipv4.octets()) };
                    let ret = libc::bind(fd,
                                         &std::mem::transmute::<libc::sockaddr_in, libc::sockaddr>(sa) as *const libc::sockaddr,
                                         std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t);
                    (true, ret)
                }
                (IpAddr::V6(ipv6), SocketAddr::V6(_)) => {
                    let mut sa: libc::sockaddr_in6 = std::mem::zeroed();
                    sa.sin6_family = libc::AF_INET6 as u16;
                    sa.sin6_port = 0u16.to_be(); // ephemeral
                    sa.sin6_addr = libc::in6_addr { s6_addr: ipv6.octets() };
                    let ret = libc::bind(fd,
                                         &std::mem::transmute::<libc::sockaddr_in6, libc::sockaddr>(sa) as *const libc::sockaddr,
                                         std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t);
                    (true, ret)
                }
                _ => (false, 0),
            };
            if bind_ok && bind_ret != 0 {
                let e = io::Error::last_os_error();
                debug!("bind(EGRESS_BIND_IP) failed: {}", e);
            } else if bind_ok {
                debug!("Bound local egress to {}", ip);
            }
        }

        // Connect
        let connect_ret = libc::connect(
            fd,
            &sockaddr_storage as *const libc::sockaddr_storage as *const libc::sockaddr,
            socklen,
        );

        if connect_ret != 0 {
            let err = io::Error::last_os_error();
            // EINPROGRESS is expected for non-blocking; let Tokio complete it
            if err.raw_os_error() != Some(libc::EINPROGRESS) {
                return Err(err);
            }
        }

        // Wrap fd into nonblocking std::net::TcpStream then into tokio::net::TcpStream
        let std_stream = std::net::TcpStream::from_raw_fd(fd);
        // prevent guard from closing; ownership moved to std_stream
        guard.0 = -1;
        std_stream.set_nonblocking(true)?;
        let tokio_stream = TcpStream::from_std(std_stream)?;
        Ok(tokio_stream)
    }
}

// ===== HTTP Protocol Detector =====

pub struct HttpDetector;

impl HttpDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProtocolDetector for HttpDetector {
    fn detect(&self, data: &[u8]) -> ProtocolDetectionResult {
        if let Ok(text) = std::str::from_utf8(data) {
            let _text_upper = text.to_uppercase();
            
            // Check for HTTP methods
            if text.starts_with("GET ") || 
               text.starts_with("POST ") || 
               text.starts_with("PUT ") || 
               text.starts_with("DELETE ") || 
               text.starts_with("HEAD ") || 
               text.starts_with("OPTIONS ") || 
               text.starts_with("CONNECT ") || 
               text.starts_with("PATCH ") {
                
                let confidence = if text.contains("HTTP/1.") { 220 } else { 180 };
                let bytes_consumed = text.lines().next().map(|line| line.len()).unwrap_or(0);
                
                return ProtocolDetectionResult::new("http", confidence, bytes_consumed)
                    .with_metadata(text.lines().next().unwrap_or("").to_string());
            }
        }
        
        ProtocolDetectionResult::unknown()
    }
    
    fn required_bytes(&self) -> usize { 16 }
    fn confidence_threshold(&self) -> u8 { 150 }
    fn protocol_name(&self) -> &str { "HTTP" }
}

// ===== HTTP Protocol Handler with Smart Routing =====

pub struct HttpHandler;

impl HttpHandler {
    pub fn new() -> Self {
        Self
    }
    
    /// Handle regular HTTP proxy requests
    async fn handle_regular_http(&self, stream: PrefixedStream<TcpStream>, request: &str) -> io::Result<()> {
        debug!("Handling regular HTTP request");
        
        // Parse the first line
        if let Some(first_line) = request.lines().next() {
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
                        
                        info!("HTTP CONNECT tunnel to {}", target_addr);
                        self.handle_connect_tunnel(stream, &target_addr).await
                    }
                    _ => {
                        // Regular HTTP proxy
                        if let Some(host) = self.extract_host_from_headers(request) {
                            let target_addr = format!("{}:80", host);
                            info!("HTTP proxy request to {}", target_addr);
                            self.handle_http_forward(stream, request.as_bytes(), &target_addr).await
                        } else {
                            self.send_error_response(stream, 400, "Bad Request").await
                        }
                    }
                }
            } else {
                self.send_error_response(stream, 400, "Bad Request").await
            }
        } else {
            self.send_error_response(stream, 400, "Bad Request").await
        }
    }
    
    async fn handle_connect_tunnel(&self, mut stream: PrefixedStream<TcpStream>, target: &str) -> io::Result<()> {
        match connect_via_egress_sys(target).await {
            Ok(remote) => {
                stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await?;
                self.relay_streams(stream, remote).await
            }
            Err(e) => {
                self.send_error_response(stream, 502, "Bad Gateway").await?;
                Err(e)
            }
        }
    }
    
    async fn handle_http_forward(&self, stream: PrefixedStream<TcpStream>, request: &[u8], target: &str) -> io::Result<()> {
        match connect_via_egress_sys(target).await {
            Ok(mut remote) => {
                remote.write_all(request).await?;
                self.relay_streams(stream, remote).await
            }
            Err(e) => {
                self.send_error_response(stream, 502, "Bad Gateway").await?;
                Err(e)
            }
        }
    }
    
    async fn relay_streams<S1, S2>(&self, mut client: S1, mut server: S2) -> io::Result<()>
    where
        S1: AsyncRead + AsyncWrite + Unpin,
        S2: AsyncRead + AsyncWrite + Unpin,
    {
        let (mut client_reader, mut client_writer) = tokio::io::split(&mut client);
        let (mut server_reader, mut server_writer) = tokio::io::split(&mut server);

        let client_to_server = tokio::io::copy(&mut client_reader, &mut server_writer);
        let server_to_client = tokio::io::copy(&mut server_reader, &mut client_writer);

        tokio::select! {
            res = client_to_server => {
                if let Err(e) = res { debug!("Error copying client to server: {}", e); }
            },
            res = server_to_client => {
                if let Err(e) = res { debug!("Error copying server to client: {}", e); }
            },
        }
        
        Ok(())
    }
    
    fn extract_host_from_headers(&self, request: &str) -> Option<String> {
        for line in request.lines() {
            let line_lower = line.to_lowercase();
            if line_lower.starts_with("host:") {
                if let Some(host_part) = line.split(':').nth(1) {
                    let host = host_part.trim();
                    // Handle port numbers in host header
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
    
    async fn send_error_response(&self, mut stream: PrefixedStream<TcpStream>, status: u16, message: &str) -> io::Result<()> {
        let response = format!(
            "HTTP/1.1 {} {}\r\nContent-Length: 0\r\n\r\n",
            status, message
        );
        stream.write_all(response.as_bytes()).await
    }
}

impl ProtocolHandler for HttpHandler {
    fn handle(&self, mut stream: PrefixedStream<TcpStream>) -> ProtocolFut {
        Box::pin(async move {
            // Read the request to determine routing
            let mut buffer = [0u8; 2048];
            let n = stream.read(&mut buffer).await?;
            
            if n == 0 {
                return Ok(());
            }
            
            let request = String::from_utf8_lossy(&buffer[..n]);
            debug!("HTTP request received: {}", request.lines().next().unwrap_or(""));
            
            // Check for specialized protocol requests first
            #[cfg(feature = "auto-discovery")]
            if pac::is_pac_request(&request).await {
                info!("Routing to PAC handler");
                return pac::handle_pac_request(stream).await;
            }
            
            #[cfg(feature = "auto-discovery")]
            if request.contains("/wpad.dat") {
                info!("Routing to WPAD handler");
                return pac::handle_wpad_request(stream).await;
            }
            
            #[cfg(feature = "auto-discovery")]
            if bonjour::is_bonjour_request(&request).await {
                info!("Routing to Bonjour handler");
                return bonjour::handle_bonjour(stream).await;
            }
            
            #[cfg(feature = "upnp")]
            if upnp::is_upnp_request(&request).await {
                info!("Routing to UPnP handler");
                return upnp::handle_upnp_request(stream).await;
            }
            
            // Handle as regular HTTP proxy
            self.handle_regular_http(stream, &request).await
        })
    }
    
    fn can_handle(&self, detection: &ProtocolDetectionResult) -> bool {
        detection.protocol_name == "http"
    }
    
    fn protocol_name(&self) -> &str { "HTTP" }
}

// ===== SOCKS5 Protocol Detector =====

pub struct Socks5Detector;

impl Socks5Detector {
    pub fn new() -> Self {
        Self
    }
}

impl ProtocolDetector for Socks5Detector {
    fn detect(&self, data: &[u8]) -> ProtocolDetectionResult {
        if data.len() >= 2 && data[0] == 0x05 {
            // SOCKS5 version byte
            let nmethods = data[1] as usize;
            if data.len() >= 2 + nmethods {
                // Complete handshake
                // Validate methods
                let valid_methods = data[2..2 + nmethods].iter().all(|&method| method <= 0xFF);
                if valid_methods {
                    return ProtocolDetectionResult::new("socks5", 250, 2 + nmethods);
                } else {
                    return ProtocolDetectionResult::new("socks5", 50, data.len());
                }
            } else {
                // Partial handshake
                return ProtocolDetectionResult::new("socks5", 200, data.len());
            }
        }
        
        ProtocolDetectionResult::unknown()
    }
    
    fn required_bytes(&self) -> usize { 2 }
    fn confidence_threshold(&self) -> u8 { 200 }
    fn protocol_name(&self) -> &str { "SOCKS5" }
}

// ===== SOCKS5 Protocol Handler =====

pub struct Socks5Handler;

impl Socks5Handler {
    pub fn new() -> Self {
        Self
    }
    
    async fn handle_socks5_connection(&self, mut stream: PrefixedStream<TcpStream>) -> io::Result<()> {
        // Handle SOCKS5 handshake - be more lenient with protocol versions
        let mut buf = [0u8; 2];
        match stream.read_exact(&mut buf).await {
            Ok(_) => {},
            Err(e) => {
                debug!("Failed to read SOCKS5 handshake: {}", e);
                return Err(e);
            }
        }
        
        if buf[0] != 5 {
            debug!("Unsupported SOCKS version: {}", buf[0]);
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported SOCKS version"));
        }
        
        let nmethods = buf[1] as usize;
        if nmethods == 0 || nmethods > 255 {
            debug!("Invalid number of methods: {}", nmethods);
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid methods count"));
        }
        
        let mut methods = vec![0u8; nmethods];
        if let Err(e) = stream.read_exact(&mut methods).await {
            debug!("Failed to read SOCKS5 methods: {}", e);
            return Err(e);
        }

        // Support both no-auth (0) and username/password (2) methods
        let selected_method = if methods.contains(&0) {
            0  // No authentication
        } else if methods.contains(&2) {
            2  // Username/password authentication (we'll accept any)
        } else {
            // No supported methods
            stream.write_all(&[5, 0xFF]).await?;
            return Err(io::Error::new(io::ErrorKind::InvalidData, "No supported authentication methods"));
        };
        
        stream.write_all(&[5, selected_method]).await?;
        
        // Handle authentication if required
        if selected_method == 2 {
            // Simple username/password auth - accept anything
            let mut auth_buf = [0u8; 1];
            stream.read_exact(&mut auth_buf).await?;
            if auth_buf[0] != 1 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid auth version"));
            }
            
            // Read username length and username
            stream.read_exact(&mut auth_buf).await?;
            let ulen = auth_buf[0] as usize;
            let mut username = vec![0u8; ulen];
            if ulen > 0 {
                stream.read_exact(&mut username).await?;
            }
            
            // Read password length and password
            stream.read_exact(&mut auth_buf).await?;
            let plen = auth_buf[0] as usize;
            let mut password = vec![0u8; plen];
            if plen > 0 {
                stream.read_exact(&mut password).await?;
            }
            
            // Accept any authentication
            stream.write_all(&[1, 0]).await?; // Success
            debug!("SOCKS5 auth completed for user: {}", String::from_utf8_lossy(&username));
        }

        // Handle SOCKS5 request
        let mut buf = [0u8; 4];
        stream.read_exact(&mut buf).await?;
        
        if buf[0] != 5 || buf[1] != 1 {
            stream.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported SOCKS command"));
        }

        let atyp = buf[3];
        let target = self.read_socks_address(&mut stream, atyp).await?;

        match connect_via_egress_sys(&target).await {
            Ok(remote) => {
                info!("SOCKS5 connection to {}", target);
                let local_addr = remote.local_addr().unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0)));
                
                let mut resp = vec![5, 0, 0];
                match local_addr {
                    SocketAddr::V4(addr) => {
                        resp.push(1);
                        resp.extend_from_slice(&addr.ip().octets());
                        resp.extend_from_slice(&addr.port().to_be_bytes());
                    }
                    SocketAddr::V6(addr) => {
                        resp.push(4);
                        resp.extend_from_slice(&addr.ip().octets());
                        resp.extend_from_slice(&addr.port().to_be_bytes());
                    }
                }
                
                stream.write_all(&resp).await?;
                
                // Create HTTP handler instance for relay functionality
                let http_handler = HttpHandler::new();
                http_handler.relay_streams(stream, remote).await
            }
            Err(e) => {
                stream.write_all(&[5, 1, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
                Err(e)
            }
        }
    }
    
    async fn read_socks_address(&self, stream: &mut PrefixedStream<TcpStream>, atyp: u8) -> io::Result<String> {
        match atyp {
            1 => {
                // IPv4
                let mut buf = [0u8; 6];
                stream.read_exact(&mut buf).await?;
                let ip = std::net::Ipv4Addr::new(buf[0], buf[1], buf[2], buf[3]);
                let port = u16::from_be_bytes([buf[4], buf[5]]);
                Ok(format!("{}:{}", ip, port))
            }
            3 => {
                // Domain name
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
            4 => {
                // IPv6
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
}

impl ProtocolHandler for Socks5Handler {
    fn handle(&self, stream: PrefixedStream<TcpStream>) -> ProtocolFut {
        Box::pin(async move {
            // move stream into the async block only; no borrow of self escapes
            self.handle_socks5_connection(stream).await
        })
    }
    
    fn can_handle(&self, detection: &ProtocolDetectionResult) -> bool {
        detection.protocol_name == "socks5"
    }
    
    fn protocol_name(&self) -> &str { "SOCKS5" }
}

// ===== TLS Protocol Detector =====

pub struct TlsDetector;

impl TlsDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProtocolDetector for TlsDetector {
    fn detect(&self, data: &[u8]) -> ProtocolDetectionResult {
        if data.len() >= 3 && data[0] == 0x16 && data[1] == 0x03 {
            // TLS handshake record
            let version = data[2];
            let confidence = match version {
                0x01 => 200, // TLS 1.0
                0x02 => 210, // TLS 1.1  
                0x03 => 230, // TLS 1.2
                0x04 => 240, // TLS 1.3
                _ => 150,    // Unknown TLS version
            };
            
            return ProtocolDetectionResult::new("tls", confidence, 3)
                .with_metadata(format!("TLS version: 1.{}", version));
        }
        
        ProtocolDetectionResult::unknown()
    }
    
    fn required_bytes(&self) -> usize { 3 }
    fn confidence_threshold(&self) -> u8 { 150 }
    fn protocol_name(&self) -> &str { "TLS" }
}

// ===== TLS Protocol Handler =====

pub struct TlsHandler;

impl TlsHandler {
    pub fn new() -> Self {
        Self
    }
}

impl ProtocolHandler for TlsHandler {
    fn handle(&self, _stream: PrefixedStream<TcpStream>) -> ProtocolFut {
        Box::pin(async move {
            info!("TLS passthrough - connection will be closed");
            // For now, just close TLS connections as we can't proxy them without termination
            // In a full implementation, you'd extract SNI and forward to the appropriate server
            Ok(())
        })
    }
    
    fn can_handle(&self, detection: &ProtocolDetectionResult) -> bool {
        detection.protocol_name == "tls"
    }
    
    fn protocol_name(&self) -> &str { "TLS" }
}

// ===== DoH (DNS-over-HTTPS) Protocol Detector =====

pub struct DohDetector;

impl DohDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProtocolDetector for DohDetector {
    fn detect(&self, data: &[u8]) -> ProtocolDetectionResult {
        if let Ok(text) = std::str::from_utf8(data) {
            // Check for DoH-specific paths and headers
            if text.starts_with("POST /dns-query") || 
               text.starts_with("GET /dns-query") ||
               text.contains("Content-Type: application/dns-message") ||
               text.contains("Accept: application/dns-message") {
                
                let confidence = if text.contains("application/dns-message") { 
                    250  // Very high confidence for proper DoH content-type
                } else if text.contains("/dns-query") {
                    230  // High confidence for DoH path
                } else {
                    200  // Medium confidence for DoH-like pattern
                };
                
                let bytes_consumed = text.lines().take(1).map(|line| line.len()).sum::<usize>();
                
                return ProtocolDetectionResult::new("doh", confidence, bytes_consumed)
                    .with_metadata(text.lines().next().unwrap_or("").to_string());
            }
        }
        
        ProtocolDetectionResult::unknown()
    }
    
    fn required_bytes(&self) -> usize { 20 }  // Enough for "POST /dns-query HTTP"
    fn confidence_threshold(&self) -> u8 { 200 }
    fn protocol_name(&self) -> &str { "DoH" }
}

// ===== DoH (DNS-over-HTTPS) Protocol Handler =====

#[cfg(feature = "doh")]
pub struct DohHandler {
    resolver: TokioAsyncResolver,
}

#[cfg(not(feature = "doh"))]
pub struct DohHandler {
    _phantom: std::marker::PhantomData<()>,
}

impl DohHandler {
    #[cfg(feature = "doh")]
    pub async fn new() -> Self {
        let config = ResolverConfig::cloudflare();
        let opts = ResolverOpts::default();
        let resolver = TokioAsyncResolver::tokio(config, opts);
        
        Self { resolver }
    }
    
    #[cfg(not(feature = "doh"))]
    pub async fn new() -> Self {
        Self { _phantom: std::marker::PhantomData }
    }
    
    #[cfg(feature = "doh")]
    pub async fn handle_doh_request(&self, stream: PrefixedStream<TcpStream>, request: &str) -> io::Result<()> {
        debug!("Handling DoH request");
        
        // Parse the HTTP request to extract DNS query
        if let Some(first_line) = request.lines().next() {
            let parts: Vec<&str> = first_line.split_whitespace().collect();
            if parts.len() >= 2 {
                let method = parts[0];
                let path = parts[1];
                
                match method {
                    "POST" => {
                        if path.starts_with("/dns-query") {
                            self.handle_doh_post(stream, request).await
                        } else {
                            self.send_doh_error(stream, 404, "Not Found").await
                        }
                    }
                    "GET" => {
                        if path.starts_with("/dns-query") {
                            self.handle_doh_get(stream, path).await
                        } else {
                            self.send_doh_error(stream, 404, "Not Found").await
                        }
                    }
                    _ => {
                        self.send_doh_error(stream, 405, "Method Not Allowed").await
                    }
                }
            } else {
                self.send_doh_error(stream, 400, "Bad Request").await
            }
        } else {
            self.send_doh_error(stream, 400, "Bad Request").await
        }
    }
    
    async fn handle_doh_post(&self, mut stream: PrefixedStream<TcpStream>, request: &str) -> io::Result<()> {
        // Extract Content-Length from headers
        let content_length = self.extract_content_length(request);
        
        if content_length == 0 {
            return self.send_doh_error(stream, 400, "Bad Request - No Content-Length").await;
        }
        
        // Find the end of headers
        let headers_end = request.find("\r\n\r\n").unwrap_or(request.len());
        let _headers = &request[..headers_end];
        let body_start = if headers_end + 4 <= request.len() { headers_end + 4 } else { request.len() };
        
        // Check if we have the complete body in the initial request
        let available_body = request.len() - body_start;
        
        let dns_query_data = if available_body >= content_length {
            // We have the complete request
            request[body_start..body_start + content_length].as_bytes().to_vec()
        } else {
            // Need to read more data
            let mut remaining_bytes = content_length - available_body;
            let mut query_data = request[body_start..].as_bytes().to_vec();
            
            while remaining_bytes > 0 {
                let mut buffer = vec![0u8; remaining_bytes.min(1024)];
                let n = stream.read(&mut buffer).await?;
                if n == 0 {
                    return self.send_doh_error(stream, 400, "Incomplete Request").await;
                }
                query_data.extend_from_slice(&buffer[..n]);
                remaining_bytes -= n;
            }
            query_data
        };
        
        // Process the DNS query using our resolver
        match self.process_dns_query(&dns_query_data).await {
            Ok(response_data) => {
                let response = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: application/dns-message\r\n\
                     Content-Length: {}\r\n\
                     Cache-Control: max-age=300\r\n\
                     \r\n",
                    response_data.len()
                );
                
                stream.write_all(response.as_bytes()).await?;
                stream.write_all(&response_data).await?;
                info!("DoH POST request processed successfully");
                Ok(())
            }
            Err(e) => {
                warn!("Failed to process DoH query: {}", e);
                self.send_doh_error(stream, 500, "Internal Server Error").await
            }
        }
    }
    
    async fn handle_doh_get(&self, mut stream: PrefixedStream<TcpStream>, path: &str) -> io::Result<()> {
        // Extract DNS query from URL parameter (RFC 8484 Section 4.1.1)
        if let Some(query_start) = path.find("?dns=") {
            let query_b64 = &path[query_start + 5..];
            
            // Decode base64url-encoded DNS query
            match self.decode_base64url(query_b64) {
                Ok(dns_query_data) => {
                    match self.process_dns_query(&dns_query_data).await {
                        Ok(response_data) => {
                            let response = format!(
                                "HTTP/1.1 200 OK\r\n\
                                 Content-Type: application/dns-message\r\n\
                                 Content-Length: {}\r\n\
                                 Cache-Control: max-age=300\r\n\
                                 \r\n",
                                response_data.len()
                            );
                            
                            stream.write_all(response.as_bytes()).await?;
                            stream.write_all(&response_data).await?;
                            info!("DoH GET request processed successfully");
                            Ok(())
                        }
                        Err(e) => {
                            warn!("Failed to process DoH query: {}", e);
                            self.send_doh_error(stream, 500, "Internal Server Error").await
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to decode DoH query: {}", e);
                    self.send_doh_error(stream, 400, "Bad Request").await
                }
            }
        } else {
            self.send_doh_error(stream, 400, "Bad Request - Missing dns parameter").await
        }
    }
    
    async fn process_dns_query(&self, query_data: &[u8]) -> io::Result<Vec<u8>> {
        // For now, we'll create a simple NXDOMAIN response
        // In a full implementation, you'd parse the DNS query and use the resolver
        
        if query_data.len() < 12 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "DNS query too short"));
        }
        
        // Extract transaction ID from query
        let transaction_id = [query_data[0], query_data[1]];
        
        // Create a simple NXDOMAIN response
        let mut response = Vec::new();
        response.extend_from_slice(&transaction_id); // Transaction ID
        response.extend_from_slice(&[0x81, 0x83]); // Flags: Response, NXDOMAIN
        response.extend_from_slice(&[0x00, 0x01]); // Questions: 1
        response.extend_from_slice(&[0x00, 0x00]); // Answer RRs: 0
        response.extend_from_slice(&[0x00, 0x00]); // Authority RRs: 0
        response.extend_from_slice(&[0x00, 0x00]); // Additional RRs: 0
        
        // Copy the question section from the query
        if query_data.len() > 12 {
            response.extend_from_slice(&query_data[12..]);
        }
        
        Ok(response)
    }
    
    fn extract_content_length(&self, request: &str) -> usize {
        for line in request.lines() {
            if line.to_lowercase().starts_with("content-length:") {
                if let Some(length_str) = line.split(':').nth(1) {
                    return length_str.trim().parse().unwrap_or(0);
                }
            }
        }
        0
    }
    
    fn decode_base64url(&self, input: &str) -> Result<Vec<u8>, io::Error> {
        // RFC 4648 Section 5 - base64url decoding
        let mut padded = input.replace('-', "+").replace('_', "/");

        // Add padding if needed
        while padded.len() % 4 != 0 {
            padded.push('=');
        }

        general_purpose::STANDARD
            .decode(&padded)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("base64 decode error: {e}")))
    }
    
    async fn send_doh_error(&self, mut stream: PrefixedStream<TcpStream>, status: u16, message: &str) -> io::Result<()> {
        let response = format!(
            "HTTP/1.1 {} {}\r\n\
             Content-Type: text/plain\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            status, message, message.len(), message
        );
        stream.write_all(response.as_bytes()).await
    }
}

impl ProtocolHandler for DohHandler {
    fn handle(&self, mut stream: PrefixedStream<TcpStream>) -> ProtocolFut {
        Box::pin(async move {
            // Read the request to determine routing
            let mut buffer = [0u8; 2048];
            let n = stream.read(&mut buffer).await?;

            if n == 0 {
                return Ok(());
            }

            let request = String::from_utf8_lossy(&buffer[..n]);
            debug!("DoH request received: {}", request.lines().next().unwrap_or(""));

            self.handle_doh_get(stream, &request).await
        })
    }
    
    fn can_handle(&self, detection: &ProtocolDetectionResult) -> bool {
        detection.protocol_name == "doh"
    }
    
    fn protocol_name(&self) -> &str { "DoH" }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_http_detection() {
        let detector = HttpDetector::new();
        
        let get_request = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let result = detector.detect(get_request);
        assert_eq!(result.protocol_name, "http");
        assert!(result.confidence >= 150);
        
        let post_request = b"POST /api HTTP/1.1\r\nContent-Length: 0\r\n\r\n";
        let result = detector.detect(post_request);
        assert_eq!(result.protocol_name, "http");
        assert!(result.confidence >= 150);
        
        let non_http = b"not an http request";
        let result = detector.detect(non_http);
        assert_eq!(result.protocol_name, "unknown");
    }
    
    #[test]
    fn test_socks5_detection() {
        let detector = Socks5Detector::new();
        
        let socks5_handshake = &[0x05, 0x01, 0x00]; // SOCKS5, 1 method, no auth
        let result = detector.detect(socks5_handshake);
        assert_eq!(result.protocol_name, "socks5");
        assert!(result.confidence >= 200);
        
        let not_socks5 = &[0x04, 0x01, 0x00]; // SOCKS4
        let result = detector.detect(not_socks5);
        assert_eq!(result.protocol_name, "unknown");
    }
    
    #[test]
    fn test_tls_detection() {
        let detector = TlsDetector::new();
        
        let tls_handshake = &[0x16, 0x03, 0x03]; // TLS 1.2 handshake
        let result = detector.detect(tls_handshake);
        assert_eq!(result.protocol_name, "tls");
        assert!(result.confidence >= 150);
        
        let not_tls = &[0x15, 0x03, 0x03];
        let result = detector.detect(not_tls);
        assert_eq!(result.protocol_name, "unknown");
    }
    
    #[test]
    fn test_doh_detection() {
        let detector = DohDetector::new();
        
        // Test POST to /dns-query
        let doh_post = b"POST /dns-query HTTP/1.1\r\nHost: example.com\r\nContent-Type: application/dns-message\r\n\r\n";
        let result = detector.detect(doh_post);
        assert_eq!(result.protocol_name, "doh");
        assert!(result.confidence >= 200);
        
        // Test GET to /dns-query
        let doh_get = b"GET /dns-query?dns=AAABAAABAAAAAAAAA3d3dwdleGFtcGxlA2NvbQAAAQAB HTTP/1.1\r\n";
        let result = detector.detect(doh_get);
        assert_eq!(result.protocol_name, "doh");
        assert!(result.confidence >= 200);
        
        // Test with application/dns-message content type
        let doh_content_type = b"POST /api HTTP/1.1\r\nContent-Type: application/dns-message\r\n\r\n";
        let result = detector.detect(doh_content_type);
        assert_eq!(result.protocol_name, "doh");
        assert!(result.confidence >= 200);
        
        // Test non-DoH request
        let not_doh = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let result = detector.detect(not_doh);
        assert_eq!(result.protocol_name, "unknown");
    }
    
    #[test]
    fn test_protocol_priority_ordering() {
        // Test that DoH takes priority over HTTP for dns-query requests
        let http_detector = HttpDetector::new();
        let doh_detector = DohDetector::new();
        
        let doh_request = b"POST /dns-query HTTP/1.1\r\nHost: example.com\r\n\r\n";
        
        let http_result = http_detector.detect(doh_request);
        let doh_result = doh_detector.detect(doh_request);
        
        // Both should detect, but DoH should have higher confidence
        assert_eq!(http_result.protocol_name, "http");
        assert_eq!(doh_result.protocol_name, "doh");
        assert!(doh_result.confidence > http_result.confidence);
    }
}