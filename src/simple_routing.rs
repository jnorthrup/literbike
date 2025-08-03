// Simple Static Routing for LiteBike Proxy
// Default: swlan0:8888,0.0.0.0:all with 127.0.0.1 fallback

use std::io;
use std::net::{IpAddr, SocketAddr};
use tokio::net::TcpListener;
use log::{info, warn, error};

/// Simple routing configuration
#[derive(Debug, Clone)]
pub struct RouteConfig {
    pub interface: String,
    pub port: u16,
    pub bind_addr: IpAddr,
    pub protocols: Vec<String>,
}

impl RouteConfig {
    /// Default configuration: swlan0:8888,0.0.0.0:all
    pub fn default() -> Self {
        Self {
            interface: "swlan0".to_string(),
            port: 8888,
            bind_addr: "0.0.0.0".parse().unwrap(),
            protocols: vec!["all".to_string()],
        }
    }
    
    /// Fallback configuration: 127.0.0.1:8888,all
    pub fn fallback() -> Self {
        Self {
            interface: "lo".to_string(),
            port: 8888,
            bind_addr: "127.0.0.1".parse().unwrap(),
            protocols: vec!["all".to_string()],
        }
    }
    
    /// Parse from string format: interface:port,addr:proto,proto
    pub fn parse(config_str: &str) -> Result<Self, String> {
        let parts: Vec<&str> = config_str.split(':').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid format: expected 'interface:port,addr:proto' got '{}'", config_str));
        }
        
        let interface = parts[0].to_string();
        
        let port_addr: Vec<&str> = parts[1].split(',').collect();
        if port_addr.len() != 2 {
            return Err(format!("Invalid port,addr format: '{}'", parts[1]));
        }
        
        let port: u16 = port_addr[0].parse()
            .map_err(|_| format!("Invalid port: '{}'", port_addr[0]))?;
        let bind_addr: IpAddr = port_addr[1].parse()
            .map_err(|_| format!("Invalid address: '{}'", port_addr[1]))?;
        
        let protocols: Vec<String> = parts[2].split(',').map(|s| s.to_string()).collect();
        
        Ok(Self {
            interface,
            port,
            bind_addr,
            protocols,
        })
    }
    
    /// Get socket address for binding
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.bind_addr, self.port)
    }
    
    /// Check if this config supports the given protocol
    pub fn supports_protocol(&self, protocol: &str) -> bool {
        self.protocols.contains(&"all".to_string()) || 
        self.protocols.contains(&protocol.to_string())
    }
}

/// Simple router that handles fallback logic
pub struct SimpleRouter {
    primary_config: RouteConfig,
    fallback_config: RouteConfig,
}

impl SimpleRouter {
    /// Create router with default and fallback configs
    pub fn new() -> Self {
        Self {
            primary_config: RouteConfig::default(),
            fallback_config: RouteConfig::fallback(),
        }
    }
    
    /// Create router with custom primary config
    pub fn with_primary(primary: RouteConfig) -> Self {
        Self {
            primary_config: primary,
            fallback_config: RouteConfig::fallback(),
        }
    }
    
    /// Attempt to bind to the configured address with fallback
    pub async fn bind_with_fallback(&self) -> io::Result<(TcpListener, RouteConfig)> {
        let primary_addr = self.primary_config.socket_addr();
        
        info!("Attempting to bind to primary: {}:{} on interface {}", 
              primary_addr.ip(), primary_addr.port(), self.primary_config.interface);
        
        // Try primary configuration first
        match TcpListener::bind(primary_addr).await {
            Ok(listener) => {
                info!("Successfully bound to primary: {}:{} ({})", 
                      primary_addr.ip(), primary_addr.port(), self.primary_config.interface);
                Ok((listener, self.primary_config.clone()))
            }
            Err(e) => {
                warn!("Primary binding failed ({}): {}, attempting fallback", 
                      self.primary_config.interface, e);
                
                let fallback_addr = self.fallback_config.socket_addr();
                match TcpListener::bind(fallback_addr).await {
                    Ok(listener) => {
                        warn!("Fallback successful: {}:{} ({})", 
                              fallback_addr.ip(), fallback_addr.port(), self.fallback_config.interface);
                        Ok((listener, self.fallback_config.clone()))
                    }
                    Err(fallback_error) => {
                        error!("Both primary and fallback binding failed. Primary: {}, Fallback: {}", 
                               e, fallback_error);
                        Err(fallback_error)
                    }
                }
            }
        }
    }
    
    /// Get the primary configuration
    pub fn primary_config(&self) -> &RouteConfig {
        &self.primary_config
    }
    
    /// Get the fallback configuration
    pub fn fallback_config(&self) -> &RouteConfig {
        &self.fallback_config
    }
}

/// Protocol support check
pub fn supports_all_protocols(config: &RouteConfig) -> bool {
    config.supports_protocol("all")
}

/// Get protocol list for a configuration
pub fn get_supported_protocols(config: &RouteConfig) -> Vec<String> {
    if config.supports_protocol("all") {
        vec![
            "http".to_string(),
            "socks5".to_string(),
            "pac".to_string(),
            "wpad".to_string(),
            "bonjour".to_string(),
            "upnp".to_string(),
            "tls".to_string(),
        ]
    } else {
        config.protocols.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = RouteConfig::default();
        assert_eq!(config.interface, "swlan0");
        assert_eq!(config.port, 8888);
        assert_eq!(config.bind_addr, "0.0.0.0".parse::<IpAddr>().unwrap());
        assert!(config.supports_protocol("http"));
        assert!(config.supports_protocol("all"));
    }
    
    #[test]
    fn test_fallback_config() {
        let config = RouteConfig::fallback();
        assert_eq!(config.interface, "lo");
        assert_eq!(config.port, 8888);
        assert_eq!(config.bind_addr, "127.0.0.1".parse::<IpAddr>().unwrap());
        assert!(config.supports_protocol("all"));
    }
    
    #[test]
    fn test_parse_config() {
        let config = RouteConfig::parse("eth0:8080,192.168.1.1:http,socks5").unwrap();
        assert_eq!(config.interface, "eth0");
        assert_eq!(config.port, 8080);
        assert_eq!(config.bind_addr, "192.168.1.1".parse::<IpAddr>().unwrap());
        assert!(config.supports_protocol("http"));
        assert!(config.supports_protocol("socks5"));
        assert!(!config.supports_protocol("pac"));
    }
    
    #[test]
    fn test_parse_config_all_protocols() {
        let config = RouteConfig::parse("wlan0:8888,0.0.0.0:all").unwrap();
        assert_eq!(config.interface, "wlan0");
        assert_eq!(config.port, 8888);
        assert_eq!(config.bind_addr, "0.0.0.0".parse::<IpAddr>().unwrap());
        assert!(config.supports_protocol("http"));
        assert!(config.supports_protocol("any_protocol"));
    }
    
    #[test]
    fn test_parse_invalid_format() {
        assert!(RouteConfig::parse("invalid").is_err());
        assert!(RouteConfig::parse("eth0:invalid_port,127.0.0.1:http").is_err());
        assert!(RouteConfig::parse("eth0:8080,invalid_ip:http").is_err());
    }
    
    #[test]
    fn test_socket_addr() {
        let config = RouteConfig::default();
        let addr = config.socket_addr();
        assert_eq!(addr.ip(), "0.0.0.0".parse::<IpAddr>().unwrap());
        assert_eq!(addr.port(), 8888);
    }
    
    #[test]
    fn test_router_creation() {
        let router = SimpleRouter::new();
        assert_eq!(router.primary_config().interface, "swlan0");
        assert_eq!(router.fallback_config().interface, "lo");
    }
    
    #[test]
    fn test_get_supported_protocols() {
        let config = RouteConfig::default();
        let protocols = get_supported_protocols(&config);
        assert!(protocols.contains(&"http".to_string()));
        assert!(protocols.contains(&"socks5".to_string()));
        assert!(protocols.contains(&"pac".to_string()));
    }
}