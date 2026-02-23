use serde::{Deserialize, Serialize};
use super::quic_error::ProtocolError;

// High-level protocol selection for endpoints
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuicProtocol {
    H3,         // HTTP/3 over QUIC
    HtxQuic,    // Betanet HTX over QUIC
    H3Datagram, // H3 + DATAGRAM/MASQUE
}

// QUIC packet types
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum QuicPacketType {
    Initial = 0x00,
    ZeroRtt = 0x01,
    Handshake = 0x02,
    Retry = 0x03,
    VersionNegotiation = 0x04,
    ShortHeader = 0x40,
}

// QUIC frame types
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum QuicFrameType {
    Padding = 0x00,
    Ping = 0x01,
    Ack = 0x02,
    ResetStream = 0x04,
    StopSending = 0x05,
    Crypto = 0x06,
    NewToken = 0x07,
    Stream = 0x08,
    MaxData = 0x10,
    MaxStreamData = 0x11,
    MaxStreams = 0x12,
    DataBlocked = 0x14,
    StreamDataBlocked = 0x15,
    StreamsBlocked = 0x16,
    NewConnectionId = 0x18,
    RetireConnectionId = 0x19,
    PathChallenge = 0x1A,
    PathResponse = 0x1B,
    ConnectionClose = 0x1C,
    HandshakeDone = 0x1E,
}

// Connection ID
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId {
    pub bytes: Vec<u8>,
}

impl ConnectionId {
    pub fn length(&self) -> usize {
        self.bytes.len()
    }
}

// QUIC packet header
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuicHeader {
    pub r#type: QuicPacketType,
    pub version: u64,
    pub destination_connection_id: ConnectionId,
    pub source_connection_id: ConnectionId,
    pub packet_number: u64,
    pub token: Option<Vec<u8>>,
}

// QUIC frame base
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum QuicFrame {
    Padding {
        length: u32,
    },
    Ping,
    Ack(AckFrame),
    ResetStream,
    StopSending,
    Crypto(CryptoFrame),
    NewToken,
    Stream(StreamFrame),
    MaxData,
    MaxStreamData(MaxStreamDataFrame), // Add MaxStreamData frame
    MaxStreams,
    DataBlocked,
    StreamDataBlocked(StreamDataBlockedFrame), // Add StreamDataBlocked frame
    StreamsBlocked,
    NewConnectionId,
    RetireConnectionId,
    PathChallenge,
    PathResponse,
    ConnectionClose,
    HandshakeDone,
}

// Stream frame
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamFrame {
    pub stream_id: u64, // QUIC 62-bit stream ID (constrained to 62 bits)
    pub offset: u64,
    pub data: Vec<u8>,
    pub fin: bool,
}

// ACK frame
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AckFrame {
    pub largest_acknowledged: u64,
    pub ack_delay: u64,
    pub ack_ranges: Vec<(u64, u64)>,
}

// Crypto frame
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CryptoFrame {
    pub offset: u64,
    pub data: Vec<u8>,
}

// MaxStreamData frame
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaxStreamDataFrame {
    pub stream_id: u64,
    pub maximum_stream_data: u64,
}

// StreamDataBlocked frame
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamDataBlockedFrame {
    pub stream_id: u64,
    pub stream_data_limit: u64,
}

// QUIC packet
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuicPacket {
    pub header: QuicHeader,
    pub frames: Vec<QuicFrame>,
    pub payload: Vec<u8>,
}

// QUIC transport parameters
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportParameters {
    pub max_stream_data: u64,
    pub max_data: u64,
    pub max_bidi_streams: u64,
    pub max_uni_streams: u64,
    pub idle_timeout: u64,
    pub max_packet_size: u64,
    pub ack_delay_exponent: u32,
    pub max_ack_delay: u64,
    pub active_connection_id_limit: u64,
    pub initial_max_data: u64,
    pub initial_max_stream_data_bidi_local: u64,
    pub initial_max_stream_data_bidi_remote: u64,
    pub initial_max_stream_data_uni: u64,
    pub initial_max_streams_bidi: u64,
    pub initial_max_streams_uni: u64,
}

impl Default for TransportParameters {
    fn default() -> Self {
        Self {
            max_stream_data: 1_048_576,
            max_data: 10_485_760,
            max_bidi_streams: 100,
            max_uni_streams: 100,
            idle_timeout: 30_000,
            max_packet_size: 1350,
            ack_delay_exponent: 3,
            max_ack_delay: 25,
            active_connection_id_limit: 4,
            initial_max_data: 10_485_760,
            initial_max_stream_data_bidi_local: 1_048_576,
            initial_max_stream_data_bidi_remote: 1_048_576,
            initial_max_stream_data_uni: 1_048_576,
            initial_max_streams_bidi: 100,
            initial_max_streams_uni: 100,
        }
    }
}

// Connection state enum
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Handshaking,
    Connected,
    Closed,
}

// QUIC connection state
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuicConnectionState {
    pub local_connection_id: ConnectionId,
    pub remote_connection_id: ConnectionId,
    pub version: u64, // QUIC v1
    pub transport_params: TransportParameters,
    pub streams: Vec<QuicStreamState>,
    pub sent_packets: Vec<QuicPacket>,
    pub received_packets: Vec<QuicPacket>,
    pub next_packet_number: u64,
    pub next_stream_id: u64,
    pub congestion_window: u64, // 10 * max_packet_size
    pub bytes_in_flight: u64,
    pub rtt: u64, // Initial RTT estimate in ms
    pub connection_state: ConnectionState, // Add connection state field
}

// Stream state
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuicStreamState {
    pub stream_id: u64,
    pub send_buffer: Vec<u8>,
    pub receive_buffer: Vec<u8>,
    pub send_offset: u64,
    pub receive_offset: u64,
    pub max_data: u64,
    pub state: StreamState,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamState {
    Idle,
    Open,
    HalfClosedLocal,
    HalfClosedRemote,
    Closed,
}

// QUIC stream IDs are 62-bit values represented as u64
// The upper 2 bits are reserved and must be zero
const STREAM_ID_MAX: u64 = (1u64 << 62) - 1;

pub fn validate_stream_id(stream_id: u64) -> Result<(), ProtocolError> {
    if stream_id > STREAM_ID_MAX {
        return Err(ProtocolError::InvalidStreamId(stream_id));
    }
    Ok(())
}

// --- Packet Serialization/Deserialization ---

pub fn serialize_packet(packet: &QuicPacket) -> Result<Vec<u8>, ProtocolError> {
    bincode::serialize(packet)
        .map_err(|e| ProtocolError::InvalidPacket(format!("Failed to serialize packet: {}", e)))
}

pub fn deserialize_packet(bytes: &[u8]) -> Result<QuicPacket, ProtocolError> {
    bincode::deserialize(bytes)
        .map_err(|e| ProtocolError::InvalidPacket(format!("Failed to deserialize packet: {}", e)))
}