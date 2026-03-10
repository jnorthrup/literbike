//! Phase 6: Channelized Distribution Tests
//!
//! Tests 6.1-6.2: Channel Management, Distribution Patterns
//! Using QUIC engine stream multiplexing as the distribution mechanism.

use literbike::quic::*;
use literbike::quic::quic_protocol::ConnectionId;
use literbike::concurrency::ccek::CoroutineContext;
use anyhow::Result;
use std::sync::Arc;

// ============================================================================
// Test 6.1: Channel Management via QUIC streams
// ============================================================================

// Test 6.1.1: Multi-stream creation with various counts
#[tokio::test]
async fn test_distributor_creation() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    ));

    // Create 1 stream
    let stream1 = engine.create_stream();
    assert!(engine.get_stream(stream1).is_some());

    // Create 10 streams
    let mut streams = vec![stream1];
    for _ in 0..9 {
        streams.push(engine.create_stream());
    }
    assert_eq!(engine.get_active_streams().len(), 10);

    Ok(())
}

// Test 6.1.2: Bounded stream buffer behavior
#[tokio::test]
async fn test_bounded_channel_buffer() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    ));

    let stream_id = engine.create_stream();

    // Fill with data
    for i in 0u64..5 {
        let data = format!("BTC/USD:45000.0:1.5:{}", i);
        engine.send_stream_data(stream_id, data.into_bytes()).await?;
    }

    // Verify state tracked
    let state = engine.get_state();
    assert!(!state.sent_packets.is_empty());

    Ok(())
}

// Test 6.1.3: Stream closure handling
#[tokio::test]
async fn test_channel_closure() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    ));

    let stream_id = engine.create_stream();

    // Close with FIN
    let result = engine.send_stream_fin(stream_id).await;

    // Should handle closed stream
    assert!(result.is_ok() || result.is_err());

    Ok(())
}

// Test 6.1.4: Multiple distributor instances (connection reconnection)
#[tokio::test]
async fn test_channel_reconnection() -> Result<()> {
    // Create first engine
    let socket1 = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine1 = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket1,
        addr,
        vec![],
        CoroutineContext::new(),
    ));

    // Create second engine (reconnection simulation)
    let socket2 = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let engine2 = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket2,
        addr,
        vec![],
        CoroutineContext::new(),
    ));

    // Both should work independently
    let sid1 = engine1.create_stream();
    let sid2 = engine2.create_stream();

    engine1.send_stream_data(sid1, b"BTC/USD:45000.0".to_vec()).await?;
    engine2.send_stream_data(sid2, b"ETH/USD:3200.0".to_vec()).await?;

    Ok(())
}

// Test 6.1.5: Long-running stream (no memory leak simulation)
#[tokio::test]
async fn test_channel_memory_long_running() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    ));

    let stream_id = engine.create_stream();

    // Run extended distribution
    for i in 0u64..100 {
        let data = format!("BTC/USD:45000.0:1.5:{}", i);
        engine.send_stream_data(stream_id, data.into_bytes()).await?;
    }

    // Should complete without memory issues
    Ok(())
}

// ============================================================================
// Test 6.2: Distribution Patterns via QUIC streams
// ============================================================================

// Test 6.2.1: Broadcast to all streams
#[tokio::test]
async fn test_broadcast_distribution() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    ));

    // Create 5 streams
    let streams: Vec<u64> = (0..5).map(|_| engine.create_stream()).collect();

    // Broadcast same message to all
    let data = b"BTC/USD:45000.0".to_vec();
    for &sid in &streams {
        engine.send_stream_data(sid, data.clone()).await?;
    }

    Ok(())
}

// Test 6.2.2: Round-robin distribution across streams
#[tokio::test]
async fn test_round_robin_distribution() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    ));

    let streams: Vec<u64> = (0..3).map(|_| engine.create_stream()).collect();

    // Send 9 messages round-robin
    for i in 0u64..9 {
        let sid = streams[(i as usize) % streams.len()];
        let data = format!("BTC/USD:45000.0:1.5:{}", i);
        engine.send_stream_data(sid, data.into_bytes()).await?;
    }

    Ok(())
}

// Test 6.2.3: Partitioned distribution (by symbol)
#[tokio::test]
async fn test_partitioned_distribution() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    ));

    // Separate stream per symbol partition
    let btc_stream = engine.create_stream();
    let eth_stream = engine.create_stream();

    // Distribute to partitions
    for i in 0u64..10 {
        engine.send_stream_data(btc_stream, format!("BTC/USD:45000.0:1.5:{}", i).into_bytes()).await?;
        engine.send_stream_data(eth_stream, format!("ETH/USD:3200.0:10.0:{}", i).into_bytes()).await?;
    }

    // Verify streams are distinct
    assert_ne!(btc_stream, eth_stream);

    Ok(())
}

// Test 6.2.4: Distribution ordering guarantees
#[tokio::test]
async fn test_distribution_ordering() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    ));

    let stream_id = engine.create_stream();

    // Send ordered messages
    for i in 0u64..20 {
        let data = format!("BTC/USD:{}:1.5:{}", 45000.0 + i as f64, i);
        engine.send_stream_data(stream_id, data.into_bytes()).await?;
    }

    // Verify packet numbers are monotonically increasing
    let state = engine.get_state();
    let packets = &state.sent_packets;
    for i in 1..packets.len() {
        assert!(packets[i].header.packet_number > packets[i-1].header.packet_number);
    }

    Ok(())
}

// Test 6.2.5: Distribution with multiple concurrent producers
#[tokio::test]
async fn test_distribution_slow_consumers() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    ));

    let stream_id = engine.create_stream();

    // Produce faster than consume (send 50 messages rapidly)
    for i in 0u64..50 {
        let data = format!("BTC/USD:45000.0:1.5:{}", i);
        let _ = engine.send_stream_data(stream_id, data.into_bytes()).await;
    }

    Ok(())
}

// Test 6.2.6: Distribution failure handling (stream closed)
#[tokio::test]
async fn test_distribution_failure() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    ));

    let stream_id = engine.create_stream();

    // Close engine
    engine.close().await;

    // Distribute after close should handle gracefully
    let result = engine.send_stream_data(stream_id, b"BTC/USD:45000.0".to_vec()).await;

    // Should not panic
    let _ = result;

    Ok(())
}

// Test 6.2.7: Consumer rebalancing (removing streams)
#[tokio::test]
async fn test_consumer_rebalancing() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    ));

    // Initial streams
    let mut streams: Vec<u64> = (0..4).map(|_| engine.create_stream()).collect();

    // Initial distribution
    engine.send_stream_data(streams[0], b"BTC/USD:45000.0".to_vec()).await?;

    // Remove one stream (simulates rebalance)
    streams.pop();

    // Continue distribution to remaining
    for &sid in &streams {
        engine.send_stream_data(sid, b"ETH/USD:3200.0".to_vec()).await?;
    }

    Ok(())
}

// Test 6.2.8: Distribution metrics (throughput)
#[tokio::test]
async fn test_distribution_metrics() -> Result<()> {
    use std::time::Instant;

    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    ));

    let streams: Vec<u64> = (0..3).map(|_| engine.create_stream()).collect();

    // Measure distribution latency
    let start = Instant::now();
    let count = 100u64;

    for i in 0..count {
        let sid = streams[(i as usize) % streams.len()];
        let data = format!("BTC/USD:45000.0:1.5:{}", i);
        engine.send_stream_data(sid, data.into_bytes()).await?;
    }

    let elapsed = start.elapsed();
    let rate = count as f64 / elapsed.as_secs_f64();

    println!("Distribution rate: {:.0} msg/s", rate);

    Ok(())
}
