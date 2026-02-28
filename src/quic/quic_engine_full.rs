//! QUIC Engine for Literbike - Ported from Trikeshed QuicEngine.kt
//!
//! Source: ../superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/net/quic/QuicEngine.kt

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

// ============================================================================
// QUIC Packet Types and Frames
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuicPacketType {
    Initial,
    Handshake,
    ZeroRtt,
    ShortHeader,
}

#[derive(Debug, Clone)]
pub struct QuicHeader {
    pub packet_type: QuicPacketType,
    pub version: u32,
    pub destination_connection_id: Vec<u8>,
    pub source_connection_id: Vec<u8>,
    pub packet_number: u64,
}

#[derive(Debug, Clone)]
pub enum QuicFrame {
    Stream(StreamFrame),
    Ack(AckFrame),
    Crypto(CryptoFrame),
}

#[derive(Debug, Clone)]
pub struct StreamFrame {
    pub stream_id: u64,
    pub offset: u64,
    pub data: Vec<u8>,
    pub fin: bool,
}

#[derive(Debug, Clone)]
pub struct AckFrame {
    pub largest_acknowledged: u64,
    pub ack_delay: u64,
    pub ack_ranges: Vec<(u64, u64)>, // (start, end)
}

#[derive(Debug, Clone)]
pub struct CryptoFrame {
    pub offset: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct QuicPacket {
    pub header: QuicHeader,
    pub frames: Vec<QuicFrame>,
    pub payload: Vec<u8>,
}

// ============================================================================
// QUIC Connection State
// ============================================================================

#[derive(Debug, Clone)]
pub struct TransportParams {
    pub max_stream_data: u64,
    pub max_data: u64,
    pub max_streams_bidi: u64,
}

impl Default for TransportParams {
    fn default() -> Self {
        Self {
            max_stream_data: 1048576, // 1MB
            max_data: 16777216,       // 16MB
            max_streams_bidi: 100,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuicConnectionState {
    pub version: u32,
    pub local_connection_id: Vec<u8>,
    pub remote_connection_id: Vec<u8>,
    pub next_packet_number: u64,
    pub sent_packets: Vec<QuicPacket>,
    pub received_packets: Vec<QuicPacket>,
    pub bytes_in_flight: u64,
    pub transport_params: TransportParams,
}

impl Default for QuicConnectionState {
    fn default() -> Self {
        Self {
            version: 1,
            local_connection_id: vec![0u8; 8],
            remote_connection_id: vec![0u8; 8],
            next_packet_number: 0,
            sent_packets: Vec::new(),
            received_packets: Vec::new(),
            bytes_in_flight: 0,
            transport_params: TransportParams::default(),
        }
    }
}

// ============================================================================
// QUIC Stream State
// ============================================================================

#[derive(Debug, Clone)]
pub struct QuicStreamState {
    pub stream_id: u64,
    pub send_buffer: Vec<u8>,
    pub send_offset: u64,
    pub receive_buffer: Vec<u8>,
    pub receive_offset: u64,
    pub max_data: u64,
    pub fin_sent: bool,
    pub fin_received: bool,
}

impl QuicStreamState {
    pub fn new(stream_id: u64, max_data: u64) -> Self {
        Self {
            stream_id,
            send_buffer: Vec::new(),
            send_offset: 0,
            receive_buffer: Vec::new(),
            receive_offset: 0,
            max_data,
            fin_sent: false,
            fin_received: false,
        }
    }
}

// ============================================================================
// QUIC Engine
// ============================================================================

pub enum QuicRole {
    Client,
    Server,
}

pub struct QuicEngine {
    role: QuicRole,
    state: QuicConnectionState,
    stream_states: Arc<RwLock<HashMap<u64, QuicStreamState>>>,
    packet_buffer: Arc<RwLock<Vec<QuicPacket>>>,
    ack_pending: Arc<RwLock<Vec<u64>>>,
    port: u16,
    private_key: Vec<u8>,
}

impl QuicEngine {
    pub fn new(role: QuicRole, port: u16, private_key: Vec<u8>) -> Self {
        Self {
            role,
            state: QuicConnectionState::default(),
            stream_states: Arc::new(RwLock::new(HashMap::new())),
            packet_buffer: Arc::new(RwLock::new(Vec::new())),
            ack_pending: Arc::new(RwLock::new(Vec::new())),
            port,
            private_key,
        }
    }

    /// Process incoming packet and return response packets
    pub fn process_packet(&self, packet: &QuicPacket) -> Vec<QuicPacket> {
        let mut responses = Vec::new();

        // Process each frame
        for frame in &packet.frames {
            match frame {
                QuicFrame::Stream(stream_frame) => {
                    self.process_stream_frame(stream_frame);
                }
                QuicFrame::Ack(ack_frame) => {
                    self.process_ack_frame(ack_frame);
                }
                QuicFrame::Crypto(crypto_frame) => {
                    self.process_crypto_frame(crypto_frame);
                }
            }
        }

        // Update state with received packet
        let mut new_packets = self.state.received_packets.clone();
        new_packets.push(packet.clone());
        self.state.received_packets = new_packets;
        self.state.next_packet_number += 1;

        // Generate ACK if needed
        {
            let ack_pending = self.ack_pending.read();
            if !ack_pending.is_empty() {
                drop(ack_pending);
                responses.push(self.create_ack_packet());
                let mut ack_pending = self.ack_pending.write();
                ack_pending.clear();
            }
        }

        responses
    }

    /// Send data on a stream
    pub fn send_stream_data(&self, stream_id: u64, data: &[u8]) -> QuicPacket {
        let mut stream_states = self.stream_states.write();
        
        let stream = stream_states.entry(stream_id).or_insert_with(|| {
            QuicStreamState::new(stream_id, self.state.transport_params.max_stream_data)
        });

        // Create stream frame
        let frame = StreamFrame {
            stream_id,
            offset: stream.send_offset,
            data: data.to_vec(),
            fin: false,
        };

        // Update stream state
        stream.send_buffer.extend_from_slice(data);
        stream.send_offset += data.len() as u64;

        // Create packet
        let packet = QuicPacket {
            header: QuicHeader {
                packet_type: QuicPacketType::ShortHeader,
                version: self.state.version,
                destination_connection_id: self.state.remote_connection_id.clone(),
                source_connection_id: self.state.local_connection_id.clone(),
                packet_number: self.state.next_packet_number,
            },
            frames: vec![QuicFrame::Stream(frame)],
            payload: data.to_vec(),
        };

        // Update connection state
        self.state.sent_packets.push(packet.clone());
        self.state.next_packet_number += 1;
        self.state.bytes_in_flight += data.len() as u64;

        packet
    }

    /// Create new stream and return stream ID
    pub fn create_stream(&self) -> u64 {
        let stream_states = self.stream_states.read();
        let next_id = stream_states.len() as u64 * 4; // Client-initiated bidi stream
        next_id
    }

    /// Get stream state
    pub fn get_stream(&self, stream_id: u64) -> Option<QuicStreamState> {
        let stream_states = self.stream_states.read();
        stream_states.get(&stream_id).cloned()
    }

    /// Get active stream IDs
    pub fn get_active_streams(&self) -> Vec<u64> {
        let stream_states = self.stream_states.read();
        stream_states.keys().copied().collect()
    }

    /// Get connection state
    pub fn get_state(&self) -> QuicConnectionState {
        self.state.clone()
    }

    // Private helper methods

    fn process_stream_frame(&self, frame: &StreamFrame) {
        let mut stream_states = self.stream_states.write();
        
        let stream = stream_states.entry(frame.stream_id).or_insert_with(|| {
            QuicStreamState::new(frame.stream_id, self.state.transport_params.max_stream_data)
        });

        // Update stream receive buffer
        stream.receive_buffer.extend_from_slice(&frame.data);
        stream.receive_offset = frame.offset + frame.data.len() as u64;

        // Mark packet for ACK
        let mut ack_pending = self.ack_pending.write();
        ack_pending.push(self.state.next_packet_number - 1);
    }

    fn process_ack_frame(&self, frame: &AckFrame) {
        // Remove acknowledged packets from bytes in flight
        let mut acked_bytes: u64 = 0;
        for (start, end) in &frame.ack_ranges {
            acked_bytes += (end - start + 1) * 1350; // Assume max packet size
        }

        self.state.bytes_in_flight = self.state.bytes_in_flight.saturating_sub(acked_bytes);
    }

    fn process_crypto_frame(&self, _frame: &CryptoFrame) {
        // Process crypto data (simplified - would involve TLS in real implementation)
        // For now, just ACK it
        let mut ack_pending = self.ack_pending.write();
        ack_pending.push(self.state.next_packet_number - 1);
    }

    fn create_ack_packet(&self) -> QuicPacket {
        let ack_pending = self.ack_pending.read();
        let mut sorted_acks: Vec<u64> = ack_pending.clone();
        sorted_acks.sort();
        drop(ack_pending);

        // Create ACK ranges
        let mut ranges: Vec<(u64, u64)> = Vec::new();
        if !sorted_acks.is_empty() {
            let mut start = sorted_acks[0];
            let mut end = sorted_acks[0];

            for i in 1..sorted_acks.len() {
                if sorted_acks[i] == end + 1 {
                    end = sorted_acks[i];
                } else {
                    ranges.push((start, end));
                    start = sorted_acks[i];
                    end = sorted_acks[i];
                }
            }
            ranges.push((start, end));
        }

        let largest_acknowledged = sorted_acks.last().copied().unwrap_or(0);

        let ack_frame = AckFrame {
            largest_acknowledged,
            ack_delay: 0,
            ack_ranges: ranges,
        };

        QuicPacket {
            header: QuicHeader {
                packet_type: QuicPacketType::ShortHeader,
                version: self.state.version,
                destination_connection_id: self.state.remote_connection_id.clone(),
                source_connection_id: self.state.local_connection_id.clone(),
                packet_number: self.state.next_packet_number,
            },
            frames: vec![QuicFrame::Ack(ack_frame)],
            payload: Vec::new(),
        }
    }
}

// ============================================================================
// Key Pair for QUIC
// ============================================================================

#[derive(Debug, Clone)]
pub struct KeyPair {
    pub private: Vec<u8>,
    pub public: Vec<u8>,
}

/// Generate a key pair for QUIC (simplified)
pub fn generate_key_pair() -> KeyPair {
    KeyPair {
        private: (0..32).map(|i| (i * 7) as u8).collect(),
        public: (0..32).map(|i| (i * 13) as u8).collect(),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quic_engine_create_stream() {
        let engine = QuicEngine::new(QuicRole::Client, 443, vec![0u8; 32]);
        
        let stream_id = engine.create_stream();
        assert_eq!(stream_id, 0);
        
        let stream_id2 = engine.create_stream();
        assert_eq!(stream_id2, 4);
    }

    #[test]
    fn test_quic_engine_send_stream_data() {
        let engine = QuicEngine::new(QuicRole::Client, 443, vec![0u8; 32]);
        
        let data = b"Hello, QUIC!";
        let packet = engine.send_stream_data(0, data);
        
        assert_eq!(packet.header.packet_type, QuicPacketType::ShortHeader);
        assert_eq!(packet.frames.len(), 1);
        
        if let QuicFrame::Stream(frame) = &packet.frames[0] {
            assert_eq!(frame.stream_id, 0);
            assert_eq!(&frame.data, data);
        } else {
            panic!("Expected Stream frame");
        }
    }

    #[test]
    fn test_quic_engine_process_packet() {
        let engine = QuicEngine::new(QuicRole::Server, 443, vec![0u8; 32]);
        
        // Create a stream frame packet
        let packet = QuicPacket {
            header: QuicHeader {
                packet_type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: vec![0u8; 8],
                source_connection_id: vec![0u8; 8],
                packet_number: 0,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 0,
                data: b"Hello".to_vec(),
                fin: false,
            })],
            payload: b"Hello".to_vec(),
        };
        
        let responses = engine.process_packet(&packet);
        
        // Should generate ACK
        assert!(!responses.is_empty());
    }

    #[test]
    fn test_quic_engine_stream_state() {
        let engine = QuicEngine::new(QuicRole::Client, 443, vec![0u8; 32]);
        
        // Send data
        let data = b"Test data";
        engine.send_stream_data(0, data);
        
        // Get stream state
        let stream = engine.get_stream(0);
        assert!(stream.is_some());
        
        let stream = stream.unwrap();
        assert_eq!(stream.stream_id, 0);
        assert_eq!(&stream.send_buffer, data);
        assert_eq!(stream.send_offset, data.len() as u64);
    }

    #[test]
    fn test_key_pair_generation() {
        let keypair = generate_key_pair();
        
        assert_eq!(keypair.private.len(), 32);
        assert_eq!(keypair.public.len(), 32);
        assert_ne!(keypair.private, keypair.public);
    }
}
