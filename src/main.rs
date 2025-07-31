// LiteBike Proxy - Minimal form, strong function
// No frameworks, no abstractions, pure speed

use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;
#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;

// Hardcoded optimal configuration
const HTTP_PORT: u16 = 8080;
const SOCKS_PORT: u16 = 1080;
const BUFFER_SIZE: usize = 65536; // 64KB for maximum throughput
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

// Bind to specific interface on Linux
#[cfg(target_os = "linux")]
fn bind_to_interface(stream: &TcpStream, interface: &str) {
    unsafe {
        let fd = stream.as_raw_fd();
        let iface_bytes = interface.as_bytes();
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_BINDTODEVICE,
            iface_bytes.as_ptr() as *const libc::c_void,
            iface_bytes.len() as libc::socklen_t,
        );
    }
}

// Ultra-fast stream copy with zero-copy where possible
fn relay_streams(mut client: TcpStream, mut remote: TcpStream) {
    let mut client2 = client.try_clone().unwrap();
    let mut remote2 = remote.try_clone().unwrap();
    
    // Spawn reverse direction
    thread::spawn(move || {
        let mut buffer = vec![0u8; BUFFER_SIZE];
        loop {
            match remote2.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    if client2.write_all(&buffer[..n]).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    
    // Forward direction
    let mut buffer = vec![0u8; BUFFER_SIZE];
    loop {
        match client.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                if remote.write_all(&buffer[..n]).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

// Minimal HTTP CONNECT handler
fn handle_http(mut stream: TcpStream) {
    let mut buffer = [0u8; 4096];
    
    // Read request
    let n = match stream.read(&mut buffer) {
        Ok(n) => n,
        Err(_) => return,
    };
    
    let request = std::str::from_utf8(&buffer[..n]).unwrap_or("");
    
    // Extract host from CONNECT
    if request.starts_with("CONNECT ") {
        let parts: Vec<&str> = request.split_whitespace().collect();
        if parts.len() >= 2 {
            let host = parts[1];
            
            // Connect to target
            let target = if host.contains(':') {
                host.to_string()
            } else {
                format!("{}:443", host)
            };
            
            match TcpStream::connect_timeout(
                &target.parse::<std::net::SocketAddr>()
                    .or_else(|_| {
                        // Try to resolve hostname
                        use std::net::ToSocketAddrs;
                        target.to_socket_addrs()?.next()
                            .ok_or_else(|| std::io::Error::new(
                                std::io::ErrorKind::InvalidInput, 
                                "Failed to resolve host"
                            ))
                    })
                    .unwrap_or_else(|_| std::net::SocketAddr::from(([127,0,0,1], 80))),
                CONNECT_TIMEOUT,
            ) {
                Ok(remote) => {
                    // Bind to egress interface if available
                    #[cfg(target_os = "linux")]
                    if let Ok(iface) = std::env::var("EGRESS_INTERFACE") {
                        bind_to_interface(&remote, &iface);
                    }
                    
                    // Send 200 OK
                    let _ = stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n");
                    
                    // Relay data
                    relay_streams(stream, remote);
                }
                Err(_) => {
                    let _ = stream.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n");
                }
            }
        }
    } else {
        // Regular HTTP - parse and forward
        if let Some(host_line) = request.lines().find(|l| l.starts_with("Host: ")) {
            let host = host_line.strip_prefix("Host: ").unwrap_or("").trim();
            let target = format!("{}:80", host);
            
            if let Ok(mut remote) = TcpStream::connect_timeout(
                &target.parse::<std::net::SocketAddr>()
                    .or_else(|_| {
                        use std::net::ToSocketAddrs;
                        target.to_socket_addrs()?.next()
                            .ok_or_else(|| std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "Failed to resolve host"
                            ))
                    })
                    .unwrap_or_else(|_| std::net::SocketAddr::from(([127,0,0,1], 80))),
                CONNECT_TIMEOUT
            ) {
                #[cfg(target_os = "linux")]
                if let Ok(iface) = std::env::var("EGRESS_INTERFACE") {
                    bind_to_interface(&remote, &iface);
                }
                
                // Forward original request
                let _ = remote.write_all(&buffer[..n]);
                
                // Relay streams
                relay_streams(stream, remote);
            }
        }
    }
}

// Minimal SOCKS5 handler
fn handle_socks5(mut stream: TcpStream) {
    let mut buffer = [0u8; 512];
    
    // Read version and methods
    if stream.read_exact(&mut buffer[..2]).is_err() {
        return;
    }
    
    let nmethods = buffer[1] as usize;
    if stream.read_exact(&mut buffer[..nmethods]).is_err() {
        return;
    }
    
    // No auth
    if stream.write_all(&[5, 0]).is_err() {
        return;
    }
    
    // Read request
    if stream.read_exact(&mut buffer[..4]).is_err() {
        return;
    }
    
    let cmd = buffer[1];
    let atyp = buffer[3];
    
    if cmd != 1 { // Only CONNECT
        let _ = stream.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]);
        return;
    }
    
    // Parse address
    let addr = match atyp {
        1 => { // IPv4
            if stream.read_exact(&mut buffer[..6]).is_err() {
                return;
            }
            let ip = format!("{}.{}.{}.{}", buffer[0], buffer[1], buffer[2], buffer[3]);
            let port = u16::from_be_bytes([buffer[4], buffer[5]]);
            format!("{}:{}", ip, port)
        }
        3 => { // Domain
            if stream.read_exact(&mut buffer[..1]).is_err() {
                return;
            }
            let len = buffer[0] as usize;
            if stream.read_exact(&mut buffer[..len + 2]).is_err() {
                return;
            }
            let domain = String::from_utf8_lossy(&buffer[..len]);
            let port = u16::from_be_bytes([buffer[len], buffer[len + 1]]);
            format!("{}:{}", domain, port)
        }
        _ => return,
    };
    
    // Connect to target
    match TcpStream::connect_timeout(
        &addr.parse::<std::net::SocketAddr>()
            .or_else(|_| {
                use std::net::ToSocketAddrs;
                addr.to_socket_addrs()?.next()
                    .ok_or_else(|| std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Failed to resolve host"
                    ))
            })
            .unwrap_or_else(|_| std::net::SocketAddr::from(([127,0,0,1], 80))),
        CONNECT_TIMEOUT
    ) {
        Ok(remote) => {
            #[cfg(target_os = "linux")]
            if let Ok(iface) = std::env::var("EGRESS_INTERFACE") {
                bind_to_interface(&remote, &iface);
            }
            
            // Success response
            let _ = stream.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
            
            // Relay
            relay_streams(stream, remote);
        }
        Err(_) => {
            let _ = stream.write_all(&[5, 1, 0, 1, 0, 0, 0, 0, 0, 0]);
        }
    }
}

// Get IP address from interface name
fn get_interface_ip(iface_name: &str) -> Option<String> {
    use std::process::Command;
    
    let output = Command::new("ip")
        .args(&["addr", "show", iface_name])
        .output()
        .ok()?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Parse IP from output like: "inet 192.168.1.100/24 brd..."
    for line in stdout.lines() {
        if line.contains("inet ") && !line.contains("inet6") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                // Extract IP without CIDR notation
                if let Some(ip) = parts[1].split('/').next() {
                    return Some(ip.to_string());
                }
            }
        }
    }
    None
}

fn main() {
    // Get bind address from env or interface name
    let bind_ip = if let Ok(iface) = std::env::var("INGRESS_INTERFACE") {
        // Get IP from interface name
        get_interface_ip(&iface).unwrap_or_else(|| "127.0.0.1".to_string())
    } else {
        std::env::var("BIND_IP").unwrap_or_else(|_| "127.0.0.1".to_string())
    };
    
    // HTTP proxy thread
    let http_addr = format!("{}:{}", bind_ip, HTTP_PORT);
    thread::spawn(move || {
        let listener = TcpListener::bind(&http_addr).expect("Failed to bind HTTP");
        println!("HTTP proxy on {}", http_addr);
        
        for stream in listener.incoming() {
            if let Ok(stream) = stream {
                thread::spawn(move || handle_http(stream));
            }
        }
    });
    
    // SOCKS5 proxy
    let socks_addr = format!("{}:{}", bind_ip, SOCKS_PORT);
    let listener = TcpListener::bind(&socks_addr).expect("Failed to bind SOCKS5");
    println!("SOCKS5 proxy on {}", socks_addr);
    
    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            thread::spawn(move || handle_socks5(stream));
        }
    }
}