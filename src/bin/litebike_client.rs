//! Symmetric LiteBike Client - Route Discovery and Host Detection
//! Uses ONLY direct syscalls via libc for Android/Termux compatibility
//! No /proc, /sys, or /dev filesystem access
//! Integrates existing netutils functionality for full network management

use libc::{c_char, c_int, c_void, sockaddr_in, sockaddr, AF_INET, SOCK_DGRAM, IPPROTO_UDP};
use std::ffi::CStr;
use std::mem;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::io::{self, Write, BufRead, BufReader, Read};
use std::process::Command;
use std::collections::HashMap;

#[cfg(feature = "auto-discovery")]
use litebike::bonjour::BonjourClient;

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

#[repr(C)]
struct ifreq {
    ifr_name: [c_char; 16],
    ifr_data: [u8; 24], // Union for different address types
}

#[repr(C)]
struct ifconf {
    ifc_len: c_int,
    ifc_req: *mut ifreq,
}

/// Network interface information
#[derive(Debug, Clone)]
struct NetworkInterface {
    name: String,
    addr: Option<Ipv4Addr>,
    flags: u32,
    is_rmnet: bool,
}

/// Route information discovered via syscalls
#[derive(Debug, Clone)]
struct RouteInfo {
    gateway: Option<Ipv4Addr>,
    interface: String,
    metric: u32,
}

/// LiteBike client for symmetric proxy connections
struct LitebikeClient {
    interfaces: Vec<NetworkInterface>,
    default_route: Option<RouteInfo>,
    target_host: Option<Ipv4Addr>,
    repl_session: Option<std::net::TcpStream>,
    netutils_cache: HashMap<String, String>,
}

impl LitebikeClient {
    /// Create new client with syscall-based network discovery
    unsafe fn new() -> io::Result<Self> {
        let mut client = Self {
            interfaces: Vec::new(),
            default_route: None,
            target_host: None,
            repl_session: None,
            netutils_cache: HashMap::new(),
        };
        
        client.discover_interfaces()?;
        client.discover_default_route()?;
        client.cache_netutils_info();
        
        Ok(client)
    }
    
    /// Discover network interfaces using syscalls only
    unsafe fn discover_interfaces(&mut self) -> io::Result<()> {
        let sock = libc::socket(AF_INET, SOCK_DGRAM, 0);
        if sock < 0 {
            return Err(io::Error::last_os_error());
        }
        
        // Allocate buffer for interface list
        const MAX_IFACES: usize = 64;
        let mut ifreqs: [ifreq; MAX_IFACES] = mem::zeroed();
        let mut ifc = ifconf {
            ifc_len: (MAX_IFACES * mem::size_of::<ifreq>()) as c_int,
            ifc_req: ifreqs.as_mut_ptr(),
        };
        
        // Get interface list via SIOCGIFCONF
        if libc::ioctl(sock, SIOCGIFCONF as _, &mut ifc as *mut _ as *mut c_void) < 0 {
            libc::close(sock);
            return Err(io::Error::last_os_error());
        }
        
        let num_ifaces = (ifc.ifc_len as usize) / mem::size_of::<ifreq>();
        
        for i in 0..num_ifaces {
            let ifr = &ifreqs[i];
            let name_bytes = CStr::from_ptr(ifr.ifr_name.as_ptr()).to_bytes();
            let name = String::from_utf8_lossy(name_bytes).to_string();
            
            // Skip empty or invalid names
            if name.is_empty() || name.starts_with('\0') {
                continue;
            }
            
            // Get interface address
            let mut addr_ifr: ifreq = mem::zeroed();
            libc::strcpy(addr_ifr.ifr_name.as_mut_ptr(), ifr.ifr_name.as_ptr());
            
            let addr = if libc::ioctl(sock, SIOCGIFADDR as _, &mut addr_ifr as *mut _ as *mut c_void) == 0 {
                let sin: &sockaddr_in = unsafe { &*(addr_ifr.ifr_data.as_ptr() as *const sockaddr_in) };
                let ip_bytes = sin.sin_addr.s_addr.to_ne_bytes();
                Some(Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]))
            } else {
                None
            };
            
            // Get interface flags
            let mut flags_ifr: ifreq = mem::zeroed();
            libc::strcpy(flags_ifr.ifr_name.as_mut_ptr(), ifr.ifr_name.as_ptr());
            
            let flags = if libc::ioctl(sock, SIOCGIFFLAGS as _, &mut flags_ifr as *mut _ as *mut c_void) == 0 {
                // flags are in a union, so we need to cast
                let flags_val: u16 = unsafe { *(flags_ifr.ifr_data.as_ptr() as *const u16) };
                flags_val as u32
            } else {
                0
            };
            
            // Check if this is an rmnet interface (Android mobile data)
            let is_rmnet = name.starts_with("rmnet");
            
            self.interfaces.push(NetworkInterface {
                name,
                addr,
                flags,
                is_rmnet,
            });
        }
        
        libc::close(sock);
        Ok(())
    }
    
    /// Discover default route to 8.8.8.8 using socket syscalls
    unsafe fn discover_default_route(&mut self) -> io::Result<()> {
        // Create UDP socket to 8.8.8.8:53 to discover routing
        let sock = libc::socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
        if sock < 0 {
            return Err(io::Error::last_os_error());
        }
        
        // Connect to 8.8.8.8:53 to trigger route resolution
        let mut dest_addr: sockaddr_in = mem::zeroed();
        dest_addr.sin_family = AF_INET as _;
        dest_addr.sin_port = 53u16.to_be(); // DNS port
        dest_addr.sin_addr.s_addr = u32::from(0x08080808u32).to_be(); // 8.8.8.8
        
        if libc::connect(sock, &dest_addr as *const _ as *const sockaddr, mem::size_of::<sockaddr_in>() as u32) < 0 {
            libc::close(sock);
            return Err(io::Error::last_os_error());
        }
        
        // Get local address that would be used for this connection
        let mut local_addr: sockaddr_in = mem::zeroed();
        let mut addr_len = mem::size_of::<sockaddr_in>() as u32;
        
        if libc::getsockname(sock, &mut local_addr as *mut _ as *mut sockaddr, &mut addr_len) < 0 {
            libc::close(sock);
            return Err(io::Error::last_os_error());
        }
        
        let local_ip_bytes = local_addr.sin_addr.s_addr.to_ne_bytes();
        let local_ip = Ipv4Addr::new(local_ip_bytes[0], local_ip_bytes[1], local_ip_bytes[2], local_ip_bytes[3]);
        
        // Find the interface that has this local IP
        let interface_name = self.interfaces.iter()
            .find(|iface| iface.addr == Some(local_ip))
            .map(|iface| iface.name.clone())
            .unwrap_or_else(|| "unknown".to_string());
        
        // Calculate likely gateway (typically .1 of the subnet)
        let gateway = if !local_ip.is_loopback() {
            let octets = local_ip.octets();
            Some(Ipv4Addr::new(octets[0], octets[1], octets[2], 1))
        } else {
            None
        };
        
        self.default_route = Some(RouteInfo {
            gateway,
            interface: interface_name,
            metric: 0,
        });
        
        // Set target host as the gateway
        self.target_host = gateway;
        
        libc::close(sock);
        Ok(())
    }
    
    /// Auto-discover litebike peers using Bonjour/mDNS
    #[cfg(feature = "auto-discovery")]
    async fn discover_peers(&mut self) -> io::Result<Ipv4Addr> {
        let peers = BonjourClient::discover_peers().await?;
        if let Some(peer) = peers.first() {
            if let Some(addr) = peer.ipv4_addr {
                println!("✓ Found litebike peer at {}", addr);
                self.target_host = Some(addr);
                return Ok(addr);
            }
        }
        
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No litebike peers found on network"
        ))
    }
    
    /// Test if litebike is running on the given address
    fn test_litebike_connection(&self, addr: Ipv4Addr, port: u16) -> bool {
        use std::net::TcpStream;
        use std::time::Duration;
        
        let socket_addr = SocketAddr::new(IpAddr::V4(addr), port);
        
        // Try to connect with short timeout
        match TcpStream::connect_timeout(&socket_addr, Duration::from_millis(500)) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    
    /// Cache network utilities information for REPL sharing
    fn cache_netutils_info(&mut self) {
        // Cache interface information
        let mut ifconfig_output = String::new();
        for iface in &self.interfaces {
            let addr_str = iface.addr
                .map(|a| a.to_string())
                .unwrap_or_else(|| "N/A".to_string());
            let status = if iface.is_rmnet { " (rmnet)" } else { "" };
            ifconfig_output.push_str(&format!("{}: {}{}\n", iface.name, addr_str, status));
        }
        self.netutils_cache.insert("ifconfig".to_string(), ifconfig_output);
        
        // Cache route information
        if let Some(ref route) = self.default_route {
            let route_output = format!(
                "Default route: {} via {} dev {}\n",
                "0.0.0.0/0",
                route.gateway.map(|g| g.to_string()).unwrap_or_else(|| "direct".to_string()),
                route.interface
            );
            self.netutils_cache.insert("route".to_string(), route_output);
        }
    }
    
    /// Attempt to connect to litebike REPL over HTTP
    fn connect_repl(&mut self, host: Ipv4Addr) -> io::Result<()> {
        println!("Connecting to litebike REPL at http://{}:8888/repl", host);
        
        // Create persistent REPL connection
        use std::net::TcpStream;
        
        let stream = TcpStream::connect(SocketAddr::new(IpAddr::V4(host), 8888))?;
        
        // Test connection with ping
        let test_request = format!(
            "POST /repl HTTP/1.1\r\n\
             Host: {}:8888\r\n\
             Content-Type: application/json\r\n\
             Content-Length: 27\r\n\
             \r\n\
             {{\"command\": \"ping\", \"args\": []}}",
            host
        );
        
        let mut test_stream = stream.try_clone()?;
        test_stream.write_all(test_request.as_bytes())?;
        
        // Read response
        let mut reader = BufReader::new(&test_stream);
        let mut response = String::new();
        reader.read_line(&mut response)?;
        
        if response.contains("200 OK") {
            println!("✓ Connected to litebike REPL");
            self.repl_session = Some(stream);
            self.start_interactive_repl(host)
        } else {
            Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "REPL connection failed"
            ))
        }
    }
    
    /// Start interactive REPL session
    fn start_interactive_repl(&mut self, host: Ipv4Addr) -> io::Result<()> {
        println!("\n=== LiteBike Symmetric REPL ===");
        println!("Connected to litebike server at {}", host);
        println!("Available commands: ifconfig, route, ip, netstat, ping, exit");
        println!("Type 'help' for more information\n");
        
        let stdin = io::stdin();
        loop {
            print!("litebike@{}> ", host);
            io::stdout().flush()?;
            
            let mut input = String::new();
            stdin.read_line(&mut input)?;
            let input = input.trim();
            
            if input.is_empty() {
                continue;
            }
            
            if input == "exit" || input == "quit" {
                println!("Goodbye!");
                break;
            }
            
            match self.execute_command(host, input) {
                Ok(output) => {
                    if !output.is_empty() {
                        println!("{}", output);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Execute command via REPL
    fn execute_command(&mut self, host: Ipv4Addr, command: &str) -> io::Result<String> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(String::new());
        }
        
        let cmd = parts[0];
        let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
        
        // Handle local commands first
        match cmd {
            "help" => {
                return Ok(format!(
                    "LiteBike Symmetric REPL Commands:\n\
                     ifconfig       - Show network interfaces\n\
                     route          - Show routing table\n\
                     ip [addr|route] - Show IP configuration\n\
                     netstat        - Show network connections\n\
                     ping <host>    - Ping a host\n\
                     local-ifconfig - Show local client interfaces\n\
                     local-route    - Show local client routing\n\
                     exit/quit      - Disconnect from REPL"
                ));
            }
            "local-ifconfig" => {
                return Ok(self.netutils_cache.get("ifconfig").cloned().unwrap_or_else(|| "No local interface data".to_string()));
            }
            "local-route" => {
                return Ok(self.netutils_cache.get("route").cloned().unwrap_or_else(|| "No local route data".to_string()));
            }
            _ => {}
        }
        
        // Send command to remote server
        if let Some(ref mut stream) = self.repl_session {
            let request_body = format!(
                "{{\"command\": \"{}\", \"args\": {}}}",
                cmd,
                serde_json_simple(&args)
            );
            
            let request = format!(
                "POST /repl HTTP/1.1\r\n\
                 Host: {}:8888\r\n\
                 Content-Type: application/json\r\n\
                 Content-Length: {}\r\n\
                 \r\n\
                 {}",
                host,
                request_body.len(),
                request_body
            );
            
            stream.write_all(request.as_bytes())?;
            
            // Read response
            let mut reader = BufReader::new(stream);
            let mut response_line = String::new();
            reader.read_line(&mut response_line)?;
            
            // Skip headers until empty line
            let mut content_length = 0;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header)?;
                if header.trim().is_empty() {
                    break;
                }
                if header.to_lowercase().starts_with("content-length:") {
                    if let Some(len_str) = header.split(':').nth(1) {
                        content_length = len_str.trim().parse().unwrap_or(0);
                    }
                }
            }
            
            // Read response body
            if content_length > 0 {
                let mut buffer = vec![0u8; content_length];
                reader.read_exact(&mut buffer)?;
                Ok(String::from_utf8_lossy(&buffer).to_string())
            } else {
                Ok("Command executed (no output)".to_string())
            }
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "No active REPL session"
            ))
        }
    }
    
    /// Attempt SSH connection to launch litebike cmdline
    fn connect_ssh(&self, host: Ipv4Addr) -> io::Result<()> {
        println!("Attempting SSH connection to {}...", host);
        
        // Use system SSH command
        let output = Command::new("ssh")
            .args(&[
                "-o", "ConnectTimeout=5",
                "-o", "StrictHostKeyChecking=no",
                &format!("u0_a471@{}", host),
                "-p", "8022",
                "litebike"
            ])
            .output()?;
        
        if output.status.success() {
            println!("✓ SSH connection successful");
            Ok(())
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                format!("SSH failed: {}", error)
            ))
        }
    }
    
    /// Print discovered network information
    fn print_network_info(&self) {
        println!("=== Network Discovery Results ===");
        
        // Print interfaces
        println!("\nInterfaces:");
        for iface in &self.interfaces {
            let addr_str = iface.addr
                .map(|a| a.to_string())
                .unwrap_or_else(|| "N/A".to_string());
            
            let status = if iface.is_rmnet { " (rmnet)" } else { "" };
            println!("  {}: {}{}", iface.name, addr_str, status);
        }
        
        // Print route info
        if let Some(ref route) = self.default_route {
            println!("\nDefault Route:");
            println!("  Interface: {}", route.interface);
            if let Some(gw) = route.gateway {
                println!("  Gateway: {}", gw);
            }
        }
        
        // Print target
        if let Some(host) = self.target_host {
            println!("\nTarget Host: {}", host);
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 && args[1] == "/client" {
        println!("LiteBike Symmetric Client - P2P Discovery Mode");
        
        // Create client with network discovery
        let mut client = unsafe {
            LitebikeClient::new().unwrap_or_else(|e| {
                eprintln!("Failed to initialize client: {}", e);
                std::process::exit(1);
            })
        };
        
        // Print discovery results
        client.print_network_info();
        
        // Discover litebike peers
        #[cfg(feature = "auto-discovery")]
        let host = client.discover_peers().await.unwrap_or_else(|e| {
            eprintln!("Peer discovery failed: {}", e);
            std::process::exit(1);
        });
        
        println!("\n=== Connection Attempts ===");
        
        // Try REPL connection first
        match client.connect_repl(host) {
            Ok(_) => {
                println!("✓ REPL connection established");
                return;
            }
            Err(e) => {
                println!("✗ REPL connection failed: {}", e);
            }
        }
        
        // Fallback to SSH
        match client.connect_ssh(host) {
            Ok(_) => {
                println!("✓ SSH connection established");
            }
            Err(e) => {
                println!("✗ SSH connection failed: {}", e);
                println!("Manual connection required:");
                println!("  ssh -L 8888:{}:8888 u0_a471@{} -p 8022", host, host);
                std::process::exit(1);
            }
        }
    } else {
        println!("Usage: {} /client", args[0]);
        println!("  /client - Discover and connect to litebike server");
        std::process::exit(1);
    }
}

/// Simple JSON serialization for Vec<String>
fn serde_json_simple(args: &[String]) -> String {
    let quoted_args: Vec<String> = args.iter()
        .map(|s| format!("\"{}\"", s.replace('"', "\\\"")))
        .collect();
    format!("[{}]", quoted_args.join(", "))
}