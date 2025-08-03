// Unified Protocol Manager for Port 8888
// Provides a high-level interface to register and manage all protocols on the unified port

use std::sync::Arc;
use log::info;

use crate::protocol_registry::ProtocolRegistry;
use crate::protocol_handlers::{
    HttpDetector, HttpHandler,
    Socks5Detector, Socks5Handler,
    DohDetector, DohHandler,
    TlsDetector, TlsHandler,
};

/// Manager for all protocols on the unified port 8888
pub struct UnifiedProtocolManager {
    registry: Arc<ProtocolRegistry>,
}

impl UnifiedProtocolManager {
    /// Create a new unified protocol manager with all core protocols registered
    pub async fn new() -> Self {
        let mut registry = ProtocolRegistry::new();
        
        // Register protocols in priority order (highest priority first)
        // DoH should be checked before regular HTTP to avoid false positives
        
        // 1. DoH (DNS-over-HTTPS) - Highest priority (255)
        info!("Registering DoH (DNS-over-HTTPS) protocol handler");
        let doh_detector = Box::new(DohDetector::new());
        let doh_handler = Box::new(DohHandler::new().await);
        registry.register(doh_detector, doh_handler, 255);
        
        // 2. SOCKS5 - High priority (200) 
        info!("Registering SOCKS5 protocol handler");
        let socks5_detector = Box::new(Socks5Detector::new());
        let socks5_handler = Box::new(Socks5Handler::new());
        registry.register(socks5_detector, socks5_handler, 200);
        
        // 3. TLS - Medium-high priority (180)
        info!("Registering TLS protocol handler");
        let tls_detector = Box::new(TlsDetector::new());
        let tls_handler = Box::new(TlsHandler::new());
        registry.register(tls_detector, tls_handler, 180);
        
        // 4. HTTP - Lower priority (150) as it's the most general
        info!("Registering HTTP protocol handler");
        let http_detector = Box::new(HttpDetector::new());
        let http_handler = Box::new(HttpHandler::new());
        registry.register(http_detector, http_handler, 150);
        
        // Set HTTP as fallback for unknown protocols
        let fallback_handler = Box::new(HttpHandler::new());
        registry.set_fallback(fallback_handler);
        
        Self {
            registry: Arc::new(registry),
        }
    }
    
    /// Get a clone of the registry for use in connection handling
    pub fn get_registry(&self) -> Arc<ProtocolRegistry> {
        Arc::clone(&self.registry)
    }
    
    /// Get statistics about registered protocols
    pub fn get_stats(&self) -> crate::protocol_registry::ProtocolRegistryStats {
        self.registry.get_stats()
    }
    
    /// Register an additional protocol detector/handler pair
    pub fn register_custom_protocol(
        &mut self,
        detector: Box<dyn crate::protocol_registry::ProtocolDetector>,
        _handler: Box<dyn crate::protocol_registry::ProtocolHandler>,
        _priority: u8,
    ) {
        // This would require making registry mutable, which conflicts with Arc
        // For now, custom protocols should be registered during creation
        info!("Custom protocol registration requested for: {}", detector.protocol_name());
        // TODO: Implement dynamic registration if needed
    }
}

/// Helper function to get the optimal buffer size for protocol detection
pub fn get_optimal_detection_buffer_size() -> usize {
    // Optimized for the registered protocols:
    // - DoH needs ~20 bytes for "POST /dns-query HTTP"
    // - SOCKS5 needs ~2 bytes minimum
    // - TLS needs ~3 bytes for handshake
    // - HTTP needs ~16 bytes for method detection
    // 
    // Use 1024 bytes to handle most protocol detection without excessive memory
    1024
}

/// Configuration for the unified port system
pub struct UnifiedPortConfig {
    pub max_detection_bytes: usize,
    pub enable_fallback: bool,
    pub enable_doh: bool,
    pub enable_socks5: bool,
    pub enable_http: bool,
    pub enable_tls: bool,
}

impl Default for UnifiedPortConfig {
    fn default() -> Self {
        Self {
            max_detection_bytes: get_optimal_detection_buffer_size(),
            enable_fallback: true,
            enable_doh: true,
            enable_socks5: true,
            enable_http: true,
            enable_tls: true,
        }
    }
}

impl UnifiedProtocolManager {
    /// Create a new manager with custom configuration
    pub async fn with_config(config: UnifiedPortConfig) -> Self {
        let mut registry = ProtocolRegistry::new();
        registry.set_max_detection_bytes(config.max_detection_bytes);
        
        // Register protocols based on configuration
        if config.enable_doh {
            info!("Registering DoH (DNS-over-HTTPS) protocol handler");
            let doh_detector = Box::new(DohDetector::new());
            let doh_handler = Box::new(DohHandler::new().await);
            registry.register(doh_detector, doh_handler, 255);
        }
        
        if config.enable_socks5 {
            info!("Registering SOCKS5 protocol handler");
            let socks5_detector = Box::new(Socks5Detector::new());
            let socks5_handler = Box::new(Socks5Handler::new());
            registry.register(socks5_detector, socks5_handler, 200);
        }
        
        if config.enable_tls {
            info!("Registering TLS protocol handler");
            let tls_detector = Box::new(TlsDetector::new());
            let tls_handler = Box::new(TlsHandler::new());
            registry.register(tls_detector, tls_handler, 180);
        }
        
        if config.enable_http {
            info!("Registering HTTP protocol handler");
            let http_detector = Box::new(HttpDetector::new());
            let http_handler = Box::new(HttpHandler::new());
            registry.register(http_detector, http_handler, 150);
        }
        
        // Set fallback if enabled
        if config.enable_fallback {
            let fallback_handler = Box::new(HttpHandler::new());
            registry.set_fallback(fallback_handler);
        }
        
        Self {
            registry: Arc::new(registry),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_unified_manager_creation() {
        let manager = UnifiedProtocolManager::new().await;
        let stats = manager.get_stats();
        
        // Should have 4 protocols registered: DoH, SOCKS5, TLS, HTTP
        assert_eq!(stats.registered_protocols, 4);
        assert!(stats.has_fallback);
        assert_eq!(stats.max_detection_bytes, 1024);
    }
    
    #[tokio::test]
    async fn test_custom_config() {
        let config = UnifiedPortConfig {
            max_detection_bytes: 2048,
            enable_doh: false,
            enable_tls: false,
            ..Default::default()
        };
        
        let manager = UnifiedProtocolManager::with_config(config).await;
        let stats = manager.get_stats();
        
        // Should only have HTTP and SOCKS5
        assert_eq!(stats.registered_protocols, 2);
        assert_eq!(stats.max_detection_bytes, 2048);
    }
}