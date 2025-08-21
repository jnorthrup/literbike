// LITERBIKE Gate System (AGPL Licensed)
// Hierarchical gating for protocols and crypto

use std::sync::Arc;
use parking_lot::RwLock;
use async_trait::async_trait;

pub mod shadowsocks_gate;
pub mod crypto_gate;
pub mod htx_gate;

/// Master gate trait for LITERBIKE
#[async_trait]
pub trait Gate: Send + Sync {
    /// Check if gate allows passage
    async fn is_open(&self) -> bool;
    
    /// Process data through gate
    async fn process(&self, data: &[u8]) -> Result<Vec<u8>, String>;
    
    /// Gate identifier
    fn name(&self) -> &str;
    
    /// Child gates
    fn children(&self) -> Vec<Arc<dyn Gate>>;
}

/// LITEBIKE master gate controller
pub struct LitebikeGateController {
    gates: Arc<RwLock<Vec<Arc<dyn Gate>>>>,
    shadowsocks_gate: Arc<shadowsocks_gate::ShadowsocksGate>,
    crypto_gate: Arc<crypto_gate::CryptoGate>,
    htx_gate: Arc<htx_gate::HTXGate>,
}

impl LitebikeGateController {
    pub fn new() -> Self {
        let shadowsocks_gate = Arc::new(shadowsocks_gate::ShadowsocksGate::new());
        let crypto_gate = Arc::new(crypto_gate::CryptoGate::new());
        let htx_gate = Arc::new(htx_gate::HTXGate::new());
        
        let gates: Vec<Arc<dyn Gate>> = vec![
            shadowsocks_gate.clone() as Arc<dyn Gate>,
            crypto_gate.clone() as Arc<dyn Gate>,
            htx_gate.clone() as Arc<dyn Gate>,
        ];
        
        Self {
            gates: Arc::new(RwLock::new(gates)),
            shadowsocks_gate,
            crypto_gate,
            htx_gate,
        }
    }
    
    /// Route data through appropriate gate
    pub async fn route(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        // Check each gate in order
        let gates = self.gates.read();
        
        for gate in gates.iter() {
            if gate.is_open().await {
                // Try to process through this gate
                if let Ok(result) = gate.process(data).await {
                    return Ok(result);
                }
            }
        }
        
        Err("No gate could process data".to_string())
    }
    
    /// Add HTX as a downstream consumer (not modifying HTX)
    pub fn connect_htx_downstream(&self, htx_endpoint: String) {
        self.htx_gate.set_endpoint(htx_endpoint);
    }
}