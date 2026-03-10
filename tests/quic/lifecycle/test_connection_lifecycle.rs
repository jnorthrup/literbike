//! Tests for QUIC connection lifecycle management
//! These tests validate connection state transitions and lifecycle operations.

use literbike::quic::quic_engine::{QuicEngine, Role};
use literbike::quic::quic_protocol::{ConnectionState, QuicConnectionState};
use literbike::concurrency::ccek::CoroutineContext;
use std::sync::Arc;
use tokio::net::UdpSocket;
use std::net::{SocketAddr, Ipv4Addr};

#[tokio::test]
async fn test_connection_state_transitions() {
    // Create a mock socket for testing
    let local_addr: SocketAddr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let socket = UdpSocket::bind(local_addr).await.unwrap();
    let remote_addr: SocketAddr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 12345);

    // Create engine — initial state is Handshaking (set by constructor)
    let engine = QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        Arc::new(socket),
        remote_addr,
        vec![],
        CoroutineContext::new(),
    );

    // Test initial state
    assert_eq!(engine.get_state().connection_state, ConnectionState::Handshaking);

    // Test state transitions
    engine.set_connection_state(ConnectionState::Connected);
    assert_eq!(engine.get_state().connection_state, ConnectionState::Connected);

    engine.set_connection_state(ConnectionState::Closed);
    assert_eq!(engine.get_state().connection_state, ConnectionState::Closed);
}

#[tokio::test]
async fn test_connection_close() {
    let local_addr: SocketAddr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let socket = UdpSocket::bind(local_addr).await.unwrap();
    let remote_addr: SocketAddr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 12345);

    let engine = QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        Arc::new(socket),
        remote_addr,
        vec![],
        CoroutineContext::new(),
    );

    // Establish connection
    engine.set_connection_state(ConnectionState::Connected);
    assert_eq!(engine.get_state().connection_state, ConnectionState::Connected);

    // Close connection
    engine.close().await;
    assert_eq!(engine.get_state().connection_state, ConnectionState::Closed);
}

#[tokio::test]
async fn test_stream_multiplexing() {
    let local_addr: SocketAddr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let socket = UdpSocket::bind(local_addr).await.unwrap();
    let remote_addr: SocketAddr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 12345);

    let engine = QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        Arc::new(socket),
        remote_addr,
        vec![],
        CoroutineContext::new(),
    );

    // Create multiple streams
    let stream1 = engine.create_stream();
    let stream2 = engine.create_stream();
    let stream3 = engine.create_stream();

    assert!(stream1 != stream2);
    assert!(stream2 != stream3);
    assert!(stream1 != stream3);

    // Verify streams exist
    assert!(engine.get_stream(stream1).is_some());
    assert!(engine.get_stream(stream2).is_some());
    assert!(engine.get_stream(stream3).is_some());

    // Get active streams
    let active_streams = engine.get_active_streams();
    assert_eq!(active_streams.len(), 3);
}

#[tokio::test]
async fn test_connection_pool_concept() {
    // This test demonstrates the concept of connection pooling
    // Actual implementation would be in the C ABI or higher-level manager

    let local_addr: SocketAddr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let socket = UdpSocket::bind(local_addr).await.unwrap();
    let remote_addr: SocketAddr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 12345);

    let engine = QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        Arc::new(socket),
        remote_addr,
        vec![],
        CoroutineContext::new(),
    );

    // Simulate connection pooling by reusing the same engine
    engine.set_connection_state(ConnectionState::Connected);
    assert_eq!(engine.get_state().connection_state, ConnectionState::Connected);

    // Create streams for different requests
    let request1_stream = engine.create_stream();
    let request2_stream = engine.create_stream();

    // Both streams should be multiplexed over the same connection
    assert!(engine.get_stream(request1_stream).is_some());
    assert!(engine.get_stream(request2_stream).is_some());

    // Close connection (would be returned to pool in real implementation)
    engine.close().await;
    assert_eq!(engine.get_state().connection_state, ConnectionState::Closed);
}
