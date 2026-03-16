//! Edge Profile Gate Implementation
//!
//! Specialized gates optimized for edge computing scenarios with
//! reduced footprint and optimized resource usage.

use std::sync::Arc;
use async_trait::async_trait;
use parking_lot::RwLock;

use super::{ExclusiveGate, GateProfile};

/// Edge-optimized crypto gate with minimal overhead
pub struct EdgeCryptoGate {
    enabled: Arc<RwLock<bool>>,
    profile: GateProfile,
}

impl EdgeCryptoGate {
    pub fn new() -> Self {
        Self {
            enabled: Arc::new(RwLock::new(false)),
            profile: GateProfile::Edge,
        }
    }

    pub fn with_enabled(enabled: bool) -> Self {
        Self {
            enabled: Arc::new(RwLock::new(enabled)),
            profile: GateProfile::Edge,
        }
    }
}

#[async_trait]
impl ExclusiveGate for EdgeCryptoGate {
    async fn is_open(&self) -> bool {
        *self.enabled.read()
    }

    async fn open(&self) -> Result<(), String> {
        *self.enabled.write() = true;
        Ok(())
    }

    async fn close(&self) -> Result<(), String> {
        *self.enabled.write() = false;
        Ok(())
    }

    fn name(&self) -> &str {
        "edge_crypto"
    }

    fn profile(&self) -> GateProfile {
        self.profile
    }
}

impl super::Gate for EdgeCryptoGate {
    async fn is_open(&self) -> bool {
        self.is_open().await
    }

    async fn process(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        if !self.is_open().await {
            return Err("Gate closed".to_string());
        }
        
        Ok(data.to_vec())
    }

    fn name(&self) -> &str {
        "edge_crypto"
    }

    fn children(&self) -> Vec<Arc<dyn super::Gate>> {
        vec![]
    }
}

impl Default for EdgeCryptoGate {
    fn default() -> Self {
        Self::new()
    }
}

/// Edge-optimized network gate
pub struct EdgeNetworkGate {
    enabled: Arc<RwLock<bool>>,
    profile: GateProfile,
    compression_enabled: bool,
}

impl EdgeNetworkGate {
    pub fn new() -> Self {
        Self {
            enabled: Arc::new(RwLock::new(false)),
            profile: GateProfile::Edge,
            compression_enabled: true,
        }
    }

    pub fn set_compression(&mut self, enabled: bool) {
        self.compression_enabled = enabled;
    }
}

#[async_trait]
impl ExclusiveGate for EdgeNetworkGate {
    async fn is_open(&self) -> bool {
        *self.enabled.read()
    }

    async fn open(&self) -> Result<(), String> {
        *self.enabled.write() = true;
        Ok(())
    }

    async fn close(&self) -> Result<(), String> {
        *self.enabled.write() = false;
        Ok(())
    }

    fn name(&self) -> &str {
        "edge_network"
    }

    fn profile(&self) -> GateProfile {
        self.profile
    }
}

impl super::Gate for EdgeNetworkGate {
    async fn is_open(&self) -> bool {
        self.is_open().await
    }

    async fn process(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        if !self.is_open().await {
            return Err("Gate closed".to_string());
        }
        
        if self.compression_enabled {
            // Simple length-prefixed compression placeholder
            let mut result = Vec::with_capacity(data.len() + 4);
            result.extend_from_slice(&(data.len() as u32).to_le_bytes());
            result.extend_from_slice(data);
            Ok(result)
        } else {
            Ok(data.to_vec())
        }
    }

    fn name(&self) -> &str {
        "edge_network"
    }

    fn children(&self) -> Vec<Arc<dyn super::Gate>> {
        vec![]
    }
}

impl Default for EdgeNetworkGate {
    fn default() -> Self {
        Self::new()
    }
}

/// Edge profile configuration
#[derive(Debug, Clone)]
pub struct EdgeProfileConfig {
    pub max_memory_mb: usize,
    pub max_connections: usize,
    pub compression_enabled: bool,
    pub crypto_enabled: bool,
    pub batch_size: usize,
}

impl Default for EdgeProfileConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: 256,
            max_connections: 50,
            compression_enabled: true,
            crypto_enabled: true,
            batch_size: 64,
        }
    }
}

impl EdgeProfileConfig {
    pub fn from_profile(profile: GateProfile) -> Self {
        match profile {
            GateProfile::Lite => Self {
                max_memory_mb: 128,
                max_connections: 20,
                compression_enabled: true,
                crypto_enabled: false,
                batch_size: 32,
            },
            GateProfile::Standard => Self::default(),
            GateProfile::Edge => Self {
                max_memory_mb: 256,
                max_connections: 50,
                compression_enabled: true,
                crypto_enabled: true,
                batch_size: 64,
            },
            GateProfile::Expert => Self {
                max_memory_mb: 512,
                max_connections: 100,
                compression_enabled: false,
                crypto_enabled: true,
                batch_size: 128,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_edge_crypto_gate() {
        let gate = EdgeCryptoGate::new();
        
        assert!(!gate.is_open().await);
        gate.open().await.unwrap();
        assert!(gate.is_open().await);
        gate.close().await.unwrap();
        assert!(!gate.is_open().await);
    }

    #[tokio::test]
    async fn test_edge_network_gate() {
        let mut gate = EdgeNetworkGate::new();
        
        assert!(!gate.is_open().await);
        
        gate.open().await.unwrap();
        assert!(gate.is_open().await);
        
        let data = b"hello edge";
        let result = gate.process(data).await.unwrap();
        assert!(result.len() > data.len()); // Has length prefix
    }

    #[test]
    fn test_edge_profile_config() {
        let edge_config = EdgeProfileConfig::from_profile(GateProfile::Edge);
        assert_eq!(edge_config.max_memory_mb, 256);
        
        let lite_config = EdgeProfileConfig::from_profile(GateProfile::Lite);
        assert!(!lite_config.crypto_enabled);
    }
}
