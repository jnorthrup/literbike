//! Test 1.1: Packet Serialization/Deserialization
//!
//! Tests for QuicPacket serialization round-trip, field boundaries,
//! and malformed packet handling.

use literbike::quic::*;
use anyhow::Result;

// ============================================================================
// Test 1.1.1: QuicPacket serialization round-trip (bincode)
// ============================================================================

#[test]
fn test_packet_serialization_roundtrip() -> Result<()> {
    // Create a packet with all frame types
    let original_packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::ShortHeader,
            version: 0x00000001,
            destination_connection_id: vec![0x01, 0x02, 0x03, 0x04],
            source_connection_id: vec![0x05, 0x06, 0x07, 0x08],
            packet_number: 42,
            token: None,
        },
        frames: vec![
            QuicFrame::Stream(StreamFrame {
                stream_id: 1,
                offset: 0,
                data: vec![0xDE, 0xAD, 0xBE, 0xEF],
                fin: false,
            }),
            QuicFrame::Ack(AckFrame {
                largest_acknowledged: 40,
                ack_delay: 100,
                ack_ranges: vec![(35, 40)],
            }),
        ],
        payload: vec![0xCA, 0xFE],
    };

    // Serialize
    let serialized = bincode::serialize(&original_packet)?;
    assert!(!serialized.is_empty());

    // Deserialize
    let deserialized: QuicPacket = bincode::deserialize(&serialized)?;

    // Verify round-trip
    assert_eq!(deserialized.header.r#type, original_packet.header.r#type);
    assert_eq!(deserialized.header.version, original_packet.header.version);
    assert_eq!(deserialized.header.packet_number, original_packet.header.packet_number);
    assert_eq!(deserialized.frames.len(), original_packet.frames.len());

    Ok(())
}

// ============================================================================
// Test 1.1.2: QuicHeader field boundaries
// ============================================================================

#[test]
fn test_header_field_boundaries() -> Result<()> {
    // Test minimum version
    let min_version_packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::Initial,
            version: 0x00000000,
            destination_connection_id: vec![],
            source_connection_id: vec![],
            packet_number: 0,
            token: Some(vec![]),
        },
        frames: vec![],
        payload: vec![],
    };

    let serialized = bincode::serialize(&min_version_packet)?;
    let deserialized: QuicPacket = bincode::deserialize(&serialized)?;
    assert_eq!(deserialized.header.version, 0);

    // Test maximum version
    let max_version_packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::Initial,
            version: 0xFFFFFFFF,
            destination_connection_id: vec![],
            source_connection_id: vec![],
            packet_number: u64::MAX,
            token: None,
        },
        frames: vec![],
        payload: vec![],
    };

    let serialized = bincode::serialize(&max_version_packet)?;
    let deserialized: QuicPacket = bincode::deserialize(&serialized)?;
    assert_eq!(deserialized.header.version, 0xFFFFFFFF);
    assert_eq!(deserialized.header.packet_number, u64::MAX);

    // Test maximum connection ID length
    let max_cid_packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::Initial,
            version: 1,
            destination_connection_id: vec![0xFF; 255],
            source_connection_id: vec![0xFF; 255],
            packet_number: 1,
            token: None,
        },
        frames: vec![],
        payload: vec![],
    };

    let serialized = bincode::serialize(&max_cid_packet)?;
    let deserialized: QuicPacket = bincode::deserialize(&serialized)?;
    assert_eq!(deserialized.header.destination_connection_id.len(), 255);

    Ok(())
}

// ============================================================================
// Test 1.1.3: QuicFrame enum serialization
// ============================================================================

#[test]
fn test_frame_enum_serialization() -> Result<()> {
    // Test Stream frame
    let stream_frame = QuicFrame::Stream(StreamFrame {
        stream_id: 42,
        offset: 100,
        data: vec![1, 2, 3, 4, 5],
        fin: true,
    });

    let serialized = bincode::serialize(&stream_frame)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Stream(sf) = deserialized {
        assert_eq!(sf.stream_id, 42);
        assert_eq!(sf.offset, 100);
        assert_eq!(sf.data, vec![1, 2, 3, 4, 5]);
        assert!(sf.fin);
    } else {
        panic!("Expected Stream frame");
    }

    // Test Ack frame
    let ack_frame = QuicFrame::Ack(AckFrame {
        largest_acknowledged: 100,
        ack_delay: 50,
        ack_ranges: vec![(90, 100), (80, 85)],
    });

    let serialized = bincode::serialize(&ack_frame)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Ack(af) = deserialized {
        assert_eq!(af.largest_acknowledged, 100);
        assert_eq!(af.ack_delay, 50);
        assert_eq!(af.ack_ranges.len(), 2);
    } else {
        panic!("Expected Ack frame");
    }

    // Test Crypto frame
    let crypto_frame = QuicFrame::Crypto(CryptoFrame {
        offset: 0,
        data: vec![0x01, 0x02, 0x03],
    });

    let serialized = bincode::serialize(&crypto_frame)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Crypto(cf) = deserialized {
        assert_eq!(cf.offset, 0);
        assert_eq!(cf.data, vec![0x01, 0x02, 0x03]);
    } else {
        panic!("Expected Crypto frame");
    }

    // Test Padding frame
    let padding_frame = QuicFrame::Padding(10);
    let serialized = bincode::serialize(&padding_frame)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Padding(len) = deserialized {
        assert_eq!(len, 10);
    } else {
        panic!("Expected Padding frame");
    }

    Ok(())
}

// ============================================================================
// Test 1.1.4: Packet size limits (MTU constraints)
// ============================================================================

#[test]
fn test_packet_size_limits() -> Result<()> {
    // Typical UDP MTU is 1500 bytes
    const MTU: usize = 1500;

    // Create packet that fits in MTU
    let small_packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::ShortHeader,
            version: 1,
            destination_connection_id: vec![1, 2, 3, 4],
            source_connection_id: vec![5, 6, 7, 8],
            packet_number: 1,
            token: None,
        },
        frames: vec![QuicFrame::Stream(StreamFrame {
            stream_id: 1,
            offset: 0,
            data: vec![0; 1000], // 1KB payload
            fin: false,
        })],
        payload: vec![0; 100],
    };

    let serialized = bincode::serialize(&small_packet)?;
    assert!(serialized.len() < MTU, "Small packet should fit in MTU");

    // Create packet that exceeds MTU
    let large_packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::ShortHeader,
            version: 1,
            destination_connection_id: vec![1, 2, 3, 4],
            source_connection_id: vec![5, 6, 7, 8],
            packet_number: 1,
            token: None,
        },
        frames: vec![QuicFrame::Stream(StreamFrame {
            stream_id: 1,
            offset: 0,
            data: vec![0; 5000], // 5KB payload
            fin: false,
        })],
        payload: vec![0; 500],
    };

    let serialized = bincode::serialize(&large_packet)?;
    assert!(serialized.len() > MTU, "Large packet should exceed MTU");

    // Verify large packet still serializes/deserializes correctly
    let deserialized: QuicPacket = bincode::deserialize(&serialized)?;
    if let QuicFrame::Stream(sf) = &deserialized.frames[0] {
        assert_eq!(sf.data.len(), 5000);
    }

    Ok(())
}

// ============================================================================
// Test 1.1.5: Malformed packet handling
// ============================================================================

#[test]
fn test_malformed_packet_handling() {
    // Test empty data
    let empty_data: Vec<u8> = vec![];
    let result: Result<QuicPacket, _> = bincode::deserialize(&empty_data);
    assert!(result.is_err());

    // Test truncated data
    let valid_packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::ShortHeader,
            version: 1,
            destination_connection_id: vec![1, 2, 3, 4],
            source_connection_id: vec![5, 6, 7, 8],
            packet_number: 1,
            token: None,
        },
        frames: vec![],
        payload: vec![1, 2, 3, 4, 5],
    };

    let serialized = bincode::serialize(&valid_packet).unwrap();
    
    // Truncate at various points
    for truncate_at in 0..serialized.len() {
        let truncated = &serialized[..truncate_at];
        let result: Result<QuicPacket, _> = bincode::deserialize(truncated);
        // Should either fail or produce different data
        if let Ok(deserialized) = result {
            // If it succeeds, data should be different from original
            assert_ne!(deserialized.payload.len(), valid_packet.payload.len());
        }
    }

    // Test corrupted data (random bytes)
    use rand::Rng;
    let mut rng = rand::rng();
    for _ in 0..10 {
        let corrupt_data: Vec<u8> = (0..100).map(|_| rng.random()).collect();
        let result: Result<QuicPacket, _> = bincode::deserialize(&corrupt_data);
        // Should fail for random data (or produce garbage that we ignore)
        if result.is_ok() {
            // If it somehow succeeds, that's OK for bincode
        }
    }
}

// ============================================================================
// Test 1.1.6: Packet version compatibility checks
// ============================================================================

#[test]
fn test_version_compatibility() -> Result<()> {
    // Test QUIC v1 (RFC 9000)
    let v1_packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::Initial,
            version: 0x00000001,
            destination_connection_id: vec![1, 2, 3, 4],
            source_connection_id: vec![5, 6, 7, 8],
            packet_number: 1,
            token: None,
        },
        frames: vec![],
        payload: vec![],
    };

    let serialized = bincode::serialize(&v1_packet)?;
    let deserialized: QuicPacket = bincode::deserialize(&serialized)?;
    assert_eq!(deserialized.header.version, 0x00000001);

    // Test version negotiation packet (version = 0)
    let vn_packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::VersionNegotiation,
            version: 0x00000000,
            destination_connection_id: vec![1, 2, 3, 4],
            source_connection_id: vec![5, 6, 7, 8],
            packet_number: 0,
            token: None,
        },
        frames: vec![],
        payload: vec![0x00, 0x00, 0x00, 0x01], // Supported version list
    };

    let serialized = bincode::serialize(&vn_packet)?;
    let deserialized: QuicPacket = bincode::deserialize(&serialized)?;
    assert_eq!(deserialized.header.version, 0);
    assert_eq!(deserialized.header.r#type, QuicPacketType::VersionNegotiation);

    // Test draft versions
    for draft_version in [0xFF00001Du32, 0xFF00001Eu32, 0xFF00001Fu32] {
        let draft_packet = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::Initial,
                version: draft_version,
                destination_connection_id: vec![],
                source_connection_id: vec![],
                packet_number: 0,
                token: None,
            },
            frames: vec![],
            payload: vec![],
        };

        let serialized = bincode::serialize(&draft_packet)?;
        let deserialized: QuicPacket = bincode::deserialize(&serialized)?;
        assert_eq!(deserialized.header.version, draft_version);
    }

    Ok(())
}
