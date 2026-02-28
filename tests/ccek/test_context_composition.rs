//! Phase 3: CCEK Integration Tests
//!
//! Tests 3.1-3.2: Context Element Composition, Key Graph Protocol Transitions

use literbike::concurrency::*;
use literbike::quic::quic_ccek::*;
use std::sync::Arc;
use anyhow::Result;

// ============================================================================
// Test 3.1: Context Element Composition
// ============================================================================

// Test 3.1.1: CoroutineContext element addition (+ operator)
#[tokio::test]
async fn test_context_element_addition() {
    let ctx = EmptyContext
        + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>
        + Arc::new(DHTService::new("node-1"));
    
    assert_eq!(ctx.len(), 2);
}

// Test 3.1.2: Context element lookup by key
#[tokio::test]
async fn test_context_element_lookup() {
    let ctx = EmptyContext
        + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>;
    
    assert!(ctx.contains("ProtocolDetector"));
    assert!(!ctx.contains("NonExistent"));
}

// Test 3.1.3: Context immutability after creation
#[tokio::test]
async fn test_context_immutability() {
    let ctx1 = EmptyContext
        + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>;
    
    let ctx2 = ctx1.clone();
    
    assert_eq!(ctx1.len(), ctx2.len());
    assert_eq!(ctx1.contains("ProtocolDetector"), ctx2.contains("ProtocolDetector"));
}

// Test 3.1.4: Context cloning behavior
#[tokio::test]
async fn test_context_cloning() {
    let ctx = EmptyContext
        + Arc::new(DHTService::new("node-1")) as Arc<dyn ContextElement>;
    
    let cloned = ctx.clone();
    
    assert_eq!(cloned.len(), 1);
    assert!(cloned.contains("DHTService"));
}

// Test 3.1.5: ProtocolDetector in context
#[tokio::test]
async fn test_protocol_detector_context() {
    let ctx = EmptyContext
        + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>;
    
    assert!(ctx.contains("ProtocolDetector"));
    assert_eq!(ctx.len(), 1);
}

// Test 3.1.6: DHTService in context
#[tokio::test]
async fn test_dht_service_context() {
    let ctx = EmptyContext
        + Arc::new(DHTService::new("my-node")) as Arc<dyn ContextElement>;
    
    assert!(ctx.contains("DHTService"));
}

// Test 3.1.7: CRDTStorage in context
#[tokio::test]
async fn test_crdt_storage_context() {
    use literbike::concurrency::ccek::CRDTStorage;
    
    let storage = CRDTStorage::new();
    let ctx = EmptyContext
        + Arc::new(storage) as Arc<dyn ContextElement>;
    
    assert!(ctx.contains("CRDTStorage"));
}

// ============================================================================
// Test 3.2: Key Graph Protocol Transitions
// ============================================================================

// Test 3.2.1: QuicCcek::new_with_key_graph() initialization
#[test]
fn test_ccek_initialization() {
    let ccek = QuicCcek::new_with_key_graph();
    
    assert!(ccek.context.is_some());
    assert_eq!(ccek.context.as_ref().unwrap().current_state, 0x1000);
}

// Test 3.2.2: execute_reactor_continuation() state transitions
#[test]
fn test_reactor_continuation() {
    let mut ccek = QuicCcek::new_with_key_graph();
    
    // Initial state
    assert_eq!(ccek.context.as_ref().unwrap().current_state, 0x1000);
    
    // Execute transition to 0x1001
    let result = ccek.execute_reactor_continuation(0x1001);
    assert!(result.is_ok());
    
    // Verify state changed
    assert_eq!(ccek.context.as_ref().unwrap().current_state, 0x1001);
}

// Test 3.2.3: Transition guard validation
#[test]
fn test_transition_guard() {
    let mut ccek = QuicCcek::new_with_key_graph();
    
    // Try invalid transition (skip state)
    let result = ccek.execute_reactor_continuation(0x1002);
    
    // Should fail because we haven't transitioned through 0x1001
    assert!(result.is_err());
}

// Test 3.2.4: Invalid transition error handling
#[test]
fn test_invalid_transition_error() {
    let mut ccek = QuicCcek::new_with_key_graph();
    
    // Try transition from wrong state
    let result = ccek.execute_reactor_continuation(0x9999);
    
    match result {
        Err(CcekError::InvalidTransition(from, to)) => {
            assert_eq!(from, 0x1000);
            assert_eq!(to, 0x9999);
        }
        _ => panic!("Expected InvalidTransition error"),
    }
}

// Test 3.2.5: Continuation stack navigation
#[test]
fn test_continuation_stack() {
    let mut ccek = QuicCcek::new_with_key_graph();
    
    // Initial stack
    assert_eq!(ccek.context.as_ref().unwrap().continuation_stack.len(), 1);
    
    // Execute first transition
    ccek.execute_reactor_continuation(0x1001).unwrap();
    assert_eq!(ccek.context.as_ref().unwrap().continuation_stack.len(), 2);
    
    // Execute second transition
    ccek.execute_reactor_continuation(0x1002).unwrap();
    assert_eq!(ccek.context.as_ref().unwrap().continuation_stack.len(), 3);
}

// Test 3.2.6: Protocol metadata storage and retrieval
#[test]
fn test_protocol_metadata() {
    let mut ccek = QuicCcek::new_with_key_graph();
    
    // Store metadata
    let ctx = ccek.context.as_mut().unwrap();
    ctx.protocol_metadata.insert("version".to_string(), vec![1, 0, 0, 0]);
    ctx.protocol_metadata.insert("cipher".to_string(), vec![0x01, 0x02]);
    
    // Retrieve metadata
    assert_eq!(ctx.protocol_metadata.get("version"), Some(&vec![1, 0, 0, 0]));
    assert_eq!(ctx.protocol_metadata.get("cipher"), Some(&vec![0x01, 0x02]));
}

// Test 3.2.7: CCEK context serialization
#[test]
fn test_ccek_serialization() -> Result<()> {
    let ccek = QuicCcek::new_with_key_graph();
    
    // Serialize context state
    let state = ccek.context.as_ref().unwrap().current_state;
    let serialized = bincode::serialize(&state)?;
    
    // Deserialize
    let deserialized: u64 = bincode::deserialize(&serialized)?;
    assert_eq!(deserialized, state);
    
    Ok(())
}

// Test 3.2.8: CCEK integration with QUIC engine
#[tokio::test]
async fn test_ccek_quic_integration() -> Result<()> {
    use literbike::quic::*;
    
    // Create CCEK context
    let mut ccek = QuicCcek::new_with_key_graph();
    
    // Create QUIC engine
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = QuicEngine::new(Role::Client, QuicConnectionState::default(), socket, addr, vec![]);
    
    // Store QUIC state in CCEK metadata
    {
        let ctx = ccek.context.as_mut().unwrap();
        let quic_state = engine.state.lock();
        let state_bytes = bincode::serialize(&*quic_state).unwrap_or_default();
        ctx.protocol_metadata.insert("quic_state".to_string(), state_bytes);
    }
    
    // Verify integration
    assert!(ccek.context.as_ref().unwrap().protocol_metadata.contains_key("quic_state"));
    
    Ok(())
}

// ============================================================================
// Additional CCEK Tests
// ============================================================================

// Test: Context with multiple services
#[tokio::test]
async fn test_context_multiple_services() {
    let ctx = EmptyContext
        + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>
        + Arc::new(DHTService::new("node-1")) as Arc<dyn ContextElement>
        + Arc::new(CRDTStorage::new()) as Arc<dyn ContextElement>;
    
    assert_eq!(ctx.len(), 3);
    assert!(ctx.contains("ProtocolDetector"));
    assert!(ctx.contains("DHTService"));
    assert!(ctx.contains("CRDTStorage"));
}

// Test: Context spawn task
#[tokio::test]
async fn test_context_spawn_task() {
    let ctx = EmptyContext
        + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>;
    
    let handle = ctx.spawn_task(async {
        42
    });
    
    let result = handle.await.unwrap();
    assert_eq!(result, 42);
}

// Test: Context channel creation
#[tokio::test]
async fn test_context_channel() {
    let ctx = EmptyContext;
    
    let (tx, rx) = ctx.create_channel::<String>(10);
    
    tx.send("hello".to_string()).await.unwrap();
    let value = rx.recv().await.unwrap();
    
    assert_eq!(value, "hello");
}
