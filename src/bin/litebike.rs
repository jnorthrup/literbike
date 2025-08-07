// LiteBike - Simple NC-style proxy concentrator
// nc on local 8888, direct routing upstream

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::thread;
use std::collections::HashMap;

#[derive(Clone)]
struct LiteBikeConfig {
    listen_port: u16,
    default_upstream: SocketAddr,
    protocol_routes: HashMap<String, SocketAddr>,
}

impl Default for LiteBikeConfig {
    fn default() -> Self {
        let mut protocol_routes = HashMap::new();
        protocol_routes.insert("socks5".to_string(), "192.168.227.91:1080".parse().unwrap());
        protocol_routes.insert("http".to_string(), "192.168.227.91:8080".parse().unwrap());
        
        Self {
            listen_port: 8888,
            default_upstream: "8.8.8.8:80".parse().unwrap(),
            protocol_routes,
        }
    }
}

fn detect_protocol(data: &[u8]) -> Option<&'static str> {
    if data.len() < 2 { return None; }
    
    if data[0] == 0x05 {
        return Some("socks5");
    }
    
    if let Ok(text) = std::str::from_utf8(&data[..data.len().min(8)]) {
        if text.starts_with("GET ") || text.starts_with("POST") || text.starts_with("CONN") {
            return Some("http");
        }
    }
    
    None
}

fn handle_client(mut client: TcpStream, config: &LiteBikeConfig) -> io::Result<()> {
    let client_addr = client.peer_addr().unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap());
    println!("Client: {}", client_addr);
    
    let mut peek_buf = [0u8; 64];
    let bytes_read = client.peek(&mut peek_buf).unwrap_or(0);
    
    let upstream = if bytes_read > 0 {
        match detect_protocol(&peek_buf[..bytes_read]) {
            Some(protocol) => {
                println!("Protocol: {}", protocol);
                config.protocol_routes.get(protocol).copied().unwrap_or(config.default_upstream)
            }
            None => config.default_upstream,
        }
    } else {
        config.default_upstream
    };
    
    println!("Routing {} -> {}", client_addr, upstream);
    
    let mut upstream_conn = TcpStream::connect(upstream)?;
    
    // Clone for bidirectional relay
    let mut client_read = client.try_clone()?;
    let mut upstream_write = upstream_conn.try_clone()?;
    let mut upstream_read = upstream_conn;
    let mut client_write = client;
    
    // Client -> upstream thread
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match client_read.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if upstream_write.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    
    // Upstream -> client (main thread)
    let mut buf = [0u8; 4096];
    loop {
        match upstream_read.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if client_write.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    
    println!("Closed: {}", client_addr);
    Ok(())
}

fn main() -> io::Result<()> {
    let config = LiteBikeConfig::default();
    
    println!("LiteBike NC Proxy");
    println!("Listen: {}", config.listen_port);
    println!("Default: {}", config.default_upstream);
    for (proto, addr) in &config.protocol_routes {
        println!("{} -> {}", proto, addr);
    }
    
    let listener = TcpListener::bind(format!("0.0.0.0:{}", config.listen_port))?;
    println!("Ready on port {}", config.listen_port);
    
    for stream in listener.incoming() {
        match stream {
            Ok(client) => {
                let config = config.clone();
                thread::spawn(move || {
                    if let Err(e) = handle_client(client, &config) {
                        eprintln!("Error: {}", e);
                    }
                });
            }
            Err(e) => eprintln!("Accept error: {}", e),
        }
    }
    
    Ok(())
}