use super::quic_engine::{QuicEngine, Role};
use super::quic_error::QuicError;
use super::quic_failure_log as qfail;
use super::quic_protocol::{
    deserialize_decoded_packet_with_dcid_len, ConnectionId, ConnectionState,
    QuicConnectionState, QuicFrame, StreamFrame, TransportParameters,
};
#[cfg(feature = "tls-quic")]
use super::quic_error::ProtocolError;
#[cfg(feature = "tls-quic")]
use super::quic_protocol::{
    decode_frames, DecodedQuicPacket, QuicHeader, QuicPacket, QuicPacketType,
};
#[cfg(feature = "tls-quic")]
use super::tls_crypto::packet_protection::{QuicAeadAlgorithm, QuicCryptoState};
#[cfg(feature = "tls-quic")]
use super::tls_crypto::secrets::{derive_initial_secrets, derive_packet_protection_keys};
use crate::rbcursive::{NetTuple, Protocol as RbProtocol, RbCursor, Signal as RbSignal};
use parking_lot::Mutex;
use socket2::{Domain, Socket, Type};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket as TokioUdpSocket;

pub struct QuicServer {
    socket: Arc<TokioUdpSocket>,
    rb: Arc<Mutex<RbCursor>>,
    connections: Arc<Mutex<HashMap<SocketAddr, Arc<QuicEngine>>>>,
    ctx: crate::concurrency::ccek::CoroutineContext,
}

impl QuicServer {
    // CCEK: Extract DCID from first bytes of QUIC packet (no decryption needed for this part)
    fn extract_dcid_from_long_header(bytes: &[u8]) -> Option<Vec<u8>> {
        if bytes.is_empty() || (bytes[0] & 0x80) == 0 {
            return None;
        }
        if bytes.len() < 6 {
            return None;
        }
        let dcid_len = bytes[5] as usize;
        if bytes.len() < 6 + dcid_len {
            return None;
        }
        Some(bytes[6..6 + dcid_len].to_vec())
    }

    #[cfg(feature = "tls-quic")]
    /// Attempt to decrypt a QUIC Initial packet using RFC 9001 key derivation.
    /// Returns a DecodedQuicPacket on success, or None if the packet is not an Initial packet
    /// or decryption fails.
    fn try_decrypt_initial_packet(packet_data: &[u8]) -> Option<DecodedQuicPacket> {
        // Must be a long header packet
        if packet_data.is_empty() || (packet_data[0] & 0x80) == 0 {
            return None;
        }

        // Parse header fields (everything before the encrypted payload)
        let mut pos = 0usize;

        let first_byte = packet_data[pos];
        pos += 1;

        // Only handle Initial packets (type bits = 0b00 in bits 5-4)
        let packet_type_bits = (first_byte >> 4) & 0x03;
        if packet_type_bits != 0 {
            return None; // not Initial
        }

        // version (4 bytes)
        if pos + 4 > packet_data.len() { return None; }
        let version = u32::from_be_bytes(packet_data[pos..pos+4].try_into().ok()?) as u64;
        pos += 4;

        // dcid
        if pos >= packet_data.len() { return None; }
        let dcid_len = packet_data[pos] as usize;
        pos += 1;
        if pos + dcid_len > packet_data.len() { return None; }
        let dcid = packet_data[pos..pos+dcid_len].to_vec();
        pos += dcid_len;

        // scid
        if pos >= packet_data.len() { return None; }
        let scid_len = packet_data[pos] as usize;
        pos += 1;
        if pos + scid_len > packet_data.len() { return None; }
        let scid = packet_data[pos..pos+scid_len].to_vec();
        pos += scid_len;

        // token (Initial packets only)
        let (token_len, token_varint_len) = Self::read_varint(packet_data, pos)?;
        pos += token_varint_len;
        if pos + token_len as usize > packet_data.len() { return None; }
        let token = packet_data[pos..pos + token_len as usize].to_vec();
        pos += token_len as usize;

        // payload_length varint (includes pn + ciphertext + 16-byte tag)
        let (payload_length, plen_varint_len) = Self::read_varint(packet_data, pos)?;
        pos += plen_varint_len;

        let pn_offset = pos;
        let payload_length = payload_length as usize;

        if pn_offset + payload_length > packet_data.len() { return None; }
        if payload_length < 4 + 16 { return None; } // need at least pn(1-4) + tag(16)

        // Derive keys from CLIENT initial secret (we're the server receiving client data)
        let (client_secret, _) = derive_initial_secrets(&dcid);
        let (key_bytes, iv_bytes, hp_key_bytes) = derive_packet_protection_keys(&client_secret);

        println!("🔓 CCEK: Decrypting Initial, DCID {:02x?}, key={}, iv={}, hp={}",
            &dcid[..dcid.len().min(8)],
            hex::encode(&key_bytes[..4]),
            hex::encode(&iv_bytes[..4]),
            hex::encode(&hp_key_bytes[..4]));

        let crypto_state = QuicCryptoState::new(
            QuicAeadAlgorithm::Aes128Gcm,
            &key_bytes,
            iv_bytes,
            &hp_key_bytes,
        ).ok()?;

        // Header protection removal
        // sample = ciphertext[4..20] (4 bytes after start of encrypted packet number field)
        let sample_offset = pn_offset + 4;
        if sample_offset + 16 > packet_data.len() { return None; }
        let sample = &packet_data[sample_offset..sample_offset + 16];
        let mask = crypto_state.generate_header_protection_mask(sample).ok()?;

        // Unprotect first byte (long header: unmask low 4 bits)
        let unprotected_first = first_byte ^ (mask[0] & 0x0f);
        let pn_len = ((unprotected_first & 0x03) + 1) as usize;

        if pn_offset + pn_len > packet_data.len() { return None; }

        // Unmask packet number bytes
        let mut pn_bytes = [0u8; 4];
        for i in 0..pn_len {
            pn_bytes[i] = packet_data[pn_offset + i] ^ mask[1 + i];
        }
        let mut packet_number: u64 = 0;
        for i in 0..pn_len {
            packet_number = (packet_number << 8) | pn_bytes[i] as u64;
        }

        // Build unprotected AAD = header bytes up through packet number (inclusive)
        let mut aad = Vec::with_capacity(pn_offset + pn_len);
        aad.push(unprotected_first);
        aad.extend_from_slice(&packet_data[1..pn_offset]);
        for i in 0..pn_len {
            aad.push(pn_bytes[i]);
        }

        // Ciphertext + tag starts after the packet number field
        let ct_start = pn_offset + pn_len;
        let ct_end = pn_offset + payload_length; // includes 16-byte tag
        if ct_end > packet_data.len() { return None; }

        let mut ciphertext_and_tag = packet_data[ct_start..ct_end].to_vec();

        // AEAD decrypt in place
        let plaintext = crypto_state.decrypt_payload(packet_number, &aad, &mut ciphertext_and_tag).ok()?;

        println!("✅ CCEK: Decrypted {} bytes of plaintext frames", plaintext.len());

        // Parse frames from plaintext
        let frames = decode_frames(plaintext).ok()?;

        Some(DecodedQuicPacket {
            packet: QuicPacket {
                header: QuicHeader {
                    r#type: QuicPacketType::Initial,
                    version,
                    destination_connection_id: ConnectionId { bytes: dcid },
                    source_connection_id: ConnectionId { bytes: scid },
                    packet_number,
                    token: Some(token),
                },
                frames,
                payload: plaintext.to_vec(),
            },
            encoded_packet_number_len: pn_len,
        })
    }

    #[cfg(feature = "tls-quic")]
    /// Minimal variable-length integer decoder per RFC 9000 §16.
    /// Returns (value, bytes_consumed) or None on underflow.
    fn read_varint(buf: &[u8], pos: usize) -> Option<(u64, usize)> {
        if pos >= buf.len() { return None; }
        let prefix = buf[pos] >> 6;
        let byte_len = 1 << prefix;
        if pos + byte_len > buf.len() { return None; }
        let mut val: u64 = (buf[pos] & 0x3f) as u64;
        for i in 1..byte_len {
            val = (val << 8) | buf[pos + i] as u64;
        }
        Some((val, byte_len))
    }

    pub async fn bind(addr: SocketAddr, ctx: crate::concurrency::ccek::CoroutineContext) -> Result<Self, QuicError> {
        let socket =
            Socket::new(Domain::for_address(addr), Type::DGRAM, None).map_err(QuicError::Io)?;

        socket.set_reuse_address(true).map_err(QuicError::Io)?;
        #[cfg(unix)]
        socket.set_reuse_port(true).map_err(QuicError::Io)?;

        socket.bind(&addr.into()).map_err(QuicError::Io)?;

        let std_socket: std::net::UdpSocket = socket.into();
        std_socket.set_nonblocking(true).map_err(QuicError::Io)?;

        let tokio_socket = TokioUdpSocket::from_std(std_socket).map_err(QuicError::Io)?;

        Ok(Self {
            socket: Arc::new(tokio_socket),
            rb: Arc::new(Mutex::new(RbCursor::new())),
            connections: Arc::new(Mutex::new(HashMap::new())),
            ctx,
        })
    }

    pub async fn start(&self) -> Result<(), QuicError> {
        let socket = self.socket.clone();
        let connections = self.connections.clone();
        let rb_cursor = self.rb.clone();
        let ctx = self.ctx.clone();

        // Spawn the UDP receive loop locally — the captured `RbCursor` is not
        // `Send` (contains raw pointers), so we use a local task. Callers must
        // run `start()` inside a `tokio::task::LocalSet` or equivalent.
        tokio::task::spawn_local(async move {
            let mut buf = vec![0u8; 65536];
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, remote_addr)) => {
                        println!("📡 Received {} bytes from {}", len, remote_addr);
                        let packet_data = &buf[..len];

                        // RbCursive preflight
                        let tuple = NetTuple::from_socket_addr(remote_addr, RbProtocol::CustomQuic);
                        let hint = if packet_data.len() > 0 {
                            vec![packet_data[0]]
                        } else {
                            vec![]
                        };
                        let signal = rb_cursor.lock().recognize(tuple, &hint);
                        println!("🔍 RbCursive signal: {:?}", signal);

                        match signal {
                            RbSignal::Accept(proto) => {
                                println!("✅ Accepted protocol: {:?}", proto);

                                // Try QUIC Initial packet decryption (RFC 9001)
                                // Falls back to raw deserialize for non-Initial or already-decrypted packets
                                #[cfg(feature = "tls-quic")]
                                let decoded_result = if (packet_data[0] & 0x80) != 0 && ((packet_data[0] >> 4) & 0x03) == 0 {
                                    // Long header Initial packet — decrypt first
                                    Self::try_decrypt_initial_packet(packet_data)
                                        .ok_or_else(|| ProtocolError::InvalidPacket(
                                            "Initial packet decryption failed".into()
                                        ))
                                } else {
                                    let short_header_dcid_len =
                                        connections.lock().get(&remote_addr).map(|engine| {
                                            println!("🔧 Found existing engine for {}", remote_addr);
                                            engine.get_state().local_connection_id.bytes.len()
                                        });
                                    deserialize_decoded_packet_with_dcid_len(packet_data, short_header_dcid_len)
                                };
                                #[cfg(not(feature = "tls-quic"))]
                                let decoded_result = {
                                    let short_header_dcid_len =
                                        connections.lock().get(&remote_addr).map(|engine| {
                                            engine.get_state().local_connection_id.bytes.len()
                                        });
                                    deserialize_decoded_packet_with_dcid_len(packet_data, short_header_dcid_len)
                                };

                                match decoded_result {
                                    Ok(decoded_packet) => {
                                        println!("📦 Deserialized packet OK");
                                        let received_packet = decoded_packet.packet.clone();
                                        println!("📦 Deserialized packet with {} frames", received_packet.frames.len());
                                        let engine_arc = {
                                            let mut connections_guard = connections.lock();
                                            connections_guard
                                                .entry(remote_addr)
                                                .or_insert_with(|| {
                                                    // Create a new engine for this connection
                                                    let local_conn_id = ConnectionId {
                                                        bytes: vec![1, 2, 3, 4, 5, 6, 7, 8],
                                                    };
                                                    let remote_conn_id = ConnectionId {
                                                        bytes: vec![8, 7, 6, 5, 4, 3, 2, 1],
                                                    };

                                                    let initial_state = QuicConnectionState {
                                                        local_connection_id: local_conn_id,
                                                        remote_connection_id: remote_conn_id,
                                                        version: 1,
                                                        transport_params:
                                                            TransportParameters::default(),
                                                        streams: Vec::new(),
                                                        sent_packets: Vec::new(),
                                                        received_packets: Vec::new(),
                                                        next_packet_number: 0,
                                                        next_stream_id: 0,
                                                        congestion_window: 14720,
                                                        bytes_in_flight: 0,
                                                        rtt: 100,
                                                        connection_state:
                                                            ConnectionState::Handshaking,
                                                    };
                                                    println!("🔧 Creating QuicEngine, context keys: {:?}", ctx.keys());
                                                    Arc::new(QuicEngine::new(
                                                        Role::Server,
                                                        initial_state,
                                                        socket.clone(),
                                                        remote_addr,
                                                        vec![0; 32],
                                                        ctx.clone(),
                                                    ))
                                                })
                                                .clone()
                                        };

                                        match engine_arc.process_decoded_packet(decoded_packet).await {
                                            Ok(()) => {
                                                for frame in received_packet.frames.iter() {
                                                    if let QuicFrame::Stream(stream_frame) = frame {
                                                        let data_str = String::from_utf8_lossy(&stream_frame.data);
                                                        println!("📄 Server received request on stream {}: {}", stream_frame.stream_id, data_str);

                                                        let response_data = if data_str.contains("index.css") {
                                                            match std::fs::read("index.css") {
                                                                Ok(d) => d,
                                                                Err(e) => { b"/* css not found */".to_vec() }
                                                            }
                                                        } else if data_str.contains("bw_test_pattern.png") {
                                                            match std::fs::read("bw_test_pattern.png") {
                                                                Ok(d) => {
                                                                    println!("🖼️ Serving bw_test_pattern.png ({} bytes)", d.len());
                                                                    d
                                                                }
                                                                Err(e) => { b"image not found".to_vec() }
                                                            }
                                                        } else {
                                                            match std::fs::read("index.html") {
                                                                Ok(d) => d,
                                                                Err(e) => { b"<!doctype html><html>not found</html>".to_vec() }
                                                            }
                                                        };

                                                        if !response_data.is_empty() {
                                                            let total_chunks = (response_data.len() + 4095) / 4096;
                                                            println!("📤 Sending {} bytes in {} chunks on stream {}",
                                                                response_data.len(), total_chunks, stream_frame.stream_id);

                                                            for chunk in response_data.chunks(4096) {
                                                                if let Err(e) = engine_arc.send_stream_data(stream_frame.stream_id, chunk.to_vec()).await {
                                                                    println!("❌ Failed to send chunk: {:?}", e);
                                                                }
                                                            }
                                                            println!("✅ Sent response ({} bytes) in {} chunks on stream {}",
                                                                response_data.len(), total_chunks, stream_frame.stream_id);
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                println!("❌ Failed to process packet: {:?}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        println!("❌ Deserialize error: {:?}", e);
                                    }
                                }
                            }
                            other => {
                                println!("🔍 RbCursive non-accept signal: {:?}", other);
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        println!("   Press Ctrl+C to stop");
        tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        println!("\n🛑 Shutting down server...");
        self.close().await;
        Ok(())
    }

    pub fn local_addr(&self) -> Result<SocketAddr, QuicError> {
        self.socket.local_addr().map_err(QuicError::Io)
    }

    pub async fn accept(&self) -> Option<Arc<QuicEngine>> {
        let connections = self.connections.lock();
        connections.values().next().cloned()
    }

    pub async fn close(&self) {
        // Shutdown handled by drop or external signals
    }
}