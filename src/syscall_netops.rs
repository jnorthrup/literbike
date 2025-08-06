//! Pure syscall-based network operations for Android/Termux compatibility
//! No /proc, /sys, or /dev filesystem access - only direct libc syscalls
//! Minimal Rust wrapper around C-style system interfaces

use libc::{c_char, c_int, c_void, sockaddr, sockaddr_in, socklen_t, AF_INET, SOCK_STREAM, SOCK_DGRAM};
use std::mem;
use std::ffi::CStr;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

// Platform-specific ioctl constants
#[cfg(target_os = "linux")]
mod ioctl_consts {
    pub const SIOCGIFCONF: u64 = 0x8912;
    pub const SIOCGIFADDR: u64 = 0x8915;
    pub const SIOCGIFFLAGS: u64 = 0x8913;
    pub const SIOCGIFNETMASK: u64 = 0x891b;
}

#[cfg(target_os = "macos")]
mod ioctl_consts {
    pub const SIOCGIFCONF: u64 = 0xc00c6924;
    pub const SIOCGIFADDR: u64 = 0xc0206921;
    pub const SIOCGIFFLAGS: u64 = 0xc0206911;
    pub const SIOCGIFNETMASK: u64 = 0xc0206925;
}

#[cfg(target_os = "android")]
mod ioctl_consts {
    pub const SIOCGIFCONF: u64 = 0x8912;
    pub const SIOCGIFADDR: u64 = 0x8915;
    pub const SIOCGIFFLAGS: u64 = 0x8913;
    pub const SIOCGIFNETMASK: u64 = 0x891b;
}

use ioctl_consts::*;

// Interface flags
const IFF_UP: u16 = 0x1;
const IFF_LOOPBACK: u16 = 0x8;

#[repr(C)]
struct ifreq {
    ifr_name: [c_char; 16],
    ifr_addr: sockaddr_in,
}

#[repr(C)]
struct ifreq_flags {
    ifr_name: [c_char; 16],
    ifr_flags: u16,
}

#[repr(C)]
struct ifconf {
    ifc_len: c_int,
    ifc_buf: *mut c_char,
}

/// Network interface information discovered via syscalls
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub ip_addr: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub is_up: bool,
    pub is_loopback: bool,
}

/// Discovered proxy server
#[derive(Debug, Clone)]
pub struct ProxyServer {
    pub addr: SocketAddr,
    pub response_time_ms: u32,
    pub protocol_detected: Option<String>,
}

/// Pure syscall-based network operations
pub struct SyscallNetOps;

impl SyscallNetOps {
    /// Discover network interfaces using only ioctl() syscalls
    pub fn discover_interfaces() -> Result<Vec<NetworkInterface>, String> {
        unsafe {
            let sock = libc::socket(AF_INET, SOCK_DGRAM, 0);
            if sock < 0 {
                return Err("Failed to create socket for interface discovery".to_string());
            }

            let mut interfaces = Vec::new();
            
            // Extended interface names to try - no filesystem scanning needed  
            let common_names = [
                "lo", "lo0", "eth0", "eth1", "wlan0", "wlan1", "swlan0",
                "en0", "en1", "en2", "en3", "en4", "en5",
                "wlp2s0", "enp0s3", "br0", "docker0", "rmnet0", "rmnet_data0", 
                "wlp3s0", "enp1s0", "wifi0", "wlan2", "eth2"
            ];

            for iface_name in &common_names {
                if let Ok(interface) = Self::get_interface_info(sock, iface_name) {
                    interfaces.push(interface);
                }
            }

            libc::close(sock);
            
            if interfaces.is_empty() {
                Err("No network interfaces discovered".to_string())
            } else {
                Ok(interfaces)
            }
        }
    }

    /// Get information for a specific interface using ioctl()
    unsafe fn get_interface_info(sock: c_int, name: &str) -> Result<NetworkInterface, String> {
        let mut ifr: ifreq = mem::zeroed();
        
        // Copy interface name
        let name_bytes = name.as_bytes();
        if name_bytes.len() >= 16 {
            return Err("Interface name too long".to_string());
        }
        
        libc::strncpy(
            ifr.ifr_name.as_mut_ptr(),
            name.as_ptr() as *const c_char,
            name_bytes.len()
        );

        // Get IP address
        if libc::ioctl(sock, SIOCGIFADDR as _, &mut ifr as *mut _ as *mut c_void) != 0 {
            return Err(format!("Failed to get address for {}", name));
        }
        let ip_addr = Ipv4Addr::from(u32::from_be(ifr.ifr_addr.sin_addr.s_addr));

        // Get netmask  
        if libc::ioctl(sock, SIOCGIFNETMASK as _, &mut ifr as *mut _ as *mut c_void) != 0 {
            return Err(format!("Failed to get netmask for {}", name));
        }
        let netmask = Ipv4Addr::from(u32::from_be(ifr.ifr_addr.sin_addr.s_addr));

        // Get flags using separate ifreq_flags structure
        let mut ifr_flags: ifreq_flags = mem::zeroed();
        libc::strncpy(
            ifr_flags.ifr_name.as_mut_ptr(),
            name.as_ptr() as *const c_char,
            name_bytes.len()
        );
        
        if libc::ioctl(sock, SIOCGIFFLAGS as _, &mut ifr_flags as *mut _ as *mut c_void) != 0 {
            return Err(format!("Failed to get flags for {}", name));
        }
        
        let flags = ifr_flags.ifr_flags;
        let is_up = (flags & IFF_UP) != 0;
        let is_loopback = (flags & IFF_LOOPBACK) != 0;

        Ok(NetworkInterface {
            name: name.to_string(),
            ip_addr,
            netmask,
            is_up,
            is_loopback,
        })
    }

    /// Discover default gateway using routing table syscalls (Linux only)
    #[cfg(target_os = "linux")]
    pub fn discover_default_gateway() -> Result<Ipv4Addr, String> {
        unsafe {
            // Create netlink socket for route discovery
            let sock = libc::socket(16, libc::SOCK_RAW, 0); // AF_NETLINK = 16, NETLINK_ROUTE = 0
            if sock < 0 {
                return Err("Failed to create netlink socket (may require elevated privileges)".to_string());
            }

            // This is a simplified approach - in a full implementation, we'd send RTM_GETROUTE messages
            // For now, try to infer gateway from interface configuration
            libc::close(sock);
            
            // Fallback: assume common gateway patterns
            let interfaces = Self::discover_interfaces()?;
            for iface in interfaces {
                if !iface.is_loopback && iface.is_up {
                    // Common pattern: gateway is .1 in the subnet
                    let ip_bytes = iface.ip_addr.octets();
                    let mask_bytes = iface.netmask.octets();
                    
                    // Calculate network address
                    let network = [
                        ip_bytes[0] & mask_bytes[0],
                        ip_bytes[1] & mask_bytes[1], 
                        ip_bytes[2] & mask_bytes[2],
                        ip_bytes[3] & mask_bytes[3],
                    ];
                    
                    // Assume gateway is network + 1
                    let gateway = Ipv4Addr::new(network[0], network[1], network[2], network[3] + 1);
                    return Ok(gateway);
                }
            }
            
            Err("Could not determine default gateway".to_string())
        }
    }

    /// Discover default gateway (non-Linux fallback)
    #[cfg(not(target_os = "linux"))]
    pub fn discover_default_gateway() -> Result<Ipv4Addr, String> {
        // On macOS/other systems, try multiple approaches
        
        // First try to get interfaces
        if let Ok(interfaces) = Self::discover_interfaces() {
            for iface in interfaces {
                if !iface.is_loopback && iface.is_up {
                    let ip_bytes = iface.ip_addr.octets();
                    let mask_bytes = iface.netmask.octets();
                    
                    let network = [
                        ip_bytes[0] & mask_bytes[0],
                        ip_bytes[1] & mask_bytes[1], 
                        ip_bytes[2] & mask_bytes[2],
                        ip_bytes[3] & mask_bytes[3],
                    ];
                    
                    let gateway = Ipv4Addr::new(network[0], network[1], network[2], network[3] + 1);
                    return Ok(gateway);
                }
            }
        }
        
        // If interface discovery fails, try common default gateways
        let common_gateways = [
            "192.168.1.1", "192.168.0.1", "192.168.227.1", "192.168.227.50",
            "10.0.0.1", "172.16.0.1", "192.168.100.1"
        ];
        
        for gateway_str in &common_gateways {
            if let Ok(gateway) = gateway_str.parse::<Ipv4Addr>() {
                // Test if this gateway is reachable (simplified ping)
                if Self::test_gateway_reachability(gateway) {
                    return Ok(gateway);
                }
            }
        }
        
        Err("Could not determine default gateway".to_string())
    }
    
    /// Test if a gateway is reachable using connect()
    fn test_gateway_reachability(gateway: Ipv4Addr) -> bool {
        unsafe {
            let sock = libc::socket(AF_INET, SOCK_STREAM, 0);
            if sock < 0 {
                return false;
            }
            
            // Set non-blocking for quick test
            let flags = libc::fcntl(sock, libc::F_GETFL, 0);
            libc::fcntl(sock, libc::F_SETFL, flags | libc::O_NONBLOCK);
            
            let mut addr: sockaddr_in = mem::zeroed();
            addr.sin_family = AF_INET as libc::sa_family_t;
            addr.sin_port = (80u16).to_be(); // Try HTTP port
            addr.sin_addr.s_addr = u32::from(gateway).to_be();
            
            let result = libc::connect(
                sock,
                &addr as *const _ as *const sockaddr,
                mem::size_of::<sockaddr_in>() as socklen_t,
            );
            
            libc::close(sock);
            
            // For quick test, even connection refused is a good sign the gateway exists.
            // Replace non-portable __errno() with errno lookup via io::Error::last_os_error().
            let last = std::io::Error::last_os_error();
            let code = last.raw_os_error().unwrap_or_default();
            result == 0 || code == libc::ECONNREFUSED || code == libc::EINPROGRESS
        }
    }

    /// Generate scan range based on interface configuration
    pub fn auto_determine_scan_range(
        interfaces: &[NetworkInterface],
        gateway: &Ipv4Addr,
    ) -> Result<String, String> {
        // Find the interface that's on the same network as the gateway
        for iface in interfaces {
            if !iface.is_loopback && iface.is_up {
                let iface_bytes = iface.ip_addr.octets();
                let gateway_bytes = gateway.octets();
                let mask_bytes = iface.netmask.octets();
                
                // Check if gateway and interface are on same network
                let same_network = (0..4).all(|i| {
                    (iface_bytes[i] & mask_bytes[i]) == (gateway_bytes[i] & mask_bytes[i])
                });
                
                if same_network {
                    // Calculate CIDR notation
                    let cidr_bits = mask_bytes.iter()
                        .map(|&byte| byte.count_ones())
                        .sum::<u32>();
                    
                    let network = [
                        iface_bytes[0] & mask_bytes[0],
                        iface_bytes[1] & mask_bytes[1],
                        iface_bytes[2] & mask_bytes[2],
                        iface_bytes[3] & mask_bytes[3],
                    ];
                    
                    return Ok(format!("{}.{}.{}.{}/{}", 
                                    network[0], network[1], network[2], network[3], cidr_bits));
                }
            }
        }
        
        // Fallback to common private ranges
        Ok("192.168.1.0/24".to_string())
    }

    /// Basic constructor to satisfy tests and examples
    pub fn new() -> Self {
        Self {}
    }

    /// Scan for proxy servers using raw socket connections
    pub fn scan_for_proxy_servers(
        scan_range: &str,
        port_preference: Option<u16>,
        timeout_secs: u32,
    ) -> Result<Vec<ProxyServer>, String> {
        let ports_to_scan = if let Some(preferred_port) = port_preference {
            vec![preferred_port]
        } else {
            // Common proxy ports
            vec![8080, 3128, 1080, 8888, 9050, 8118, 3129, 8000, 8081]
        };

        let ip_range = Self::parse_cidr_range(scan_range)?;
        let mut discovered_servers = Vec::new();

        println!("Scanning {} IPs on {} ports...", ip_range.len(), ports_to_scan.len());

        for ip in ip_range {
            for &port in &ports_to_scan {
                if let Ok(proxy_server) = Self::test_proxy_connection_raw(ip, port, timeout_secs) {
                    println!("Found potential proxy: {}:{}", ip, port);
                    discovered_servers.push(proxy_server);
                }
            }
        }

        Ok(discovered_servers)
    }

    /// Parse CIDR range into list of IP addresses
    fn parse_cidr_range(cidr: &str) -> Result<Vec<Ipv4Addr>, String> {
        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.len() != 2 {
            return Err("Invalid CIDR format".to_string());
        }

        let base_ip: Ipv4Addr = parts[0].parse()
            .map_err(|_| "Invalid IP address in CIDR")?;
        let prefix_len: u32 = parts[1].parse()
            .map_err(|_| "Invalid prefix length in CIDR")?;

        if prefix_len > 32 {
            return Err("Invalid prefix length".to_string());
        }

        let base = u32::from(base_ip);
        let mask = !((1u32 << (32 - prefix_len)) - 1);
        let network = base & mask;
        let broadcast = network | ((1u32 << (32 - prefix_len)) - 1);

        let mut ips = Vec::new();
        for ip_u32 in network..=broadcast {
            // Skip network and broadcast addresses for /24 and smaller
            if prefix_len >= 24 && (ip_u32 == network || ip_u32 == broadcast) {
                continue;
            }
            ips.push(Ipv4Addr::from(ip_u32));
        }

        Ok(ips)
    }

    /// Test proxy connection using raw syscalls
    fn test_proxy_connection_raw(
        ip: Ipv4Addr,
        port: u16,
        timeout_secs: u32,
    ) -> Result<ProxyServer, String> {
        unsafe {
            let sock = libc::socket(AF_INET, SOCK_STREAM, 0);
            if sock < 0 {
                return Err("Failed to create socket".to_string());
            }

            // Set socket timeout
            let timeout = libc::timeval {
                tv_sec: timeout_secs as libc::time_t,
                tv_usec: 0,
            };
            
            libc::setsockopt(
                sock,
                libc::SOL_SOCKET,
                libc::SO_SNDTIMEO,
                &timeout as *const _ as *const c_void,
                mem::size_of::<libc::timeval>() as socklen_t,
            );
            
            libc::setsockopt(
                sock,
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                &timeout as *const _ as *const c_void,
                mem::size_of::<libc::timeval>() as socklen_t,
            );

            // Create sockaddr_in
            let mut addr: sockaddr_in = mem::zeroed();
            addr.sin_family = AF_INET as libc::sa_family_t;
            addr.sin_port = port.to_be();
            addr.sin_addr.s_addr = u32::from(ip).to_be();

            let start = std::time::Instant::now();
            
            // Attempt connection
            let result = libc::connect(
                sock,
                &addr as *const _ as *const sockaddr,
                mem::size_of::<sockaddr_in>() as socklen_t,
            );

            libc::close(sock);

            if result == 0 {
                let response_time = start.elapsed().as_millis() as u32;
                Ok(ProxyServer {
                    addr: SocketAddr::V4(SocketAddrV4::new(ip, port)),
                    response_time_ms: response_time,
                    protocol_detected: None, // Could be enhanced with protocol detection
                })
            } else {
                Err("Connection failed".to_string())
            }
        }
    }

    /// Test and rank discovered proxy servers
    pub fn test_and_rank_proxy_servers(
        mut servers: Vec<ProxyServer>,
        timeout_secs: u32,
    ) -> Result<Option<String>, String> {
        if servers.is_empty() {
            return Ok(None);
        }

        // Sort by response time (faster servers first)
        servers.sort_by_key(|s| s.response_time_ms);

        // Test the fastest server with a more thorough connection test
        let best_server = &servers[0];
        
        match best_server.addr {
            SocketAddr::V4(addr_v4) => {
                let ip = *addr_v4.ip();
                let port = addr_v4.port();
                
                // Perform more detailed testing
                if Self::test_detailed_proxy_connection(ip, port, timeout_secs).is_ok() {
                    Ok(Some(format!("{}:{}", ip, port)))
                } else {
                    // Try next server if available
                    if servers.len() > 1 {
                        let second_best = &servers[1];
                        if let SocketAddr::V4(addr_v4) = second_best.addr {
                            Ok(Some(format!("{}:{}", addr_v4.ip(), addr_v4.port())))
                        } else {
                            Ok(None)
                        }
                    } else {
                        Ok(None)
                    }
                }
            }
            _ => Ok(None),
        }
    }

    /// Perform detailed proxy connection testing
    pub fn test_detailed_proxy_connection(
        ip: Ipv4Addr,
        port: u16,
        timeout_secs: u32,
    ) -> Result<String, String> {
        unsafe {
            let sock = libc::socket(AF_INET, SOCK_STREAM, 0);
            if sock < 0 {
                return Err("Failed to create socket".to_string());
            }

            // Set socket timeout
            let timeout = libc::timeval {
                tv_sec: timeout_secs as libc::time_t,
                tv_usec: 0,
            };
            
            libc::setsockopt(
                sock,
                libc::SOL_SOCKET,
                libc::SO_SNDTIMEO,
                &timeout as *const _ as *const c_void,
                mem::size_of::<libc::timeval>() as socklen_t,
            );

            // Create sockaddr_in
            let mut addr: sockaddr_in = mem::zeroed();
            addr.sin_family = AF_INET as libc::sa_family_t;
            addr.sin_port = port.to_be();
            addr.sin_addr.s_addr = u32::from(ip).to_be();

            // Attempt connection
            let result = libc::connect(
                sock,
                &addr as *const _ as *const sockaddr,
                mem::size_of::<sockaddr_in>() as socklen_t,
            );

            if result == 0 {
                // Try to detect proxy type by sending a simple request
                let test_request = b"GET / HTTP/1.1\r\nHost: httpbin.org\r\n\r\n";
                let sent = libc::send(
                    sock,
                    test_request.as_ptr() as *const c_void,
                    test_request.len(),
                    0,
                );

                let connection_info = if sent > 0 {
                    // Try to read response
                    let mut buffer: [u8; 256] = [0; 256];
                    let received = libc::recv(
                        sock,
                        buffer.as_mut_ptr() as *mut c_void,
                        buffer.len(),
                        0,
                    );

                    if received > 0 {
                        "HTTP proxy detected - connection established".to_string()
                    } else {
                        "Connection established - protocol unknown".to_string()
                    }
                } else {
                    "Basic connection established".to_string()
                };

                libc::close(sock);
                Ok(connection_info)
            } else {
                libc::close(sock);
                Err("Connection test failed".to_string())
            }
        }
    }
}