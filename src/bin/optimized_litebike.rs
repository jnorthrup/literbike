// Optimized LiteBike - Minimize processing via NC forwarding, minimize traffic via discretized listeners
// Separate/overlapping listeners for hosts and ports with intelligent forwarding

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::thread;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Discretized listener configuration
#[derive(Debug, Clone)]
struct DiscretizedListener {
    /// Port this listener handles
    port: u16,
    /// Specific hosts this listener serves (empty = all hosts)
    target_hosts: Vec<String>,
    /// Direct NC forward target (minimize processing)
    nc_forward: Option<SocketAddr>,
    /// Whether this is an overlapping listener
    overlapping: bool,
}

/// Optimized LiteBike configuration
#[derive(Debug, Clone)]
struct OptimizedConfig {
    /// Map of port -> listener config
    port_listeners: HashMap<u16, DiscretizedListener>,
    /// Host-specific routing (minimize traffic)
    host_routes: HashMap<String, SocketAddr>,
    /// Default NC forward for unknown traffic
    default_nc: Option<SocketAddr>,
}

impl Default for OptimizedConfig {
    fn default() -> Self {
        let mut port_listeners = HashMap::new();
        let mut host_routes = HashMap::new();
        
        // Discretized listeners for common ports
        port_listeners.insert(8888, DiscretizedListener {
            port: 8888,
            target_hosts: vec![], // Accept all hosts
            nc_forward: Some("127.0.0.1:8080".parse().unwrap()), // Forward to HTTP proxy
            overlapping: false,
        });
        
        port_listeners.insert(1080, DiscretizedListener {
            port: 1080,
            target_hosts: vec!["socks.local".to_string(), "proxy.local".to_string()],
            nc_forward: Some("127.0.0.1:1080".parse().unwrap()), // Direct SOCKS5 forward
            overlapping: false,
        });
        
        port_listeners.insert(3128, DiscretizedListener {
            port: 3128,
            target_hosts: vec!["squid.local".to_string(), "cache.local".to_string()],
            nc_forward: Some("127.0.0.1:3128".parse().unwrap()), // Squid proxy forward
            overlapping: true, // Can overlap with 8888 for HTTP
        });
        
        // Host-specific routes (minimize traffic by direct routing)
        host_routes.insert("mac.local".to_string(), "192.168.227.91:8888".parse().unwrap());
        host_routes.insert("phone.local".to_string(), "127.0.0.1:8888".parse().unwrap());
        host_routes.insert("upstream.proxy".to_string(), "10.0.0.1:8080".parse().unwrap());
        
        Self {
            port_listeners,
            host_routes,
            default_nc: Some("127.0.0.1:8080".parse().unwrap()),
        }
    }
}

/// Minimal processing forwarder - just pass bytes through
fn minimal_nc_forward(mut client: TcpStream, target: SocketAddr) -> io::Result<()> {
    let client_addr = client.peer_addr().unwrap_or_else(|_| "unknown".parse().unwrap());
    
    // Connect to target with minimal overhead
    let target_stream = TcpStream::connect(target)?;
    println!("🔄 NC Forward: {} -> {} (zero processing)", client_addr, target);
    
    // Clone streams for bidirectional forwarding
    let mut client_read = client.try_clone()?;
    let mut target_write = target_stream.try_clone()?;
    let mut target_read = target_stream;
    let mut client_write = client;
    
    // Spawn minimal forwarding thread (client -> target)
    let target_addr = target;
    let forward_thread = thread::spawn(move || {
        let mut buf = [0u8; 16384]; // Larger buffer for less syscalls
        let mut bytes_forwarded = 0u64;
        
        loop {
            match client_read.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    bytes_forwarded += n as u64;
                    if target_write.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        bytes_forwarded
    });
    
    // Main thread handles target -> client (minimal processing)
    let mut buf = [0u8; 16384];
    let mut bytes_received = 0u64;
    
    loop {
        match target_read.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                bytes_received += n as u64;
                if client_write.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    
    // Get forwarding stats with minimal overhead
    let bytes_sent = forward_thread.join().unwrap_or(0);
    println!("📊 NC Stats: {} ↑{}B ↓{}B -> {}", 
             client_addr, bytes_sent, bytes_received, target_addr);
    
    Ok(())
}

/// Handle connection with discretized routing (minimize traffic)
fn handle_discretized_connection(
    client: TcpStream, 
    listener_config: &DiscretizedListener,
    global_config: &OptimizedConfig
) -> io::Result<()> {
    let client_addr = client.peer_addr().unwrap_or_else(|_| "unknown".parse().unwrap());
    
    // Check if we can do direct NC forwarding (minimize processing)
    if let Some(nc_target) = listener_config.nc_forward {
        println!("🚀 Direct NC forward from port {} -> {}", 
                listener_config.port, nc_target);
        return minimal_nc_forward(client, nc_target);
    }
    
    // If no direct forward, check host-based routing (minimize traffic)
    let target = if let Some(host_route) = try_resolve_host_route(client_addr, global_config) {
        println!("🎯 Host-based route: {} -> {}", client_addr, host_route);
        host_route
    } else if let Some(default) = global_config.default_nc {
        println!("🔄 Default NC route: {} -> {}", client_addr, default);
        default
    } else {
        println!("🚫 No route available for {}", client_addr);
        return Ok(());
    };
    
    minimal_nc_forward(client, target)
}

/// Try to resolve host-specific routing to minimize traffic
fn try_resolve_host_route(client_addr: SocketAddr, config: &OptimizedConfig) -> Option<SocketAddr> {
    // Simple IP-based routing (could be enhanced with DNS/hostname lookup)
    let ip_str = client_addr.ip().to_string();
    
    // Check for known host patterns
    for (host_pattern, route) in &config.host_routes {
        if host_pattern.contains("mac") && ip_str.contains("192.168") {
            return Some(*route);
        }
        if host_pattern.contains("phone") && (ip_str.contains("127.0") || ip_str.contains("192.168")) {
            return Some(*route);
        }
        if host_pattern.contains("upstream") && !ip_str.starts_with("192.168") {
            return Some(*route);
        }
    }
    
    None
}

/// Start discretized listeners on separate ports (overlapping where beneficial)
fn start_discretized_listeners(config: OptimizedConfig) -> io::Result<()> {
    let config = Arc::new(RwLock::new(config));
    let mut handles = Vec::new();
    
    // Clone port listeners to avoid borrow issues
    let listeners = {
        let cfg = config.read().unwrap();
        cfg.port_listeners.clone()
    };
    
    for (port, listener_config) in listeners {
        let config_clone = Arc::clone(&config);
        let listener_config_clone = listener_config.clone();
        
        println!("🎧 Starting discretized listener on port {}", port);
        if !listener_config.target_hosts.is_empty() {
            println!("   📍 Target hosts: {:?}", listener_config.target_hosts);
        }
        if let Some(nc) = listener_config.nc_forward {
            println!("   🔄 NC Forward: -> {}", nc);
        }
        
        let handle = thread::spawn(move || {
            if let Err(e) = run_port_listener(port, listener_config_clone, config_clone) {
                println!("❌ Port {} listener error: {}", port, e);
            }
        });
        
        handles.push(handle);
    }
    
    println!("\n🚀 All discretized listeners started");
    println!("💡 Minimizing processing via NC forwarding");
    println!("💡 Minimizing traffic via host/port discretization");
    
    // Wait for all listeners
    for handle in handles {
        let _ = handle.join();
    }
    
    Ok(())
}

/// Run individual port listener
fn run_port_listener(
    port: u16, 
    listener_config: DiscretizedListener,
    global_config: Arc<RwLock<OptimizedConfig>>
) -> io::Result<()> {
    let bind_addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&bind_addr)?;
    
    println!("✅ Port {} listener ready", port);
    
    for stream in listener.incoming() {
        match stream {
            Ok(client) => {
                let listener_cfg = listener_config.clone();
                let global_cfg = {
                    global_config.read().unwrap().clone()
                };
                
                thread::spawn(move || {
                    if let Err(e) = handle_discretized_connection(client, &listener_cfg, &global_cfg) {
                        println!("⚠️  Connection handling error on port {}: {}", port, e);
                    }
                });
            }
            Err(e) => {
                println!("❌ Accept error on port {}: {}", port, e);
            }
        }
    }
    
    Ok(())
}

fn main() -> io::Result<()> {
    let config = OptimizedConfig::default();
    
    println!("⚡ Optimized LiteBike - Minimize Processing & Traffic");
    println!("====================================================");
    println!("🎯 Strategy: NC forwarding + discretized listeners");
    
    println!("\n📋 Discretized Listeners:");
    for (port, listener) in &config.port_listeners {
        println!("  Port {}: {:?}", port, listener);
    }
    
    println!("\n🌐 Host Routes:");
    for (host, route) in &config.host_routes {
        println!("  {} -> {}", host, route);
    }
    
    start_discretized_listeners(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discretized_config() {
        let config = OptimizedConfig::default();
        
        // Should have multiple port listeners
        assert!(config.port_listeners.len() > 1);
        
        // Should have 8888 as primary port
        assert!(config.port_listeners.contains_key(&8888));
        
        // Should have host routes for traffic minimization
        assert!(!config.host_routes.is_empty());
    }

    #[test]
    fn test_host_routing() {
        let config = OptimizedConfig::default();
        let mac_client: SocketAddr = "192.168.1.100:12345".parse().unwrap();
        
        // Should route Mac clients appropriately
        let route = try_resolve_host_route(mac_client, &config);
        assert!(route.is_some());
    }
}