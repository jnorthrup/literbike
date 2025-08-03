// Protocol Registry System for Unified Port 8888
// Provides a DRY, extensible architecture for multi-protocol detection and handling

use std::io;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use log::{debug, info, warn};
use async_trait::async_trait;

use crate::universal_listener::PrefixedStream;

/// Protocol detection result with confidence scoring
#[derive(Debug, Clone)]
pub struct ProtocolDetectionResult {
    pub protocol_name: String,
    pub confidence: u8,  // 0-255, higher is more confident
    pub bytes_consumed: usize,
    pub metadata: Option<String>,
}

impl ProtocolDetectionResult {
    pub fn unknown() -> Self {
        Self {
            protocol_name: "unknown".to_string(),
            confidence: 0,
            bytes_consumed: 0,
            metadata: None,
        }
    }
    
    pub fn new(name: &str, confidence: u8, bytes_consumed: usize) -> Self {
        Self {
            protocol_name: name.to_string(),
            confidence,
            bytes_consumed,
            metadata: None,
        }
    }
    
    pub fn with_metadata(mut self, metadata: String) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Trait for protocol detection logic
#[async_trait]
pub trait ProtocolDetector: Send + Sync {
    /// Detect protocol from initial bytes
    fn detect(&self, data: &[u8]) -> ProtocolDetectionResult;
    
    /// Minimum bytes needed for reliable detection
    fn required_bytes(&self) -> usize;
    
    /// Confidence threshold for this detector (0-255)
    fn confidence_threshold(&self) -> u8;
    
    /// Protocol name for logging/debugging
    fn protocol_name(&self) -> &str;
}

/// Trait for protocol handling logic
#[async_trait]
pub trait ProtocolHandler: Send + Sync {
    /// Handle a connection using this protocol
    async fn handle(&self, stream: PrefixedStream<TcpStream>) -> io::Result<()>;
    
    /// Check if this handler can process the detection result
    fn can_handle(&self, detection: &ProtocolDetectionResult) -> bool;
    
    /// Protocol name for logging
    fn protocol_name(&self) -> &str;
}

/// Registry entry combining detector and handler
pub struct ProtocolEntry {
    pub detector: Box<dyn ProtocolDetector>,
    pub handler: Box<dyn ProtocolHandler>,
    pub priority: u8,  // Higher priority = checked first
}

/// Central protocol registry managing all protocols
pub struct ProtocolRegistry {
    entries: Vec<ProtocolEntry>,
    fallback_handler: Option<Box<dyn ProtocolHandler>>,
    max_detection_bytes: usize,
}

impl ProtocolRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            fallback_handler: None,
            max_detection_bytes: 1024,  // Default buffer size for detection
        }
    }
    
    /// Register a protocol detector/handler pair
    pub fn register(&mut self, detector: Box<dyn ProtocolDetector>, handler: Box<dyn ProtocolHandler>, priority: u8) {
        let entry = ProtocolEntry {
            detector,
            handler,
            priority,
        };
        self.entries.push(entry);
        
        // Sort by priority (highest first)
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        info!("Registered protocol handler: {} (priority: {})", 
              self.entries.last().unwrap().detector.protocol_name(), priority);
    }
    
    /// Set fallback handler for unknown protocols
    pub fn set_fallback(&mut self, handler: Box<dyn ProtocolHandler>) {
        info!("Set fallback handler: {}", handler.protocol_name());
        self.fallback_handler = Some(handler);
    }
    
    /// Set maximum bytes to read for protocol detection
    pub fn set_max_detection_bytes(&mut self, bytes: usize) {
        self.max_detection_bytes = bytes;
    }
    
    /// Handle an incoming connection
    pub async fn handle_connection(&self, mut stream: TcpStream) -> io::Result<()> {
        let peer_addr = stream.peer_addr().unwrap_or_else(|_| "unknown".parse().unwrap());
        debug!("New connection from {}", peer_addr);
        
        // Read initial data for protocol detection
        let mut buffer = vec![0u8; self.max_detection_bytes];
        let bytes_read = stream.read(&mut buffer).await?;
        
        if bytes_read == 0 {
            debug!("Connection from {} closed immediately", peer_addr);
            return Ok(());
        }
        
        buffer.truncate(bytes_read);
        debug!("Read {} bytes from {} for protocol detection", bytes_read, peer_addr);
        
        // Try each detector in priority order
        for entry in &self.entries {
            let detection_result = entry.detector.detect(&buffer);
            
            debug!("Protocol {} detection: confidence={}, threshold={}", 
                   entry.detector.protocol_name(), 
                   detection_result.confidence, 
                   entry.detector.confidence_threshold());
            
            if detection_result.confidence >= entry.detector.confidence_threshold() &&
               entry.handler.can_handle(&detection_result) {
                
                info!("Routing {} to {} handler (confidence: {})", 
                      peer_addr, entry.handler.protocol_name(), detection_result.confidence);
                
                let prefixed_stream = PrefixedStream::new(stream, buffer);
                return entry.handler.handle(prefixed_stream).await;
            }
        }
        
        // Use fallback handler if available
        if let Some(ref fallback) = self.fallback_handler {
            warn!("No protocol detected for {}, using fallback handler: {}", 
                  peer_addr, fallback.protocol_name());
            
            let prefixed_stream = PrefixedStream::new(stream, buffer);
            return fallback.handle(prefixed_stream).await;
        }
        
        // No handler available
        warn!("No handler available for connection from {}", peer_addr);
        Err(io::Error::new(io::ErrorKind::InvalidData, "No suitable protocol handler"))
    }
    
    /// Get protocol statistics
    pub fn get_stats(&self) -> ProtocolRegistryStats {
        ProtocolRegistryStats {
            registered_protocols: self.entries.len(),
            has_fallback: self.fallback_handler.is_some(),
            max_detection_bytes: self.max_detection_bytes,
        }
    }
}

/// Statistics about the protocol registry
#[derive(Debug)]
pub struct ProtocolRegistryStats {
    pub registered_protocols: usize,
    pub has_fallback: bool,
    pub max_detection_bytes: usize,
}

impl Clone for ProtocolRegistry {
    fn clone(&self) -> Self {
        // Note: This is a simplified clone that doesn't copy the actual handlers
        // In practice, you'd want to use Arc<ProtocolRegistry> to share the registry
        Self::new()
    }
}

// Utility function to create a shared registry
pub fn create_shared_registry() -> Arc<ProtocolRegistry> {
    Arc::new(ProtocolRegistry::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    
    // Mock detector for testing
    struct MockDetector {
        name: String,
        pattern: Vec<u8>,
        confidence: u8,
    }
    
    impl MockDetector {
        fn new(name: &str, pattern: Vec<u8>, confidence: u8) -> Self {
            Self {
                name: name.to_string(),
                pattern,
                confidence,
            }
        }
    }
    
    #[async_trait]
    impl ProtocolDetector for MockDetector {
        fn detect(&self, data: &[u8]) -> ProtocolDetectionResult {
            if data.starts_with(&self.pattern) {
                ProtocolDetectionResult::new(&self.name, self.confidence, self.pattern.len())
            } else {
                ProtocolDetectionResult::unknown()
            }
        }
        
        fn required_bytes(&self) -> usize { self.pattern.len() }
        fn confidence_threshold(&self) -> u8 { 128 }
        fn protocol_name(&self) -> &str { &self.name }
    }
    
    // Mock handler for testing
    struct MockHandler {
        name: String,
    }
    
    impl MockHandler {
        fn new(name: &str) -> Self {
            Self { name: name.to_string() }
        }
    }
    
    #[async_trait]
    impl ProtocolHandler for MockHandler {
        async fn handle(&self, mut _stream: PrefixedStream<TcpStream>) -> io::Result<()> {
            // Just close the connection for testing
            Ok(())
        }
        
        fn can_handle(&self, detection: &ProtocolDetectionResult) -> bool {
            detection.protocol_name == self.name
        }
        
        fn protocol_name(&self) -> &str { &self.name }
    }
    
    #[test]
    fn test_registry_creation() {
        let registry = ProtocolRegistry::new();
        let stats = registry.get_stats();
        
        assert_eq!(stats.registered_protocols, 0);
        assert!(!stats.has_fallback);
        assert_eq!(stats.max_detection_bytes, 1024);
    }
    
    #[test]
    fn test_protocol_registration() {
        let mut registry = ProtocolRegistry::new();
        
        let detector = Box::new(MockDetector::new("test", b"TEST".to_vec(), 200));
        let handler = Box::new(MockHandler::new("test"));
        
        registry.register(detector, handler, 10);
        
        let stats = registry.get_stats();
        assert_eq!(stats.registered_protocols, 1);
    }
    
    #[test]
    fn test_detection_result() {
        let result = ProtocolDetectionResult::new("http", 200, 4)
            .with_metadata("GET / HTTP/1.1".to_string());
        
        assert_eq!(result.protocol_name, "http");
        assert_eq!(result.confidence, 200);
        assert_eq!(result.bytes_consumed, 4);
        assert!(result.metadata.is_some());
    }
}