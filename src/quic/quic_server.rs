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

/// Encode a variable-length integer per RFC 9000 §16.
fn encode_varint(val: u64) -> Vec<u8> {
    if val < 64 {
        vec![val as u8]
    } else if val < 16384 {
        vec![0x40 | (val >> 8) as u8, val as u8]
    } else if val < 1_073_741_824 {
        vec![
            0x80 | (val >> 24) as u8,
            (val >> 16) as u8,
            (val >> 8) as u8,
            val as u8,
        ]
    } else {
        vec![
            0xc0 | (val >> 56) as u8,
            (val >> 48) as u8,
            (val >> 40) as u8,
            (val >> 32) as u8,
            (val >> 24) as u8,
            (val >> 16) as u8,
            (val >> 8) as u8,
            val as u8,
        ]
    }
}

/// Build a minimal QPACK-encoded HEADERS block with :status 200 and content-type.
/// Uses QPACK static table entries (no dynamic table, no Huffman).
fn build_h3_headers_block(content_type: &str) -> Vec<u8> {
    // QPACK Required Insert Count = 0, S=0, Delta Base = 0
    let mut block = vec![0x00, 0x00];
    // :status 200 — static table index 25 (0-indexed), encoded as indexed field line
    // Static table ref: bit pattern 0b11xxxxxx, index 25 => 0xd9
    block.push(0xd9);
    // content-type: <value> — literal with static name ref
    // content-type is static index 31 (0-indexed)
    // Literal field line with name reference: 0b0101_xxxx, static=1, idx=31 => 0x5f 0x00 (idx 31 = 0x1f needs two bytes)
    // Simpler: use literal with literal name (no Huffman)
    // Format: 0b00100000 = literal field line with literal name, no huffman
    block.push(0x37); // literal field line with static name ref, index=23 (content-type in QPACK static table)
    // name index 23 for content-type in QPACK (RFC 9204 Appendix A)
    // Actually let's just encode content-type as literal name + literal value
    // 0b0010_0000 = Literal Field Line With Literal Name, no Huffman
    // Remove last byte and do it properly:
    block.pop();
    // Literal Field Line With Literal Name (0x20), no Huffman on name, no Huffman on value
    block.push(0x37); // 0x37 = Literal Field Line With Name Reference, static, index 23 (content-type)
    let ct_bytes = content_type.as_bytes();
    block.extend_from_slice(&encode_varint(ct_bytes.len() as u64));
    block.extend_from_slice(ct_bytes);
    block
}

/// Wrap body in HTTP/3 HEADERS frame (type=0x01) + DATA frame (type=0x00).
fn build_h3_response(content_type: &str, body: &[u8]) -> Vec<u8> {
    let headers_block = build_h3_headers_block(content_type);

    let mut out = Vec::new();
    // HEADERS frame: type=0x01, length=varint, payload=headers_block
    out.extend_from_slice(&encode_varint(0x01));
    out.extend_from_slice(&encode_varint(headers_block.len() as u64));
    out.extend_from_slice(&headers_block);

    // DATA frame: type=0x00, length=varint, payload=body
    out.extend_from_slice(&encode_varint(0x00));
    out.extend_from_slice(&encode_varint(body.len() as u64));
    out.extend_from_slice(body);

    out
}

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
            println!("🔓 Not an Initial packet - not long header");
            return None;
        }

        // Parse header fields (everything before the encrypted payload)
        let mut pos = 0usize;

        let first_byte = packet_data[pos];
        pos += 1;

        // Only handle Initial packets (type bits = 0b00 in bits 5-4)
        let packet_type_bits = (first_byte >> 5) & 0x03;
        if packet_type_bits != 0 {
            println!("🔓 Not Initial packet - type bits = {}", packet_type_bits);
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

        let crypto_state = match QuicCryptoState::new(
            QuicAeadAlgorithm::Aes128Gcm,
            &key_bytes,
            iv_bytes,
            &hp_key_bytes,
        ) {
            Ok(s) => s,
            Err(e) => {
                println!("❌ Failed to create crypto state: {:?}", e);
                return None;
            }
        };

        // Header protection removal
        // sample = ciphertext[4..20] (4 bytes after start of encrypted packet number field)
        let sample_offset = pn_offset + 4;
        if sample_offset + 16 > packet_data.len() {
            println!("❌ Sample offset out of bounds");
            return None;
        }
        let sample = &packet_data[sample_offset..sample_offset + 16];
        let mask = match crypto_state.generate_header_protection_mask(sample) {
            Ok(m) => m,
            Err(e) => {
                println!("❌ HP mask failed: {:?}", e);
                return None;
            }
        };

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
        let plaintext = match crypto_state.decrypt_payload(packet_number, &aad, &mut ciphertext_and_tag) {
            Ok(p) => p,
            Err(e) => {
                println!("❌ Decrypt failed: {:?}", e);
                return None;
            }
        };

        println!("✅ CCEK: Decrypted {} bytes of plaintext frames", plaintext.len());

        // Parse frames from plaintext
        let frames = match decode_frames(plaintext) {
            Ok(f) => f,
            Err(e) => {
                println!("❌ Decode frames failed: {:?}", e);
                return None;
            }
        };

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

    #[cfg(feature = "tls-quic")]
    /// Attempt to decrypt a 1-RTT packet (short header, bit7=0).
    /// Uses onertt_remote keys from rustls to decrypt client→server 1-RTT packets.
    fn try_decrypt_1rtt_packet(
        packet_data: &[u8],
        dcid_len: usize,
        crypto_provider: &Arc<dyn crate::quic::quic_crypto::QuicCryptoProvider>,
    ) -> Option<DecodedQuicPacket> {
        use crate::quic::quic_crypto::EncryptionLevel;

        // Must be a short header packet (bit7=0)
        if packet_data.is_empty() || (packet_data[0] & 0x80) != 0 {
            return None;
        }

        // Parse short header: first_byte | dcid | payload
        let mut pos = 0usize;
        let first_byte = packet_data[pos];
        pos += 1;

        // Extract DCID
        if pos + dcid_len > packet_data.len() { return None; }
        let dcid = packet_data[pos..pos+dcid_len].to_vec();
        pos += dcid_len;

        let pn_offset = pos;

        // Determine initial pn_len from protected first byte (low 2 bits)
        let pn_len_bits = first_byte & 0x03;
        let pn_len = (pn_len_bits + 1) as usize;
        if pn_offset + pn_len > packet_data.len() { return None; }

        // Need at least pn_len + 16-byte tag
        let payload_len = packet_data.len() - pn_offset;
        if payload_len < pn_len + 16 { return None; }

        // Header protection removal
        let sample_offset = pn_offset + 4;
        if sample_offset + 16 > packet_data.len() {
            println!("❌ Sample offset out of bounds for 1-RTT");
            return None;
        }
        let sample = &packet_data[sample_offset..sample_offset + 16];
        let mut unprotected_first = first_byte;
        let mut pn_bytes = [0u8; 4];
        for i in 0..pn_len {
            pn_bytes[i] = packet_data[pn_offset + i];
        }

        if crypto_provider.remove_header_protection(EncryptionLevel::OneRtt, sample, &mut unprotected_first, &mut pn_bytes[..pn_len]).is_err() {
            println!("❌ Failed to remove 1-RTT header protection");
            return None;
        }

        let pn_len = ((unprotected_first & 0x03) + 1) as usize;
        if pn_offset + pn_len > packet_data.len() { return None; }

        // pn_bytes are now unmasked, extract packet number
        let mut packet_number: u64 = 0;
        for i in 0..pn_len {
            packet_number = (packet_number << 8) | pn_bytes[i] as u64;
        }

        // Build AAD
        let mut aad = Vec::with_capacity(pn_offset + pn_len);
        aad.push(unprotected_first);
        aad.extend_from_slice(&packet_data[1..pn_offset]);
        for i in 0..pn_len {
            aad.push(pn_bytes[i]);
        }

        // Ciphertext + tag
        let ct_start = pn_offset + pn_len;
        let ct_end = packet_data.len();
        if ct_start > ct_end { return None; }

        let mut ciphertext_and_tag = packet_data[ct_start..ct_end].to_vec();

        // Try to decrypt using 1-RTT remote keys
        let plaintext = match crypto_provider.decrypt_packet(EncryptionLevel::OneRtt, packet_number, &aad, &mut ciphertext_and_tag) {
            Ok(p) => p,
            Err(e) => {
                println!("❌ 1-RTT decrypt failed: {:?}", e);
                return None;
            }
        };

        println!("✅ Decrypted 1-RTT packet: {} bytes of frames", plaintext.len());

        // Parse frames from plaintext
        let frames = match decode_frames(&plaintext) {
            Ok(f) => f,
            Err(e) => {
                println!("❌ Decode 1-RTT frames failed: {:?}", e);
                return None;
            }
        };

        Some(DecodedQuicPacket {
            packet: QuicPacket {
                header: QuicHeader {
                    r#type: QuicPacketType::ShortHeader,
                    version: 1,
                    destination_connection_id: ConnectionId { bytes: dcid },
                    source_connection_id: ConnectionId { bytes: Vec::new() }, // Short headers don't have SCID
                    packet_number,
                    token: None,
                },
                frames,
                payload: plaintext,
            },
            encoded_packet_number_len: pn_len,
        })
    }

    #[cfg(feature = "tls-quic")]
    /// Attempt to decrypt a QUIC Handshake packet (long header, type 0x02).
    /// Uses the handshake_remote keys from rustls to decrypt client→server Handshake packets.
    fn try_decrypt_handshake_packet(
        packet_data: &[u8],
        crypto_provider: &Arc<dyn crate::quic::quic_crypto::QuicCryptoProvider>,
    ) -> Option<DecodedQuicPacket> {
        use crate::quic::quic_crypto::EncryptionLevel;

        // Must be a long header packet
        if packet_data.is_empty() || (packet_data[0] & 0x80) == 0 {
            return None;
        }

        // Parse header fields
        let mut pos = 0usize;
        let first_byte = packet_data[pos];
        pos += 1;

        // Check if Handshake packet (type bits = 0b10 in bits 5-4)
        let packet_type_bits = (first_byte >> 5) & 0x03;
        if packet_type_bits != 2 {
            return None; // not Handshake
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

        // Handshake packets have no token, so proceed directly to payload_length
        let (payload_length, plen_varint_len) = Self::read_varint(packet_data, pos)?;
        pos += plen_varint_len;

        let pn_offset = pos;
        let payload_length = payload_length as usize;

        if pn_offset + payload_length > packet_data.len() { return None; }
        if payload_length < 4 + 16 { return None; } // need at least pn(1-4) + tag(16)

        // First, determine pn_len from the protected first byte (low 2 bits)
        let pn_len_bits = first_byte & 0x03;
        let initial_pn_len = (pn_len_bits + 1) as usize;
        if pn_offset + initial_pn_len > packet_data.len() { return None; }

        // Header protection removal using handshake_remote keys
        let sample_offset = pn_offset + 4;
        if sample_offset + 16 > packet_data.len() {
            println!("❌ Sample offset out of bounds for Handshake");
            return None;
        }
        let sample = &packet_data[sample_offset..sample_offset + 16];
        let mut unprotected_first = first_byte;
        let mut pn_bytes = [0u8; 4];
        for i in 0..initial_pn_len {
            pn_bytes[i] = packet_data[pn_offset + i];
        }

        // Try to remove header protection using crypto provider
        if crypto_provider.remove_header_protection(EncryptionLevel::Handshake, sample, &mut unprotected_first, &mut pn_bytes[..initial_pn_len]).is_err() {
            println!("❌ Failed to remove Handshake header protection");
            return None;
        }

        let pn_len = ((unprotected_first & 0x03) + 1) as usize;
        if pn_offset + pn_len > packet_data.len() { return None; }

        // pn_bytes are now unmasked
        let mut packet_number: u64 = 0;
        for i in 0..pn_len {
            packet_number = (packet_number << 8) | pn_bytes[i] as u64;
        }

        // Build unprotected AAD
        let mut aad = Vec::with_capacity(pn_offset + pn_len);
        aad.push(unprotected_first);
        aad.extend_from_slice(&packet_data[1..pn_offset]);
        for i in 0..pn_len {
            aad.push(pn_bytes[i]);
        }

        // Ciphertext + tag
        let ct_start = pn_offset + pn_len;
        let ct_end = pn_offset + payload_length;
        if ct_end > packet_data.len() { return None; }

        let mut ciphertext_and_tag = packet_data[ct_start..ct_end].to_vec();

        // Try to decrypt using handshake_remote keys
        let plaintext = match crypto_provider.decrypt_packet(EncryptionLevel::Handshake, packet_number, &aad, &mut ciphertext_and_tag) {
            Ok(p) => p,
            Err(e) => {
                println!("❌ Handshake decrypt failed: {:?}", e);
                return None;
            }
        };

        println!("✅ Decrypted Handshake packet: {} bytes of frames", plaintext.len());

        // Parse frames from plaintext
        let frames = match decode_frames(&plaintext) {
            Ok(f) => f,
            Err(e) => {
                println!("❌ Decode Handshake frames failed: {:?}", e);
                return None;
            }
        };

        Some(DecodedQuicPacket {
            packet: QuicPacket {
                header: QuicHeader {
                    r#type: QuicPacketType::Handshake,
                    version,
                    destination_connection_id: ConnectionId { bytes: dcid },
                    source_connection_id: ConnectionId { bytes: scid },
                    packet_number,
                    token: None,
                },
                frames,
                payload: plaintext,
            },
            encoded_packet_number_len: pn_len,
        })
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

                                // Try QUIC Initial/Handshake packet decryption (RFC 9001)
                                // Falls back to raw deserialize for non-Initial/Handshake or already-decrypted packets
                                #[cfg(feature = "tls-quic")]
                                let decoded_result = if (packet_data[0] & 0x80) != 0 && ((packet_data[0] >> 4) & 0x03) == 0 {
                                    // Long header Initial packet — decrypt first
                                    Self::try_decrypt_initial_packet(packet_data)
                                        .ok_or_else(|| ProtocolError::InvalidPacket(
                                            "Initial packet decryption failed".into()
                                        ))
                                } else if (packet_data[0] & 0x80) != 0 && ((packet_data[0] >> 4) & 0x03) == 2 {
                                    // Long header Handshake packet — try decryption with engine keys
                                    if let Some(engine_arc) = connections.lock().get(&remote_addr).cloned() {
                                        Self::try_decrypt_handshake_packet(packet_data, &engine_arc.get_crypto_provider())
                                            .ok_or_else(|| ProtocolError::InvalidPacket(
                                                "Handshake packet decryption failed".into()
                                            ))
                                    } else {
                                        println!("⚠️  Handshake packet received but no engine found, trying unencrypted");
                                        let short_header_dcid_len = None;
                                        deserialize_decoded_packet_with_dcid_len(packet_data, short_header_dcid_len)
                                    }
                                } else if (packet_data[0] & 0x80) == 0 {
                                    // Short header packet — try 1-RTT decryption
                                    println!("📦 Short header packet detected, {} bytes", packet_data.len());
                                    if let Some(engine_arc) = connections.lock().get(&remote_addr).cloned() {
                                        let dcid_len = engine_arc.get_state().local_connection_id.bytes.len();
                                        println!("📦 Attempting 1-RTT decryption with DCID len={}", dcid_len);
                                        match Self::try_decrypt_1rtt_packet(packet_data, dcid_len, &engine_arc.get_crypto_provider()) {
                                            Some(decoded) => {
                                                println!("✅ 1-RTT decryption succeeded");
                                                Ok(decoded)
                                            }
                                            None => {
                                                println!("⚠️  1-RTT decryption failed, not attempting unencrypted parse for 1-RTT");
                                                Err(ProtocolError::InvalidPacket("1-RTT decryption failed".into()))
                                            }
                                        }
                                    } else {
                                        println!("⚠️  Short header packet but no engine found");
                                        let short_header_dcid_len = None;
                                        deserialize_decoded_packet_with_dcid_len(packet_data, short_header_dcid_len)
                                    }
                                } else {
                                    // Other long-header packet types
                                    let short_header_dcid_len = None;
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

                                        // Get the client's SCID from the packet header for use in remote_connection_id
                                        let client_scid = received_packet.header.source_connection_id.bytes.clone();
                                        let client_dcid = received_packet.header.destination_connection_id.bytes.clone();

                                        let engine_arc = {
                                            let mut connections_guard = connections.lock();
                                            connections_guard
                                                .entry(remote_addr)
                                                .or_insert_with(|| {
                                                    // Create a new engine for this connection
                                                    let local_conn_id = ConnectionId {
                                                        // Server's CID - what client uses to reach us
                                                        bytes: client_dcid.clone(),
                                                    };
                                                    let remote_conn_id = ConnectionId {
                                                        // Client's CID - what we use to reach the client
                                                        bytes: client_scid.clone(),
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
                                                        client_dcid.clone(),
                                                        ctx.clone(),
                                                    ))
                                                })
                                                .clone()
                                        };

                                        match engine_arc.process_decoded_packet(decoded_packet).await {
                                            Ok(()) => {
                                                for frame in received_packet.frames.iter() {
                                                    if let QuicFrame::Stream(stream_frame) = frame {
                                                        // Only respond to stream 0 (HTTP request), not stream 2 (QPACK)
                                                        if stream_frame.stream_id != 0 {
                                                            println!("📄 Ignoring stream {} (QPACK)", stream_frame.stream_id);
                                                            continue;
                                                        }
                                                        let data_str = String::from_utf8_lossy(&stream_frame.data);
                                                        println!("📄 Server received request on stream {} ({} bytes): {}", stream_frame.stream_id, stream_frame.data.len(), data_str);

                                                        // For now, always serve the PNG as a quick test
                                                        // TODO: properly decode HTTP/3 QPACK headers to determine the path
                                                        let body = std::fs::read("bw_test_pattern.png")
                                                            .unwrap_or_else(|_| b"image not found".to_vec());
                                                        println!("🖼️ Serving bw_test_pattern.png ({} bytes)", body.len());
                                                        let (body, content_type) = (body, "image/png");

                                                        // Wrap in HTTP/3 HEADERS (200 OK) + DATA frames
                                                        let response_data = build_h3_response(content_type, &body);

                                                        if !response_data.is_empty() {
                                                            let total_chunks = (response_data.len() + 4095) / 4096;
                                                            println!("📤 Sending {} bytes in {} chunks on stream {} (HTTP/3 framed)",
                                                                response_data.len(), total_chunks, stream_frame.stream_id);

                                                            for chunk in response_data.chunks(4096) {
                                                                if let Err(e) = engine_arc.send_stream_data(stream_frame.stream_id, chunk.to_vec()).await {
                                                                    println!("❌ Failed to send chunk: {:?}", e);
                                                                }
                                                            }
                                                            println!("✅ Sent HTTP/3 200 response ({} bytes body) on stream {}",
                                                                body.len(), stream_frame.stream_id);
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

                        // Call send_handshake_responses for any packet that might have crypto data
                        // This ensures we respond to handshake packets from the client
                        if let Some(eng) = connections.lock().get(&remote_addr).cloned() {
                            if let Err(e) = eng.send_handshake_responses().await {
                                println!("❌ Handshake send failed: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ recv_from error: {:?}", e);
                        break;
                    }
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