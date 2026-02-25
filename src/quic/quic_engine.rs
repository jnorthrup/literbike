use super::quic_crypto::{
    CryptoFrameDisposition, HandshakePhase, InboundHeaderProtectionContext, NoopQuicCryptoProvider,
    OutboundHeaderProtectionContext, QuicCryptoProvider,
};
use super::quic_error::*;
use super::quic_protocol::{serialize_packet, ConnectionState, *};
use parking_lot::Mutex;
use rand::Rng;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Role {
    Client,
    Server,
}

pub struct QuicEngine {
    role: Role,
    state: Arc<Mutex<QuicConnectionState>>,
    stream_states: Arc<Mutex<HashMap<u64, QuicStreamState>>>,
    ack_pending: Arc<Mutex<Vec<u64>>>,
    crypto_provider: Arc<dyn QuicCryptoProvider>,
    socket: Arc<UdpSocket>,  // Add socket field
    remote_addr: SocketAddr, // Add remote address for sending
}

impl QuicEngine {
    pub fn new(
        role: Role,
        initial_state: QuicConnectionState,
        socket: Arc<UdpSocket>,
        remote_addr: SocketAddr,
        private_key: Vec<u8>,
    ) -> Self {
        self::QuicEngine::new_with_crypto_provider(
            role,
            initial_state,
            socket,
            remote_addr,
            private_key,
            Arc::new(NoopQuicCryptoProvider),
        )
    }

    pub fn new_with_crypto_provider(
        role: Role,
        initial_state: QuicConnectionState,
        socket: Arc<UdpSocket>,
        remote_addr: SocketAddr,
        _private_key: Vec<u8>,
        crypto_provider: Arc<dyn QuicCryptoProvider>,
    ) -> Self {
        // private_key is not used in this simplified engine, but kept for signature parity
        let mut state = initial_state;
        state.connection_state = super::quic_protocol::ConnectionState::Handshaking;
        QuicEngine {
            role,
            state: Arc::new(Mutex::new(state)),
            stream_states: Arc::new(Mutex::new(HashMap::new())),
            ack_pending: Arc::new(Mutex::new(Vec::new())),
            crypto_provider,
            socket,
            remote_addr,
        }
    }

    fn expected_inbound_packet_number(state_guard: &QuicConnectionState) -> u64 {
        state_guard
            .received_packets
            .last()
            .map(|p| p.header.packet_number.saturating_add(1))
            .unwrap_or(0)
    }

    fn infer_packet_number_len(truncated_packet_number: u64) -> usize {
        if truncated_packet_number <= 0xFF {
            1
        } else if truncated_packet_number <= 0xFFFF {
            2
        } else if truncated_packet_number <= 0xFF_FFFF {
            3
        } else {
            4
        }
    }

    pub fn reconstruct_packet_number(
        expected_packet_number: u64,
        truncated_packet_number: u64,
        packet_number_len: usize,
    ) -> Result<u64, QuicError> {
        if !(1..=4).contains(&packet_number_len) {
            return Err(QuicError::Protocol(ProtocolError::InvalidPacket(
                "packet number length must be in 1..=4".into(),
            )));
        }

        let pn_nbits = packet_number_len * 8;
        let packet_number_window = 1u64 << pn_nbits;
        let packet_number_half_window = packet_number_window / 2;
        let packet_number_mask = packet_number_window - 1;

        let mut candidate = (expected_packet_number & !packet_number_mask)
            | (truncated_packet_number & packet_number_mask);

        if candidate <= u64::MAX.saturating_sub(packet_number_window)
            && candidate.saturating_add(packet_number_half_window) <= expected_packet_number
        {
            candidate = candidate.saturating_add(packet_number_window);
        } else if candidate > expected_packet_number.saturating_add(packet_number_half_window)
            && candidate >= packet_number_window
        {
            candidate = candidate.saturating_sub(packet_number_window);
        }

        Ok(candidate)
    }

    fn apply_outbound_header_protection_hook(
        &self,
        packet: &mut QuicPacket,
    ) -> Result<(), QuicError> {
        let packet_number_len = Self::infer_packet_number_len(packet.header.packet_number);
        let ctx = OutboundHeaderProtectionContext {
            packet_number: packet.header.packet_number,
            packet_number_len,
        };
        self.crypto_provider
            .on_outbound_header(&mut packet.header, &ctx)
    }

    pub async fn process_packet(&self, mut packet: QuicPacket) -> Result<(), QuicError> {
        // Prepare ACK data in a separate scope to drop guards early
        let (_ack_packet_opt, serialized_ack_opt) = {
            let mut state_guard = self.state.lock();
            let mut stream_states_guard = self.stream_states.lock();
            let mut ack_pending_guard = self.ack_pending.lock();

            let truncated_packet_number = packet.header.packet_number;
            let packet_number_len = Self::infer_packet_number_len(truncated_packet_number);
            let expected_packet_number = Self::expected_inbound_packet_number(&state_guard);
            let reconstructed_packet_number = Self::reconstruct_packet_number(
                expected_packet_number,
                truncated_packet_number,
                packet_number_len,
            )?;

            let inbound_ctx = InboundHeaderProtectionContext {
                expected_packet_number,
                truncated_packet_number,
                packet_number_len,
            };
            self.crypto_provider
                .on_inbound_header(&mut packet.header, &inbound_ctx)?;
            packet.header.packet_number = reconstructed_packet_number;

            // Process each frame
            for frame in packet.frames.iter() {
                match frame {
                    QuicFrame::Stream(stream_frame) => {
                        self.process_stream_frame(
                            stream_frame,
                            &mut stream_states_guard,
                            &mut ack_pending_guard,
                            &state_guard,
                            reconstructed_packet_number,
                        )?;
                    }
                    QuicFrame::Ack(ack_frame) => {
                        self.process_ack_frame(ack_frame, &mut state_guard);
                        // Transition to Connected state after receiving an ACK (simplified handshake)
                        if state_guard.connection_state == ConnectionState::Handshaking {
                            state_guard.connection_state = ConnectionState::Connected;
                            tracing::info!("Connection state transitioned to Connected.");
                        }
                    }
                    QuicFrame::Crypto(crypto_frame) => {
                        self.process_crypto_frame(
                            crypto_frame,
                            &mut ack_pending_guard,
                            &mut state_guard,
                            reconstructed_packet_number,
                        )?;
                    }
                    _ => { /* Ignore other frame types for now */ }
                }
            }

            // Update state with received packet
            state_guard.received_packets.push(packet);
            state_guard.next_packet_number += 1;

            // Generate ACK if needed
            if !ack_pending_guard.is_empty() {
                let mut ack_packet = self.create_ack_packet(&state_guard, &ack_pending_guard)?;
                self.apply_outbound_header_protection_hook(&mut ack_packet)?;
                let serialized_ack = serialize_packet(&ack_packet)?;
                ack_pending_guard.clear();
                (Some(ack_packet), Some(serialized_ack))
            } else {
                (None, None)
            }
            // Guards are automatically dropped here
        };

        // Send ACK outside the locked scope
        if let Some(serialized_ack) = serialized_ack_opt {
            self.socket
                .send_to(&serialized_ack, self.remote_addr)
                .await
                .map_err(QuicError::Io)?;
        }

        Ok(())
    }

    pub async fn send_stream_data(&self, stream_id: u64, data: Vec<u8>) -> Result<(), QuicError> {
        // Prepare packet data in a separate scope to drop guards early
        let serialized_packet = {
            let mut state_guard = self.state.lock();
            let mut stream_states_guard = self.stream_states.lock();

            let stream = stream_states_guard
                .entry(stream_id)
                .or_insert_with(|| QuicStreamState {
                    stream_id,
                    send_buffer: Vec::new(),
                    receive_buffer: Vec::new(),
                    send_offset: 0,
                    receive_offset: 0,
                    max_data: state_guard.transport_params.max_stream_data,
                    state: StreamState::Idle,
                });

            // Create stream frame
            let frame = StreamFrame {
                stream_id,
                offset: stream.send_offset,
                data: data.clone(),
                fin: false,
            };

            // Update stream state
            stream.send_buffer.extend_from_slice(&data);
            stream.send_offset += data.len() as u64;

            // Create packet
            let mut packet = QuicPacket {
                header: QuicHeader {
                    r#type: QuicPacketType::ShortHeader,
                    version: state_guard.version,
                    destination_connection_id: state_guard.remote_connection_id.clone(),
                    source_connection_id: state_guard.local_connection_id.clone(),
                    packet_number: state_guard.next_packet_number,
                    token: None,
                },
                frames: vec![QuicFrame::Stream(frame)],
                payload: data.clone(),
            };

            self.apply_outbound_header_protection_hook(&mut packet)?;

            // Update connection state
            state_guard.sent_packets.push(packet.clone());
            state_guard.next_packet_number += 1;
            state_guard.bytes_in_flight += data.len() as u64;

            // Serialize the packet
            let result = serialize_packet(&packet)?;
            Ok::<Vec<u8>, QuicError>(result)
            // Guards are automatically dropped here
        }?;

        // Send packet outside the locked scope
        self.socket
            .send_to(&serialized_packet, self.remote_addr)
            .await
            .map_err(QuicError::Io)?;

        Ok(())
    }

    pub fn create_stream(&self) -> u64 {
        // Simplified implementation - in a real QUIC stack, this would involve
        // allocating a new stream ID based on protocol rules.
        // For now, return a simple incrementing ID or timestamp-like value.
        let mut state_guard = self.state.lock();
        let new_stream_id = state_guard.next_stream_id;
        state_guard.next_stream_id += 1;
        new_stream_id
    }

    fn process_stream_frame(
        &self,
        frame: &StreamFrame,
        stream_states_guard: &mut HashMap<u64, QuicStreamState>,
        ack_pending_guard: &mut Vec<u64>,
        state_guard: &QuicConnectionState,
        received_packet_number: u64,
    ) -> Result<(), QuicError> {
        let stream = stream_states_guard
            .entry(frame.stream_id)
            .or_insert_with(|| QuicStreamState {
                stream_id: frame.stream_id,
                send_buffer: Vec::new(),
                receive_buffer: Vec::new(),
                send_offset: 0,
                receive_offset: 0,
                max_data: state_guard.transport_params.max_stream_data,
                state: StreamState::Idle,
            });

        // Update stream receive buffer
        stream.receive_buffer.extend_from_slice(&frame.data);
        stream.receive_offset = frame.offset + frame.data.len() as u64;

        // Mark actual received packet number for ACK generation.
        ack_pending_guard.push(received_packet_number);
        Ok(())
    }

    fn process_ack_frame(&self, frame: &AckFrame, state_guard: &mut QuicConnectionState) {
        // Remove acknowledged packets from bytes in flight
        let mut acked_bytes = 0u64;
        for (start, end) in frame.ack_ranges.iter() {
            // Simplified: assume max packet size for calculation
            acked_bytes += (end - start + 1) * 1350;
        }

        state_guard.bytes_in_flight = state_guard.bytes_in_flight.saturating_sub(acked_bytes);
    }

    fn process_crypto_frame(
        &self,
        frame: &CryptoFrame,
        ack_pending_guard: &mut Vec<u64>,
        state_guard: &mut QuicConnectionState,
        received_packet_number: u64,
    ) -> Result<(), QuicError> {
        let disposition = self.crypto_provider.on_crypto_frame(frame, state_guard)?;
        if matches!(disposition, CryptoFrameDisposition::ProgressedHandshake)
            && self.crypto_provider.header_protection_ready()
            && state_guard.connection_state == ConnectionState::Handshaking
        {
            state_guard.connection_state = ConnectionState::Connected;
            tracing::info!("Connection state transitioned to Connected via CRYPTO path.");
        }
        ack_pending_guard.push(received_packet_number);
        Ok(())
    }

    fn create_ack_packet(
        &self,
        state_guard: &QuicConnectionState,
        ack_pending_guard: &[u64],
    ) -> Result<QuicPacket, QuicError> {
        let mut sorted_acks = ack_pending_guard.to_owned();
        sorted_acks.sort_unstable();

        let mut ranges = Vec::new();
        if !sorted_acks.is_empty() {
            let mut start = sorted_acks[0];
            let mut end = sorted_acks[0];

            for &v in sorted_acks.iter().skip(1) {
                if v == end + 1 {
                    end = v;
                } else {
                    ranges.push((start, end));
                    start = v;
                    end = v;
                }
            }
            ranges.push((start, end));
        }

        let ack_frame = AckFrame {
            largest_acknowledged: *sorted_acks.last().unwrap_or(&0),
            ack_delay: 0,
            ack_ranges: ranges,
        };

        Ok(QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: state_guard.version,
                destination_connection_id: state_guard.remote_connection_id.clone(),
                source_connection_id: state_guard.local_connection_id.clone(),
                packet_number: state_guard.next_packet_number,
                token: None,
            },
            frames: vec![QuicFrame::Ack(ack_frame)],
            payload: Vec::new(),
        })
    }

    pub fn get_state(&self) -> QuicConnectionState {
        self.state.lock().clone()
    }

    pub fn handshake_phase(&self) -> HandshakePhase {
        self.crypto_provider.handshake_phase()
    }

    pub fn set_connection_state(&self, new_state: super::quic_protocol::ConnectionState) {
        let mut s = self.state.lock();
        s.connection_state = new_state;
    }

    pub fn get_stream(&self, stream_id: u64) -> Option<QuicStreamState> {
        self.stream_states.lock().get(&stream_id).cloned()
    }

    pub fn get_active_streams(&self) -> Vec<u64> {
        self.stream_states.lock().keys().cloned().collect()
    }

    pub async fn close(&self) {
        let mut s = self.state.lock();
        s.connection_state = ConnectionState::Closed;
    }
}

pub struct KeyPair {
    pub private: Vec<u8>,
    pub public: Vec<u8>,
}

pub fn generate_key_pair() -> KeyPair {
    let mut rng = rand::thread_rng();
    let private_key: Vec<u8> = (0..32).map(|_| rng.gen::<u8>()).collect();
    let public_key: Vec<u8> = (0..32).map(|_| rng.gen::<u8>()).collect();
    KeyPair {
        private: private_key,
        public: public_key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quic::quic_protocol::{ConnectionId, TransportParameters};

    struct RejectInboundHeaderCrypto;

    impl QuicCryptoProvider for RejectInboundHeaderCrypto {
        fn on_inbound_header(
            &self,
            _header: &mut QuicHeader,
            _ctx: &InboundHeaderProtectionContext,
        ) -> Result<(), QuicError> {
            Err(QuicError::Protocol(ProtocolError::Crypto(
                "test inbound header hook rejection".into(),
                None,
            )))
        }
    }

    fn sample_state() -> QuicConnectionState {
        QuicConnectionState {
            local_connection_id: ConnectionId { bytes: vec![1; 8] },
            remote_connection_id: ConnectionId { bytes: vec![2; 8] },
            version: 1,
            transport_params: TransportParameters::default(),
            streams: Vec::new(),
            sent_packets: Vec::new(),
            received_packets: Vec::new(),
            next_packet_number: 0,
            next_stream_id: 0,
            congestion_window: 14720,
            bytes_in_flight: 0,
            rtt: 100,
            connection_state: ConnectionState::Handshaking,
        }
    }

    #[test]
    fn reconstruct_packet_number_uses_expected_window() {
        // Example: expected = 0x100, truncated 0x01 on 1-byte packet number => 0x101
        let pn = QuicEngine::reconstruct_packet_number(0x100, 0x01, 1).unwrap();
        assert_eq!(pn, 0x101);

        // Example near wrap: expected = 0x200, truncated 0xFF on 1-byte packet number => 0x1FF
        let pn = QuicEngine::reconstruct_packet_number(0x200, 0xFF, 1).unwrap();
        assert_eq!(pn, 0x1FF);

        // RFC-style adjustment forward into the closest window.
        let pn = QuicEngine::reconstruct_packet_number(0xABE8_BC, 0x9B32, 2).unwrap();
        assert_eq!(pn, 0xAB9B32);
    }

    #[tokio::test]
    async fn process_packet_surfaces_inbound_header_hook_error() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
        let engine = QuicEngine::new_with_crypto_provider(
            Role::Server,
            sample_state(),
            socket,
            remote_addr,
            vec![],
            Arc::new(RejectInboundHeaderCrypto),
        );

        let packet = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![1; 8] },
                source_connection_id: ConnectionId { bytes: vec![] },
                packet_number: 1,
                token: None,
            },
            frames: vec![QuicFrame::Ping],
            payload: Vec::new(),
        };

        let err = engine.process_packet(packet).await.unwrap_err();
        assert!(matches!(
            err,
            QuicError::Protocol(ProtocolError::Crypto(_, _))
        ));
    }
}
