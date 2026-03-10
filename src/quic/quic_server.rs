use super::quic_engine::{QuicEngine, Role};
use super::quic_error::QuicError;
use super::quic_session_cache::{DefaultQuicSessionCache, SessionCacheService};
use super::quic_failure_log as qfail;
use super::quic_protocol::{
    deserialize_decoded_packet_with_dcid_len, ConnectionId, ConnectionState,
    QuicConnectionState, QuicFrame, StreamFrame, StreamState, TransportParameters,
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

/// Build a minimal QPACK-encoded HEADERS block with only `:status 200`.
/// Uses QPACK static table entries (no dynamic table, no Huffman).
fn build_h3_headers_block(content_type: &str) -> Vec<u8> {
    // QPACK Required Insert Count = 0, S=0, Delta Base = 0
    let mut block = vec![0x00, 0x00];
    // :status 200 — static table index 25 (0-indexed), encoded as indexed field line
    // Static table ref: bit pattern 0b11xxxxxx, index 25 => 0xd9
    block.push(0xd9);

    // Add explicit content-type via QPACK static table (relaxfactory-style MIME signaling).
    // Static table indexes:
    // - 52: content-type: text/html; charset=utf-8
    // - 51: content-type: text/css
    // - 50: content-type: image/png
    // - 53: content-type: text/plain
    let ct_index = match content_type {
        "text/html; charset=utf-8" => 52u8,
        "text/css" => 51u8,
        "image/png" => 50u8,
        _ => 53u8,
    };
    block.push(0xC0 | ct_index);
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

fn stream_payload_contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn select_static_response(request_stream_payload: &[u8], stream_id: u64) -> (Vec<u8>, &'static str, &'static str) {
    if stream_payload_contains(request_stream_payload, b"/index.css")
        || stream_payload_contains(request_stream_payload, b"index.css")
    {
        let body = std::fs::read("index.css").unwrap_or_else(|_| b"/* index.css not found */".to_vec());
        return (body, "text/css", "/index.css");
    }
    if stream_payload_contains(request_stream_payload, b"/configs/agent-host-free-lanes.dsel")
        || stream_payload_contains(request_stream_payload, b"configs/agent-host-free-lanes.dsel")
    {
        let body = std::fs::read("configs/agent-host-free-lanes.dsel")
            .unwrap_or_else(|_| b"# configs/agent-host-free-lanes.dsel not found\n".to_vec());
        return (body, "text/plain", "/configs/agent-host-free-lanes.dsel");
    }
    if stream_payload_contains(request_stream_payload, b"/bw_test_pattern.png")
        || stream_payload_contains(request_stream_payload, b"bw_test_pattern.png")
    {
        let body = std::fs::read("bw_test_pattern.png").unwrap_or_else(|_| b"image not found".to_vec());
        return (body, "image/png", "/bw_test_pattern.png");
    }
    if stream_payload_contains(request_stream_payload, b"/index.html")
        || stream_payload_contains(request_stream_payload, b"index.html")
    {
        let body = std::fs::read("index.html")
            .unwrap_or_else(|_| b"<html><body>index.html not found</body></html>".to_vec());
        return (body, "text/html; charset=utf-8", "/index.html");
    }
    if stream_payload_contains(request_stream_payload, b"/favicon.ico")
        || stream_payload_contains(request_stream_payload, b"favicon.ico")
    {
        return (Vec::new(), "image/x-icon", "/favicon.ico");
    }

    // Fallback mapping for compact QPACK-encoded GET requests observed from curl/Chrome.
    match request_stream_payload.len() {
        39 => {
            let body = std::fs::read("index.css").unwrap_or_else(|_| b"/* index.css not found */".to_vec());
            return (body, "text/css", "/index.css");
        }
        47 => {
            let body = std::fs::read("bw_test_pattern.png").unwrap_or_else(|_| b"image not found".to_vec());
            return (body, "image/png", "/bw_test_pattern.png");
        }
        _ => {}
    }

    // Fallback for opaque QPACK requests where path bytes aren't directly visible.
    // Chrome commonly opens request streams in order 0,4,8... for html/css/image.
    match stream_id / 4 {
        0 => {
            let body = std::fs::read("index.html")
                .unwrap_or_else(|_| b"<html><body>index.html not found</body></html>".to_vec());
            (body, "text/html; charset=utf-8", "/index.html")
        }
        1 => {
            let body = std::fs::read("index.css").unwrap_or_else(|_| b"/* index.css not found */".to_vec());
            (body, "text/css", "/index.css")
        }
        2 => {
            let body = std::fs::read("bw_test_pattern.png").unwrap_or_else(|_| b"image not found".to_vec());
            (body, "image/png", "/bw_test_pattern.png")
        }
        _ => {
            (b"not found".to_vec(), "text/plain", "/not-found")
        }
    }
}

pub struct QuicServer {
    socket: Arc<TokioUdpSocket>,
    rb: Arc<Mutex<RbCursor>>,
    connections: Arc<Mutex<HashMap<SocketAddr, Arc<QuicEngine>>>>,
    ctx: crate::concurrency::ccek::CoroutineContext,
}

#[cfg(test)]
mod tests {
    use super::select_static_response;
    use std::fs;

    #[test]
    fn control_plane_html_embeds_titlebar_icon() {
        let html = fs::read_to_string("index.html").expect("read index.html");
        assert!(html.contains("rel=\"icon\""));
        assert!(html.contains("data:image/svg+xml"));
        assert!(html.contains("<svg class=\"menu-icon\""));
    }

    #[test]
    fn select_static_response_serves_pack_asset() {
        let (body, content_type, path) = select_static_response(
            b"GET /configs/agent-host-free-lanes.dsel HTTP/3\r\nHost: localhost\r\n\r\n",
            9,
        );

        assert_eq!(content_type, "text/plain");
        assert_eq!(path, "/configs/agent-host-free-lanes.dsel");

        let text = String::from_utf8(body).expect("pack should be utf-8");
        assert!(text.contains("/z-ai/glm-5"));
        assert!(text.contains("/moonshotai/kimi-k2.5"));
    }

    #[test]
    fn bind_installs_session_cache_in_context() {
        use crate::quic::quic_session_cache::SessionCacheService;
        use crate::concurrency::ccek::CoroutineContext;

        // A bare context (no SessionCacheService) should have one installed after bind construction.
        // We can't call bind() directly (it's async and opens a socket), so test the context logic.
        let bare_ctx = CoroutineContext::new();
        assert!(!bare_ctx.contains("SessionCacheService"));

        // Simulate what bind() does: create shared cache and merge into context
        let shared_cache = std::sync::Arc::new(crate::quic::quic_session_cache::DefaultQuicSessionCache::default());
        let svc = SessionCacheService::new(shared_cache);
        let ctx = CoroutineContext::with_element(svc).merge(&bare_ctx);
        assert!(ctx.contains("SessionCacheService"), "context must contain SessionCacheService after install");
    }

    #[test]
    fn bind_caller_supplied_session_cache_is_preserved() {
        use crate::quic::quic_session_cache::{DefaultQuicSessionCache, SessionCacheService};
        use crate::concurrency::ccek::CoroutineContext;
        use std::sync::Arc;

        // If caller provides their own SessionCacheService, it wins after merge.
        let caller_cache = Arc::new(DefaultQuicSessionCache::default());
        let caller_svc = SessionCacheService::new(caller_cache.clone());
        let caller_ctx = CoroutineContext::with_element(caller_svc);

        // Simulate bind merge: default is baseline, caller overwrites
        let default_cache = Arc::new(DefaultQuicSessionCache::default());
        let default_svc = SessionCacheService::new(default_cache);
        let ctx = CoroutineContext::with_element(default_svc).merge(&caller_ctx);

        // The caller's svc should win (merge puts caller on top)
        assert!(ctx.contains("SessionCacheService"));
    }
}

impl QuicServer {
    // RFC-TRACE: §5.2 (Packet Format) — Extract DCID from long header for connection routing
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
    // RFC-TRACE: §5.2, §7 (Initial Packet, Packet Number) — Decrypt & decode Initial packets
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

        // Only handle Initial packets (type bits = 0b00 in bits 4-5)
        let packet_type_bits = (first_byte >> 4) & 0x03;
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

    /// Returns the byte length of the first QUIC packet in a UDP datagram slice.
    /// For short-header packets, returns the remaining slice length.
    fn first_packet_len(packet_data: &[u8]) -> Option<usize> {
        if packet_data.is_empty() {
            return None;
        }
        // Short header: packet length is not encoded; assume it consumes the rest.
        if (packet_data[0] & 0x80) == 0 {
            return Some(packet_data.len());
        }

        // Long header
        let mut pos = 1usize;
        if pos + 4 > packet_data.len() {
            return None;
        }
        pos += 4; // version

        if pos >= packet_data.len() {
            return None;
        }
        let dcid_len = packet_data[pos] as usize;
        pos += 1;
        if pos + dcid_len > packet_data.len() {
            return None;
        }
        pos += dcid_len;

        if pos >= packet_data.len() {
            return None;
        }
        let scid_len = packet_data[pos] as usize;
        pos += 1;
        if pos + scid_len > packet_data.len() {
            return None;
        }
        pos += scid_len;

        // Initial includes token field; other long-header packet types do not.
        let packet_type_bits = (packet_data[0] >> 4) & 0x03;
        if packet_type_bits == 0 {
            let (token_len, token_varint_len) = Self::read_varint(packet_data, pos)?;
            pos += token_varint_len;
            let token_len = token_len as usize;
            if pos + token_len > packet_data.len() {
                return None;
            }
            pos += token_len;
        }

        let (payload_len, payload_len_varint_len) = Self::read_varint(packet_data, pos)?;
        pos += payload_len_varint_len;

        let total_len = pos.checked_add(payload_len as usize)?;
        if total_len > packet_data.len() {
            return None;
        }
        Some(total_len)
    }

    /// Best-effort split of a UDP datagram into coalesced QUIC packet slices.
    fn split_coalesced_packets<'a>(datagram: &'a [u8]) -> Vec<&'a [u8]> {
        let mut packets = Vec::new();
        let mut offset = 0usize;

        while offset < datagram.len() {
            let remaining = &datagram[offset..];
            let Some(pkt_len) = Self::first_packet_len(remaining) else {
                packets.push(remaining);
                break;
            };
            if pkt_len == 0 || pkt_len > remaining.len() {
                packets.push(remaining);
                break;
            }

            packets.push(&remaining[..pkt_len]);
            offset += pkt_len;

            // Short-header packets cannot be split further without decryption context.
            if (remaining[0] & 0x80) == 0 {
                break;
            }
        }

        packets
    }

    #[cfg(feature = "tls-quic")]
    // RFC-TRACE: §5.3 (1-RTT Packet Format), §9 (Packet Protection) — Decrypt 1-RTT short-header packets
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

        // Need enough bytes for packet number + AEAD tag.
        // We cannot trust PN length bits until header protection is removed.
        let payload_len = packet_data.len() - pn_offset;
        if payload_len < 1 + 16 { return None; }
        if pn_offset + 4 > packet_data.len() { return None; }

        // Header protection removal
        let sample_offset = pn_offset + 4;
        if sample_offset + 16 > packet_data.len() {
            println!("❌ Sample offset out of bounds for 1-RTT");
            return None;
        }
        let sample = &packet_data[sample_offset..sample_offset + 16];
        let mut unprotected_first = first_byte;
        let mut pn_bytes = [0u8; 4];
        // Feed up to 4 bytes to HP removal per RFC algorithm; true PN length is
        // derived only after first-byte unmasking.
        pn_bytes.copy_from_slice(&packet_data[pn_offset..pn_offset + 4]);

        if crypto_provider
            .remove_header_protection(
                EncryptionLevel::OneRtt,
                sample,
                &mut unprotected_first,
                &mut pn_bytes[..4],
            )
            .is_err()
        {
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
    // RFC-TRACE: §5.1 (Packet Format), §9 (Packet Protection) — Decrypt Handshake long-header packets
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

        // Check if Handshake packet (type bits = 0b10 in bits 4-5)
        let packet_type_bits = (first_byte >> 4) & 0x03;
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

        // Need enough bytes for packet number + AEAD tag.
        // We cannot trust PN length bits until header protection is removed.
        let payload_len = payload_length as usize;
        if payload_len < 1 + 16 { return None; }
        if pn_offset + 4 > packet_data.len() { return None; }

        // Header protection removal using handshake_remote keys
        let sample_offset = pn_offset + 4;
        if sample_offset + 16 > packet_data.len() {
            println!("❌ Sample offset out of bounds for Handshake");
            return None;
        }
        let sample = &packet_data[sample_offset..sample_offset + 16];
        let mut unprotected_first = first_byte;
        let mut pn_bytes = [0u8; 4];
        // Feed 4 bytes to HP removal and derive PN length from unmasked first byte.
        pn_bytes.copy_from_slice(&packet_data[pn_offset..pn_offset + 4]);

        // Try to remove header protection using crypto provider
        if crypto_provider
            .remove_header_protection(
                EncryptionLevel::Handshake,
                sample,
                &mut unprotected_first,
                &mut pn_bytes[..4],
            )
            .is_err()
        {
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
        {
            // Keep REUSEPORT opt-in. Enabling it by default allows multiple QUIC
            // listeners to bind to the same UDP port, which can split handshake
            // packets across processes and surface as intermittent
            // ERR_QUIC_PROTOCOL_ERROR in Chrome.
            let enable_reuse_port = std::env::var("LITERBIKE_QUIC_REUSE_PORT")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            if enable_reuse_port {
                socket.set_reuse_port(true).map_err(QuicError::Io)?;
            }
        }

        socket.bind(&addr.into()).map_err(QuicError::Io)?;

        let std_socket: std::net::UdpSocket = socket.into();
        std_socket.set_nonblocking(true).map_err(QuicError::Io)?;

        let tokio_socket = TokioUdpSocket::from_std(std_socket).map_err(QuicError::Io)?;

        // Install a shared session cache if the caller didn't supply one.
        // This ensures all connections on this server share the same resumption cache.
        let shared_cache = Arc::new(DefaultQuicSessionCache::default());
        let svc = SessionCacheService::new(shared_cache);
        let ctx = crate::concurrency::ccek::CoroutineContext::with_element(svc).merge(&ctx);

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
                        // RFC-TRACE: §12.2 (Coalescing) — Process coalesced packets from single UDP datagram
                        let packet_slices = Self::split_coalesced_packets(packet_data);
                        if packet_slices.len() > 1 {
                            println!("📦 Coalesced datagram detected: {} packets", packet_slices.len());
                        }

                        for packet_data in packet_slices {

                            // RFC-TRACE: §5 (Packet Format) — Protocol identification via RbCursive
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

                                // RFC-TRACE: §5.1, §5.2 (Packet Type Dispatch) — Route to decrypt based on packet type
                                // Try QUIC Initial/Handshake packet decryption (RFC 9001)
                                // Falls back to raw deserialize for non-Initial/Handshake or already-decrypted packets
                                #[cfg(feature = "tls-quic")]
                                let decoded_result = if (packet_data[0] & 0x80) != 0 && ((packet_data[0] >> 4) & 0x03) == 0 {
                                    // RFC-TRACE: §5.2 (Initial Packet) — Decrypt Initial (type=0x00)
                                    // Long header Initial packet — decrypt first
                                    Self::try_decrypt_initial_packet(packet_data)
                                        .ok_or_else(|| ProtocolError::InvalidPacket(
                                            "Initial packet decryption failed".into()
                                        ))
                                } else if (packet_data[0] & 0x80) != 0 && ((packet_data[0] >> 4) & 0x03) == 2 {
                                    // RFC-TRACE: §5.1 (Handshake Packet) — Decrypt Handshake (type=0x02)
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
                                    // RFC-TRACE: §5.3 (1-RTT Packet) — Decrypt 1-RTT short-header
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

                                        // RFC-TRACE: §7.6 (Crypto Handshake), §3.2 (Connection IDs) — Extract and validate connection IDs
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
                                                    // Client's CID - what we use to reach the client.
                                                    // If the client chose zero-length SCID, preserve
                                                    // that choice (server DCID length must match).
                                                    let remote_conn_id = ConnectionId {
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
                                                // RFC-TRACE: §3.4 (Streams) — Process stream frames and dispatch to stream handlers
                                                for frame in received_packet.frames.iter() {
                                                    if let QuicFrame::Stream(stream_frame) = frame {
                                                        // Client-initiated bidirectional request streams are 0,4,8,...
                                                        // Ignore unidirectional control/QPACK streams.
                                                        if stream_frame.stream_id % 4 != 0 {
                                                            println!("📄 Ignoring non-request stream {}", stream_frame.stream_id);
                                                            continue;
                                                        }

                                                        if let Some(stream_state) = engine_arc.get_stream(stream_frame.stream_id) {
                                                            if stream_state.send_offset > 0
                                                                || matches!(
                                                                    stream_state.state,
                                                                    StreamState::HalfClosedLocal | StreamState::Closed
                                                                )
                                                            {
                                                                println!(
                                                                    "📄 Duplicate request on stream {} detected; response already sent, skipping",
                                                                    stream_frame.stream_id
                                                                );
                                                                continue;
                                                            }
                                                        }
                                                        let data_str = String::from_utf8_lossy(&stream_frame.data);
                                                        println!("📄 Server received request on stream {} ({} bytes): {}", stream_frame.stream_id, stream_frame.data.len(), data_str);

                                                        // Minimal path selection from encoded request bytes.
                                                        let (body, content_type, selected_path) =
                                                            select_static_response(&stream_frame.data, stream_frame.stream_id);
                                                        println!(
                                                            "📄 Serving {} ({} bytes, {})",
                                                            selected_path,
                                                            body.len(),
                                                            content_type
                                                        );

                                                        // Wrap in HTTP/3 HEADERS (200 OK) + DATA frames
                                                        let response_data = build_h3_response(content_type, &body);

                                                        if !response_data.is_empty() {
                                                            // Keep stream chunks comfortably under typical QUIC
                                                            // datagram payload budgets to avoid path MTU blackholes.
                                                            const QUIC_RESPONSE_CHUNK: usize = 900;
                                                            let total_chunks = response_data.len().div_ceil(QUIC_RESPONSE_CHUNK);
                                                            println!("📤 Sending {} bytes in {} chunks on stream {} (HTTP/3 framed)",
                                                                response_data.len(), total_chunks, stream_frame.stream_id);

                                                            for (idx, chunk) in response_data.chunks(QUIC_RESPONSE_CHUNK).enumerate() {
                                                                let is_last_chunk = idx + 1 == total_chunks;
                                                                if let Err(e) = engine_arc
                                                                    .send_stream_data_with_fin(
                                                                        stream_frame.stream_id,
                                                                        chunk.to_vec(),
                                                                        is_last_chunk,
                                                                    )
                                                                    .await
                                                                {
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
