use super::quic_crypto::{
    CryptoFrameDisposition, EncryptionLevel, HandshakePhase, InboundHeaderProtectionContext, NoopQuicCryptoProvider,
    OutboundHeaderProtectionContext, QuicCryptoProvider,
};
use super::quic_error::*;
use super::quic_protocol::{serialize_packet, ConnectionState, *};
use parking_lot::Mutex;
use rand::Rng;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::net::UdpSocket;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Role {
    Client,
    Server,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuicEngineDiagnosticsSnapshot {
    pub total_stream_overlap_conflict_count: u64,
    pub per_stream_overlap_conflict_counts: HashMap<u64, u64>,
    pub per_stream_pending_fragment_counts: HashMap<u64, usize>,
    pub total_pending_fragment_bytes: u64,
    pub per_stream_pending_fragment_bytes: HashMap<u64, u64>,
    pub per_stream_contiguous_receive_offsets: HashMap<u64, u64>,
    pub per_stream_highest_seen_receive_offsets: HashMap<u64, u64>,
}

pub struct QuicEngine {
    role: Role,
    state: Arc<Mutex<QuicConnectionState>>,
    stream_states: Arc<Mutex<HashMap<u64, QuicStreamState>>>,
    stream_contiguous_receive_offsets: Arc<Mutex<HashMap<u64, u64>>>,
    stream_pending_fragments: Arc<Mutex<HashMap<u64, Vec<(u64, Vec<u8>)>>>>,
    stream_overlap_conflict_counts: Arc<Mutex<HashMap<u64, u64>>>,
    total_stream_overlap_conflict_count: Arc<Mutex<u64>>,
    ack_pending: Arc<Mutex<Vec<u64>>>,
    crypto_provider: Arc<dyn QuicCryptoProvider>,
    socket: Arc<UdpSocket>,  // Add socket field
    remote_addr: SocketAddr, // Add remote address for sending
    last_activity: Arc<Mutex<Instant>>,
    idle_timeout: Duration,
    ctx: crate::concurrency::ccek::CoroutineContext,
    initial_pn_counter: Arc<Mutex<u64>>,
    handshake_pn_counter: Arc<Mutex<u64>>,
    onertt_pn_counter: Arc<Mutex<u64>>,
    initial_crypto_send_offset: Arc<Mutex<u64>>,
    handshake_crypto_send_offset: Arc<Mutex<u64>>,
    handshake_done_sent: Arc<Mutex<bool>>,
}

impl QuicEngine {
    pub fn new(
        role: Role,
        initial_state: QuicConnectionState,
        socket: Arc<UdpSocket>,
        remote_addr: SocketAddr,
        private_key: Vec<u8>,
        ctx: crate::concurrency::ccek::CoroutineContext,
    ) -> Self {
        self::QuicEngine::new_with_crypto_provider(
            role,
            initial_state,
            socket,
            remote_addr,
            private_key,
            Arc::new(NoopQuicCryptoProvider),
            ctx,
        )
    }

    pub fn new_with_crypto_provider(
        role: Role,
        initial_state: QuicConnectionState,
        socket: Arc<UdpSocket>,
        remote_addr: SocketAddr,
        private_key: Vec<u8>,
        crypto_provider: Arc<dyn QuicCryptoProvider>,
        ctx: crate::concurrency::ccek::CoroutineContext,
    ) -> Self {
        // private_key is used for key derivation in the crypto provider
        let mut state = initial_state;
        state.connection_state = super::quic_protocol::ConnectionState::Handshaking;
        // Try to get TlsCcekService from context to upgrade crypto_provider automatically
        let mut final_provider = crypto_provider;
        #[cfg(feature = "tls-quic")]
        if let Some(tls_svc) = ctx.get_typed::<crate::quic::tls_ccek::TlsCcekService>("TlsCcekService") {
            {
                println!("🔐 TLS service found in context, attempting to upgrade...");
                // private_key is actually the client's original DCID, use it as orig_dcid
                // For SCID, use the server's local connection ID
                let scid = &state.local_connection_id.bytes;
                let transport_params = Self::encode_server_transport_params(&private_key, scid);
                if let Ok(server_conn) = rustls::quic::ServerConnection::new(tls_svc.config.clone(), rustls::quic::Version::V1, transport_params) {
                    println!("🔐 Created rustls ServerConnection");
                    if let Ok(rustls_provider) = crate::quic::tls_crypto::provider::RustlsCryptoProvider::new_server(
                        rustls::quic::Connection::from(server_conn),
                        &private_key
                    ) {
                        println!("🔐 Created RustlsCryptoProvider - CRYPTO ENABLED!");
                        final_provider = Arc::new(rustls_provider);
                    } else {
                        println!("❌ Failed to create RustlsCryptoProvider");
                    }
                } else {
                    println!("❌ Failed to create rustls ServerConnection");
                }
            }
        }

        QuicEngine {
            role,
            state: Arc::new(Mutex::new(state)),
            stream_states: Arc::new(Mutex::new(HashMap::new())),
            stream_contiguous_receive_offsets: Arc::new(Mutex::new(HashMap::new())),
            stream_pending_fragments: Arc::new(Mutex::new(HashMap::new())),
            stream_overlap_conflict_counts: Arc::new(Mutex::new(HashMap::new())),
            total_stream_overlap_conflict_count: Arc::new(Mutex::new(0)),
            ack_pending: Arc::new(Mutex::new(Vec::new())),
            crypto_provider: final_provider,
            socket,
            remote_addr,
            last_activity: Arc::new(Mutex::new(Instant::now())),
            idle_timeout: Duration::from_secs(30),
            ctx,
            initial_pn_counter: Arc::new(Mutex::new(0)),
            handshake_pn_counter: Arc::new(Mutex::new(0)),
            onertt_pn_counter: Arc::new(Mutex::new(0)),
            initial_crypto_send_offset: Arc::new(Mutex::new(0)),
            handshake_crypto_send_offset: Arc::new(Mutex::new(0)),
            handshake_done_sent: Arc::new(Mutex::new(false)),
        }
    }

    /// Encode minimal QUIC transport parameters for a server (RFC 9000 §18).
    /// `orig_dcid` = client's original DCID from their first Initial packet.
    /// `scid` = our server source connection ID.
    #[cfg(feature = "tls-quic")]
    fn encode_server_transport_params(orig_dcid: &[u8], scid: &[u8]) -> Vec<u8> {
        fn write_varint(buf: &mut Vec<u8>, v: u64) {
            if v < 64 {
                buf.push(v as u8);
            } else if v < 16384 {
                buf.push(0x40 | ((v >> 8) as u8));
                buf.push((v & 0xff) as u8);
            } else if v < 1_073_741_824 {
                buf.push(0x80 | ((v >> 24) as u8));
                buf.push(((v >> 16) & 0xff) as u8);
                buf.push(((v >> 8) & 0xff) as u8);
                buf.push((v & 0xff) as u8);
            }
        }
        fn write_param(buf: &mut Vec<u8>, id: u64, data: &[u8]) {
            write_varint(buf, id);
            write_varint(buf, data.len() as u64);
            buf.extend_from_slice(data);
        }
        fn write_int_param(buf: &mut Vec<u8>, id: u64, v: u64) {
            let mut tmp = Vec::new();
            write_varint(&mut tmp, v);
            write_param(buf, id, &tmp);
        }

        let mut buf = Vec::new();
        // original_destination_connection_id (0x0000)
        write_param(&mut buf, 0x0000, orig_dcid);
        // initial_source_connection_id (0x000f)
        write_param(&mut buf, 0x000f, scid);
        // max_idle_timeout (0x0001) = 30s in ms
        write_int_param(&mut buf, 0x0001, 30_000);
        // initial_max_data (0x0004) = 1MB
        write_int_param(&mut buf, 0x0004, 1_048_576);
        // initial_max_stream_data_bidi_local (0x0005) = 256KB
        write_int_param(&mut buf, 0x0005, 262_144);
        // initial_max_stream_data_bidi_remote (0x0006) = 256KB
        write_int_param(&mut buf, 0x0006, 262_144);
        // initial_max_stream_data_uni (0x0007) = 256KB
        write_int_param(&mut buf, 0x0007, 262_144);
        // initial_max_streams_bidi (0x0008) = 100
        write_int_param(&mut buf, 0x0008, 100);
        // initial_max_streams_uni (0x0009) = 100
        write_int_param(&mut buf, 0x0009, 100);
        buf
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

    pub async fn process_packet(&self, packet: QuicPacket) -> Result<(), QuicError> {
        self.process_packet_internal(packet, None).await
    }

    pub async fn process_decoded_packet(&self, decoded: DecodedQuicPacket) -> Result<(), QuicError> {
        self.process_packet_internal(decoded.packet, Some(decoded.encoded_packet_number_len))
            .await
    }

    async fn process_packet_internal(
        &self,
        mut packet: QuicPacket,
        encoded_packet_number_len: Option<usize>,
    ) -> Result<(), QuicError> {
        // Prepare ACK data in a separate scope to drop guards early
        let (_ack_packet_opt, ack_level_opt, serialized_ack_opt): (
            Option<QuicPacket>,
            Option<EncryptionLevel>,
            Option<Vec<u8>>,
        ) = {
            let mut state_guard = self.state.lock();
            let mut stream_states_guard = self.stream_states.lock();
            let mut ack_pending_guard = self.ack_pending.lock();

            let truncated_packet_number = packet.header.packet_number;
            let packet_number_len = encoded_packet_number_len
                .unwrap_or_else(|| Self::infer_packet_number_len(truncated_packet_number));
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
            let inbound_packet_type = packet.header.r#type;

            // Process each frame
            for frame in packet.frames.iter() {
                match frame {
                    QuicFrame::Stream(stream_frame) => {
                        println!("📄 Frame: Stream frame on stream {}", stream_frame.stream_id);
                        self.process_stream_frame(
                            stream_frame,
                            &mut stream_states_guard,
                            &mut ack_pending_guard,
                            &state_guard,
                            reconstructed_packet_number,
                        )?;
                    }
                    QuicFrame::Ack(ack_frame) => {
                        println!("📬 Frame: ACK frame");
                        self.process_ack_frame(ack_frame, &mut state_guard);
                        // Transition to Connected state after receiving an ACK (simplified handshake)
                        if state_guard.connection_state == ConnectionState::Handshaking {
                            state_guard.connection_state = ConnectionState::Connected;
                            tracing::info!("Connection state transitioned to Connected.");
                        }
                    }
                    QuicFrame::Crypto(crypto_frame) => {
                        println!("🔐 Frame: Crypto frame, {} bytes", crypto_frame.data.len());
                        match self.process_crypto_frame(
                            crypto_frame,
                            packet.header.r#type,
                            &mut ack_pending_guard,
                            &mut state_guard,
                            reconstructed_packet_number,
                        ) {
                            Ok(()) => println!("✅ Crypto frame processed"),
                            Err(e) => {
                                println!("❌ Crypto frame error: {:?}", e);
                                return Err(e);
                            }
                        }
                    }
                        QuicFrame::Ping => {
                            println!("📡 Frame: Ping (ack-eliciting)");
                            ack_pending_guard.push(reconstructed_packet_number);
                        }
                    other => {
                        println!("❓ Frame: Other frame type: {:?}", other);
                    }
                }
            }

            // Update state with received packet
            state_guard.received_packets.push(packet);
            state_guard.next_packet_number += 1;

            // Generate ACK if needed.
            #[cfg(feature = "tls-quic")]
            {
                if !ack_pending_guard.is_empty() {
                    let ack_level = match inbound_packet_type {
                        QuicPacketType::Initial => Some(EncryptionLevel::Initial),
                        QuicPacketType::Handshake => Some(EncryptionLevel::Handshake),
                        QuicPacketType::ShortHeader => Some(EncryptionLevel::OneRtt),
                        _ => None,
                    };

                    if let Some(level) = ack_level {
                        let ack_packet = self.create_ack_packet(&state_guard, &ack_pending_guard)?;
                        let ack_frames = encode_frames(&ack_packet.frames).map_err(QuicError::Protocol)?;
                        ack_pending_guard.clear();
                        (Some(ack_packet), Some(level), Some(ack_frames))
                    } else {
                        println!(
                            "📬 Skipping ACK emission for unsupported packet type {:?}",
                            inbound_packet_type
                        );
                        ack_pending_guard.clear();
                        (None, None, None)
                    }
                } else {
                    (None, None, None)
                }
            }

            #[cfg(not(feature = "tls-quic"))]
            {
                if !ack_pending_guard.is_empty() {
                    let mut ack_packet = self.create_ack_packet(&state_guard, &ack_pending_guard)?;
                    self.apply_outbound_header_protection_hook(&mut ack_packet)?;
                    let serialized_ack = serialize_packet(&ack_packet)?;
                    ack_pending_guard.clear();
                    (Some(ack_packet), None, Some(serialized_ack))
                } else {
                    (None, None, None)
                }
            }
            // Guards are automatically dropped here
        };

        // Send ACK outside the locked scope
        if let Some(serialized_ack) = serialized_ack_opt {
            println!("📬 Sending ACK for packet (size={} bytes)", serialized_ack.len());
            #[cfg(feature = "tls-quic")]
            {
                if let Some(level) = ack_level_opt {
                    self.send_encrypted_frames(level, serialized_ack).await?;
                }
            }
            #[cfg(not(feature = "tls-quic"))]
            {
                self.socket
                    .send_to(&serialized_ack, self.remote_addr)
                    .await
                    .map_err(QuicError::Io)?;
            }
        } else {
            println!("📬 No ACK needed");
        }

        Ok(())
    }

    pub async fn send_stream_data(&self, stream_id: u64, data: Vec<u8>) -> Result<(), QuicError> {
        self.send_stream_data_with_fin(stream_id, data, false).await
    }

    pub async fn send_stream_data_with_fin(
        &self,
        stream_id: u64,
        data: Vec<u8>,
        fin: bool,
    ) -> Result<(), QuicError> {
        validate_stream_id(stream_id).map_err(QuicError::Protocol)?;

        let (offset, previous_state, previous_send_offset, previous_send_buffer_len) = {
            let state_guard = self.state.lock();
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

            let offset = stream.send_offset;
            let previous_state = stream.state;
            let previous_send_offset = stream.send_offset;
            let previous_send_buffer_len = stream.send_buffer.len();

            stream.send_buffer.extend_from_slice(&data);
            stream.send_offset = stream.send_offset.saturating_add(data.len() as u64);
            stream.state = match (stream.state, fin) {
                (StreamState::Idle, false) => StreamState::Open,
                (StreamState::Idle, true) => StreamState::HalfClosedLocal,
                (StreamState::Open, true) => StreamState::HalfClosedLocal,
                (StreamState::HalfClosedRemote, true) => StreamState::Closed,
                (StreamState::HalfClosedRemote, false) => StreamState::Closed,
                (s, _) => s,
            };

            (offset, previous_state, previous_send_offset, previous_send_buffer_len)
        };

        // STREAM frame type byte (RFC 9000 §19.8):
        // 0x08 base, OFF=0x04, LEN=0x02, FIN=0x01.
        let mut frame_type = 0x08u8 | 0x02u8; // Always include LEN for deterministic parsing.
        if offset > 0 {
            frame_type |= 0x04;
        }
        if fin {
            frame_type |= 0x01;
        }

        let mut frame = Vec::with_capacity(data.len() + 32);
        frame.push(frame_type);
        write_varint(stream_id, &mut frame).map_err(QuicError::Protocol)?;
        if offset > 0 {
            write_varint(offset, &mut frame).map_err(QuicError::Protocol)?;
        }
        write_varint(data.len() as u64, &mut frame).map_err(QuicError::Protocol)?;
        frame.extend_from_slice(&data);

        if let Err(err) = self.send_1rtt_frames(frame).await {
            if let Some(stream) = self.stream_states.lock().get_mut(&stream_id) {
                stream.state = previous_state;
                stream.send_offset = previous_send_offset;
                stream.send_buffer.truncate(previous_send_buffer_len);
            }
            return Err(err);
        }

        Ok(())
    }

    pub async fn send_stream_fin(&self, stream_id: u64) -> Result<(), QuicError> {
        self.send_stream_data_with_fin(stream_id, Vec::new(), true).await
    }

    pub async fn send_handshake_responses(&self) -> Result<(), QuicError> {
        let writes = self.crypto_provider.drain_crypto_writes();
        println!("🔄 send_handshake_responses: got {} writes", writes.len());
        if !writes.is_empty() {
            let (local_cid, remote_cid) = {
                let s = self.state.lock();
                (
                    s.local_connection_id.bytes.clone(),
                    s.remote_connection_id.bytes.clone(),
                )
            };
            let version: u32 = 1;

            for write in writes {
                let pn = match write.level {
                    EncryptionLevel::Initial => {
                        let mut c = self.initial_pn_counter.lock();
                        let n = *c;
                        *c += 1;
                        n
                    }
                    EncryptionLevel::Handshake => {
                        let mut c = self.handshake_pn_counter.lock();
                        let n = *c;
                        *c += 1;
                        n
                    }
                    EncryptionLevel::OneRtt => continue, // not yet
                };

                let type_bits: u8 = match write.level {
                    EncryptionLevel::Initial => 0x00,
                    EncryptionLevel::Handshake => 0x02,
                    EncryptionLevel::OneRtt => continue,
                };

                let crypto_data = &write.data;
                let crypto_offset = match write.level {
                    EncryptionLevel::Initial => {
                        let mut off = self.initial_crypto_send_offset.lock();
                        let cur = *off;
                        *off = off.saturating_add(crypto_data.len() as u64);
                        cur
                    }
                    EncryptionLevel::Handshake => {
                        let mut off = self.handshake_crypto_send_offset.lock();
                        let cur = *off;
                        *off = off.saturating_add(crypto_data.len() as u64);
                        cur
                    }
                    EncryptionLevel::OneRtt => continue,
                };

                // Encode CRYPTO frame with monotonically increasing per-level offset.
                let mut frame = Vec::new();
                frame.push(0x06u8); // CRYPTO frame type
                write_varint(crypto_offset, &mut frame).map_err(QuicError::Protocol)?;
                write_varint(crypto_data.len() as u64, &mut frame).map_err(QuicError::Protocol)?;
                frame.extend_from_slice(crypto_data);

                let tag_len = 16usize;
                let pn_len = 1usize;
                let payload_len = pn_len + frame.len() + tag_len;

                // Build unprotected header (everything before PN)
                let mut hdr = Vec::new();
                let first_byte: u8 = 0xC0 | (type_bits << 4) | ((pn_len - 1) as u8);
                hdr.push(first_byte);
                hdr.extend_from_slice(&version.to_be_bytes());
                // DCID = remote (client's connection ID)
                hdr.push(remote_cid.len() as u8);
                hdr.extend_from_slice(&remote_cid);
                // SCID = local (server's connection ID)
                hdr.push(local_cid.len() as u8);
                hdr.extend_from_slice(&local_cid);
                if write.level == EncryptionLevel::Initial {
                    hdr.push(0x00); // token length = 0
                }
                // payload_length varint
                write_varint(payload_len as u64, &mut hdr).map_err(QuicError::Protocol)?;

                let pn_offset = hdr.len();

                // AAD = hdr + pn bytes (before encryption)
                let mut aad = hdr.clone();
                aad.push(pn as u8); // pn_len = 1 byte

                // Encrypt: aad = full unprotected header, payload = frame plaintext
                let mut payload = frame.clone();
                if let Err(e) = self
                    .crypto_provider
                    .encrypt_packet(write.level, pn, &aad, &mut payload)
                {
                    println!("❌ encrypt_packet failed: {:?}", e);
                    continue;
                }

                // Build full packet
                let mut packet = hdr.clone();
                packet.push(pn as u8); // unprotected pn
                packet.extend_from_slice(&payload); // ciphertext + tag

                // Apply header protection
                let sample_offset = pn_offset + 4;
                if sample_offset + 16 <= packet.len() {
                    let sample: [u8; 16] = packet[sample_offset..sample_offset + 16]
                        .try_into()
                        .unwrap_or([0u8; 16]);
                    let mut first = packet[0];
                    let mut pn_byte = [packet[pn_offset]];
                    if self
                        .crypto_provider
                        .apply_header_protection(write.level, &sample, &mut first, &mut pn_byte)
                        .is_ok()
                    {
                        packet[0] = first;
                        packet[pn_offset] = pn_byte[0];
                    }
                }

                match self.socket.send_to(&packet, self.remote_addr).await {
                    Ok(_) => println!(
                        "📤 Sent {:?}-level handshake ({} bytes)",
                        write.level,
                        packet.len()
                    ),
                    Err(e) => println!("❌ Send handshake failed: {:?}", e),
                }
            }
        }

        // After crypto writes, send HANDSHAKE_DONE + HTTP/3 SETTINGS
        // We send this once the handshake is truly complete (client Finished received)
        // Per RFC 9000, HANDSHAKE_DONE must not be sent until handshake is complete
        let should_send_done = {
            let done_lock = self.handshake_done_sent.lock();
            if *done_lock {
                false
            } else {
                // Only send HANDSHAKE_DONE when handshake is complete
                self.crypto_provider.handshake_complete()
            }
        };
        if should_send_done {
            *self.handshake_done_sent.lock() = true;

            // 1. Send HANDSHAKE_DONE (frame type 0x1e) in a 1-RTT packet
            let mut hd_frames = Vec::new();
            hd_frames.push(0x1eu8); // HANDSHAKE_DONE

            // 2. Also include QUIC NEW_CONNECTION_ID and stream setup in same packet
            // HTTP/3 server control stream: stream ID 3, unidirectional
            // Stream frame: type=0x0a (STREAM|LEN, no offset, no FIN), stream_id=3, data=[0x00,0x04,0x00]
            // HTTP/3 server control stream (stream ID 3, unidirectional server-initiated)
            let h3_settings: &[u8] = &[0x00u8, 0x04, 0x00]; // stream type=control(0x00) + empty SETTINGS(0x04, 0x00)
            hd_frames.push(0x0au8); // STREAM | LEN, no offset, no FIN
            // stream_id = 3 (varint)
            hd_frames.push(0x03u8);
            // length = 3 (varint)
            hd_frames.push(h3_settings.len() as u8);
            hd_frames.extend_from_slice(h3_settings);

            println!("🔑 1-RTT keys available, sending HANDSHAKE_DONE");
            if let Err(e) = self.send_1rtt_frames(hd_frames).await {
                println!("❌ Failed to send HANDSHAKE_DONE: {:?}", e);
            } else {
                println!("📤 Sent HANDSHAKE_DONE + HTTP/3 SETTINGS");
            }
        }

        Ok(())
    }

    async fn send_encrypted_frames(
        &self,
        level: EncryptionLevel,
        frames: Vec<u8>,
    ) -> Result<(), QuicError> {
        if level == EncryptionLevel::OneRtt {
            return self.send_1rtt_frames(frames).await;
        }

        let (local_cid, remote_cid) = {
            let s = self.state.lock();
            (
                s.local_connection_id.bytes.clone(),
                s.remote_connection_id.bytes.clone(),
            )
        };

        let pn = match level {
            EncryptionLevel::Initial => {
                let mut c = self.initial_pn_counter.lock();
                let n = *c;
                *c += 1;
                n
            }
            EncryptionLevel::Handshake => {
                let mut c = self.handshake_pn_counter.lock();
                let n = *c;
                *c += 1;
                n
            }
            EncryptionLevel::OneRtt => unreachable!(),
        };

        let type_bits: u8 = match level {
            EncryptionLevel::Initial => 0x00,
            EncryptionLevel::Handshake => 0x02,
            EncryptionLevel::OneRtt => unreachable!(),
        };

        let tag_len = 16usize;
        let pn_len = 1usize;
        let payload_len = pn_len + frames.len() + tag_len;

        let mut hdr = Vec::new();
        let first_byte: u8 = 0xC0 | (type_bits << 4) | ((pn_len - 1) as u8);
        hdr.push(first_byte);
        hdr.extend_from_slice(&1u32.to_be_bytes());
        hdr.push(remote_cid.len() as u8);
        hdr.extend_from_slice(&remote_cid);
        hdr.push(local_cid.len() as u8);
        hdr.extend_from_slice(&local_cid);
        if level == EncryptionLevel::Initial {
            hdr.push(0x00); // token length = 0
        }
        if payload_len < 64 {
            hdr.push(payload_len as u8);
        } else if payload_len < 16384 {
            hdr.push(0x40 | ((payload_len >> 8) as u8));
            hdr.push((payload_len & 0xff) as u8);
        } else {
            hdr.push(0x80 | ((payload_len >> 24) as u8));
            hdr.push(((payload_len >> 16) & 0xff) as u8);
            hdr.push(((payload_len >> 8) & 0xff) as u8);
            hdr.push((payload_len & 0xff) as u8);
        }

        let pn_offset = hdr.len();
        let mut aad = hdr.clone();
        aad.push(pn as u8);

        let mut payload = frames;
        self.crypto_provider
            .encrypt_packet(level, pn, &aad, &mut payload)?;

        let mut packet = hdr;
        packet.push(pn as u8);
        packet.extend_from_slice(&payload);

        let sample_offset = pn_offset + 4;
        if sample_offset + 16 <= packet.len() {
            let sample: [u8; 16] = packet[sample_offset..sample_offset + 16]
                .try_into()
                .unwrap_or([0u8; 16]);
            let mut first = packet[0];
            let mut pn_byte = [packet[pn_offset]];
            if self
                .crypto_provider
                .apply_header_protection(level, &sample, &mut first, &mut pn_byte)
                .is_ok()
            {
                packet[0] = first;
                packet[pn_offset] = pn_byte[0];
            }
        }

        self.socket
            .send_to(&packet, self.remote_addr)
            .await
            .map_err(QuicError::Io)?;
        Ok(())
    }

    async fn send_1rtt_frames(&self, frames: Vec<u8>) -> Result<(), QuicError> {
        // For 1-RTT packets (short header):
        // Per RFC 9000 §17.3, DCID = connection ID the sender wants receiver to use
        // For server→client: DCID should be the server's connection ID
        let (local_cid, remote_cid) = {
            let s = self.state.lock();
            (
                s.local_connection_id.bytes.clone(),
                s.remote_connection_id.bytes.clone(),
            )
        };

        // Use remote_cid (client's SCID) as DCID in short header
        // Per RFC 9000 §17.3: DCID = ID the sender wants receiver to use
        // Server→client packet: DCID = client's SCID (stored in remote_connection_id)
        let dcid = remote_cid;
        println!("📨 1-RTT packet: DCID={:02x?}, local_cid={:02x?}", &dcid[..dcid.len().min(8)], &local_cid[..local_cid.len().min(8)]);

        let pn = {
            let mut c = self.onertt_pn_counter.lock();
            let n = *c;
            *c += 1;
            n
        };

        let pn_len = 1usize;
        let tag_len = 16usize;
        let payload_len = pn_len + frames.len() + tag_len;
        println!("📨 1-RTT payload: {} bytes frames, {} pn_len, {} tag = {} total", frames.len(), pn_len, tag_len, payload_len);

        // Short header first byte: bit7=0, bit6=1 (fixed), bits1-0 = pn_len - 1
        let first_byte: u8 = 0x40 | ((pn_len - 1) as u8);

        // Short header: first_byte | DCID (what client expects - server's CID)
        let mut hdr = Vec::new();
        hdr.push(first_byte);
        hdr.extend_from_slice(&dcid);

        let pn_offset = hdr.len();
        let mut aad = hdr.clone();
        aad.push(pn as u8);

        println!("📨 1-RTT before encrypt: aad={} bytes, first={:02x}, pn={}", aad.len(), first_byte, pn);

        let mut payload = frames;
        self.crypto_provider.encrypt_packet(EncryptionLevel::OneRtt, pn, &aad, &mut payload)
            .map_err(|e| { println!("❌ 1-RTT encrypt failed: {:?}", e); e })?;

        let mut packet = hdr;
        packet.push(pn as u8);
        packet.extend_from_slice(&payload);

        println!("📨 1-RTT after encrypt: packet={} bytes", packet.len());

        // Apply header protection
        let sample_offset = pn_offset + 4;
        if sample_offset + 16 <= packet.len() {
            let sample: [u8; 16] = packet[sample_offset..sample_offset + 16].try_into().unwrap_or([0u8; 16]);
            let mut first = packet[0];
            let mut pn_byte = [packet[pn_offset]];
            if self.crypto_provider.apply_header_protection(EncryptionLevel::OneRtt, &sample, &mut first, &mut pn_byte).is_ok() {
                packet[0] = first;
                packet[pn_offset] = pn_byte[0];
                println!("📨 1-RTT header protection applied");
            } else {
                println!("⚠️  1-RTT header protection failed");
            }
        } else {
            println!("⚠️  1-RTT sample offset out of bounds: sample_offset={}, packet_len={}", sample_offset, packet.len());
        }

        println!("📨 Sending 1-RTT packet: {} bytes to {}", packet.len(), self.remote_addr);
        self.socket.send_to(&packet, self.remote_addr).await.map_err(QuicError::Io)?;
        Ok(())
    }

    async fn send_stream_frame(
        &self,
        stream_id: u64,
        data: Vec<u8>,
        fin: bool,
    ) -> Result<(), QuicError> {
        // Prepare packet data in a separate scope to drop guards early
        let (serialized_packet, packet_number, wire_len) = {
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
                fin,
            };

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

            let result = serialize_packet(&packet)?;
            let wire_len = result.len() as u64;

            // Commit stream + connection state only after packet construction and
            // serialization succeed, so encode/hook failures don't drift state.
            stream.send_buffer.extend_from_slice(&data);
            stream.send_offset += data.len() as u64;
            stream.state = match (stream.state, fin) {
                (StreamState::Idle, false) => StreamState::Open,
                (StreamState::Idle, true) => StreamState::HalfClosedLocal,
                (StreamState::Open, true) => StreamState::HalfClosedLocal,
                (StreamState::HalfClosedRemote, true) => StreamState::Closed,
                (StreamState::HalfClosedRemote, false) => StreamState::Closed,
                (s, _) => s,
            };
            state_guard.sent_packets.push(packet.clone());
            state_guard.next_packet_number += 1;
            state_guard.bytes_in_flight += wire_len;

            Ok::<(Vec<u8>, u64, u64), QuicError>((result, packet.header.packet_number, wire_len))
            // Guards are automatically dropped here
        }?;

        // Send packet outside the locked scope
        if let Err(err) = self.socket.send_to(&serialized_packet, self.remote_addr).await {
            self.rollback_failed_stream_send(stream_id, packet_number, &data, wire_len);
            return Err(QuicError::Io(err));
        }

        Ok(())
    }

    pub fn create_stream(&self) -> u64 {
        let mut state_guard = self.state.lock();
        // Preserve any caller-provided non-zero seed, but align the default seed
        // to a role-specific stream-ID lane and allocate subsequent IDs in steps
        // of four (matching QUIC stream-type/initiator bit lanes).
        if state_guard.next_stream_id == 0 {
            state_guard.next_stream_id = match self.role {
                Role::Client => 1,
                Role::Server => 0,
            };
        }

        let new_stream_id = state_guard.next_stream_id;
        state_guard.next_stream_id = state_guard.next_stream_id.saturating_add(4);
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
        let mut contiguous_offsets_guard = self.stream_contiguous_receive_offsets.lock();
        let contiguous_receive_offset = contiguous_offsets_guard.entry(frame.stream_id).or_insert(0);
        let mut pending_fragments_guard = self.stream_pending_fragments.lock();
        let pending_fragments = pending_fragments_guard.entry(frame.stream_id).or_default();

        // Minimal fragment buffering: append contiguous bytes immediately and
        // stash gapped fragments for later drain when the gap is filled.
        if !frame.data.is_empty() {
            if frame.offset > *contiguous_receive_offset {
                let had_overlap_conflict = Self::queue_pending_stream_fragment(
                    pending_fragments,
                    frame.offset,
                    frame.data.clone(),
                );
                if had_overlap_conflict {
                    let mut conflict_counts = self.stream_overlap_conflict_counts.lock();
                    *conflict_counts.entry(frame.stream_id).or_insert(0) += 1;
                    *self.total_stream_overlap_conflict_count.lock() += 1;
                }
            } else {
                let append_from = if frame.offset == *contiguous_receive_offset {
                    0usize
                } else {
                    (*contiguous_receive_offset - frame.offset)
                        .min(frame.data.len() as u64) as usize
                };
                if append_from < frame.data.len() {
                    stream.receive_buffer.extend_from_slice(&frame.data[append_from..]);
                    let appended_len = (frame.data.len() - append_from) as u64;
                    let append_start = frame.offset.saturating_add(append_from as u64);
                    let base = (*contiguous_receive_offset).max(append_start);
                    *contiguous_receive_offset = base.saturating_add(appended_len);
                }
            }

            // Drain any newly contiguous pending fragments. Keep this simple and
            // bounded by pending list size; dedup/coalescing can come later.
            loop {
                let mut advanced = false;
                let mut i = 0usize;
                while i < pending_fragments.len() {
                    let (seg_offset, seg_data) = &pending_fragments[i];
                    let append_from = if *seg_offset > *contiguous_receive_offset {
                        i += 1;
                        continue;
                    } else if *seg_offset == *contiguous_receive_offset {
                        0usize
                    } else {
                        (*contiguous_receive_offset - *seg_offset)
                            .min(seg_data.len() as u64) as usize
                    };
                    if append_from < seg_data.len() {
                        stream.receive_buffer.extend_from_slice(&seg_data[append_from..]);
                        let appended_len = (seg_data.len() - append_from) as u64;
                        let append_start = seg_offset.saturating_add(append_from as u64);
                        let base = (*contiguous_receive_offset).max(append_start);
                        *contiguous_receive_offset = base.saturating_add(appended_len);
                        advanced = true;
                    }
                    pending_fragments.remove(i);
                }
                if !advanced {
                    break;
                }
            }
        }
        let frame_end = frame.offset.saturating_add(frame.data.len() as u64);
        stream.receive_offset = stream.receive_offset.max(frame_end);
        stream.state = match (stream.state, frame.fin) {
            (StreamState::Idle, false) => StreamState::Open,
            (StreamState::Idle, true) => StreamState::HalfClosedRemote,
            (StreamState::Open, true) => StreamState::HalfClosedRemote,
            (StreamState::HalfClosedLocal, true) => StreamState::Closed,
            (s, _) => s,
        };

        // Mark actual received packet number for ACK generation.
        ack_pending_guard.push(received_packet_number);
        Ok(())
    }

    fn queue_pending_stream_fragment(
        pending_fragments: &mut Vec<(u64, Vec<u8>)>,
        offset: u64,
        data: Vec<u8>,
    ) -> bool {
        if data.is_empty() {
            return false;
        }

        if pending_fragments
            .iter()
            .any(|(existing_offset, existing_data)| *existing_offset == offset && *existing_data == data)
        {
            return false;
        }

        pending_fragments.push((offset, data));
        pending_fragments.sort_by_key(|(seg_offset, _)| *seg_offset);

        let mut coalesced: Vec<(u64, Vec<u8>)> = Vec::with_capacity(pending_fragments.len());
        let mut saw_conflicting_overlap = false;
        for (seg_offset, seg_data) in pending_fragments.drain(..) {
            if seg_data.is_empty() {
                continue;
            }

            let Some((last_offset, last_data)) = coalesced.last_mut() else {
                coalesced.push((seg_offset, seg_data));
                continue;
            };

            let last_end = last_offset.saturating_add(last_data.len() as u64);
            if seg_offset > last_end {
                coalesced.push((seg_offset, seg_data));
                continue;
            }

            let overlap_or_adjacent = last_end.saturating_sub(seg_offset) as usize;
            let actual_overlap = overlap_or_adjacent.min(seg_data.len());
            if actual_overlap > 0 {
                let last_overlap_start = seg_offset.saturating_sub(*last_offset) as usize;
                let last_overlap_end = last_overlap_start.saturating_add(actual_overlap);
                if last_overlap_end <= last_data.len()
                    && last_data[last_overlap_start..last_overlap_end] != seg_data[..actual_overlap]
                {
                    saw_conflicting_overlap = true;
                    tracing::warn!(
                        pending_offset = seg_offset,
                        pending_len = seg_data.len(),
                        overlap_bytes = actual_overlap,
                        "Conflicting overlapping pending QUIC STREAM fragment bytes; keeping existing bytes"
                    );
                }
            }
            if overlap_or_adjacent < seg_data.len() {
                last_data.extend_from_slice(&seg_data[overlap_or_adjacent..]);
            }
            // If fully covered, drop the segment. Overlap conflicts are detected
            // and logged above; existing buffered bytes take precedence.
        }

        *pending_fragments = coalesced;
        saw_conflicting_overlap
    }

    fn process_ack_frame(&self, frame: &AckFrame, state_guard: &mut QuicConnectionState) {
        let mut acked_bytes = 0u64;
        let mut remaining_packets = Vec::with_capacity(state_guard.sent_packets.len());

        for packet in state_guard.sent_packets.drain(..) {
            if Self::ack_frame_acknowledges_packet(frame, packet.header.packet_number) {
                acked_bytes = acked_bytes.saturating_add(Self::wire_len_for_accounting(&packet));
            } else {
                remaining_packets.push(packet);
            }
        }
        state_guard.sent_packets = remaining_packets;

        state_guard.bytes_in_flight = state_guard.bytes_in_flight.saturating_sub(acked_bytes);
    }

    fn process_crypto_frame(
        &self,
        frame: &CryptoFrame,
        packet_type: super::quic_protocol::QuicPacketType,
        ack_pending_guard: &mut Vec<u64>,
        state_guard: &mut QuicConnectionState,
        received_packet_number: u64,
    ) -> Result<(), QuicError> {
        use super::quic_crypto::EncryptionLevel;
        use super::quic_protocol::QuicPacketType;
        let level = match packet_type {
            QuicPacketType::Initial => EncryptionLevel::Initial,
            QuicPacketType::Handshake => EncryptionLevel::Handshake,
            QuicPacketType::ShortHeader => EncryptionLevel::OneRtt,
            _ => EncryptionLevel::Initial,
        };
        println!("🔄 process_crypto: calling on_crypto_frame at level {:?}", level);
        let disposition = self.crypto_provider.on_crypto_frame(frame, level, state_guard)?;
        println!("🔄 process_crypto: disposition = {:?}", disposition);
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
        sorted_acks.dedup();

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

    pub fn get_crypto_provider(&self) -> Arc<dyn QuicCryptoProvider> {
        self.crypto_provider.clone()
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

    pub fn get_stream_overlap_conflict_count(&self, stream_id: u64) -> u64 {
        self.stream_overlap_conflict_counts
            .lock()
            .get(&stream_id)
            .copied()
            .unwrap_or(0)
    }

    pub fn get_total_stream_overlap_conflict_count(&self) -> u64 {
        *self.total_stream_overlap_conflict_count.lock()
    }


    pub fn mark_activity(&self) {
        let mut last = self.last_activity.lock();
        *last = Instant::now();
    }

    pub fn check_timeouts(&self) -> Result<bool, String> {
        let last_activity = *self.last_activity.lock();
        if last_activity.elapsed() > self.idle_timeout {
            return Ok(true);
        }
        Ok(false)
    }

    pub fn diagnostics_snapshot(&self) -> QuicEngineDiagnosticsSnapshot {
        let pending = self.stream_pending_fragments.lock();
        let per_stream_contiguous_receive_offsets =
            self.stream_contiguous_receive_offsets.lock().clone();
        let per_stream_highest_seen_receive_offsets = self
            .stream_states
            .lock()
            .iter()
            .map(|(stream_id, state)| (*stream_id, state.receive_offset))
            .collect();
        let per_stream_pending_fragment_counts: HashMap<u64, usize> = pending
            .iter()
            .map(|(stream_id, frags)| (*stream_id, frags.len()))
            .collect();
        let per_stream_pending_fragment_bytes: HashMap<u64, u64> = pending
            .iter()
            .map(|(stream_id, frags)| {
                let bytes = frags.iter().map(|(_, data)| data.len() as u64).sum();
                (*stream_id, bytes)
            })
            .collect();
        let total_pending_fragment_bytes =
            per_stream_pending_fragment_bytes.values().copied().sum::<u64>();

        QuicEngineDiagnosticsSnapshot {
            total_stream_overlap_conflict_count: *self.total_stream_overlap_conflict_count.lock(),
            per_stream_overlap_conflict_counts: self.stream_overlap_conflict_counts.lock().clone(),
            per_stream_pending_fragment_counts,
            total_pending_fragment_bytes,
            per_stream_pending_fragment_bytes,
            per_stream_contiguous_receive_offsets,
            per_stream_highest_seen_receive_offsets,
        }
    }

    pub fn get_active_streams(&self) -> Vec<u64> {
        self.stream_states.lock().keys().cloned().collect()
    }

    pub async fn close(&self) {
        let mut s = self.state.lock();
        s.connection_state = ConnectionState::Closed;
    }

    fn ack_frame_acknowledges_packet(frame: &AckFrame, packet_number: u64) -> bool {
        if frame.ack_ranges.is_empty() {
            return frame.largest_acknowledged == packet_number;
        }

        frame.ack_ranges.iter().any(|&(start, end)| {
            let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
            (lo..=hi).contains(&packet_number)
        })
    }

    fn wire_len_for_accounting(packet: &QuicPacket) -> u64 {
        serialize_packet(packet)
            .map(|encoded| encoded.len() as u64)
            .unwrap_or(packet.payload.len() as u64)
    }

    fn rollback_failed_stream_send(
        &self,
        stream_id: u64,
        packet_number: u64,
        data: &[u8],
        wire_len: u64,
    ) {
        {
            let mut state_guard = self.state.lock();
            if let Some(idx) = state_guard
                .sent_packets
                .iter()
                .position(|p| p.header.packet_number == packet_number)
            {
                state_guard.sent_packets.remove(idx);
                state_guard.bytes_in_flight = state_guard.bytes_in_flight.saturating_sub(wire_len);
            }
        }

        let mut stream_states_guard = self.stream_states.lock();
        let Some(stream) = stream_states_guard.get_mut(&stream_id) else {
            return;
        };
        let len = data.len();
        if len == 0 {
            return;
        }
        if stream.send_offset < len as u64 || stream.send_buffer.len() < len {
            return;
        }

        let tail_start = stream.send_buffer.len() - len;
        if &stream.send_buffer[tail_start..] == data {
            stream.send_buffer.truncate(tail_start);
            stream.send_offset = stream.send_offset.saturating_sub(len as u64);
        }
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
    use crate::quic::quic_protocol::{serialize_packet, ConnectionId, TransportParameters};
    use parking_lot::Mutex as ParkingMutex;

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

    struct CaptureInboundHeaderCtxCrypto {
        seen_pn_lens: Arc<ParkingMutex<Vec<usize>>>,
    }

    impl QuicCryptoProvider for CaptureInboundHeaderCtxCrypto {
        fn on_inbound_header(
            &self,
            _header: &mut QuicHeader,
            ctx: &InboundHeaderProtectionContext,
        ) -> Result<(), QuicError> {
            self.seen_pn_lens.lock().push(ctx.packet_number_len);
            Ok(())
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

    fn sample_stream_packet(packet_number: u64, stream_id: u64, data: &[u8]) -> QuicPacket {
        QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id,
                offset: 0,
                data: data.to_vec(),
                fin: false,
            })],
            payload: data.to_vec(),
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
            crate::concurrency::ccek::CoroutineContext::new(),
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

    #[tokio::test]
    async fn process_decoded_packet_uses_wire_packet_number_len_for_header_hook() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
        let seen_pn_lens = Arc::new(ParkingMutex::new(Vec::new()));
        let engine = QuicEngine::new_with_crypto_provider(
            Role::Server,
            sample_state(),
            socket,
            remote_addr,
            vec![],
            Arc::new(CaptureInboundHeaderCtxCrypto {
                seen_pn_lens: seen_pn_lens.clone(),
            }),
            crate::concurrency::ccek::CoroutineContext::new(),
        );

        let packet = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![1; 8] },
                source_connection_id: ConnectionId { bytes: vec![] },
                packet_number: 1, // would infer len=1 if metadata were ignored
                token: None,
            },
            frames: vec![QuicFrame::Ping],
            payload: Vec::new(),
        };

        engine
            .process_decoded_packet(DecodedQuicPacket {
                packet,
                encoded_packet_number_len: 4,
            })
            .await
            .unwrap();

        let seen = seen_pn_lens.lock().clone();
        assert_eq!(seen, vec![4]);
    }

    #[tokio::test]
    async fn process_ack_frame_uses_sent_packet_wire_lengths_and_prunes_acknowledged_packets() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();

        let engine = QuicEngine::new(
            Role::Client,
            sample_state(),
            socket,
            remote_addr,
            vec![],
            crate::concurrency::ccek::CoroutineContext::new(),
        );

        let pkt1 = sample_stream_packet(10, 0, b"a");
        let pkt2 = sample_stream_packet(11, 4, b"hello");
        let pkt1_len = serialize_packet(&pkt1).unwrap().len() as u64;
        let pkt2_len = serialize_packet(&pkt2).unwrap().len() as u64;

        let mut state = sample_state();
        state.sent_packets = vec![pkt1.clone(), pkt2.clone()];
        state.bytes_in_flight = pkt1_len + pkt2_len;

        let ack = AckFrame {
            largest_acknowledged: 10,
            ack_delay: 0,
            ack_ranges: vec![(10, 10)],
        };

        engine.process_ack_frame(&ack, &mut state);

        assert_eq!(state.bytes_in_flight, pkt2_len);
        assert_eq!(state.sent_packets.len(), 1);
        assert_eq!(state.sent_packets[0].header.packet_number, 11);
    }

    #[tokio::test]
    async fn create_ack_packet_deduplicates_ack_pending_before_range_encoding() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();

        let engine = QuicEngine::new(
            Role::Server,
            sample_state(),
            socket,
            remote_addr,
            vec![],
            crate::concurrency::ccek::CoroutineContext::new(),
        );
        let state = sample_state();
        let ack_packet = engine
            .create_ack_packet(&state, &[5, 5, 6, 8, 8])
            .unwrap();

        match &ack_packet.frames[0] {
            QuicFrame::Ack(frame) => {
                assert_eq!(frame.largest_acknowledged, 8);
                assert_eq!(frame.ack_ranges, vec![(5, 6), (8, 8)]);
            }
            other => panic!("expected ACK frame, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn send_stream_data_rolls_back_state_when_udp_send_fails() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        // IPv4 socket -> IPv6 destination should fail with address-family mismatch.
        let remote_addr: SocketAddr = "[::1]:4433".parse().unwrap();

        let engine = QuicEngine::new(Role::Client, sample_state(), socket, remote_addr, vec![], crate::concurrency::ccek::CoroutineContext::new());
        let stream_id = engine.create_stream();

        let err = engine
            .send_stream_data(stream_id, b"rollback-me".to_vec())
            .await
            .unwrap_err();
        assert!(matches!(err, QuicError::Io(_)));

        let state = engine.get_state();
        assert!(state.sent_packets.is_empty());
        assert_eq!(state.bytes_in_flight, 0);
        // Packet numbers may still be consumed by failed sends; gaps are acceptable.
        assert_eq!(state.next_packet_number, 1);

        let stream = engine.get_stream(stream_id).expect("stream state exists");
        assert_eq!(stream.send_offset, 0);
        assert!(stream.send_buffer.is_empty());
    }

    #[tokio::test]
    async fn send_stream_data_transitions_stream_from_idle_to_open() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr = socket.local_addr().unwrap();
        let engine = QuicEngine::new(Role::Client, sample_state(), socket, remote_addr, vec![], crate::concurrency::ccek::CoroutineContext::new());

        let stream_id = engine.create_stream();
        engine.send_stream_data(stream_id, b"hello".to_vec()).await.unwrap();

        let stream = engine.get_stream(stream_id).expect("stream state exists");
        assert_eq!(stream.state, StreamState::Open);
    }

    #[tokio::test]
    async fn process_stream_frame_fin_marks_remote_half_close_and_local_send_closes() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
        let engine = QuicEngine::new(Role::Server, sample_state(), socket, remote_addr, vec![], crate::concurrency::ccek::CoroutineContext::new());

        engine
            .process_packet(sample_stream_packet(1, 0, b"abc"))
            .await
            .unwrap();
        assert_eq!(engine.get_stream(0).unwrap().state, StreamState::Open);

        let fin_packet = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 2,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 3,
                data: Vec::new(),
                fin: true,
            })],
            payload: Vec::new(),
        };
        engine.process_packet(fin_packet).await.unwrap();
        assert_eq!(
            engine.get_stream(0).unwrap().state,
            StreamState::HalfClosedRemote
        );

        engine.send_stream_data(0, b"reply".to_vec()).await.unwrap();
        assert_eq!(engine.get_stream(0).unwrap().state, StreamState::Closed);
    }

    #[tokio::test]
    async fn send_stream_fin_marks_local_half_close() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr = socket.local_addr().unwrap();
        let engine = QuicEngine::new(Role::Client, sample_state(), socket, remote_addr, vec![], crate::concurrency::ccek::CoroutineContext::new());

        let stream_id = engine.create_stream();
        engine.send_stream_fin(stream_id).await.unwrap();

        let stream = engine.get_stream(stream_id).expect("stream state exists");
        assert_eq!(stream.state, StreamState::HalfClosedLocal);
        assert_eq!(stream.send_offset, 0);
    }

    #[tokio::test]
    async fn send_stream_fin_after_remote_fin_closes_stream() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
        let engine = QuicEngine::new(Role::Server, sample_state(), socket, remote_addr, vec![], crate::concurrency::ccek::CoroutineContext::new());

        let fin_packet = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 1,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 0,
                data: Vec::new(),
                fin: true,
            })],
            payload: Vec::new(),
        };
        engine.process_packet(fin_packet).await.unwrap();
        assert_eq!(
            engine.get_stream(0).unwrap().state,
            StreamState::HalfClosedRemote
        );

        engine.send_stream_fin(0).await.unwrap();
        assert_eq!(engine.get_stream(0).unwrap().state, StreamState::Closed);
    }

    #[tokio::test]
    async fn process_stream_frame_duplicate_does_not_move_receive_offset_backwards() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
        let engine = QuicEngine::new(Role::Server, sample_state(), socket, remote_addr, vec![], crate::concurrency::ccek::CoroutineContext::new());

        engine
            .process_packet(sample_stream_packet(1, 0, b"abcdef"))
            .await
            .unwrap();
        assert_eq!(engine.get_stream(0).unwrap().receive_offset, 6);
        assert_eq!(engine.get_stream(0).unwrap().receive_buffer, b"abcdef");

        // Duplicate earlier range should not reduce receive_offset.
        engine
            .process_packet(sample_stream_packet(2, 0, b"abc"))
            .await
            .unwrap();
        let stream = engine.get_stream(0).unwrap();
        assert_eq!(stream.receive_offset, 6);
        assert_eq!(stream.receive_buffer, b"abcdef");
    }

    #[tokio::test]
    async fn process_stream_frame_out_of_order_advances_receive_offset_to_highest_end() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
        let engine = QuicEngine::new(Role::Server, sample_state(), socket, remote_addr, vec![], crate::concurrency::ccek::CoroutineContext::new());

        let out_of_order = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 1,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 10,
                data: b"xyz".to_vec(),
                fin: false,
            })],
            payload: b"xyz".to_vec(),
        };
        engine.process_packet(out_of_order).await.unwrap();
        assert_eq!(engine.get_stream(0).unwrap().receive_offset, 13);
        assert!(engine.get_stream(0).unwrap().receive_buffer.is_empty());

        // Earlier fragment arrives later; highest-seen offset remains 13, but
        // contiguous buffering can now progress independently.
        engine
            .process_packet(sample_stream_packet(2, 0, b"abcd"))
            .await
            .unwrap();
        let stream = engine.get_stream(0).unwrap();
        assert_eq!(stream.receive_offset, 13);
        assert_eq!(stream.receive_buffer, b"abcd");
    }

    #[tokio::test]
    async fn process_stream_frame_partial_overlap_appends_only_new_tail_bytes() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
        let engine = QuicEngine::new(Role::Server, sample_state(), socket, remote_addr, vec![], crate::concurrency::ccek::CoroutineContext::new());

        engine
            .process_packet(sample_stream_packet(1, 0, b"abcdef"))
            .await
            .unwrap();

        let overlap_packet = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 2,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 4, // overlaps "ef", new tail is "gh"
                data: b"efgh".to_vec(),
                fin: false,
            })],
            payload: b"efgh".to_vec(),
        };
        engine.process_packet(overlap_packet).await.unwrap();

        let stream = engine.get_stream(0).unwrap();
        assert_eq!(stream.receive_offset, 8);
        assert_eq!(stream.receive_buffer, b"abcdefgh");
    }

    #[tokio::test]
    async fn process_stream_frame_gap_is_bridged_when_middle_fragment_arrives() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
        let engine = QuicEngine::new(Role::Server, sample_state(), socket, remote_addr, vec![], crate::concurrency::ccek::CoroutineContext::new());

        engine
            .process_packet(sample_stream_packet(1, 0, b"abcd"))
            .await
            .unwrap();

        let gap_tail = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 2,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 8,
                data: b"ijkl".to_vec(),
                fin: false,
            })],
            payload: b"ijkl".to_vec(),
        };
        engine.process_packet(gap_tail).await.unwrap();
        let stream = engine.get_stream(0).unwrap();
        assert_eq!(stream.receive_offset, 12);
        assert_eq!(stream.receive_buffer, b"abcd");

        let middle = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 3,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 4,
                data: b"efgh".to_vec(),
                fin: false,
            })],
            payload: b"efgh".to_vec(),
        };
        engine.process_packet(middle).await.unwrap();
        let stream = engine.get_stream(0).unwrap();
        assert_eq!(stream.receive_offset, 12);
        assert_eq!(stream.receive_buffer, b"abcdefghijkl");
    }

    #[tokio::test]
    async fn process_stream_frame_duplicate_pending_fragment_is_deduped() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
        let engine = QuicEngine::new(Role::Server, sample_state(), socket, remote_addr, vec![], crate::concurrency::ccek::CoroutineContext::new());

        engine
            .process_packet(sample_stream_packet(1, 0, b"abcd"))
            .await
            .unwrap();

        let gap_tail = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 2,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 8,
                data: b"ijkl".to_vec(),
                fin: false,
            })],
            payload: b"ijkl".to_vec(),
        };
        engine.process_packet(gap_tail.clone()).await.unwrap();
        engine.process_packet(gap_tail).await.unwrap();

        let pending_len = engine
            .stream_pending_fragments
            .lock()
            .get(&0)
            .map(|segments| segments.len())
            .unwrap_or(0);
        assert_eq!(pending_len, 1);

        let middle = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 3,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 4,
                data: b"efgh".to_vec(),
                fin: false,
            })],
            payload: b"efgh".to_vec(),
        };
        engine.process_packet(middle).await.unwrap();

        let stream = engine.get_stream(0).unwrap();
        assert_eq!(stream.receive_buffer, b"abcdefghijkl");
    }

    #[tokio::test]
    async fn process_stream_frame_overlapping_pending_fragments_are_coalesced() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
        let engine = QuicEngine::new(Role::Server, sample_state(), socket, remote_addr, vec![], crate::concurrency::ccek::CoroutineContext::new());

        engine
            .process_packet(sample_stream_packet(1, 0, b"abcd"))
            .await
            .unwrap();

        let gap_tail1 = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 2,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 8,
                data: b"ijkl".to_vec(),
                fin: false,
            })],
            payload: b"ijkl".to_vec(),
        };
        engine.process_packet(gap_tail1).await.unwrap();

        let gap_tail2 = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 3,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 10, // overlaps pending "ijkl" on "kl", extends with "mn"
                data: b"klmn".to_vec(),
                fin: false,
            })],
            payload: b"klmn".to_vec(),
        };
        engine.process_packet(gap_tail2).await.unwrap();

        let pending = engine
            .stream_pending_fragments
            .lock()
            .get(&0)
            .cloned()
            .unwrap_or_default();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, 8);
        assert_eq!(pending[0].1, b"ijklmn");

        let middle = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 4,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 4,
                data: b"efgh".to_vec(),
                fin: false,
            })],
            payload: b"efgh".to_vec(),
        };
        engine.process_packet(middle).await.unwrap();

        let stream = engine.get_stream(0).unwrap();
        assert_eq!(stream.receive_offset, 14);
        assert_eq!(stream.receive_buffer, b"abcdefghijklmn");
    }

    #[test]
    fn queue_pending_stream_fragment_detects_conflicting_overlap_and_keeps_existing_bytes() {
        let mut pending = vec![(8u64, b"ijkl".to_vec())];

        let had_conflict = QuicEngine::queue_pending_stream_fragment(&mut pending, 10, b"XXmn".to_vec());

        assert!(had_conflict);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, 8);
        assert_eq!(pending[0].1, b"ijklmn");
    }

    #[tokio::test]
    async fn process_stream_frame_conflicting_pending_overlap_increments_conflict_counter() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
        let engine = QuicEngine::new(Role::Server, sample_state(), socket, remote_addr, vec![], crate::concurrency::ccek::CoroutineContext::new());

        engine
            .process_packet(sample_stream_packet(1, 0, b"abcd"))
            .await
            .unwrap();
        assert_eq!(engine.get_stream_overlap_conflict_count(0), 0);
        assert_eq!(engine.get_total_stream_overlap_conflict_count(), 0);

        let gap_tail1 = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 2,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 8,
                data: b"ijkl".to_vec(),
                fin: false,
            })],
            payload: b"ijkl".to_vec(),
        };
        engine.process_packet(gap_tail1).await.unwrap();
        assert_eq!(engine.get_stream_overlap_conflict_count(0), 0);
        assert_eq!(engine.get_total_stream_overlap_conflict_count(), 0);

        let conflicting_overlap = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 3,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 10,
                data: b"XXmn".to_vec(),
                fin: false,
            })],
            payload: b"XXmn".to_vec(),
        };
        engine.process_packet(conflicting_overlap).await.unwrap();

        assert_eq!(engine.get_stream_overlap_conflict_count(0), 1);
        assert_eq!(engine.get_total_stream_overlap_conflict_count(), 1);

        let pending = engine
            .stream_pending_fragments
            .lock()
            .get(&0)
            .cloned()
            .unwrap_or_default();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, 8);
        assert_eq!(pending[0].1, b"ijklmn");
    }

    #[tokio::test]
    async fn diagnostics_snapshot_reports_overlap_conflict_telemetry() {
        let socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let remote_addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
        let engine = QuicEngine::new(Role::Server, sample_state(), socket, remote_addr, vec![], crate::concurrency::ccek::CoroutineContext::new());

        let snapshot = engine.diagnostics_snapshot();
        assert_eq!(snapshot.total_stream_overlap_conflict_count, 0);
        assert!(snapshot.per_stream_overlap_conflict_counts.is_empty());
        assert!(snapshot.per_stream_pending_fragment_counts.is_empty());
        assert_eq!(snapshot.total_pending_fragment_bytes, 0);
        assert!(snapshot.per_stream_pending_fragment_bytes.is_empty());
        assert!(snapshot.per_stream_contiguous_receive_offsets.is_empty());
        assert!(snapshot.per_stream_highest_seen_receive_offsets.is_empty());

        engine
            .process_packet(sample_stream_packet(1, 0, b"abcd"))
            .await
            .unwrap();

        let gap_tail = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 2,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 8,
                data: b"ijkl".to_vec(),
                fin: false,
            })],
            payload: b"ijkl".to_vec(),
        };
        engine.process_packet(gap_tail).await.unwrap();

        let snapshot = engine.diagnostics_snapshot();
        assert_eq!(
            snapshot.per_stream_pending_fragment_counts.get(&0).copied(),
            Some(1)
        );
        assert_eq!(snapshot.total_pending_fragment_bytes, 4);
        assert_eq!(
            snapshot.per_stream_pending_fragment_bytes.get(&0).copied(),
            Some(4)
        );
        assert_eq!(
            snapshot
                .per_stream_contiguous_receive_offsets
                .get(&0)
                .copied(),
            Some(4)
        );
        assert_eq!(
            snapshot
                .per_stream_highest_seen_receive_offsets
                .get(&0)
                .copied(),
            Some(12)
        );

        let conflicting_overlap = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: ConnectionId { bytes: vec![2; 8] },
                source_connection_id: ConnectionId { bytes: vec![1; 8] },
                packet_number: 3,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 0,
                offset: 10,
                data: b"XXmn".to_vec(),
                fin: false,
            })],
            payload: b"XXmn".to_vec(),
        };
        engine.process_packet(conflicting_overlap).await.unwrap();

        let snapshot = engine.diagnostics_snapshot();
        assert_eq!(snapshot.total_stream_overlap_conflict_count, 1);
        assert_eq!(
            snapshot.per_stream_overlap_conflict_counts.get(&0).copied(),
            Some(1)
        );
        assert_eq!(
            snapshot.per_stream_pending_fragment_counts.get(&0).copied(),
            Some(1)
        );
        assert_eq!(snapshot.total_pending_fragment_bytes, 6);
        assert_eq!(
            snapshot.per_stream_pending_fragment_bytes.get(&0).copied(),
            Some(6)
        );
        assert_eq!(
            snapshot
                .per_stream_contiguous_receive_offsets
                .get(&0)
                .copied(),
            Some(4)
        );
        assert_eq!(
            snapshot
                .per_stream_highest_seen_receive_offsets
                .get(&0)
                .copied(),
            Some(14)
        );
    }
}
