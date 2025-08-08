//! High-performance proxy server implementation using syscalls

use std::io;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional};
use log::{info, warn, error, debug};
use crate::universal_listener::{detect_protocol, Protocol};
// use crate::protocol_handlers::{HttpHandler, Socks5Handler};
use crate::libc_listener::{bind_with_options, ListenerOptions};

/// Proxy server configuration
#[derive(Debug, Clone)]
pub struct ProxyServerConfig {
    pub port: u16,
    pub bind_addr: IpAddr,
    pub protocols: Vec<String>,
    pub daemon: bool,
}

impl Default for ProxyServerConfig {
    fn default() -> Self {
        Self {
            port: 8888,
            bind_addr: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            protocols: vec!["all".to_string()],
            daemon: false,
        }
    }
}

/// Multi-protocol proxy server
pub struct ProxyServer {
    config: ProxyServerConfig,
    listener: Option<TcpListener>,
}

impl ProxyServer {
    pub fn new(config: ProxyServerConfig) -> Self {
        Self {
            config,
            listener: None,
        }
    }

    /// Start the proxy server
    pub async fn start(&mut self) -> io::Result<()> {
        let addr = SocketAddr::new(self.config.bind_addr, self.config.port);
        
        info!("Starting LiteBike proxy server on {}", addr);
        info!("Supported protocols: {:?}", self.config.protocols);

        // Use high-performance listener with socket options
        let listener_options = ListenerOptions {
            reuse_addr: true,
            reuse_port: true,
            backlog: 1024,
        };

        let listener = bind_with_options(addr, &listener_options).await?;
        self.listener = Some(listener);

        info!("✅ LiteBike proxy server listening on {}", addr);
        
        // Accept connections
        loop {
            let listener = self.listener.as_ref().unwrap();
            
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    debug!("New connection from {}", peer_addr);
                    
                    let protocols = self.config.protocols.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, peer_addr, protocols).await {
                            warn!("Connection error from {}: {}", peer_addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    }

    /// Handle a single connection
    async fn handle_connection(
        mut stream: TcpStream,
        peer_addr: SocketAddr,
        protocols: Vec<String>,
    ) -> io::Result<()> {
        // Detect protocol
        let (protocol, initial_data) = detect_protocol(&mut stream).await?;
        
        debug!("Detected protocol: {:?} from {}", protocol, peer_addr);

        // Route to appropriate handler based on protocol and configuration
        match protocol {
            Protocol::Http if protocols.contains(&"all".to_string()) || protocols.contains(&"http".to_string()) => {
                info!("Handling HTTP connection from {}", peer_addr);
                Self::handle_http_proxy(stream, initial_data).await
            }
            Protocol::Socks5 if protocols.contains(&"all".to_string()) || protocols.contains(&"socks5".to_string()) => {
                info!("Handling SOCKS5 connection from {}", peer_addr);
                Self::handle_socks5_proxy(stream, initial_data).await
            }
            _ if protocols.contains(&"all".to_string()) => {
                info!("Handling raw TCP tunnel from {}", peer_addr);
                Self::handle_tcp_tunnel(stream, initial_data).await
            }
            _ => {
                warn!("Protocol {:?} not supported in current configuration", protocol);
                Ok(())
            }
        }
    }

    /// Handle HTTP proxy requests
    async fn handle_http_proxy(
        mut stream: TcpStream,
        initial_data: Vec<u8>,
    ) -> io::Result<()> {
        // Parse HTTP request from initial data
        if let Ok(request) = String::from_utf8(initial_data.clone()) {
            if let Some(connect_line) = request.lines().next() {
                if connect_line.starts_with("CONNECT ") {
                    // Handle CONNECT method for HTTPS tunneling
                    let parts: Vec<&str> = connect_line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let target = parts[1];
                        return Self::handle_connect_tunnel(stream, target).await;
                    }
                }
                // Handle regular HTTP proxy
                return Self::handle_http_request(stream, initial_data).await;
            }
        }
        
        Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid HTTP request"))
    }

    /// Handle SOCKS5 proxy requests
    async fn handle_socks5_proxy(
        mut stream: TcpStream,
        initial_data: Vec<u8>,
    ) -> io::Result<()> {
        // Restore initial data to stream
        if !initial_data.is_empty() {
            // Need to handle SOCKS5 handshake
            if initial_data.len() >= 3 && initial_data[0] == 0x05 {
                let nmethods = initial_data[1] as usize;
                if initial_data.len() >= 2 + nmethods {
                    // Send auth method selection (no auth)
                    stream.write_all(&[0x05, 0x00]).await?;
                    
                    // Read connection request
                    let mut req_buf = vec![0u8; 1024];
                    let n = stream.read(&mut req_buf).await?;
                    if n >= 10 {
                        return Self::handle_socks5_request(stream, &req_buf[..n]).await;
                    }
                }
            }
        }
        
        Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid SOCKS5 request"))
    }

    /// Handle raw TCP tunneling
    async fn handle_tcp_tunnel(mut stream: TcpStream, _initial_data: Vec<u8>) -> io::Result<()> {
        // For raw TCP, we need to extract destination from SNI or other means
        // For now, implement echo server as fallback
        info!("Handling raw TCP connection");
        
        let mut buf = vec![0u8; 4096];
        loop {
            match stream.read(&mut buf).await {
                Ok(0) => break, // Connection closed
                Ok(n) => {
                    stream.write_all(&buf[..n]).await?;
                }
                Err(e) => {
                    warn!("TCP tunnel error: {}", e);
                    break;
                }
            }
        }
        
        Ok(())
    }

    /// Handle HTTP CONNECT tunneling
    async fn handle_connect_tunnel(mut client_stream: TcpStream, target: &str) -> io::Result<()> {
        info!("CONNECT tunnel to {}", target);
        
        // Connect to target
        match TcpStream::connect(target).await {
            Ok(mut target_stream) => {
                // Send 200 Connection Established
                client_stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await?;
                
                // Start bidirectional copying
                let (_, _) = copy_bidirectional(&mut client_stream, &mut target_stream).await?;
                Ok(())
            }
            Err(e) => {
                // Send error response
                let response = format!("HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\n\r\nFailed to connect to {}: {}", target, e);
                client_stream.write_all(response.as_bytes()).await?;
                Err(e)
            }
        }
    }

    /// Handle regular HTTP proxy requests
    async fn handle_http_request(mut stream: TcpStream, initial_data: Vec<u8>) -> io::Result<()> {
        if let Ok(request) = String::from_utf8(initial_data) {
            // Parse request line
            if let Some(first_line) = request.lines().next() {
                let parts: Vec<&str> = first_line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let method = parts[0];
                    let url = parts[1];
                    
                    info!("HTTP {} {}", method, url);
                    
                    // For now, return a simple response
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nLiteBike Proxy Server\nReceived: {} {}\n",
                        method, url
                    );
                    stream.write_all(response.as_bytes()).await?;
                    return Ok(());
                }
            }
        }
        
        Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid HTTP request"))
    }

    /// Handle SOCKS5 connection requests
    async fn handle_socks5_request(mut stream: TcpStream, request: &[u8]) -> io::Result<()> {
        if request.len() < 10 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "SOCKS5 request too short"));
        }

        if request[0] != 0x05 || request[1] != 0x01 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid SOCKS5 request"));
        }

        let addr_type = request[3];
        let mut target_addr = String::new();
        let mut port_offset = 0;

        match addr_type {
            0x01 => {
                // IPv4
                if request.len() < 10 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid IPv4 address"));
                }
                target_addr = format!("{}.{}.{}.{}", request[4], request[5], request[6], request[7]);
                port_offset = 8;
            }
            0x03 => {
                // Domain name
                let domain_len = request[4] as usize;
                if request.len() < 7 + domain_len {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid domain name"));
                }
                target_addr = String::from_utf8_lossy(&request[5..5 + domain_len]).to_string();
                port_offset = 5 + domain_len;
            }
            _ => {
                // Send error response
                stream.write_all(&[0x05, 0x08, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).await?;
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported address type"));
            }
        }

        let port = u16::from_be_bytes([request[port_offset], request[port_offset + 1]]);
        let target = format!("{}:{}", target_addr, port);

        info!("SOCKS5 connection to {}", target);

        // Connect to target
        match TcpStream::connect(&target).await {
            Ok(mut target_stream) => {
                // Send success response
                stream.write_all(&[0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).await?;
                
                // Start bidirectional copying
                let (_, _) = copy_bidirectional(&mut stream, &mut target_stream).await?;
                Ok(())
            }
            Err(_) => {
                // Send error response
                stream.write_all(&[0x05, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).await?;
                Err(io::Error::new(io::ErrorKind::ConnectionRefused, "Failed to connect to target"))
            }
        }
    }
}

/// Parse server arguments from command line
pub fn parse_server_args(args: &[String]) -> ProxyServerConfig {
    let mut config = ProxyServerConfig::default();
    
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" | "-p" if i + 1 < args.len() => {
                if let Ok(port) = args[i + 1].parse() {
                    config.port = port;
                }
                i += 2;
            }
            "--bind" | "-b" if i + 1 < args.len() => {
                if let Ok(addr) = args[i + 1].parse() {
                    config.bind_addr = addr;
                }
                i += 2;
            }
            "--protocols" if i + 1 < args.len() => {
                config.protocols = args[i + 1].split(',').map(|s| s.trim().to_string()).collect();
                i += 2;
            }
            "--daemon" | "-d" => {
                config.daemon = true;
                i += 1;
            }
            _ => i += 1,
        }
    }
    
    config
}