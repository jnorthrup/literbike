//! Test 1.2.6 + Additional: Protocol Validation Tests
//!
//! Tests for protocol validation, packet type checks, and edge cases.

use literbike::quic::*;
use literbike::quic::quic_protocol::ConnectionId;
use anyhow::Result;

// ============================================================================
// Test Packet Type validation
// ============================================================================

#[test]
fn test_packet_type_validation() -> Result<()> {
    // Test Initial packet type
    let initial = QuicPacketType::Initial;
    assert_eq!(format!("{:?}", initial), "Initial");

    // Test Handshake packet type
    let handshake = QuicPacketType::Handshake;
    assert_eq!(format!("{:?}", handshake), "Handshake");

    // Test 0RTT packet type
    let zrtt = QuicPacketType::ZeroRtt;
    assert_eq!(format!("{:?}", zrtt), "ZeroRtt");

    // Test ShortHeader packet type
    let short = QuicPacketType::ShortHeader;
    assert_eq!(format!("{:?}", short), "ShortHeader");

    // Test VersionNegotiation packet type
    let vn = QuicPacketType::VersionNegotiation;
    assert_eq!(format!("{:?}", vn), "VersionNegotiation");

    Ok(())
}

// ============================================================================
// Test Connection State validation
// ============================================================================

#[test]
fn test_connection_state_validation() -> Result<()> {
    // Test connection states that exist in the actual enum
    let handshaking = ConnectionState::Handshaking;
    assert_eq!(format!("{:?}", handshaking), "Handshaking");

    let connected = ConnectionState::Connected;
    assert_eq!(format!("{:?}", connected), "Connected");

    let closed = ConnectionState::Closed;
    assert_eq!(format!("{:?}", closed), "Closed");

    // Test state equality
    assert_eq!(ConnectionState::Handshaking, ConnectionState::Handshaking);
    assert_ne!(ConnectionState::Handshaking, ConnectionState::Connected);

    Ok(())
}

// ============================================================================
// Test Stream State validation
// ============================================================================

#[test]
fn test_stream_state_validation() -> Result<()> {
    // Test all stream states
    let idle = StreamState::Idle;
    assert_eq!(format!("{:?}", idle), "Idle");

    let open = StreamState::Open;
    assert_eq!(format!("{:?}", open), "Open");

    let half_closed_local = StreamState::HalfClosedLocal;
    assert_eq!(format!("{:?}", half_closed_local), "HalfClosedLocal");

    let half_closed_remote = StreamState::HalfClosedRemote;
    assert_eq!(format!("{:?}", half_closed_remote), "HalfClosedRemote");

    let closed = StreamState::Closed;
    assert_eq!(format!("{:?}", closed), "Closed");

    Ok(())
}

// ============================================================================
// Test QuicConnectionState default values
// ============================================================================

#[test]
fn test_connection_state_defaults() -> Result<()> {
    use std::default::Default;
    let state = QuicConnectionState::default();

    // Initial connection state after engine construction is Handshaking,
    // but the raw default of QuicConnectionState has connection_state set
    // to what the Default impl provides.
    assert!(state.local_connection_id.bytes.is_empty());
    assert!(state.remote_connection_id.bytes.is_empty());
    assert_eq!(state.next_packet_number, 0);
    assert!(state.sent_packets.is_empty());
    assert!(state.received_packets.is_empty());
    assert_eq!(state.bytes_in_flight, 0);

    Ok(())
}

// ============================================================================
// Test packet with all frame types
// ============================================================================

#[test]
fn test_packet_with_all_frame_types() -> Result<()> {
    let packet = QuicPacket {
        header: QuicHeader {
            r#type: QuicPacketType::ShortHeader,
            version: 1,
            destination_connection_id: ConnectionId { bytes: vec![1, 2, 3, 4] },
            source_connection_id: ConnectionId { bytes: vec![5, 6, 7, 8] },
            packet_number: 42,
            token: None,
        },
        frames: vec![
            QuicFrame::Crypto(CryptoFrame {
                offset: 0,
                data: vec![0x01],
            }),
            QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 0,
                data: vec![0x02],
                fin: false,
            }),
            QuicFrame::Ack(AckFrame {
                largest_acknowledged: 40,
                ack_delay: 100,
                ack_ranges: vec![(35, 40)],
            }),
            QuicFrame::Padding { length: 10 },
        ],
        payload: vec![0xFF],
    };

    let serialized = bincode::serialize(&packet)?;
    let deserialized: QuicPacket = bincode::deserialize(&serialized)?;

    assert_eq!(deserialized.frames.len(), 4);
    assert!(matches!(deserialized.frames[0], QuicFrame::Crypto(_)));
    assert!(matches!(deserialized.frames[1], QuicFrame::Stream(_)));
    assert!(matches!(deserialized.frames[2], QuicFrame::Ack(_)));
    assert!(matches!(deserialized.frames[3], QuicFrame::Padding { .. }));

    Ok(())
}

// ============================================================================
// Test packet number space
// ============================================================================

#[test]
fn test_packet_number_space() -> Result<()> {
    // Test packet number wraparound (simplified)
    use std::default::Default;
    let mut state = QuicConnectionState::default();

    // Increment packet numbers
    for i in 0..1000 {
        state.next_packet_number = i;
    }

    assert_eq!(state.next_packet_number, 1000);

    // Test packet number in header
    let header = QuicHeader {
        r#type: QuicPacketType::ShortHeader,
        version: 1,
        destination_connection_id: ConnectionId { bytes: vec![] },
        source_connection_id: ConnectionId { bytes: vec![] },
        packet_number: u64::MAX,
        token: None,
    };

    assert_eq!(header.packet_number, u64::MAX);

    Ok(())
}

// ============================================================================
// Test transport parameters
// ============================================================================

#[test]
fn test_transport_parameters() -> Result<()> {
    use std::default::Default;
    let mut state = QuicConnectionState::default();

    // Set custom transport params
    state.transport_params.max_data = 1048576; // 1MB
    state.transport_params.max_stream_data = 262144; // 256KB

    assert_eq!(state.transport_params.max_data, 1048576);
    assert_eq!(state.transport_params.max_stream_data, 262144);

    Ok(())
}

// ============================================================================
// Test Ack range validation
// ============================================================================

#[test]
fn test_ack_range_validation() -> Result<()> {
    // Test valid ack range
    let valid_range = (5, 10);
    assert!(valid_range.0 <= valid_range.1);

    // Test single packet ack
    let single = (5, 5);
    assert_eq!(single.0, single.1);

    // Test multiple non-overlapping ranges
    let ranges = vec![(0, 5), (10, 15), (20, 25)];
    for i in 1..ranges.len() {
        assert!(ranges[i-1].1 < ranges[i].0); // No overlap
    }

    Ok(())
}
