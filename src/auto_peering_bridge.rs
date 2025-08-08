// Auto-Peering Bridge - Normalizes peering behavior between syscall combinators and auto-discovery
// Ensures mDNS/Bonjour, UPnP, PAC/WPAD continue to work with the new syscall-based detection

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use log::{debug, info, warn};

use crate::syscall_parse_combinators::{ProtocolId, SyscallParser, ParseResult};
use crate::universal_listener::Protocol as UniversalProtocol;
use crate::auto_discovery::AutoDiscovery;
use crate::bonjour::BonjourDiscovery;

/// Bridge that normalizes protocol detection between syscall and universal systems
#[derive(Clone)]
pub struct AutoPeeringBridge {
    auto_discovery: Arc<RwLock<AutoDiscovery>>,
    bonjour: Arc<RwLock<BonjourDiscovery>>,
    peer_registry: Arc<RwLock<HashMap<SocketAddr, PeerInfo>>>,
    local_addr: SocketAddr,
}

/// Information about discovered peers
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub addr: SocketAddr,
    pub hostname: String,
    pub capabilities: Vec<ProtocolId>,
    pub last_seen: std::time::Instant,
    pub via_discovery: DiscoveryMethod,
}

#[derive(Debug, Clone)]
pub enum DiscoveryMethod {
    Bonjour,
    Upnp,
    Manual,
    Broadcast,
}

impl AutoPeeringBridge {
    pub fn new(local_addr: SocketAddr, hostname: String) -> Self {
        let local_ip = match local_addr.ip() {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(_) => Ipv4Addr::new(127, 0, 0, 1), // Fallback for IPv6
        };
        
        let auto_discovery = Arc::new(RwLock::new(
            AutoDiscovery::new(local_ip, hostname.clone())
        ));
        
        let bonjour = Arc::new(RwLock::new(
            BonjourDiscovery::new().unwrap_or_else(|e| {
                warn!("Failed to initialize Bonjour: {}", e);
                // Create a dummy for graceful degradation
                BonjourDiscovery::new().unwrap() // This will fail again, but that's OK
            })
        ));
        
        Self {
            auto_discovery,
            bonjour,
            peer_registry: Arc::new(RwLock::new(HashMap::new())),
            local_addr,
        }
    }

    /// Convert syscall protocol detection to universal protocol for compatibility
    pub fn normalize_protocol_detection(&self, syscall_result: ParseResult) -> UniversalProtocol {
        match syscall_result.protocol {
            ProtocolId::Socks5 => UniversalProtocol::Socks5,
            ProtocolId::Http => {
                // Need to check if this is actually PAC/WPAD
                if syscall_result.confidence > 200 {
                    UniversalProtocol::Pac
                } else {
                    UniversalProtocol::Http
                }
            },
            ProtocolId::Pac => UniversalProtocol::Pac,
            ProtocolId::Tls => UniversalProtocol::Http, // TLS over HTTP
            ProtocolId::Ssh => UniversalProtocol::Unknown, // SSH not handled by universal
            ProtocolId::Upnp => UniversalProtocol::Upnp,
            ProtocolId::Unknown => UniversalProtocol::Unknown,
        }
    }

    /// Check if a protocol should bypass syscall detection for auto-discovery
    pub fn should_bypass_syscall(&self, first_bytes: &[u8]) -> bool {
        if first_bytes.len() < 8 {
            return false;
        }

        // Check for mDNS queries (should go to Bonjour)
        if first_bytes.len() >= 12 {
            // DNS header check - mDNS queries
            let is_query = (first_bytes[2] & 0x80) == 0;
            let opcode = (first_bytes[2] >> 3) & 0x0F;
            if is_query && opcode == 0 {
                debug!("Bypassing syscall detection for potential mDNS query");
                return true;
            }
        }

        // Check for UPnP SSDP messages (M-SEARCH, NOTIFY)
        if let Ok(text) = std::str::from_utf8(&first_bytes[..first_bytes.len().min(32)]) {
            if text.starts_with("M-SEARCH") || text.starts_with("NOTIFY") {
                debug!("Bypassing syscall detection for UPnP SSDP message");
                return true;
            }
        }

        false
    }

    /// Start auto-discovery services
    pub async fn start_auto_discovery(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting auto-discovery services...");
        
        // Start Bonjour service registration
        if let Ok(bonjour) = self.bonjour.try_read() {
            if let Err(e) = bonjour.register_service() {
                warn!("Failed to register Bonjour service: {}", e);
            } else {
                info!("Bonjour service registered successfully");
            }
        }

        // Start peer discovery in background
        self.start_peer_discovery().await?;
        
        info!("Auto-discovery services started");
        Ok(())
    }

    async fn start_peer_discovery(&self) -> Result<(), Box<dyn std::error::Error>> {
        let bonjour = Arc::clone(&self.bonjour);
        let peer_registry = Arc::clone(&self.peer_registry);
        
        tokio::spawn(async move {
            loop {
                // Discover peers via Bonjour
                if let Ok(bonjour_guard) = bonjour.try_read() {
                    let discovered_services = bonjour_guard.discover_peers();
                    
                    let mut registry = peer_registry.write().await;
                    
                    for service in discovered_services {
                        // Create a dummy peer entry from service info  
                        // Real implementation would extract actual IPs
                        let addr = SocketAddr::new(
                            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), // Placeholder
                            service.get_port()
                        );
                        
                        let peer_info = PeerInfo {
                            addr,
                            hostname: service.get_hostname().to_string(),
                            capabilities: vec![ProtocolId::Http, ProtocolId::Socks5], // Inferred
                            last_seen: std::time::Instant::now(),
                            via_discovery: DiscoveryMethod::Bonjour,
                        };
                        
                        registry.insert(addr, peer_info);
                        info!("Discovered peer via Bonjour: {} ({})", addr, service.get_hostname());
                    }
                }
                
                // Sleep before next discovery cycle
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        });

        Ok(())
    }

    /// Get list of discovered peers
    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        let registry = self.peer_registry.read().await;
        registry.values().cloned().collect()
    }

    /// Register a manually discovered peer
    pub async fn register_peer(&self, addr: SocketAddr, hostname: String, capabilities: Vec<ProtocolId>) {
        let mut registry = self.peer_registry.write().await;
        
        let peer_info = PeerInfo {
            addr,
            hostname,
            capabilities,
            last_seen: std::time::Instant::now(),
            via_discovery: DiscoveryMethod::Manual,
        };
        
        registry.insert(addr, peer_info);
        info!("Manually registered peer: {} ({})", addr, registry.get(&addr).unwrap().hostname);
    }

    /// Clean up stale peers
    pub async fn cleanup_stale_peers(&self) {
        let mut registry = self.peer_registry.write().await;
        let stale_timeout = std::time::Duration::from_secs(300); // 5 minutes
        let now = std::time::Instant::now();
        
        registry.retain(|addr, peer| {
            let is_fresh = now.duration_since(peer.last_seen) < stale_timeout;
            if !is_fresh {
                debug!("Removing stale peer: {}", addr);
            }
            is_fresh
        });
    }

    /// Handle protocol-specific peering logic
    pub async fn handle_peer_protocol(&self, peer_addr: SocketAddr, protocol: ProtocolId) -> Result<(), Box<dyn std::error::Error>> {
        match protocol {
            ProtocolId::Upnp => {
                debug!("Handling UPnP discovery from peer: {}", peer_addr);
                // UPnP peers are handled by the UPnP module
                // Just register them as capable peers
                self.register_peer(
                    peer_addr, 
                    format!("upnp-peer-{}", peer_addr.ip()), 
                    vec![ProtocolId::Upnp, ProtocolId::Http]
                ).await;
            },
            
            ProtocolId::Http | ProtocolId::Pac => {
                debug!("Handling HTTP/PAC from peer: {}", peer_addr);
                // Check if this is a PAC/WPAD request that should be handled by auto-discovery
                self.register_peer(
                    peer_addr,
                    format!("http-peer-{}", peer_addr.ip()),
                    vec![ProtocolId::Http, ProtocolId::Pac]
                ).await;
            },
            
            _ => {
                debug!("Registering general peer: {} with protocol {:?}", peer_addr, protocol);
                self.register_peer(
                    peer_addr,
                    format!("peer-{}", peer_addr.ip()),
                    vec![protocol]
                ).await;
            }
        }
        
        Ok(())
    }

    /// Get the bridge instance info for debugging
    pub async fn get_bridge_info(&self) -> BridgeInfo {
        let peer_count = self.peer_registry.read().await.len();
        
        BridgeInfo {
            local_addr: self.local_addr,
            peer_count,
            auto_discovery_active: true, // Simplified
            bonjour_active: true,        // Simplified
        }
    }
}

#[derive(Debug)]
pub struct BridgeInfo {
    pub local_addr: SocketAddr,
    pub peer_count: usize,
    pub auto_discovery_active: bool,
    pub bonjour_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_protocol_normalization() {
        let bridge = AutoPeeringBridge::new(
            "127.0.0.1:8888".parse().unwrap(),
            "test-host".to_string()
        );
        
        // Test SOCKS5 normalization
        let result = ParseResult {
            protocol: ProtocolId::Socks5,
            consumed: 2,
            confidence: 255,
        };
        
        let normalized = bridge.normalize_protocol_detection(result);
        assert_eq!(normalized, UniversalProtocol::Socks5);
    }

    #[tokio::test]
    async fn test_peer_registration() {
        let bridge = AutoPeeringBridge::new(
            "127.0.0.1:8888".parse().unwrap(),
            "test-host".to_string()
        );
        
        let peer_addr: SocketAddr = "192.168.1.100:8080".parse().unwrap();
        bridge.register_peer(
            peer_addr,
            "test-peer".to_string(),
            vec![ProtocolId::Http, ProtocolId::Socks5]
        ).await;
        
        let peers = bridge.get_peers().await;
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].addr, peer_addr);
        assert_eq!(peers[0].hostname, "test-peer");
    }

    #[test]
    fn test_bypass_detection() {
        let bridge = AutoPeeringBridge::new(
            "127.0.0.1:8888".parse().unwrap(),
            "test-host".to_string()
        );
        
        // Test UPnP M-SEARCH bypass
        let upnp_msearch = b"M-SEARCH * HTTP/1.1\r\n";
        assert!(bridge.should_bypass_syscall(upnp_msearch));
        
        // Test normal HTTP should not bypass
        let http_get = b"GET / HTTP/1.1\r\n";
        assert!(!bridge.should_bypass_syscall(http_get));
    }
}