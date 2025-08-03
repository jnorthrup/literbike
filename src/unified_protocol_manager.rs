// Unified Protocol Manager for Port 8888
// Provides a high-level interface to register and manage all protocols on the unified port

use std::collections::HashMap;
use std::sync::Arc;
use log::info;

use crate::detection_orchestrator::DetectionOrchestrator;
use crate::protocol_registry::ProtocolHandler;
use crate::protocol_handlers::{
    HttpHandler, Socks5Handler, DohHandler, TlsHandler,
};
use crate::abstractions::{
    HttpDetector, TlsDetector,
    UpnpDetector, ShadowsocksDetector,
    ProtocolDetector as AbstractionsProtocolDetector,
};

/// Manager for all protocols on the unified port 8888
pub struct UnifiedProtocolManager {
    pub orchestrator: Arc<DetectionOrchestrator>,
    pub handlers: Arc<HashMap<String, Arc<dyn ProtocolHandler>>>,
    pub fallback_handler: Option<Arc<dyn ProtocolHandler>>,
}

/// Configuration for the unified port system
pub struct UnifiedPortConfig {
    pub enable_fallback: bool,
    pub enable_doh: bool,
    pub enable_socks5: bool,
    pub enable_http: bool,
    pub enable_tls: bool,
}

impl Default for UnifiedPortConfig {
    fn default() -> Self {
        Self {
            enable_fallback: true,
            enable_doh: true,
            enable_socks5: true,
            enable_http: true,
            enable_tls: true,
        }
    }
}

impl UnifiedProtocolManager {
    /// Create a new unified protocol manager with all core protocols registered
    pub async fn new() -> Self {
        Self::with_config(UnifiedPortConfig::default()).await
    }

    /// Create a new manager with custom configuration
    pub async fn with_config(config: UnifiedPortConfig) -> Self {
        let mut orchestrator = DetectionOrchestrator::new();
        let mut handlers = HashMap::new();

        // Register protocols based on configuration
        if config.enable_doh {
            info!("Registering DoH (DNS-over-HTTPS) protocol handler");
            // Use HTTP detector with DoH hinting for now; a dedicated DohDetector is not in abstractions
            orchestrator.add_detector(Box::new(HttpDetector) as Box<dyn AbstractionsProtocolDetector>);
            handlers.insert("doh".to_string(), Arc::new(DohHandler::new().await) as Arc<dyn ProtocolHandler>);
        }
        
        if config.enable_socks5 {
            info!("Registering SOCKS5 protocol handler");
            // Use fast-path universal_listener-based routing; no abstractions::Socks5Detector available
            handlers.insert("socks5".to_string(), Arc::new(Socks5Handler::new()) as Arc<dyn ProtocolHandler>);
        }
        
        if config.enable_tls {
            info!("Registering TLS protocol handler");
            orchestrator.add_detector(Box::new(TlsDetector) as Box<dyn AbstractionsProtocolDetector>);
            handlers.insert("tls".to_string(), Arc::new(TlsHandler::new()) as Arc<dyn ProtocolHandler>);
        }
        
        if config.enable_http {
            info!("Registering HTTP protocol handler");
            orchestrator.add_detector(Box::new(HttpDetector) as Box<dyn AbstractionsProtocolDetector>);
            handlers.insert("http".to_string(), Arc::new(HttpHandler::new()) as Arc<dyn ProtocolHandler>);
        }
        
        // Set fallback if enabled
        let fallback_handler = if config.enable_fallback {
            info!("Setting HTTP as fallback handler");
            Some(Arc::new(HttpHandler::new()) as Arc<dyn ProtocolHandler>)
        } else {
            None
        };
        
        Self {
            orchestrator: Arc::new(orchestrator),
            handlers: Arc::new(handlers),
            fallback_handler,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_unified_manager_creation() {
        let manager = UnifiedProtocolManager::new().await;
        
        assert!(manager.handlers.contains_key("http"));
        assert!(manager.handlers.contains_key("socks5"));
        assert!(manager.handlers.contains_key("tls"));
        assert!(manager.handlers.contains_key("doh"));
        assert!(manager.fallback_handler.is_some());
    }
    
    #[tokio::test]
    async fn test_custom_config() {
        let config = UnifiedPortConfig {
            enable_doh: false,
            enable_tls: false,
            ..Default::default()
        };
        
        let manager = UnifiedProtocolManager::with_config(config).await;
        
        assert!(manager.handlers.contains_key("http"));
        assert!(manager.handlers.contains_key("socks5"));
        assert!(!manager.handlers.contains_key("tls"));
        assert!(!manager.handlers.contains_key("doh"));
    }
}