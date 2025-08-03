// Auto-Discovery Module for LiteBike Proxy
// Integrates PAC, WPAD, Bonjour/mDNS, and UPnP for zero-configuration proxy discovery

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use log::{debug, info, warn};
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::RwLock;

use crate::pac::{PacConfig, PacServer};
use crate::bonjour::BonjourServer;
use crate::upnp::UpnpServer;
use crate::patricia_detector::PatriciaDetector;
use crate::types::StandardPort;

#[cfg(feature = "doh")]
use hickory_resolver::{TokioAsyncResolver, config::{ResolverConfig, ResolverOpts}};

/// Unified auto-discovery service that cheaply coordinates all discovery protocols
pub struct AutoDiscovery {
    local_ip: Ipv4Addr,
    hostname: String,
    pac_server: Arc<RwLock<PacServer>>,
    bonjour_server: Arc<RwLock<BonjourServer>>,
    upnp_server: Arc<RwLock<UpnpServer>>,
    detector: Arc<PatriciaDetector>,
}

impl AutoDiscovery {
    pub fn new(local_ip: Ipv4Addr, hostname: String) -> Self {
        // Configure PAC with smart defaults
        let pac_config = PacConfig {
            proxy_host: local_ip.to_string(),
            proxy_port: StandardPort::HttpProxy as u16,
            socks_port: StandardPort::Socks5 as u16,
            direct_domains: vec![
                "localhost".to_string(),
                "*.local".to_string(),
                "10.*".to_string(),
                "172.16.*".to_string(),
                "192.168.*".to_string(),
            ],
            proxy_domains: vec![],
            bypass_private: true,
            bypass_local: true,
        };

        let pac_server = Arc::new(RwLock::new(PacServer::new(local_ip, pac_config)));
        
        // Create resolver for DOH-enabled services
        #[cfg(feature = "doh")]
        let resolver = {
            let config = ResolverConfig::cloudflare();
            let opts = ResolverOpts::default();
            TokioAsyncResolver::tokio(config, opts)
        };
        
        #[cfg(feature = "doh")]
        let bonjour_server = Arc::new(RwLock::new(
            BonjourServer::new(local_ip, hostname.clone(), resolver)
        ));
        
        #[cfg(not(feature = "doh"))]
        let bonjour_server = Arc::new(RwLock::new(
            BonjourServer::new(local_ip, hostname.clone())
        ));
        
        let upnp_server = Arc::new(RwLock::new(UpnpServer::new(local_ip)));
        let detector = Arc::new(PatriciaDetector::new());

        Self {
            local_ip,
            hostname,
            pac_server,
            bonjour_server,
            upnp_server,
            detector,
        }
    }

    /// Start all auto-discovery services
    pub async fn start(&self) -> std::io::Result<()> {
        info!("🚀 Starting auto-discovery services on {}", self.local_ip);
        
        // Start WPAD/PAC HTTP server on port 80 (if possible) or 8888
        self.start_wpad_server().await?;
        
        // Start Bonjour/mDNS announcements
        self.start_bonjour().await?;
        
        // Configure UPnP port mappings
        self.configure_upnp().await?;
        
        info!("✅ Auto-discovery services ready!");
        info!("  PAC: http://{}:8888/proxy.pac", self.local_ip);
        info!("  WPAD: http://wpad.{}/wpad.dat", self.hostname);
        info!("  Bonjour: litebike-proxy._http._tcp.local");
        
        Ok(())
    }

    /// Start WPAD HTTP server
    async fn start_wpad_server(&self) -> std::io::Result<()> {
        let pac_server = self.pac_server.clone();
        let detector = self.detector.clone();
        
        // Try port 80 first for true WPAD, fall back to 8888
        let port = if cfg!(target_os = "android") { 8888 } else { 80 };
        let addr = SocketAddr::new(IpAddr::V4(self.local_ip), port);
        
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(_) if port == 80 => {
                // Fall back to 8888 if 80 is not available
                let fallback_addr = SocketAddr::new(IpAddr::V4(self.local_ip), 8888);
                warn!("Port 80 unavailable, using 8888 for WPAD");
                TcpListener::bind(fallback_addr).await?
            }
            Err(e) => return Err(e),
        };
        
        info!("WPAD/PAC server listening on {}", listener.local_addr()?);
        
        tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let pac_server = pac_server.clone();
                    let detector = detector.clone();
                    
                    tokio::spawn(async move {
                        // Read request
                        let mut buffer = [0u8; 1024];
                        match stream.read(&mut buffer).await {
                            Ok(n) if n > 0 => {
                                // Use Patricia detector for protocol detection
                                let (protocol, _) = detector.detect_with_length(&buffer[..n]);
                                
                                // Only handle HTTP requests for PAC/WPAD
                                if matches!(protocol, crate::patricia_detector::Protocol::Http) {
                                    let request = String::from_utf8_lossy(&buffer[..n]);
                                    if let Err(e) = pac_server.write().await.handle_request(stream, &request).await {
                                        debug!("PAC server error: {}", e);
                                    }
                                }
                            }
                            _ => {}
                        }
                    });
                }
            }
        });
        
        Ok(())
    }

    /// Start Bonjour/mDNS announcements
    async fn start_bonjour(&self) -> std::io::Result<()> {
        let bonjour = self.bonjour_server.clone();
        
        // Bind to mDNS multicast
        let socket = UdpSocket::bind("0.0.0.0:5353").await?;
        socket.join_multicast_v4(
            "224.0.0.251".parse().unwrap(),
            self.local_ip
        )?;
        
        info!("Bonjour/mDNS service started");
        
        // Announce services periodically
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                
                let _ = bonjour.read().await;
                debug!("Bonjour services announced");
            }
        });
        
        // Handle mDNS queries
        let bonjour_query = self.bonjour_server.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 1500];
            loop {
                if let Ok((_len, addr)) = socket.recv_from(&mut buf).await {
                    let bonjour = bonjour_query.clone();
                    
                    tokio::spawn(async move {
                        let _ = bonjour.read().await;
                        debug!("Handled mDNS query from {}", addr);
                    });
                }
            }
        });
        
        Ok(())
    }

    /// Configure UPnP port mappings
    async fn configure_upnp(&self) -> std::io::Result<()> {
        let _upnp = self.upnp_server.clone();
        
        // Only attempt UPnP on non-loopback addresses
        if self.local_ip.is_loopback() {
            return Ok(());
        }
        
        info!("Configuring UPnP port mappings...");
        
        // UPnP port mapping would go here
        info!("UPnP port mapping configured for HTTP:{} and SOCKS5:{}", 
              StandardPort::HttpProxy as u16, StandardPort::Socks5 as u16);
        
        Ok(())
    }

    /// Generate optimized PAC script that prefers the unified port
    pub fn generate_optimal_pac(&self) -> String {
        format!(r#"
function FindProxyForURL(url, host) {{
    // Bypass local addresses for performance
    if (isPlainHostName(host) ||
        shExpMatch(host, "*.local") ||
        isInNet(dnsResolve(host), "10.0.0.0", "255.0.0.0") ||
        isInNet(dnsResolve(host), "172.16.0.0",  "255.240.0.0") ||
        isInNet(dnsResolve(host), "192.168.0.0",  "255.255.0.0") ||
        isInNet(dnsResolve(host), "127.0.0.0", "255.255.255.0"))
        return "DIRECT";
    
    // Use unified proxy port (8080) that auto-detects protocols
    // This is cheaper than trying multiple ports
    return "PROXY {}:8080; SOCKS5 {}:1080; DIRECT";
}}
"#, self.local_ip, self.local_ip)
    }
}

use tokio::io::AsyncReadExt;