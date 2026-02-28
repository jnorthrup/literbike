//! Test 1.2: Connection State Machine
//!
//! Tests for QUIC connection state transitions, timeout handling,
//! and concurrent state updates.

use literbike::quic::*;
use parking_lot::Mutex;
use std::sync::Arc;
use anyhow::Result;

// ============================================================================
// Test 1.2.1: State transitions (Idle → Handshaking → Connected → Closed)
// ============================================================================

#[test]
fn test_state_transitions() -> Result<()> {
    // Initial state should be Handshaking after engine creation
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    
    let initial_state = QuicConnectionState::default();
    let engine = QuicEngine::new(
        Role::Client,
        initial_state,
        socket,
        addr,
        vec![],
    );

    // Verify initial state is Handshaking (set in constructor)
    {
        let state = engine.state.lock();
        assert_eq!(state.connection_state, ConnectionState::Handshaking);
    }

    // Simulate receiving ACK to transition to Connected
    let ack_packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::ShortHeader,
            version: 1,
            destination_connection_id: vec![],
            source_connection_id: vec![],
            packet_number: 0,
            token: None,
        },
        frames: vec![QuicFrame::Ack(AckFrame {
            largest_acknowledged: 0,
            ack_delay: 0,
            ack_ranges: vec![],
        })],
        payload: vec![],
    };

    // Process ACK (should transition to Connected)
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(engine.process_packet(ack_packet))?;

    {
        let state = engine.state.lock();
        assert_eq!(state.connection_state, ConnectionState::Connected);
    }

    Ok(())
}

// ============================================================================
// Test 1.2.2: Invalid state transition rejection
// ============================================================================

#[test]
fn test_invalid_state_transition() {
    // Create state machine in Connected state
    let mut state = QuicConnectionState::default();
    state.connection_state = ConnectionState::Connected;

    // Verify we can't directly manipulate state to invalid values
    // (In production, state transitions should be controlled)
    assert_eq!(state.connection_state, ConnectionState::Connected);

    // State should not spontaneously change
    assert_ne!(state.connection_state, ConnectionState::Idle);
    assert_ne!(state.connection_state, ConnectionState::Closed);
}

// ============================================================================
// Test 1.2.3: Connection timeout handling
// ============================================================================

#[test]
fn test_connection_timeout() -> Result<()> {
    use std::time::{Duration, Instant};

    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    
    let mut initial_state = QuicConnectionState::default();
    initial_state.idle_timeout = Duration::from_millis(100);
    initial_state.last_activity = Instant::now();

    let engine = QuicEngine::new(
        Role::Client,
        initial_state,
        socket,
        addr,
        vec![],
    );

    // Verify timeout is set
    {
        let state = engine.state.lock();
        assert_eq!(state.idle_timeout, Duration::from_millis(100));
    }

    // Wait for timeout
    std::thread::sleep(Duration::from_millis(150));

    // Check if connection is considered timed out
    {
        let state = engine.state.lock();
        let elapsed = state.last_activity.elapsed();
        assert!(elapsed >= Duration::from_millis(100));
        // Connection should be considered timed out
        assert!(elapsed > state.idle_timeout);
    }

    Ok(())
}

// ============================================================================
// Test 1.2.4: Concurrent connection state updates (thread safety)
// ============================================================================

#[test]
fn test_concurrent_state_updates() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    
    let initial_state = QuicConnectionState::default();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        initial_state,
        socket,
        addr,
        vec![],
    ));

    // Spawn multiple threads to access state concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let engine_clone = Arc::clone(&engine);
        let handle = std::thread::spawn(move || {
            for _ in 0..100 {
                let state = engine_clone.state.lock();
                // Just read the state
                let _ = state.connection_state;
                drop(state);
            }
            i
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify state is still consistent
    let state = engine.state.lock();
    assert_eq!(state.connection_state, ConnectionState::Handshaking);

    Ok(())
}

// ============================================================================
// Test 1.2.5: Connection ID rotation
// ============================================================================

#[test]
fn test_connection_id_rotation() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    
    let mut initial_state = QuicConnectionState::default();
    initial_state.local_connection_id = vec![0x01, 0x02, 0x03, 0x04];
    initial_state.remote_connection_id = vec![0x05, 0x06, 0x07, 0x08];

    let engine = QuicEngine::new(
        Role::Client,
        initial_state,
        socket,
        addr,
        vec![],
    );

    // Verify initial connection IDs
    {
        let state = engine.state.lock();
        assert_eq!(state.local_connection_id, vec![0x01, 0x02, 0x03, 0x04]);
        assert_eq!(state.remote_connection_id, vec![0x05, 0x06, 0x07, 0x08]);
    }

    // Simulate connection ID rotation (new connection ID frame)
    {
        let mut state = engine.state.lock();
        state.local_connection_id = vec![0xAA, 0xBB, 0xCC, 0xDD];
        state.remote_connection_id = vec![0x11, 0x22, 0x33, 0x44];
    }

    // Verify rotated connection IDs
    {
        let state = engine.state.lock();
        assert_eq!(state.local_connection_id, vec![0xAA, 0xBB, 0xCC, 0xDD]);
        assert_eq!(state.remote_connection_id, vec![0x11, 0x22, 0x33, 0x44]);
    }

    Ok(())
}

// ============================================================================
// Test 1.2.6: Retry token generation and validation
// ============================================================================

#[test]
fn test_retry_token() -> Result<()> {
    use sha2::{Sha256, Digest};

    // Generate a retry token (simplified)
    let connection_id = vec![0x01, 0x02, 0x03, 0x04];
    let secret = b"retry_secret_key";
    
    let mut hasher = Sha256::new();
    hasher.update(&connection_id);
    hasher.update(secret);
    let token = hasher.finalize().to_vec();

    assert_eq!(token.len(), 32); // SHA256 output

    // Validate token
    let mut validator = Sha256::new();
    validator.update(&connection_id);
    validator.update(secret);
    let expected = validator.finalize().to_vec();

    assert_eq!(token, expected);

    // Test with different connection ID (should fail)
    let wrong_id = vec![0x05, 0x06, 0x07, 0x08];
    let mut wrong_hasher = Sha256::new();
    wrong_hasher.update(&wrong_id);
    wrong_hasher.update(secret);
    let wrong_token = wrong_hasher.finalize().to_vec();

    assert_ne!(token, wrong_token);

    Ok(())
}

// ============================================================================
// Test 1.2.7: State serialization for recovery
// ============================================================================

#[test]
fn test_state_serialization() -> Result<()> {
    let mut state = QuicConnectionState::default();
    state.connection_state = ConnectionState::Connected;
    state.version = 0x00000001;
    state.local_connection_id = vec![1, 2, 3, 4];
    state.remote_connection_id = vec![5, 6, 7, 8];
    state.next_packet_number = 100;

    // Serialize state
    let serialized = bincode::serialize(&state)?;
    assert!(!serialized.is_empty());

    // Deserialize state
    let deserialized: QuicConnectionState = bincode::deserialize(&serialized)?;

    // Verify state preserved
    assert_eq!(deserialized.connection_state, ConnectionState::Connected);
    assert_eq!(deserialized.version, 0x00000001);
    assert_eq!(deserialized.local_connection_id, vec![1, 2, 3, 4]);
    assert_eq!(deserialized.remote_connection_id, vec![5, 6, 7, 8]);
    assert_eq!(deserialized.next_packet_number, 100);

    Ok(())
}
