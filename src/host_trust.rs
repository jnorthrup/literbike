// LITERBIKE Direct Host Trust Mechanisms
// For private networks and carrier freedom environments

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::time::Duration;
use std::fs;
use std::path::Path;

/// Host trust manager for literbike carrier freedom
pub struct HostTrust {
    trusted_hosts: HashMap<String, TrustLevel>,
    trusted_networks: Vec<TrustedNetwork>,
    trust_policies: TrustPolicies,
    auto_trust_private: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrustLevel {
    /// Full trust - no verification, direct connection
    Full,
    /// Basic trust - minimal verification
    Basic,
    /// Conditional trust - verify specific conditions
    Conditional(Vec<TrustCondition>),
    /// Untrusted - apply all security measures
    Untrusted,
}

#[derive(Debug, Clone)]
pub struct TrustedNetwork {
    pub network: String,
    pub mask: u8,
    pub level: TrustLevel,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct TrustPolicies {
    pub trust_private_networks: bool,
    pub trust_local_subnet: bool,
    pub trust_known_hosts: bool,
    pub auto_trust_ssh_known: bool,
    pub trust_upnp_devices: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrustCondition {
    PortOpen(u16),
    ServiceRunning(String),
    SshKeyPresent,
    HttpResponseContains(String),
    UPnPDevice,
}

impl HostTrust {
    pub fn new() -> Self {
        Self {
            trusted_hosts: HashMap::new(),
            trusted_networks: Self::default_trusted_networks(),
            trust_policies: TrustPolicies::carrier_freedom_defaults(),
            auto_trust_private: true,
        }
    }
    
    /// Default trusted networks for carrier freedom
    fn default_trusted_networks() -> Vec<TrustedNetwork> {
        vec![
            // RFC 1918 private networks - trust by default for carrier freedom
            TrustedNetwork {
                network: "10.0.0.0".to_string(),
                mask: 8,
                level: TrustLevel::Full,
                description: "RFC1918 Class A private".to_string(),
            },
            TrustedNetwork {
                network: "172.16.0.0".to_string(),
                mask: 12,
                level: TrustLevel::Full,
                description: "RFC1918 Class B private".to_string(),
            },
            TrustedNetwork {
                network: "192.168.0.0".to_string(),
                mask: 16,
                level: TrustLevel::Full,
                description: "RFC1918 Class C private".to_string(),
            },
            // Carrier CGNAT ranges - trust for tethering bypass
            TrustedNetwork {
                network: "100.64.0.0".to_string(),
                mask: 10,
                level: TrustLevel::Basic,
                description: "CGNAT carrier networks".to_string(),
            },
            // Link-local
            TrustedNetwork {
                network: "169.254.0.0".to_string(),
                mask: 16,
                level: TrustLevel::Full,
                description: "Link-local auto-config".to_string(),
            },
            // Loopback
            TrustedNetwork {
                network: "127.0.0.0".to_string(),
                mask: 8,
                level: TrustLevel::Full,
                description: "Loopback".to_string(),
            },
            // IPv6 private ranges
            TrustedNetwork {
                network: "fc00::".to_string(),
                mask: 7,
                level: TrustLevel::Full,
                description: "IPv6 unique local".to_string(),
            },
            TrustedNetwork {
                network: "fe80::".to_string(),
                mask: 10,
                level: TrustLevel::Full,
                description: "IPv6 link-local".to_string(),
            },
        ]
    }
    
    /// Check if host should be trusted
    pub fn should_trust(&self, host: &str) -> TrustLevel {
        println!("🔍 Evaluating trust for host: {}", host);
        
        // Check explicit host trust
        if let Some(level) = self.trusted_hosts.get(host) {
            println!("✓ Host {} explicitly trusted: {:?}", host, level);
            return level.clone();
        }
        
        // Parse IP address from host
        if let Ok(ip) = host.parse::<IpAddr>() {
            // Check network-based trust
            if let Some(network_trust) = self.check_network_trust(&ip) {
                println!("✓ Host {} trusted via network: {:?}", host, network_trust);
                return network_trust;
            }
        }
        
        // Try to resolve hostname to IP
        if let Ok(addrs) = std::net::ToSocketAddrs::to_socket_addrs(&format!("{}:80", host)) {
            for addr in addrs {
                if let Some(network_trust) = self.check_network_trust(&addr.ip()) {
                    println!("✓ Host {} trusted via resolved IP: {:?}", host, network_trust);
                    return network_trust;
                }
            }
        }
        
        // Check SSH known hosts if enabled
        if self.trust_policies.auto_trust_ssh_known {
            if self.is_ssh_known_host(host) {
                println!("✓ Host {} trusted via SSH known_hosts", host);
                return TrustLevel::Basic;
            }
        }
        
        // Check for UPnP devices if enabled
        if self.trust_policies.trust_upnp_devices {
            if self.is_upnp_device(host) {
                println!("✓ Host {} trusted as UPnP device", host);
                return TrustLevel::Basic;
            }
        }
        
        // Default: untrusted
        println!("⚠ Host {} not trusted", host);
        TrustLevel::Untrusted
    }
    
    /// Check if IP is in a trusted network
    fn check_network_trust(&self, ip: &IpAddr) -> Option<TrustLevel> {
        for network in &self.trusted_networks {
            if self.ip_in_network(ip, &network.network, network.mask) {
                return Some(network.level.clone());
            }
        }
        None
    }
    
    /// Check if IP is in the specified network/mask
    fn ip_in_network(&self, ip: &IpAddr, network: &str, mask: u8) -> bool {
        match (ip, network.parse::<IpAddr>()) {
            (IpAddr::V4(ip4), Ok(IpAddr::V4(net4))) => {
                let ip_int = u32::from(*ip4);
                let net_int = u32::from(net4);
                let mask_int = !((1u32 << (32 - mask)) - 1);
                (ip_int & mask_int) == (net_int & mask_int)
            }
            (IpAddr::V6(ip6), Ok(IpAddr::V6(net6))) => {
                let ip_bytes = ip6.octets();
                let net_bytes = net6.octets();
                let bytes_to_check = (mask / 8) as usize;
                let bits_to_check = mask % 8;
                
                // Check full bytes
                if ip_bytes[..bytes_to_check] != net_bytes[..bytes_to_check] {
                    return false;
                }
                
                // Check partial byte if needed
                if bits_to_check > 0 && bytes_to_check < 16 {
                    let mask_byte = 0xFF << (8 - bits_to_check);
                    return (ip_bytes[bytes_to_check] & mask_byte) == 
                           (net_bytes[bytes_to_check] & mask_byte);
                }
                
                true
            }
            _ => false,
        }
    }
    
    /// Check if host is in SSH known_hosts
    fn is_ssh_known_host(&self, host: &str) -> bool {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let known_hosts_path = Path::new(&home).join(".ssh/known_hosts");
        
        if let Ok(content) = fs::read_to_string(known_hosts_path) {
            for line in content.lines() {
                if line.trim().is_empty() || line.starts_with('#') {
                    continue;
                }
                
                // Parse known_hosts format: host[,host2] keytype key
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(hosts) = parts.get(0) {
                    for known_host in hosts.split(',') {
                        if known_host.trim() == host {
                            return true;
                        }
                    }
                }
            }
        }
        
        false
    }
    
    /// Check if host is a UPnP device
    fn is_upnp_device(&self, host: &str) -> bool {
        // Quick UPnP detection - check for common UPnP ports
        let upnp_ports = [1900, 5000, 49152];
        
        for port in upnp_ports {
            let addr = format!("{}:{}", host, port);
            if let Ok(addr) = addr.parse::<SocketAddr>() {
                if TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok() {
                    return true;
                }
            }
        }
        
        false
    }
    
    /// Add explicit host trust
    pub fn trust_host(&mut self, host: &str, level: TrustLevel) {
        println!("✅ Adding explicit trust for {}: {:?}", host, level);
        self.trusted_hosts.insert(host.to_string(), level);
    }
    
    /// Remove host trust
    pub fn untrust_host(&mut self, host: &str) {
        println!("❌ Removing trust for {}", host);
        self.trusted_hosts.remove(host);
    }
    
    /// Discover and auto-trust hosts on local network
    pub fn discover_local_hosts(&mut self) -> Result<Vec<String>, String> {
        println!("🔍 Discovering local network hosts for auto-trust");
        
        let local_ip = crate::syscall_net::get_default_local_ipv4()
            .map_err(|e| format!("Failed to get local IP: {}", e))?;
        
        // Determine local subnet
        let subnet = self.get_local_subnet(&local_ip)?;
        let mut discovered_hosts = Vec::new();
        
        // Scan local subnet for responsive hosts
        for host_octet in 1..255 {
            let test_ip = Ipv4Addr::new(
                subnet.0, subnet.1, subnet.2, host_octet
            );
            
            // Quick connectivity test
            if self.test_host_connectivity(&test_ip.to_string()) {
                let host_str = test_ip.to_string();
                discovered_hosts.push(host_str.clone());
                
                // Auto-trust private network hosts
                if self.trust_policies.trust_local_subnet {
                    self.trust_host(&host_str, TrustLevel::Basic);
                    println!("✓ Auto-trusted local host: {}", host_str);
                }
            }
        }
        
        println!("📊 Discovered {} hosts on local network", discovered_hosts.len());
        Ok(discovered_hosts)
    }
    
    /// Get local subnet from IP
    fn get_local_subnet(&self, ip: &Ipv4Addr) -> Result<(u8, u8, u8, u8), String> {
        let octets = ip.octets();
        // Assume /24 subnet for simplicity
        Ok((octets[0], octets[1], octets[2], 0))
    }
    
    /// Test basic connectivity to host
    fn test_host_connectivity(&self, host: &str) -> bool {
        let test_ports = [22, 80, 443, 8080];
        
        for port in test_ports {
            let addr = format!("{}:{}", host, port);
            if let Ok(addr) = addr.parse::<SocketAddr>() {
                if TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok() {
                    return true;
                }
            }
        }
        
        false
    }
    
    /// Load trusted hosts from file
    pub fn load_trusted_hosts(&mut self, path: &str) -> Result<(), String> {
        println!("📖 Loading trusted hosts from {}", path);
        
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read trust file: {}", e))?;
        
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            // Format: host:trust_level
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 2 {
                let host = parts[0].trim();
                let level = match parts[1].trim().to_lowercase().as_str() {
                    "full" => TrustLevel::Full,
                    "basic" => TrustLevel::Basic,
                    "untrusted" => TrustLevel::Untrusted,
                    _ => TrustLevel::Basic, // Default
                };
                
                self.trust_host(host, level);
            }
        }
        
        println!("✅ Loaded {} trusted hosts", self.trusted_hosts.len());
        Ok(())
    }
    
    /// Save trusted hosts to file
    pub fn save_trusted_hosts(&self, path: &str) -> Result<(), String> {
        println!("💾 Saving trusted hosts to {}", path);
        
        let mut content = String::new();
    content.push_str("# LiterBike Trusted Hosts\n");
        content.push_str("# Format: host:trust_level\n\n");
        
        for (host, level) in &self.trusted_hosts {
            let level_str = match level {
                TrustLevel::Full => "full",
                TrustLevel::Basic => "basic",
                TrustLevel::Conditional(_) => "conditional",
                TrustLevel::Untrusted => "untrusted",
            };
            content.push_str(&format!("{}:{}\n", host, level_str));
        }
        
        fs::write(path, content)
            .map_err(|e| format!("Failed to write trust file: {}", e))?;
        
        println!("✅ Saved {} trusted hosts", self.trusted_hosts.len());
        Ok(())
    }
    
    /// Get connection strategy based on trust level
    pub fn get_connection_strategy(&self, host: &str) -> ConnectionStrategy {
        match self.should_trust(host) {
            TrustLevel::Full => ConnectionStrategy {
                verify_certificates: false,
                use_encryption: false,
                timeout: Duration::from_secs(30),
                retries: 3,
                bypass_proxy: true,
                direct_connection: true,
            },
            TrustLevel::Basic => ConnectionStrategy {
                verify_certificates: false,
                use_encryption: true,
                timeout: Duration::from_secs(20),
                retries: 2,
                bypass_proxy: true,
                direct_connection: true,
            },
            TrustLevel::Conditional(_) => ConnectionStrategy {
                verify_certificates: true,
                use_encryption: true,
                timeout: Duration::from_secs(15),
                retries: 2,
                bypass_proxy: false,
                direct_connection: false,
            },
            TrustLevel::Untrusted => ConnectionStrategy {
                verify_certificates: true,
                use_encryption: true,
                timeout: Duration::from_secs(10),
                retries: 1,
                bypass_proxy: false,
                direct_connection: false,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionStrategy {
    pub verify_certificates: bool,
    pub use_encryption: bool,
    pub timeout: Duration,
    pub retries: u32,
    pub bypass_proxy: bool,
    pub direct_connection: bool,
}

impl TrustPolicies {
    /// Default policies for carrier freedom environments
    fn carrier_freedom_defaults() -> Self {
        Self {
            trust_private_networks: true,  // Trust private networks by default
            trust_local_subnet: true,      // Trust local subnet
            trust_known_hosts: true,       // Trust SSH known hosts
            auto_trust_ssh_known: true,    // Auto-trust from SSH
            trust_upnp_devices: true,      // Trust UPnP devices (for local services)
        }
    }
}

/// Convenience functions
pub fn create_carrier_trust() -> HostTrust {
    HostTrust::new()
}

pub fn is_host_trusted(host: &str) -> bool {
    let trust = HostTrust::new();
    !matches!(trust.should_trust(host), TrustLevel::Untrusted)
}

pub fn trust_local_network() -> Result<Vec<String>, String> {
    let mut trust = HostTrust::new();
    trust.discover_local_hosts()
}