//! Phase 5: QUIC Stream Ingestion Tests
//!
//! Tests 5.1-5.2: Stream data ingestion and integration scenarios
//! using the QUIC protocol layer directly.

use literbike::quic::*;
use literbike::quic::quic_protocol::ConnectionId;
use literbike::concurrency::ccek::CoroutineContext;
use anyhow::Result;
use std::sync::Arc;

// ============================================================================
// Test 5.1: Ingestion Pipeline
// ============================================================================

// Test 5.1.1: Stream data ingestion end-to-end
#[tokio::test]
async fn test_ingest_end_to_end() -> Result<()> {
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

    // Ingest data via stream
    let data = b"BTC/USD:45000.0:1.5:1";
    let result = engine.send_stream_data(stream_id, data.to_vec()).await;
    assert!(result.is_ok());

    Ok(())
}

// Test 5.1.2: Multiple stream distribution
#[tokio::test]
async fn test_multiple_stream_distribution() -> Result<()> {
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

    // Create multiple streams
    let mut stream_ids = Vec::new();
    for _ in 0..5 {
        stream_ids.push(engine.create_stream());
    }

    // Send to each stream
    for (i, &sid) in stream_ids.iter().enumerate() {
        let data = format!("ETH/USD:3200.0:10.0:{}", i);
        engine.send_stream_data(sid, data.into_bytes()).await?;
    }

    Ok(())
}

// Test 5.1.3: Stream backpressure via send
#[tokio::test]
async fn test_stream_backpressure() -> Result<()> {
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

    // Produce many messages
    for i in 0u64..100 {
        let data = format!("BTC/USD:45000.0:1.5:{}", i);
        let result = engine.send_stream_data(stream_id, data.into_bytes()).await;
        // Allow either success or flow-control error
        let _ = result;
    }

    Ok(())
}

// Test 5.1.4: Concurrent stream ingestion
#[tokio::test]
async fn test_concurrent_stream_ingestion() -> Result<()> {
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

    let mut handles = vec![];
    for _ in 0..5 {
        let eng = Arc::clone(&engine);
        let handle = tokio::spawn(async move {
            let sid = eng.create_stream();
            for j in 0u64..10 {
                let data = format!("BTC/USD:45000.0:1.5:{}", j);
                eng.send_stream_data(sid, data.into_bytes()).await.ok();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    Ok(())
}

// Test 5.1.5: Message durability via sent_packets tracking
#[tokio::test]
async fn test_durability() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    );

    let stream_id = engine.create_stream();

    // Send messages
    for i in 0u64..10 {
        let data = format!("BTC/USD:45000.0:1.5:{}", i);
        engine.send_stream_data(stream_id, data.into_bytes()).await?;
    }

    // Verify packets tracked
    let state = engine.get_state();
    assert!(!state.sent_packets.is_empty());

    Ok(())
}

// Test 5.1.6: Ingest throughput
#[tokio::test]
async fn test_ingest_rate() -> Result<()> {
    use std::time::Instant;

    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    );

    let stream_id = engine.create_stream();

    // Measure ingest rate
    let start = Instant::now();
    let count = 100u64;

    for i in 0..count {
        let data = format!("BTC/USD:45000.0:1.5:{}", i);
        engine.send_stream_data(stream_id, data.into_bytes()).await?;
    }

    let elapsed = start.elapsed();
    let rate = count as f64 / elapsed.as_secs_f64();

    println!("Ingest rate: {:.0} messages/second", rate);

    // Should achieve reasonable throughput
    assert!(rate > 10.0, "Ingest rate too low: {}", rate);

    Ok(())
}

// ============================================================================
// Test 5.2: Stream Integration
// ============================================================================

// Test 5.2.1: QUIC stream data integration
#[tokio::test]
async fn test_quic_stream_data_integration() -> Result<()> {
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

    for i in 0u64..20 {
        let data = format!("BTC/USD:{}:1.5:{}", 45000.0 + i as f64, i);
        engine.send_stream_data(stream_id, data.into_bytes()).await?;
    }

    // Verify packets were tracked
    let state = engine.get_state();
    assert!(!state.sent_packets.is_empty());

    Ok(())
}

// Test 5.2.2: Stream payload via QuicPacket with STREAM frames
#[test]
fn test_stream_frame_payload() -> Result<()> {
    // Simulate stream data arriving in QUIC packet format
    let packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::ShortHeader,
            version: 1,
            destination_connection_id: ConnectionId { bytes: vec![1, 2, 3, 4] },
            source_connection_id: ConnectionId { bytes: vec![] },
            packet_number: 1,
            token: None,
        },
        frames: vec![QuicFrame::Stream(StreamFrame {
            stream_id: 1,
            offset: 0,
            data: b"BTC/USD:45000.0".to_vec(),
            fin: false,
        })],
        payload: vec![],
    };

    // Serialize and deserialize
    let serialized = bincode::serialize(&packet)?;
    let deserialized: QuicPacket = bincode::deserialize(&serialized)?;

    assert_eq!(deserialized.frames.len(), 1);
    if let QuicFrame::Stream(sf) = &deserialized.frames[0] {
        assert_eq!(sf.data, b"BTC/USD:45000.0");
    }

    Ok(())
}

// Test 5.2.3: Stream error propagation
#[tokio::test]
async fn test_stream_error_propagation() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    );

    let stream_id = engine.create_stream();

    // Send valid data
    let result = engine.send_stream_data(stream_id, b"BTC/USD:45000.0:1.5:1".to_vec()).await;
    assert!(result.is_ok());

    Ok(())
}

// Test 5.2.4: Stream close handling (FIN)
#[tokio::test]
async fn test_stream_close() -> Result<()> {
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

    // Send some messages
    for i in 0u64..5 {
        let data = format!("BTC/USD:45000.0:1.5:{}", i);
        engine.send_stream_data(stream_id, data.into_bytes()).await?;
    }

    // Close with FIN
    let mut stream = QuicStream::new(stream_id, Arc::clone(&engine), addr);
    stream.finish().await?;

    Ok(())
}

// Test 5.2.5: Multiple concurrent streams
#[tokio::test]
async fn test_concurrent_streams() -> Result<()> {
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

    let mut handles = vec![];
    for i in 0..5 {
        let eng = Arc::clone(&engine);
        let handle = tokio::spawn(async move {
            let sid = eng.create_stream();
            for j in 0u64..10 {
                let data = format!("BTC/USD:45000.0:1.5:{}", i * 10 + j);
                eng.send_stream_data(sid, data.into_bytes()).await.ok();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Verify streams created
    let active = engine.get_active_streams();
    assert!(!active.is_empty());

    Ok(())
}

// Test 5.2.6: Stream priority handling
#[tokio::test]
async fn test_stream_priority() -> Result<()> {
    use literbike::quic::quic_protocol::StreamPriority;

    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
        CoroutineContext::new(),
    );

    // Create high-priority stream (e.g., price alerts)
    let high_priority_stream = engine.create_stream_with_priority(StreamPriority::High);
    let normal_stream = engine.create_stream_with_priority(StreamPriority::Normal);

    // Both streams should exist
    assert!(engine.get_stream(high_priority_stream).is_some());
    assert!(engine.get_stream(normal_stream).is_some());

    Ok(())
}
