// LiteBike Concentrator - Maps protocols to NC connections 
// Client litebike has map of protocols, creates map of NC's (netcat connections)

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::thread;
use std::time::Duration;
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct LiteBikeConfig {
    /// Local listen port (8888)
    listen_port: u16,
    /// Peer LiteBike address (Mac <-> Phone)
    peer_litebike: Option<SocketAddr>,
    /// Known upstream proxies by protocol
    upstream_proxies: HashMap<String, SocketAddr>,
    /// Whether we're the phone or mac instance
    is_phone: bool,
}

impl Default for LiteBikeConfig {
    fn default() -> Self {
        let mut upstream_proxies = HashMap::new();
        
        // Default upstream proxy mappings
        upstream_proxies.insert("socks5".to_string(), "127.0.0.1:1080".parse().unwrap());
        upstream_proxies.insert("http".to_string(), "127.0.0.1:8080".parse().unwrap());
        upstream_proxies.insert("ssh".to_string(), "127.0.0.1:22".parse().unwrap());
        
        Self {
            listen_port: 8888,
            peer_litebike: None, // Set this to connect phone <-> mac
            upstream_proxies,
            is_phone: true, // Assume phone by default
        }
    }
}

/// Detect protocol from initial bytes
fn detect_protocol_fast(data: &[u8]) -> Option<&'static str> {
    if data.len() < 2 {
        return None;
    }
    
    match data[0] {
        // SOCKS5: 0x05
        0x05 => Some("socks5"),
        
        // HTTP methods (ASCII)
        b'G' if data.len() >= 4 && &data[..4] == b"GET " => Some("http"),
        b'P' if data.len() >= 5 && &data[..5] == b"POST " => Some("http"),
        b'C' if data.len() >= 8 && &data[..8] == b"CONNECT " => Some("http"),
        
        // SSH: "SSH-"
        b'S' if data.len() >= 4 && &data[..4] == b"SSH-" => Some("ssh"),
        
        // TLS handshake
        0x16 if data.len() >= 3 && data[1] == 0x03 => Some("tls"),
        
        _ => None,
    }
}

/// Handle incoming connection with symmetric routing logic
fn handle_connection(mut client: TcpStream, config: &LiteBikeConfig) -> io::Result<()> {
    let client_addr = client.peer_addr().unwrap_or_else(|_| "unknown".parse().unwrap());
    println!("📱 Connection from: {}", client_addr);
    
    // Read first packet to detect protocol
    let mut peek_buf = [0u8; 128];
    let bytes_read = match client.peek(&mut peek_buf) {
        Ok(n) if n > 0 => n,
        Ok(0) => {
            println!("📭 Empty connection from {}", client_addr);
            return Ok(());
        }
        Ok(_) => 0, // This handles the Ok(1_usize..) case
        Err(e) => {
            println!("❌ Peek failed from {}: {}", client_addr, e);
            return Ok(());
        }
    };
    
    let protocol = detect_protocol_fast(&peek_buf[..bytes_read]);
    println!("🔍 Detected protocol: {:?}", protocol.unwrap_or("unknown"));
    
    // Routing decision logic
    let upstream = match protocol {
        Some(proto) => {
            // First check if we have a local upstream proxy
            if let Some(&local_upstream) = config.upstream_proxies.get(proto) {
                // Try to connect to local upstream first
                match TcpStream::connect_timeout(&local_upstream, Duration::from_millis(100)) {
                    Ok(_test_conn) => {
                        println!("🎯 Routing {} to local upstream: {}", proto, local_upstream);
                        Some(local_upstream)
                    }
                    Err(_) => {
                        println!("⚠️  Local upstream {} unavailable for {}", local_upstream, proto);
                        // Fallback to peer LiteBike if available
                        if let Some(peer) = config.peer_litebike {
                            println!("🔗 Fallback to peer LiteBike: {}", peer);
                            Some(peer)
                        } else {
                            None
                        }
                    }
                }
            } else {
                // No local upstream, try peer LiteBike
                if let Some(peer) = config.peer_litebike {
                    println!("🔗 Routing {} to peer LiteBike: {}", proto, peer);
                    Some(peer)
                } else {
                    None
                }
            }
        }
        None => {
            // Unknown protocol - try peer LiteBike if available
            if let Some(peer) = config.peer_litebike {
                println!("❓ Unknown protocol, trying peer LiteBike: {}", peer);
                Some(peer)
            } else {
                None
            }
        }
    };
    
    if let Some(target) = upstream {
        relay_connection(client, target, protocol.unwrap_or("unknown"))
    } else {
        println!("🚫 No upstream available for {} from {}", 
                protocol.unwrap_or("unknown"), client_addr);
        
        // Send a simple response and close
        let _ = client.write_all(b"LiteBike: No upstream available\r\n");
        Ok(())
    }
}

/// Relay connection to upstream with logging
fn relay_connection(mut client: TcpStream, upstream: SocketAddr, protocol: &str) -> io::Result<()> {
    let client_addr = client.peer_addr().unwrap_or_else(|_| "unknown".parse().unwrap());
    
    println!("🔄 Starting relay: {} [{}] <-> {}", client_addr, protocol, upstream);
    
    // Connect to upstream
    let upstream_conn = match TcpStream::connect(upstream) {
        Ok(conn) => {
            println!("✅ Connected to upstream: {}", upstream);
            conn
        }
        Err(e) => {
            println!("❌ Failed to connect to upstream {}: {}", upstream, e);
            let _ = client.write_all(format!("LiteBike: Upstream {} unavailable\r\n", upstream).as_bytes());
            return Err(e);
        }
    };
    
    // Clone streams for bidirectional relay
    let mut client_read = client.try_clone()?;
    let mut upstream_write = upstream_conn.try_clone()?;
    let mut upstream_read = upstream_conn;
    let mut client_write = client;
    
    // Spawn thread for client -> upstream
    let client_addr_copy = client_addr;
    let upstream_copy = upstream;
    let protocol_copy = protocol.to_string();
    
    let client_to_upstream = thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut total_bytes = 0u64;
        
        loop {
            match client_read.read(&mut buf) {
                Ok(0) => {
                    println!("📤 Client {} closed [{}] (sent {} bytes)", 
                           client_addr_copy, protocol_copy, total_bytes);
                    break;
                }
                Ok(n) => {
                    total_bytes += n as u64;
                    if let Err(e) = upstream_write.write_all(&buf[..n]) {
                        println!("❌ Error writing to upstream {}: {}", upstream_copy, e);
                        break;
                    }
                }
                Err(e) => {
                    println!("❌ Error reading from client {}: {}", client_addr_copy, e);
                    break;
                }
            }
        }
        total_bytes
    });
    
    // Main thread handles upstream -> client
    let mut buf = [0u8; 8192];
    let mut total_bytes = 0u64;
    
    loop {
        match upstream_read.read(&mut buf) {
            Ok(0) => {
                println!("📥 Upstream {} closed [{}] (received {} bytes)", 
                       upstream, protocol, total_bytes);
                break;
            }
            Ok(n) => {
                total_bytes += n as u64;
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
    
    // Wait for client->upstream thread and get stats
    let sent_bytes = client_to_upstream.join().unwrap_or(0);
    
    println!("📊 Session complete: {} [{}] ↑{}B ↓{}B", 
           client_addr, protocol, sent_bytes, total_bytes);
    
    Ok(())
}

/// Detect if we're running on phone (Android/Termux) - acts as concentrator
fn detect_platform() -> bool {
    // Check for Termux-specific paths
    std::path::Path::new("/data/data/com.termux").exists()
}

fn main() -> io::Result<()> {
    let mut config = LiteBikeConfig::default();
    config.is_phone = detect_platform();
    
    // Configure peer based on platform
    if config.is_phone {
        // Phone -> Mac (use the IP from your Mac interface)
        config.peer_litebike = Some("192.168.227.91:8888".parse().unwrap());
        println!("📱 LiteBike Phone Concentrator");
    } else {
        // Mac -> Phone (would need phone's IP)
        config.peer_litebike = Some("192.168.1.100:8888".parse().unwrap()); // Placeholder
        println!("💻 LiteBike Mac Concentrator");
    }
    
    println!("🚀 Symmetric LiteBike Starting");
    println!("==============================");
    println!("📍 Listen port: {}", config.listen_port);
    if let Some(peer) = config.peer_litebike {
        println!("🤝 Peer LiteBike: {}", peer);
    }
    
    println!("📋 Upstream proxies:");
    for (proto, addr) in &config.upstream_proxies {
        println!("   {} -> {}", proto, addr);
    }
    
    let listener = TcpListener::bind(format!("0.0.0.0:{}", config.listen_port))?;
    println!("\n🎧 Listening on port {} (like nc -l {})...", 
             config.listen_port, config.listen_port);
    
    for stream in listener.incoming() {
        match stream {
            Ok(client) => {
                let config = config.clone();
                thread::spawn(move || {
                    if let Err(e) = handle_connection(client, &config) {
                        println!("❌ Connection handling error: {}", e);
                    }
                });
            }
            Err(e) => {
                println!("❌ Accept error: {}", e);
            }
        }
    }
    
    Ok(())
}