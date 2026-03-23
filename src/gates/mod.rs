// LITERBIKE Gate System (AGPL Licensed)
// Hierarchical gating for protocols and crypto

use std::sync::Arc;
use parking_lot::RwLock;
use async_trait::async_trait;

pub mod shadowsocks_gate;
pub mod crypto_gate;
pub mod ssh_gate;
pub mod exclusive;
pub mod edge_profile;
pub mod daily_driver;

pub use exclusive::{ExclusiveGate, ExclusiveGateController, GateProfile};
pub use edge_profile::{EdgeCryptoGate, EdgeNetworkGate, EdgeProfileConfig};
pub use daily_driver::{DailyDriverConfig, DailyDriverState, DriverMode, ConnectionTracker};
pub use daily_driver::cli::{Cli, Commands, DriverStatus, RuntimeSwitches};
pub use daily_driver::driver::CliDriver;

#[cfg(test)]
mod tests;

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
    ssh_gate: Arc<ssh_gate::SSHGate>,
}

impl LitebikeGateController {
    pub fn new() -> Self {
        let shadowsocks_gate = Arc::new(shadowsocks_gate::ShadowsocksGate::new());
        let crypto_gate = Arc::new(crypto_gate::CryptoGate::new());
        let ssh_gate = Arc::new(ssh_gate::SSHGate::new());

        let gates: Vec<Arc<dyn Gate>> = vec![
            shadowsocks_gate.clone() as Arc<dyn Gate>,
            crypto_gate.clone() as Arc<dyn Gate>,
            ssh_gate.clone() as Arc<dyn Gate>,
        ];

        Self {
            gates: Arc::new(RwLock::new(gates)),
            shadowsocks_gate,
            crypto_gate,
            ssh_gate,
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
}