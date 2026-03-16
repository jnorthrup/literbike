//! Integration tests for exclusive gate system
//!
//! Tests profile-based routing, edge gate integration, and gate controller behavior.

use std::sync::Arc;
use tokio::test;

use crate::gates::exclusive::{ExclusiveGate, ExclusiveGateController, GateProfile};
use crate::gates::edge_profile::{EdgeCryptoGate, EdgeNetworkGate};

#[tokio::test]
async fn test_edge_gates_registration() {
    let controller = ExclusiveGateController::with_edge_gates();
    
    controller.open_gate("edge_crypto").await.unwrap();
    controller.open_gate("edge_network").await.unwrap();
    
    let data = b"test data";
    let result = controller.route_through_gates(data).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_profile_based_routing() {
    let controller = ExclusiveGateController::with_edge_gates();
    
    controller.set_profile(GateProfile::Lite);
    assert!(!controller.is_edge_mode());
    
    controller.set_profile(GateProfile::Edge);
    assert!(controller.is_edge_mode());
    
    controller.set_profile(GateProfile::Expert);
    assert!(controller.is_edge_mode());
}

#[tokio::test]
async fn test_gate_state_management() {
    let controller = ExclusiveGateController::new();
    let crypto_gate = Arc::new(EdgeCryptoGate::new());
    
    controller.register_gate(crypto_gate.clone());
    
    assert!(!crypto_gate.is_open().await);
    
    crypto_gate.open().await.unwrap();
    assert!(crypto_gate.is_open().await);
    
    crypto_gate.close().await.unwrap();
    assert!(!crypto_gate.is_open().await);
}

#[tokio::test]
async fn test_network_gate_compression() {
    let mut gate = EdgeNetworkGate::new();
    
    gate.open().await.unwrap();
    
    let data = b"test data for compression";
    let result = gate.process(data).await;
    
    assert!(result.is_ok());
    let processed = result.unwrap();
    
    assert!(processed.len() > data.len()); // Should have length prefix
}

#[tokio::test]
async fn test_controller_with_all_gates() {
    let controller = ExclusiveGateController::with_all_gates();
    
    controller.set_profile(GateProfile::Edge);
    assert_eq!(controller.get_profile(), GateProfile::Edge);
    
    controller.open_gate("edge_crypto").await.unwrap();
    controller.open_gate("edge_network").await.unwrap();
    
    let data = b"test routing";
    let result = controller.route_through_gates(data).await;
    
    assert!(result.is_ok());
}

#[test]
fn test_edge_profile_config() {
    let lite_config = crate::gates::EdgeProfileConfig::from_profile(GateProfile::Lite);
    assert!(!lite_config.crypto_enabled);
    assert_eq!(lite_config.max_connections, 10);
    
    let edge_config = crate::gates::EdgeProfileConfig::from_profile(GateProfile::Edge);
    assert!(edge_config.crypto_enabled);
    assert_eq!(edge_config.max_connections, 50);
}
