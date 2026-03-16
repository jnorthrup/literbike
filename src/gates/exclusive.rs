//! Exclusive Gate System with Edge Profile Support
//!
//! Provides hierarchical gating for Litebike with profile-based
//! down-integration for edge computing scenarios.

use std::sync::Arc;
use parking_lot::RwLock;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod edge_profile;
pub mod daily_driver;

use crate::gates::edge_profile::{EdgeCryptoGate, EdgeNetworkGate};
use crate::gates::daily_driver::DriverMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateProfile {
    Lite,
    Standard,
    Edge,
    Expert,
}

impl Default for GateProfile {
    fn default() -> Self {
        GateProfile::Lite
    }
}

impl GateProfile {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "lite" => GateProfile::Lite,
            "standard" | "std" => GateProfile::Standard,
            "edge" => GateProfile::Edge,
            "expert" => GateProfile::Expert,
            _ => GateProfile::Lite,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            GateProfile::Lite => "lite",
            GateProfile::Standard => "standard",
            GateProfile::Edge => "edge",
            GateProfile::Expert => "expert",
        }
    }
}

/// Exclusive gate that can be in only one state at a time
#[async_trait]
pub trait ExclusiveGate: Send + Sync {
    async fn is_open(&self) -> bool;
    async fn open(&self) -> Result<(), String>;
    async fn close(&self) -> Result<(), String>;
    fn name(&self) -> &str;
    fn profile(&self) -> GateProfile;
}

/// Gate controller with profile-aware routing
pub struct ExclusiveGateController {
    gates: Arc<RwLock<Vec<Arc<dyn ExclusiveGate>>>>,
    current_profile: Arc<RwLock<GateProfile>>,
    edge_mode: Arc<RwLock<bool>>,
}

impl ExclusiveGateController {
    pub fn new() -> Self {
        Self {
            gates: Arc::new(RwLock::new(Vec::new())),
            current_profile: Arc::new(RwLock::new(GateProfile::default())),
            edge_mode: Arc::new(RwLock::new(false)),
        }
    }

    /// Create a gate controller with edge-optimized gates pre-registered
    pub fn with_edge_gates() -> Self {
        let controller = Self::new();
        
        let crypto_gate = Arc::new(EdgeCryptoGate::new()) as Arc<dyn ExclusiveGate>;
        let network_gate = Arc::new(EdgeNetworkGate::new()) as Arc<dyn ExclusiveGate>;
        
        controller.register_gate(crypto_gate);
        controller.register_gate(network_gate);
        
        controller
    }

    /// Create a gate controller with standard gates and edge gates
    pub fn with_all_gates() -> Self {
        let controller = Self::with_edge_gates();
        
        controller
    }

    pub fn register_gate(&self, gate: Arc<dyn ExclusiveGate>) {
        let mut gates = self.gates.write();
        gates.push(gate);
    }

    pub fn set_profile(&self, profile: GateProfile) {
        let mut current = self.current_profile.write();
        *current = profile;
        
        let mut edge = self.edge_mode.write();
        *edge = profile == GateProfile::Edge || profile == GateProfile::Expert;
    }

    pub fn get_profile(&self) -> GateProfile {
        *self.current_profile.read()
    }

    pub fn is_edge_mode(&self) -> bool {
        *self.edge_mode.read()
    }

    pub async fn route_through_gates(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        let profile = self.get_profile();
        let gates = self.gates.read();

        for gate in gates.iter() {
            if gate.profile() <= profile && gate.is_open().await {
                match gate.process(data).await {
                    Ok(result) => return Ok(result),
                    Err(_) => continue,
                }
            }
        }

        Err("No gate could process data".to_string())
    }

    pub async fn open_gate(&self, name: &str) -> Result<(), String> {
        let gates = self.gates.read();
        
        for gate in gates.iter() {
            if gate.name() == name {
                return gate.open().await;
            }
        }
        
        Err(format!("Gate '{}' not found", name))
    }

    pub async fn close_gate(&self, name: &str) -> Result<(), String> {
        let gates = self.gates.read();
        
        for gate in gates.iter() {
            if gate.name() == name {
                return gate.close().await;
            }
        }
        
        Err(format!("Gate '{}' not found", name))
    }
}

impl Default for ExclusiveGateController {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Gate for ExclusiveGateController {
    async fn is_open(&self) -> bool {
        let gates = self.gates.read();
        gates.iter().any(|g| futures::executor::block_on(g.is_open()))
    }

    async fn process(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        self.route_through_gates(data).await
    }

    fn name(&self) -> &str {
        "exclusive_controller"
    }

    fn children(&self) -> Vec<Arc<dyn Gate>> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_defaults() {
        assert_eq!(GateProfile::default(), GateProfile::Lite);
    }

    #[test]
    fn test_profile_parsing() {
        assert_eq!(GateProfile::from_str("edge"), GateProfile::Edge);
        assert_eq!(GateProfile::from_str("EDGE"), GateProfile::Edge);
        assert_eq!(GateProfile::from_str("lite"), GateProfile::Lite);
    }

    #[tokio::test]
    async fn test_gate_controller() {
        let controller = ExclusiveGateController::new();
        controller.set_profile(GateProfile::Edge);
        
        assert_eq!(controller.get_profile(), GateProfile::Edge);
        assert!(controller.is_edge_mode());
    }
}
