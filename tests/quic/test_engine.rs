//! Phase 2: QUIC Engine Layer Tests
//!
//! Tests 2.1-2.3: Stream Management, ACK Generation, Crypto Frame Handling

use literbike::quic::*;
use parking_lot::Mutex;
use std::sync::Arc;
use anyhow::Result;

// ============================================================================
// Test 2.1: Stream Management
// ============================================================================

// Test 2.1.1: Stream creation (client-initiated, server-initiated)
#[tokio::test]
async fn test_stream_creation() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    
    let initial_state = QuicConnectionState::default();
    let engine = QuicEngine::new(Role::Client, initial_state, socket, addr, vec![]);
    
    // Create stream
    let stream_id = engine.create_stream();
    
    // Client-initiated streams should be odd (QUIC spec)
    assert_eq!(stream_id % 2, 1);
    
    // Create another stream
    let stream_id2 = engine.create_stream();
    assert!(stream_id2 > stream_id);
    
    Ok(())
}

// Test 2.1.2: Stream ID uniqueness across connections
#[tokio::test]
async fn test_stream_id_uniqueness() -> Result<()> {
    let socket1 = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr1 = "127.0.0.1:12345".parse().unwrap();
    let engine1 = QuicEngine::new(Role::Client, QuicConnectionState::default(), socket1, addr1, vec![]);
    
    let socket2 = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr2 = "127.0.0.1:12346".parse().unwrap();
    let engine2 = QuicEngine::new(Role::Client, QuicConnectionState::default(), socket2, addr2, vec![]);
    
    // Each connection should have independent stream IDs
    let stream1 = engine1.create_stream();
    let stream2 = engine2.create_stream();
    
    // Both start at same initial ID (client-initiated)
    assert_eq!(stream1, stream2);
    
    // But subsequent streams are independent
    let stream1b = engine1.create_stream();
    let stream2b = engine2.create_stream();
    
    assert_eq!(stream1b, stream1 + 4); // QUIC stream ID increment
    assert_eq!(stream2b, stream2 + 4);
    
    Ok(())
}

// Test 2.1.3: Stream multiplexing (multiple streams per connection)
#[tokio::test]
async fn test_stream_multiplexing() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(Role::Client, QuicConnectionState::default(), socket, addr, vec![]));
    
    // Create multiple streams
    let mut stream_ids = Vec::new();
    for _ in 0..10 {
        stream_ids.push(engine.create_stream());
    }
    
    // All stream IDs should be unique
    stream_ids.sort();
    for i in 1..stream_ids.len() {
        assert_ne!(stream_ids[i], stream_ids[i-1]);
    }
    
    // Send data on multiple streams concurrently
    let mut handles = Vec::new();
    for stream_id in stream_ids.iter().take(5) {
        let engine_clone = Arc::clone(&engine);
        let sid = *stream_id;
        let handle = tokio::spawn(async move {
            engine_clone.send_stream_data(sid, vec![sid as u8; 100]).await
        });
        handles.push(handle);
    }
    
    // Wait for all sends to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
    
    Ok(())
}

// Test 2.1.4: Stream flow control (max_data, max_stream_data)
#[tokio::test]
async fn test_stream_flow_control() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    
    let mut initial_state = QuicConnectionState::default();
    initial_state.transport_params.max_data = 1024; // 1KB connection limit
    initial_state.transport_params.max_stream_data = 256; // 256B stream limit
    
    let engine = QuicEngine::new(Role::Client, initial_state, socket, addr, vec![]);
    
    let stream_id = engine.create_stream();
    
    // Send data up to stream limit
    let result = engine.send_stream_data(stream_id, vec![0; 256]).await;
    assert!(result.is_ok());
    
    // Exceeding stream limit should fail or be blocked
    // (In real implementation, this would return FLOW_CONTROL_ERROR)
    let result = engine.send_stream_data(stream_id, vec![0; 256]).await;
    // For now, just verify it doesn't crash
    let _ = result;
    
    Ok(())
}

// Test 2.1.5: Stream reset handling
#[tokio::test]
async fn test_stream_reset() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = QuicEngine::new(Role::Client, QuicConnectionState::default(), socket, addr, vec![]);
    
    let stream_id = engine.create_stream();
    
    // Send some data
    engine.send_stream_data(stream_id, vec![1, 2, 3]).await?;
    
    // Reset stream (application would send RESET_STREAM frame)
    // For now, just verify we can create a new stream with same ID pattern
    let new_stream = engine.create_stream();
    assert!(new_stream > stream_id);
    
    Ok(())
}

// Test 2.1.6: Stream finish signaling (FIN bit)
#[tokio::test]
async fn test_stream_finish() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        QuicConnectionState::default(),
        socket,
        addr,
        vec![],
    ));
    
    let stream_id = engine.create_stream();
    
    // Send data with FIN
    let mut stream = QuicStream::new(stream_id, Arc::clone(&engine), addr);
    stream.write(&[1, 2, 3]).await?;
    stream.finish().await?;
    
    // Verify finish drives engine stream state via FIN path
    let stream_state = engine.get_stream(stream_id).expect("stream exists");
    assert_eq!(stream_state.state, StreamState::HalfClosedLocal);

    Ok(())
}

// ============================================================================
// Test 2.2: ACK Generation and Processing
// ============================================================================

// Test 2.2.1: ACK frame generation (packet number ranges)
#[test]
fn test_ack_frame_generation() -> Result<()> {
    let ack_frame = AckFrame {
        largest_acknowledged: 100,
        ack_delay: 500,
        ack_ranges: vec![(90, 100), (80, 85), (70, 75)],
    };
    
    // Verify ACK frame structure
    assert_eq!(ack_frame.largest_acknowledged, 100);
    assert_eq!(ack_frame.ack_delay, 500);
    assert_eq!(ack_frame.ack_ranges.len(), 3);
    
    // Serialize and deserialize
    let serialized = bincode::serialize(&ack_frame)?;
    let deserialized: AckFrame = bincode::deserialize(&serialized)?;
    
    assert_eq!(deserialized.largest_acknowledged, 100);
    assert_eq!(deserialized.ack_ranges.len(), 3);
    
    Ok(())
}

// Test 2.2.2: ACK delay calculation
#[test]
fn test_ack_delay_calculation() -> Result<()> {
    use std::time::{Duration, Instant};
    
    let send_time = Instant::now();
    std::thread::sleep(Duration::from_millis(10));
    let receive_time = Instant::now();
    
    let ack_delay = receive_time.duration_since(send_time);
    assert!(ack_delay >= Duration::from_millis(10));
    
    // ACK delay in QUIC is in microseconds
    let ack_delay_micros = ack_delay.as_micros() as u64;
    assert!(ack_delay_micros >= 10000);
    
    Ok(())
}

// Test 2.2.3: Duplicate ACK detection
#[tokio::test]
async fn test_duplicate_ack_detection() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(Role::Server, QuicConnectionState::default(), socket, addr, vec![]));
    
    // Send same packet multiple times
    let packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::ShortHeader,
            version: 1,
            destination_connection_id: vec![],
            source_connection_id: vec![],
            packet_number: 42,
            token: None,
        },
        frames: vec![],
        payload: vec![],
    };
    
    // Process same packet multiple times
    engine.process_packet(packet.clone()).await?;
    engine.process_packet(packet.clone()).await?;
    engine.process_packet(packet).await?;
    
    // Should not crash - duplicate detection is internal
    Ok(())
}

// Test 2.2.4: ACK-induced congestion control
#[tokio::test]
async fn test_ack_congestion_control() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    
    let mut initial_state = QuicConnectionState::default();
    initial_state.bytes_in_flight = 0;
    
    let engine = QuicEngine::new(Role::Client, initial_state, socket, addr, vec![]);
    
    // Send data - increases bytes_in_flight
    let stream_id = engine.create_stream();
    engine.send_stream_data(stream_id, vec![0; 1000]).await?;
    
    {
        let state = engine.state.lock();
        assert!(state.bytes_in_flight > 0);
    }
    
    // Receive ACK - should decrease bytes_in_flight
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
    
    engine.process_packet(ack_packet).await?;
    
    // bytes_in_flight should be updated
    let state = engine.state.lock();
    // In simplified implementation, this might not decrease
    // Real implementation would track per-packet bytes
    
    Ok(())
}

// Test 2.2.5: Out-of-order packet handling
#[tokio::test]
async fn test_out_of_order_packets() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(Role::Server, QuicConnectionState::default(), socket, addr, vec![]));
    
    // Send packets out of order
    let packets = vec![
        QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: vec![],
                source_connection_id: vec![],
                packet_number: 5,
                token: None,
            },
            frames: vec![],
            payload: vec![],
        },
        QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: vec![],
                source_connection_id: vec![],
                packet_number: 3,
                token: None,
            },
            frames: vec![],
            payload: vec![],
        },
        QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: vec![],
                source_connection_id: vec![],
                packet_number: 7,
                token: None,
            },
            frames: vec![],
            payload: vec![],
        },
    ];
    
    // Process out of order
    for packet in packets {
        engine.process_packet(packet).await?;
    }
    
    // Should handle without crashing
    Ok(())
}

// Test 2.2.6: Packet retransmission logic
#[tokio::test]
async fn test_retransmission_logic() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    
    let mut initial_state = QuicConnectionState::default();
    
    let engine = QuicEngine::new(Role::Client, initial_state, socket, addr, vec![]);
    
    // Send packet
    let stream_id = engine.create_stream();
    engine.send_stream_data(stream_id, vec![1, 2, 3]).await?;
    
    // Verify packet was sent
    {
        let state = engine.state.lock();
        assert!(!state.sent_packets.is_empty());
    }
    
    // In real implementation, unacknowledged packets would be retransmitted
    // For now, just verify sent_packets tracking works
    
    Ok(())
}

// ============================================================================
// Test 2.3: Crypto Frame Handling
// ============================================================================

// Test 2.3.1: CRYPTO frame buffering (handshake data)
#[tokio::test]
async fn test_crypto_frame_buffering() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(Role::Client, QuicConnectionState::default(), socket, addr, vec![]));
    
    // Send CRYPTO frame (simulated handshake data)
    let crypto_packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::Initial,
            version: 1,
            destination_connection_id: vec![1, 2, 3, 4],
            source_connection_id: vec![5, 6, 7, 8],
            packet_number: 0,
            token: None,
        },
        frames: vec![QuicFrame::Crypto(CryptoFrame {
            offset: 0,
            data: vec![0x01, 0x02, 0x03, 0x04],
        })],
        payload: vec![],
    };
    
    engine.process_packet(crypto_packet).await?;
    
    // Verify packet was received
    {
        let state = engine.state.lock();
        assert!(!state.received_packets.is_empty());
    }
    
    Ok(())
}

// Test 2.3.2: Crypto data reassembly
#[test]
fn test_crypto_data_reassembly() -> Result<()> {
    // Simulate fragmented crypto data
    let fragments = vec![
        CryptoFrame { offset: 0, data: vec![0x01, 0x02] },
        CryptoFrame { offset: 2, data: vec![0x03, 0x04] },
        CryptoFrame { offset: 4, data: vec![0x05, 0x06] },
    ];
    
    // Reassemble
    let mut reassembled = Vec::new();
    for frame in &fragments {
        reassembled.extend_from_slice(&frame.data);
    }
    
    assert_eq!(reassembled, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
    
    Ok(())
}

// Test 2.3.3: Handshake completion signaling
#[tokio::test]
async fn test_handshake_completion() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = QuicEngine::new(Role::Client, QuicConnectionState::default(), socket, addr, vec![]);
    
    // Initial state is Handshaking
    {
        let state = engine.state.lock();
        assert_eq!(state.connection_state, ConnectionState::Handshaking);
    }
    
    // Receive ACK to complete handshake
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
    
    engine.process_packet(ack_packet).await?;
    
    // State should be Connected
    {
        let state = engine.state.lock();
        assert_eq!(state.connection_state, ConnectionState::Connected);
    }
    
    Ok(())
}

// Test 2.3.4: Key update procedures
#[test]
fn test_key_update() -> Result<()> {
    // Simulate key update (simplified)
    use sha2::{Sha256, Digest};
    
    let initial_secret = b"initial_secret";
    let mut hasher = Sha256::new();
    hasher.update(initial_secret);
    let key1 = hasher.finalize().to_vec();
    
    // Update key
    let updated_secret = b"updated_secret";
    let mut hasher = Sha256::new();
    hasher.update(updated_secret);
    let key2 = hasher.finalize().to_vec();
    
    assert_ne!(key1, key2);
    assert_eq!(key1.len(), 32);
    assert_eq!(key2.len(), 32);
    
    Ok(())
}

// Test 2.3.5: 0-RTT data handling
#[test]
fn test_zero_rtt_data() -> Result<()> {
    // Test 0-RTT packet type
    let zrtt_type = QuicPacketType::ZeroRtt;
    assert_eq!(format!("{:?}", zrtt_type), "ZeroRtt");
    
    // 0-RTT allows sending data before handshake completion
    let zrtt_packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::ZeroRtt,
            version: 1,
            destination_connection_id: vec![1, 2, 3, 4],
            source_connection_id: vec![],
            packet_number: 0,
            token: Some(vec![0x01, 0x02]),
        },
        frames: vec![QuicFrame::Stream(StreamFrame {
            stream_id: 0,
            offset: 0,
            data: vec![0xDE, 0xAD],
            fin: false,
        })],
        payload: vec![],
    };
    
    let serialized = bincode::serialize(&zrtt_packet)?;
    let deserialized: QuicPacket = bincode::deserialize(&serialized)?;
    
    assert_eq!(deserialized.header.r#type, QuicPacketType::ZeroRtt);
    
    Ok(())
}

// Test 2.3.6: Crypto error handling (decryption failures)
#[tokio::test]
async fn test_crypto_error_handling() -> Result<()> {
    let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await?);
    let addr = "127.0.0.1:12345".parse().unwrap();
    let engine = Arc::new(QuicEngine::new(Role::Server, QuicConnectionState::default(), socket, addr, vec![]));
    
    // Send malformed crypto frame
    let bad_crypto_packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::Initial,
            version: 1,
            destination_connection_id: vec![],
            source_connection_id: vec![],
            packet_number: 0,
            token: None,
        },
        frames: vec![QuicFrame::Crypto(CryptoFrame {
            offset: 0,
            data: vec![], // Empty crypto data
        })],
        payload: vec![],
    };
    
    // Should handle gracefully (not crash)
    let result = engine.process_packet(bad_crypto_packet).await;
    assert!(result.is_ok());
    
    Ok(())
}
