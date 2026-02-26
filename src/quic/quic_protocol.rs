use super::quic_error::ProtocolError;
use serde::{Deserialize, Serialize};

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
    Padding { length: u32 },
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

/// Wire-decoded packet plus metadata needed by the engine before header
/// protection / packet-number reconstruction completes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedQuicPacket {
    pub packet: QuicPacket,
    pub encoded_packet_number_len: usize,
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
    pub rtt: u64,                          // Initial RTT estimate in ms
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
const QUIC_VARINT_MAX: u64 = (1u64 << 62) - 1;

pub fn validate_stream_id(stream_id: u64) -> Result<(), ProtocolError> {
    if stream_id > STREAM_ID_MAX {
        return Err(ProtocolError::InvalidStreamId(stream_id));
    }
    Ok(())
}

// --- Packet Serialization/Deserialization ---

pub fn serialize_packet(packet: &QuicPacket) -> Result<Vec<u8>, ProtocolError> {
    let payload_bytes = encode_frames(&packet.frames)?;
    match packet.header.r#type {
        QuicPacketType::ShortHeader => serialize_short_header_packet(packet, &payload_bytes),
        QuicPacketType::Initial | QuicPacketType::ZeroRtt | QuicPacketType::Handshake => {
            serialize_long_header_packet(packet, &payload_bytes)
        }
        QuicPacketType::Retry | QuicPacketType::VersionNegotiation => {
            Err(ProtocolError::InvalidPacket(
                "Retry/VersionNegotiation serialization not supported in foundational codec".into(),
            ))
        }
    }
}

pub fn deserialize_packet(bytes: &[u8]) -> Result<QuicPacket, ProtocolError> {
    deserialize_packet_with_dcid_len(bytes, None)
}

pub fn deserialize_packet_with_dcid_len(
    bytes: &[u8],
    short_header_dcid_len: Option<usize>,
) -> Result<QuicPacket, ProtocolError> {
    Ok(deserialize_decoded_packet_with_dcid_len(bytes, short_header_dcid_len)?.packet)
}

pub fn deserialize_decoded_packet(bytes: &[u8]) -> Result<DecodedQuicPacket, ProtocolError> {
    deserialize_decoded_packet_with_dcid_len(bytes, None)
}

pub fn deserialize_decoded_packet_with_dcid_len(
    bytes: &[u8],
    short_header_dcid_len: Option<usize>,
) -> Result<DecodedQuicPacket, ProtocolError> {
    if bytes.is_empty() {
        return Err(ProtocolError::InvalidPacket("Empty packet".into()));
    }
    let first = bytes[0];
    if (first & 0x80) != 0 {
        deserialize_long_header_packet(bytes)
    } else {
        deserialize_short_header_packet(bytes, short_header_dcid_len)
    }
}

fn serialize_short_header_packet(
    packet: &QuicPacket,
    payload_bytes: &[u8],
) -> Result<Vec<u8>, ProtocolError> {
    let (pn_len_code, pn_bytes) = encode_packet_number(packet.header.packet_number);
    let mut out = Vec::with_capacity(
        1 + packet.header.destination_connection_id.bytes.len()
            + pn_bytes.len()
            + payload_bytes.len(),
    );

    // Short header: fixed bit set, packet number length in low 2 bits.
    out.push(0x40 | pn_len_code);
    out.extend_from_slice(&packet.header.destination_connection_id.bytes);
    out.extend_from_slice(&pn_bytes);
    out.extend_from_slice(payload_bytes);
    Ok(out)
}

fn serialize_long_header_packet(
    packet: &QuicPacket,
    payload_bytes: &[u8],
) -> Result<Vec<u8>, ProtocolError> {
    let type_bits = match packet.header.r#type {
        QuicPacketType::Initial => 0u8,
        QuicPacketType::ZeroRtt => 1u8,
        QuicPacketType::Handshake => 2u8,
        _ => {
            return Err(ProtocolError::InvalidPacket(
                "Unsupported long-header packet type".into(),
            ))
        }
    };

    let (pn_len_code, pn_bytes) = encode_packet_number(packet.header.packet_number);
    let mut out = Vec::new();
    out.push(0xC0 | (type_bits << 4) | pn_len_code);
    write_u32_be(&mut out, packet.header.version as u32);

    let dcid = &packet.header.destination_connection_id.bytes;
    let scid = &packet.header.source_connection_id.bytes;
    if dcid.len() > u8::MAX as usize || scid.len() > u8::MAX as usize {
        return Err(ProtocolError::InvalidPacket(
            "Connection ID too long for foundational codec".into(),
        ));
    }
    out.push(dcid.len() as u8);
    out.extend_from_slice(dcid);
    out.push(scid.len() as u8);
    out.extend_from_slice(scid);

    if matches!(packet.header.r#type, QuicPacketType::Initial) {
        let token = packet.header.token.as_deref().unwrap_or(&[]);
        write_varint(token.len() as u64, &mut out)?;
        out.extend_from_slice(token);
    }

    let payload_len = pn_bytes.len() + payload_bytes.len();
    write_varint(payload_len as u64, &mut out)?;
    out.extend_from_slice(&pn_bytes);
    out.extend_from_slice(payload_bytes);
    Ok(out)
}

fn deserialize_long_header_packet(bytes: &[u8]) -> Result<DecodedQuicPacket, ProtocolError> {
    let mut pos = 0usize;
    let first = read_u8(bytes, &mut pos)?;
    let pn_len = ((first & 0x03) + 1) as usize;
    let packet_type = match (first >> 4) & 0x03 {
        0 => QuicPacketType::Initial,
        1 => QuicPacketType::ZeroRtt,
        2 => QuicPacketType::Handshake,
        3 => QuicPacketType::Retry,
        _ => unreachable!(),
    };
    if matches!(packet_type, QuicPacketType::Retry) {
        return Err(ProtocolError::InvalidPacket(
            "Retry parsing not supported in foundational codec".into(),
        ));
    }

    let version = read_u32_be(bytes, &mut pos)? as u64;
    if version == 0 {
        return Err(ProtocolError::InvalidPacket(
            "Version negotiation packets not supported in foundational codec".into(),
        ));
    }

    let dcid_len = read_u8(bytes, &mut pos)? as usize;
    let dcid = read_bytes(bytes, &mut pos, dcid_len)?.to_vec();
    let scid_len = read_u8(bytes, &mut pos)? as usize;
    let scid = read_bytes(bytes, &mut pos, scid_len)?.to_vec();

    let token = if matches!(packet_type, QuicPacketType::Initial) {
        let token_len = read_varint(bytes, &mut pos)? as usize;
        Some(read_bytes(bytes, &mut pos, token_len)?.to_vec())
    } else {
        None
    };

    let length = read_varint(bytes, &mut pos)? as usize;
    if length < pn_len {
        return Err(ProtocolError::InvalidPacket(
            "Long-header payload length smaller than packet number length".into(),
        ));
    }
    if bytes.len().saturating_sub(pos) < length {
        return Err(ProtocolError::InvalidPacket(
            "Long-header packet truncated".into(),
        ));
    }

    let packet_number = read_packet_number(bytes, &mut pos, pn_len)?;
    let frame_payload_len = length - pn_len;
    let frame_payload = read_bytes(bytes, &mut pos, frame_payload_len)?.to_vec();
    let frames = decode_frames(&frame_payload)?;

    if pos != bytes.len() {
        return Err(ProtocolError::InvalidPacket(
            "Trailing bytes after single QUIC packet (coalesced packets not yet supported)".into(),
        ));
    }

    Ok(DecodedQuicPacket {
        packet: QuicPacket {
            header: QuicHeader {
                r#type: packet_type,
                version,
                destination_connection_id: ConnectionId { bytes: dcid },
                source_connection_id: ConnectionId { bytes: scid },
                packet_number,
                token,
            },
            frames,
            payload: frame_payload,
        },
        encoded_packet_number_len: pn_len,
    })
}

fn deserialize_short_header_packet(
    bytes: &[u8],
    short_header_dcid_len: Option<usize>,
) -> Result<DecodedQuicPacket, ProtocolError> {
    let first = bytes[0];
    let pn_len = ((first & 0x03) + 1) as usize;

    if let Some(dcid_len) = short_header_dcid_len {
        if dcid_len + 1 + pn_len > bytes.len() {
            return Err(ProtocolError::InvalidPacket(
                "Short-header DCID length hint exceeds packet size".into(),
            ));
        }
        return try_deserialize_short_with_dcid(bytes, dcid_len);
    }

    // Short headers do not carry DCID length on wire; try common lengths.
    let mut candidates = vec![8usize, 16, 20, 4, 12, 0];
    candidates.retain(|len| *len + 1 + pn_len <= bytes.len());
    candidates.sort_unstable();
    candidates.dedup();
    candidates.reverse(); // Prefer common non-zero lengths first after manual list shuffle.

    let mut last_err: Option<ProtocolError> = None;
    for dcid_len in candidates {
        match try_deserialize_short_with_dcid(bytes, dcid_len) {
            Ok(packet) => return Ok(packet),
            Err(err) => last_err = Some(err),
        }
    }

    Err(last_err.unwrap_or_else(|| {
        ProtocolError::InvalidPacket("Unable to parse short-header packet".into())
    }))
}

fn try_deserialize_short_with_dcid(
    bytes: &[u8],
    dcid_len: usize,
) -> Result<DecodedQuicPacket, ProtocolError> {
    let mut pos = 0usize;
    let first = read_u8(bytes, &mut pos)?;
    let pn_len = ((first & 0x03) + 1) as usize;

    let dcid = read_bytes(bytes, &mut pos, dcid_len)?.to_vec();
    let packet_number = read_packet_number(bytes, &mut pos, pn_len)?;
    let frame_payload = bytes[pos..].to_vec();
    let frames = decode_frames(&frame_payload)?;

    Ok(DecodedQuicPacket {
        packet: QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 0,
                destination_connection_id: ConnectionId { bytes: dcid },
                source_connection_id: ConnectionId { bytes: Vec::new() },
                packet_number,
                token: None,
            },
            frames,
            payload: frame_payload,
        },
        encoded_packet_number_len: pn_len,
    })
}

fn encode_frames(frames: &[QuicFrame]) -> Result<Vec<u8>, ProtocolError> {
    let mut out = Vec::new();
    for frame in frames {
        encode_frame(frame, &mut out)?;
    }
    Ok(out)
}

fn encode_frame(frame: &QuicFrame, out: &mut Vec<u8>) -> Result<(), ProtocolError> {
    match frame {
        QuicFrame::Padding { length } => {
            let count = (*length).max(1) as usize;
            out.extend(std::iter::repeat_n(0x00u8, count));
            Ok(())
        }
        QuicFrame::Ping => {
            out.push(QuicFrameType::Ping as u8);
            Ok(())
        }
        QuicFrame::Crypto(frame) => {
            out.push(QuicFrameType::Crypto as u8);
            write_varint(frame.offset, out)?;
            write_varint(frame.data.len() as u64, out)?;
            out.extend_from_slice(&frame.data);
            Ok(())
        }
        QuicFrame::Stream(frame) => {
            validate_stream_id(frame.stream_id)?;
            let mut ty = QuicFrameType::Stream as u8;
            if frame.offset > 0 {
                ty |= 0x04; // OFF bit
            }
            ty |= 0x02; // LEN bit
            if frame.fin {
                ty |= 0x01; // FIN bit
            }
            out.push(ty);
            write_varint(frame.stream_id, out)?;
            if frame.offset > 0 {
                write_varint(frame.offset, out)?;
            }
            write_varint(frame.data.len() as u64, out)?;
            out.extend_from_slice(&frame.data);
            Ok(())
        }
        QuicFrame::Ack(frame) => encode_ack_frame(frame, out),
        _ => Err(ProtocolError::InvalidPacket(
            "Unsupported frame type in foundational wire codec".into(),
        )),
    }
}

fn encode_ack_frame(frame: &AckFrame, out: &mut Vec<u8>) -> Result<(), ProtocolError> {
    out.push(QuicFrameType::Ack as u8);

    let mut ranges = if frame.ack_ranges.is_empty() {
        vec![(frame.largest_acknowledged, frame.largest_acknowledged)]
    } else {
        frame.ack_ranges.clone()
    };
    for (start, end) in &ranges {
        if start > end {
            return Err(ProtocolError::InvalidPacket("ACK range start > end".into()));
        }
    }
    ranges.sort_unstable_by_key(|(start, _)| *start);
    let mut merged: Vec<(u64, u64)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        if let Some((_, last_end)) = merged.last_mut() {
            if start <= *last_end + 1 {
                *last_end = (*last_end).max(end);
                continue;
            }
        }
        merged.push((start, end));
    }

    let largest_ack = merged
        .last()
        .map(|(_, end)| *end)
        .unwrap_or(frame.largest_acknowledged);
    let mut desc = merged;
    desc.reverse();

    write_varint(largest_ack, out)?;
    write_varint(frame.ack_delay, out)?;
    write_varint(desc.len().saturating_sub(1) as u64, out)?;

    let (first_start, first_end) = desc[0];
    write_varint(first_end - first_start, out)?;

    let mut prev_start = first_start;
    for (start, end) in desc.into_iter().skip(1) {
        let gap = prev_start.checked_sub(end + 2).ok_or_else(|| {
            ProtocolError::InvalidPacket("ACK ranges overlap or are out of order".into())
        })?;
        let range_len = end - start;
        write_varint(gap, out)?;
        write_varint(range_len, out)?;
        prev_start = start;
    }
    Ok(())
}

fn decode_frames(bytes: &[u8]) -> Result<Vec<QuicFrame>, ProtocolError> {
    let mut pos = 0usize;
    let mut frames = Vec::new();

    while pos < bytes.len() {
        let ty = bytes[pos];
        if ty == QuicFrameType::Padding as u8 {
            let start = pos;
            while pos < bytes.len() && bytes[pos] == 0x00 {
                pos += 1;
            }
            frames.push(QuicFrame::Padding {
                length: (pos - start) as u32,
            });
            continue;
        }

        pos += 1;
        match ty {
            x if x == QuicFrameType::Ping as u8 => frames.push(QuicFrame::Ping),
            x if x == QuicFrameType::Ack as u8 => {
                let largest_acknowledged = read_varint(bytes, &mut pos)?;
                let ack_delay = read_varint(bytes, &mut pos)?;
                let ack_range_count = read_varint(bytes, &mut pos)? as usize;
                let first_ack_range = read_varint(bytes, &mut pos)?;
                let mut current_end = largest_acknowledged;
                let current_start = current_end.checked_sub(first_ack_range).ok_or_else(|| {
                    ProtocolError::InvalidPacket("Invalid first ACK range".into())
                })?;
                let mut ranges_desc = vec![(current_start, current_end)];

                let mut prev_start = current_start;
                for _ in 0..ack_range_count {
                    let gap = read_varint(bytes, &mut pos)?;
                    let range_len = read_varint(bytes, &mut pos)?;
                    current_end = prev_start
                        .checked_sub(gap + 2)
                        .ok_or_else(|| ProtocolError::InvalidPacket("Invalid ACK gap".into()))?;
                    let start = current_end.checked_sub(range_len).ok_or_else(|| {
                        ProtocolError::InvalidPacket("Invalid ACK range length".into())
                    })?;
                    ranges_desc.push((start, current_end));
                    prev_start = start;
                }
                ranges_desc.reverse();
                frames.push(QuicFrame::Ack(AckFrame {
                    largest_acknowledged,
                    ack_delay,
                    ack_ranges: ranges_desc,
                }));
            }
            x if x == QuicFrameType::Crypto as u8 => {
                let offset = read_varint(bytes, &mut pos)?;
                let len = read_varint(bytes, &mut pos)? as usize;
                let data = read_bytes(bytes, &mut pos, len)?.to_vec();
                frames.push(QuicFrame::Crypto(CryptoFrame { offset, data }));
            }
            x if (x & 0b1111_1000) == (QuicFrameType::Stream as u8) => {
                let fin = (x & 0x01) != 0;
                let has_len = (x & 0x02) != 0;
                let has_offset = (x & 0x04) != 0;
                let stream_id = read_varint(bytes, &mut pos)?;
                validate_stream_id(stream_id)?;
                let offset = if has_offset {
                    read_varint(bytes, &mut pos)?
                } else {
                    0
                };
                let data_len = if has_len {
                    read_varint(bytes, &mut pos)? as usize
                } else {
                    bytes.len() - pos
                };
                let data = read_bytes(bytes, &mut pos, data_len)?.to_vec();
                frames.push(QuicFrame::Stream(StreamFrame {
                    stream_id,
                    offset,
                    data,
                    fin,
                }));
            }
            _ => {
                return Err(ProtocolError::InvalidPacket(format!(
                    "Unsupported QUIC frame type 0x{ty:02x}"
                )))
            }
        }
    }

    Ok(frames)
}

fn encode_packet_number(packet_number: u64) -> (u8, Vec<u8>) {
    if packet_number <= 0xFF {
        (0, vec![packet_number as u8])
    } else if packet_number <= 0xFFFF {
        (1, (packet_number as u16).to_be_bytes().to_vec())
    } else if packet_number <= 0xFF_FFFF {
        (
            2,
            vec![
                ((packet_number >> 16) & 0xFF) as u8,
                ((packet_number >> 8) & 0xFF) as u8,
                (packet_number & 0xFF) as u8,
            ],
        )
    } else {
        let pn = (packet_number as u32).to_be_bytes();
        (3, pn.to_vec())
    }
}

fn read_packet_number(bytes: &[u8], pos: &mut usize, len: usize) -> Result<u64, ProtocolError> {
    let raw = read_bytes(bytes, pos, len)?;
    let mut value = 0u64;
    for b in raw {
        value = (value << 8) | (*b as u64);
    }
    Ok(value)
}

fn write_varint(value: u64, out: &mut Vec<u8>) -> Result<(), ProtocolError> {
    match value {
        0..=63 => out.push(value as u8),
        64..=16_383 => {
            let v = (0b01u16 << 14) | (value as u16);
            out.extend_from_slice(&v.to_be_bytes());
        }
        16_384..=1_073_741_823 => {
            let v = (0b10u32 << 30) | (value as u32);
            out.extend_from_slice(&v.to_be_bytes());
        }
        1_073_741_824..=QUIC_VARINT_MAX => {
            let v = (0b11u64 << 62) | value;
            out.extend_from_slice(&v.to_be_bytes());
        }
        _ => {
            return Err(ProtocolError::InvalidPacket(
                "QUIC varint exceeds 62-bit range".into(),
            ))
        }
    }
    Ok(())
}

fn read_varint(bytes: &[u8], pos: &mut usize) -> Result<u64, ProtocolError> {
    let first = *bytes.get(*pos).ok_or_else(|| {
        ProtocolError::InvalidPacket("Unexpected EOF while reading varint".into())
    })?;
    let prefix = first >> 6;
    let len = match prefix {
        0 => 1,
        1 => 2,
        2 => 4,
        _ => 8,
    };
    let raw = read_bytes(bytes, pos, len)?;
    let mut value = 0u64;
    for b in raw {
        value = (value << 8) | (*b as u64);
    }
    let mask = match len {
        1 => 0x3F,
        2 => 0x3FFF,
        4 => 0x3FFF_FFFF,
        8 => 0x3FFF_FFFF_FFFF_FFFF,
        _ => unreachable!(),
    };
    Ok(value & mask)
}

fn write_u32_be(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn read_u32_be(bytes: &[u8], pos: &mut usize) -> Result<u32, ProtocolError> {
    let raw = read_bytes(bytes, pos, 4)?;
    Ok(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn read_u8(bytes: &[u8], pos: &mut usize) -> Result<u8, ProtocolError> {
    let b = *bytes
        .get(*pos)
        .ok_or_else(|| ProtocolError::InvalidPacket("Unexpected EOF while reading byte".into()))?;
    *pos += 1;
    Ok(b)
}

fn read_bytes<'a>(bytes: &'a [u8], pos: &mut usize, len: usize) -> Result<&'a [u8], ProtocolError> {
    if bytes.len().saturating_sub(*pos) < len {
        return Err(ProtocolError::InvalidPacket(
            "Unexpected EOF while reading packet bytes".into(),
        ));
    }
    let start = *pos;
    *pos += len;
    Ok(&bytes[start..start + len])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_cid(n: u8) -> ConnectionId {
        ConnectionId { bytes: vec![n; 8] }
    }

    #[test]
    fn quic_varint_roundtrip() {
        let vals = [
            0u64,
            63,
            64,
            1523,
            16_383,
            16_384,
            1_000_000,
            1_073_741_823,
            1_073_741_824,
            (1u64 << 62) - 1,
        ];
        for v in vals {
            let mut out = Vec::new();
            write_varint(v, &mut out).unwrap();
            let mut pos = 0usize;
            let decoded = read_varint(&out, &mut pos).unwrap();
            assert_eq!(decoded, v);
            assert_eq!(pos, out.len());
        }
    }

    #[test]
    fn short_header_stream_roundtrip() {
        let pkt = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: sample_cid(0xAA),
                source_connection_id: sample_cid(0xBB),
                packet_number: 0x1234,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 4,
                offset: 0,
                data: b"hello quic".to_vec(),
                fin: true,
            })],
            payload: b"hello quic".to_vec(),
        };

        let encoded = serialize_packet(&pkt).unwrap();
        let decoded = deserialize_packet(&encoded).unwrap();
        assert_eq!(decoded.header.r#type, QuicPacketType::ShortHeader);
        assert_eq!(decoded.header.packet_number, pkt.header.packet_number);
        assert_eq!(
            decoded.header.destination_connection_id.bytes,
            pkt.header.destination_connection_id.bytes
        );

        match &decoded.frames[0] {
            QuicFrame::Stream(frame) => {
                assert_eq!(frame.stream_id, 4);
                assert_eq!(frame.data, b"hello quic");
                assert!(frame.fin);
            }
            other => panic!("unexpected frame: {other:?}"),
        }
    }

    #[test]
    fn short_header_roundtrip_with_explicit_dcid_hint() {
        let pkt = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId {
                    bytes: vec![0xAB, 0xCD, 0xEF],
                },
                source_connection_id: sample_cid(0xBB),
                packet_number: 5,
                token: None,
            },
            frames: vec![QuicFrame::Ping],
            payload: Vec::new(),
        };

        let encoded = serialize_packet(&pkt).unwrap();
        let decoded = deserialize_packet_with_dcid_len(&encoded, Some(3)).unwrap();
        assert_eq!(
            decoded.header.destination_connection_id.bytes,
            vec![0xAB, 0xCD, 0xEF]
        );
        assert!(matches!(decoded.frames.as_slice(), [QuicFrame::Ping]));
    }

    #[test]
    fn long_header_initial_ack_roundtrip() {
        let pkt = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::Initial,
                version: 1,
                destination_connection_id: sample_cid(0x01),
                source_connection_id: sample_cid(0x02),
                packet_number: 7,
                token: Some(vec![1, 2, 3]),
            },
            frames: vec![QuicFrame::Ack(AckFrame {
                largest_acknowledged: 10,
                ack_delay: 0,
                ack_ranges: vec![(1, 3), (8, 10)],
            })],
            payload: Vec::new(),
        };

        let encoded = serialize_packet(&pkt).unwrap();
        let decoded = deserialize_packet(&encoded).unwrap();
        assert_eq!(decoded.header.r#type, QuicPacketType::Initial);
        assert_eq!(decoded.header.version, 1);
        assert_eq!(decoded.header.token, Some(vec![1, 2, 3]));
        match &decoded.frames[0] {
            QuicFrame::Ack(ack) => {
                assert_eq!(ack.largest_acknowledged, 10);
                assert_eq!(ack.ack_ranges, vec![(1, 3), (8, 10)]);
            }
            other => panic!("unexpected frame: {other:?}"),
        }
    }

    #[test]
    fn rejects_unsupported_frame_type() {
        // Short header + 8-byte DCID + 1-byte PN + unsupported frame type 0x1c (CONNECTION_CLOSE)
        let mut raw = vec![0x40, 1, 1, 1, 1, 1, 1, 1, 1, 0x05, 0x1c];
        let err = deserialize_packet(&raw).unwrap_err();
        match err {
            ProtocolError::InvalidPacket(msg) => {
                assert!(msg.contains("Unsupported QUIC frame type"))
            }
            _ => panic!("unexpected error variant"),
        }
        raw.clear();
    }
}
