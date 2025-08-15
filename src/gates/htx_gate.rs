// HTX Gate for LITEBIKE
// Gates connection to Betanet HTX (without modifying HTX)

use async_trait::async_trait;
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};

pub struct HTXGate {
    enabled: Arc<RwLock<bool>>,
    endpoint: Arc<RwLock<String>>,
    connected: Arc<RwLock<bool>>,
}

impl HTXGate {
    pub fn new() -> Self {
        Self {
            enabled: Arc::new(RwLock::new(false)),
            endpoint: Arc::new(RwLock::new("127.0.0.1:443".to_string())),
            connected: Arc::new(RwLock::new(false)),
        }
    }
    
    pub fn set_endpoint(&self, endpoint: String) {
        *self.endpoint.write() = endpoint;
        *self.connected.write() = false; // Reset connection
    }
    
    pub fn enable(&self) {
        *self.enabled.write() = true;
    }
    
    pub fn disable(&self) {
        *self.enabled.write() = false;
        *self.connected.write() = false;
    }
    
    async fn forward_to_htx(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        let endpoint = self.endpoint.read().clone();
        
        // Connect to HTX server
        let mut stream = TcpStream::connect(&endpoint).await
            .map_err(|e| format!("Failed to connect to HTX at {}: {}", endpoint, e))?;
        
        // Send data to HTX
        stream.write_all(data).await
            .map_err(|e| format!("Failed to write to HTX: {}", e))?;
        
        // Read response
        let mut response = Vec::new();
        let mut buffer = [0u8; 4096];
        
        match stream.read(&mut buffer).await {
            Ok(n) if n > 0 => {
                response.extend_from_slice(&buffer[..n]);
                Ok(response)
            }
            Ok(_) => Ok(vec![]),
            Err(e) => Err(format!("Failed to read from HTX: {}", e)),
        }
    }
    
    fn detect_htx(&self, data: &[u8]) -> bool {
        // Check for HTX patterns
        if data.len() < 8 {
            return false;
        }
        
        // Check for HTX magic bytes or access ticket
        data.starts_with(b"HTX/") || 
        data.starts_with(b"betanet/htx") ||
        (data.len() >= 24 && data.len() <= 64) // Access ticket size
    }
}

#[async_trait]
impl super::Gate for HTXGate {
    async fn is_open(&self) -> bool {
        *self.enabled.read()
    }
    
    async fn process(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        if !self.is_open().await {
            return Err("HTX gate is closed".to_string());
        }
        
        if !self.detect_htx(data) {
            return Err("Not HTX protocol".to_string());
        }
        
        println!("Forwarding through HTX gate to {}", self.endpoint.read());
        
        // Forward to actual HTX server
        self.forward_to_htx(data).await
    }
    
    fn name(&self) -> &str {
        "htx"
    }
    
    fn children(&self) -> Vec<Arc<dyn super::Gate>> {
        vec![]
    }
}