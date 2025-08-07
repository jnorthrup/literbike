// Simple 8888 Router - netcat-style connection with direct routing upstream
// Routes to separate ports as needed based on configuration

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::thread;
use std::collections::HashMap;

/// Simple routing configuration
#[derive(Debug, Clone)]
struct RouteConfig {
    /// Listen port (default 8888)
    listen_port: u16,
    /// Default upstream target
    default_upstream: SocketAddr,
    /// Protocol-specific routing
    protocol_routes: HashMap<String, SocketAddr>,
    /// Port-based routing  
    port_routes: HashMap<u16, SocketAddr>,
}

impl Default for RouteConfig {
    fn default() -> Self {
        let mut protocol_routes = HashMap::new();
        protocol_routes.insert("socks5".to_string(), "127.0.0.1:1080".parse().unwrap());
        protocol_routes.insert("http".to_string(), "127.0.0.1:8080".parse().unwrap());
        
        let mut port_routes = HashMap::new();
        port_routes.insert(1080, "127.0.0.1:1080".parse().unwrap());
        port_routes.insert(8080, "127.0.0.1:8080".parse().unwrap());
        port_routes.insert(3128, "127.0.0.1:3128".parse().unwrap());
        
        Self {
            listen_port: 8888,
            default_upstream: "127.0.0.1:8080".parse().unwrap(),
            protocol_routes,
            port_routes,
        }
    }
}

/// Simple binary protocol detector
fn detect_protocol_simple(data: &[u8]) -> Option<&'static str> {
    if data.len() < 2 {
        return None;
    }
    
    // SOCKS5: starts with 0x05
    if data[0] == 0x05 {
        return Some("socks5");
    }
    
    // HTTP: starts with ASCII method
    if let Ok(text) = std::str::from_utf8(&data[..data.len().min(8)]) {
        if text.starts_with("GET ") || text.starts_with("POST") || 
           text.starts_with("CONN") || text.starts_with("PUT ") {
            return Some("http");
        }
    }
    
    None
}

/// Handle a client connection with direct routing
fn handle_client(mut client: TcpStream, config: &RouteConfig) -> io::Result<()> {
    let client_addr = client.peer_addr().unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap());
    println!("📥 Client connected from {}", client_addr);
    
    // Read first few bytes to detect protocol
    let mut peek_buf = [0u8; 64];
    let bytes_read = match client.peek(&mut peek_buf) {
        Ok(n) => n,
        Err(_) => {
            // If peek fails, use default upstream
            return route_to_upstream(client, config.default_upstream, "default");
        }
    };
    
    if bytes_read == 0 {
        return Ok(());
    }
    
    // Determine upstream target
    let upstream = match detect_protocol_simple(&peek_buf[..bytes_read]) {
        Some(protocol) => {
            println!("🔍 Detected protocol: {}", protocol);
            config.protocol_routes.get(protocol).copied()
                .unwrap_or(config.default_upstream)
        }
        None => {
            println!("🔍 Unknown protocol, using default upstream");
            config.default_upstream
        }
    };
    
    route_to_upstream(client, upstream, "detected")
}

/// Route connection to upstream server
fn route_to_upstream(mut client: TcpStream, upstream: SocketAddr, route_type: &str) -> io::Result<()> {
    println!("🚀 Routing {} to upstream: {}", route_type, upstream);
    
    // Connect to upstream
    let mut upstream_conn = match TcpStream::connect(upstream) {
        Ok(conn) => {
            println!("✅ Connected to upstream {}", upstream);
            conn
        }
        Err(e) => {
            println!("❌ Failed to connect to upstream {}: {}", upstream, e);
            return Err(e);
        }
    };
    
    // Set up bidirectional relay
    let client_addr = client.peer_addr().unwrap_or_else(|_| "unknown".parse().unwrap());
    println!("🔄 Starting relay: {} <-> {}", client_addr, upstream);
    
    // Clone streams for bidirectional relay
    let mut client_read = client.try_clone()?;
    let mut upstream_write = upstream_conn.try_clone()?;
    let mut upstream_read = upstream_conn;
    let mut client_write = client;
    
    // Spawn thread for client -> upstream
    let upstream_addr = upstream;
    let client_to_upstream = thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut total_bytes = 0;
        
        loop {
            match client_read.read(&mut buf) {
                Ok(0) => {
                    println!("📤 Client {} closed connection (sent {} bytes)", client_addr, total_bytes);
                    break;
                }
                Ok(n) => {
                    total_bytes += n;
                    if let Err(e) = upstream_write.write_all(&buf[..n]) {
                        println!("❌ Error writing to upstream {}: {}", upstream_addr, e);
                        break;
                    }
                }
                Err(e) => {
                    println!("❌ Error reading from client {}: {}", client_addr, e);
                    break;
                }
            }
        }
    });
    
    // Main thread handles upstream -> client
    let mut buf = [0u8; 4096];
    let mut total_bytes = 0;
    
    loop {
        match upstream_read.read(&mut buf) {
            Ok(0) => {
                println!("📥 Upstream {} closed connection (received {} bytes)", upstream, total_bytes);
                break;
            }
            Ok(n) => {
                total_bytes += n;
                if let Err(e) = client_write.write_all(&buf[..n]) {
                    println!("❌ Error writing to client {}: {}", client_addr, e);
                    break;
                }
            }
            Err(e) => {
                println!("❌ Error reading from upstream {}: {}", upstream, e);
                break;
            }
        }
    }
    
    // Wait for client->upstream thread to finish
    let _ = client_to_upstream.join();
    
    println!("🔚 Connection closed: {} <-> {}", client_addr, upstream);
    Ok(())
}

fn main() -> io::Result<()> {
    let config = RouteConfig::default();
    
    println!("🌐 Simple 8888 Router Starting");
    println!("================================");
    println!("📍 Listen port: {}", config.listen_port);
    println!("🎯 Default upstream: {}", config.default_upstream);
    println!("📋 Protocol routes:");
    for (proto, addr) in &config.protocol_routes {
        println!("   {} -> {}", proto, addr);
    }
    
    let listener = TcpListener::bind(format!("0.0.0.0:{}", config.listen_port))?;
    println!("\n🚀 Listening on port {}...", config.listen_port);
    println!("💡 Connect with: nc localhost {}", config.listen_port);
    
    for stream in listener.incoming() {
        match stream {
            Ok(client) => {
                let config = config.clone();
                thread::spawn(move || {
                    if let Err(e) = handle_client(client, &config) {
                        println!("❌ Client handling error: {}", e);
                    }
                });
            }
            Err(e) => {
                println!("❌ Connection error: {}", e);
            }
        }
    }
    
    Ok(())
}