use super::quic_engine::{QuicEngine, Role};
use super::quic_error::QuicError;
use super::quic_failure_log as qfail;
use super::quic_protocol::{
    deserialize_decoded_packet_with_dcid_len, ConnectionId, ConnectionState, QuicConnectionState,
    QuicFrame, TransportParameters,
};
use super::tls_crypto::secrets::{derive_initial_secrets, derive_packet_protection_keys, hkdf_expand_label};
use super::tls_crypto::packet_protection::QuicCryptoState;
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

    // CCEK: Try to decrypt an Initial packet using the server's Initial secret
    fn try_decrypt_initial_packet(packet_data: &[u8]) -> Option<Vec<u8>> {
        let dcid = Self::extract_dcid_from_long_header(packet_data)?;
        println!("🔓 CCEK: Extracted DCID from packet: {:02x?}", &dcid[..dcid.len().min(8)]);

        let (_, server_initial_secret) = derive_initial_secrets(&dcid);
        let (key, iv, hp_key) = derive_packet_protection_keys(&server_initial_secret);

        if packet_data.len() < 20 {
            println!("🔓 CCEK: Packet too short");
            return None;
        }

        let crypto_state = QuicCryptoState::new(
            super::tls_crypto::QuicAeadAlgorithm::Aes128Gcm,
            &key,
            iv,
            &hp_key
        ).ok()?;

        // CCEK: Full Initial packet decryption
        // For Long Header Initial: first byte + version (4) + DCID len + DCID + SCID len + SCID + token len + token + length
        // HP sample is at bytes[4..20] (after first byte and version)
        // For simplicity, let's try to decrypt assuming 1-byte packet number (most common for Initial)

        let first_byte = packet_data[0];
        let pn_len = ((first_byte & 0x03) + 1) as usize;

        // Calculate where the protected payload starts
        // first(1) + version(4) + dcid_len(1) + dcid + scid_len(1) + scid + token_len(varint) + token + length(varint) + pn(pn_len)
        // For simplicity, let's extract what's after the first byte and version for the AAD
        let mut pos = 5; // Skip first byte + version

        // Read DCID
        if pos >= packet_data.len() { return None; }
        let dcid_len = packet_data[pos] as usize;
        pos += 1 + dcid_len;

        // Read SCID
        if pos >= packet_data.len() { return None; }
        let scid_len = packet_data[pos] as usize;
        pos += 1 + scid_len;

        // For Initial, read token length and token
        if (first_byte >> 4) & 0x01 == 0 { // Initial packet
            if pos >= packet_data.len() { return None; }
            let token_len = packet_data[pos] as usize; // Simplified - should be varint
            pos += 1 + token_len;
        }

        // Read length
        if pos >= packet_data.len() { return None; }
        let length = packet_data[pos] as usize; // Simplified - should be varint
        pos += 1;

        // The packet number and payload start here
        let pn_start = pos;
        let payload_start = pos + pn_len;

        if payload_start > packet_data.len() {
            println!("🔓 CCEK: Not enough data for PN + payload");
            return None;
        }

        // Extract the protected packet number and ciphertext
        let protected_pn_and_payload = &packet_data[pn_start..];
        let sample_start = 4.min(protected_pn_and_payload.len().saturating_sub(16));

        // Try HP removal - derive mask from sample
        let sample = &protected_pn_and_payload[sample_start..sample_start + 16];
        let mask = match crypto_state.generate_header_protection_mask(sample) {
            Ok(m) => m,
            Err(e) => {
                println!("🔓 CCEK: HP mask generation failed: {:?}", e);
                return None;
            }
        };

        // Remove header protection from first byte
        let mut unprotected_first = first_byte ^ mask[0];
        let pn_length = ((unprotected_first & 0x03) + 1) as usize;

        // Extract packet number (it's at the end of the protected section, masked)
        let pn_bytes = &protected_pn_and_payload[..pn_length];
        let mut actual_pn: u64 = 0;
        for (i, &b) in pn_bytes.iter().enumerate() {
            actual_pn |= ((b ^ mask[1 + i]) as u64) << (i * 8);
        }

        println!("🔓 CCEK: Packet number extracted: {}", actual_pn);

        // Now decrypt the payload
        let ciphertext = &mut protected_pn_and_payload[pn_length..].to_vec();
        let aad = &packet_data[..pn_start]; // Everything before PN is AAD

        match crypto_state.decrypt_payload(actual_pn, aad, ciphertext) {
            Ok(decrypted) => {
                println!("🔓 CCEK: Decryption successful! {} bytes", decrypted.len());
                // Rebuild the unprotected packet
                let mut result = Vec::with_capacity(pn_start + pn_length + decrypted.len());
                result.extend_from_slice(&[unprotected_first]); // First byte with PN length
                result.extend_from_slice(&packet_data[1..pn_start]); // Version, DCID, SCID, token, length
                result.extend_from_slice(decrypted);
                Some(result)
            }
            Err(e) => {
                println!("🔓 CCEK: Payload decryption failed: {:?}", e);
                None
            }
        }
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
        let connections = self.connections.clone(); // Clone for the spawned task
        let rb_cursor = self.rb.clone();
        let ctx = self.ctx.clone(); // Clone context for the spawned task

        // Spawn the UDP receive loop locally — the captured `RbCursor` is not
        // `Send` (contains raw pointers), so we use a local task. Callers must
        // run `start()` inside a `tokio::task::LocalSet` or equivalent.
        tokio::task::spawn_local(async move {
            let mut buf = vec![0u8; 65536]; // Max UDP packet size
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
                                let short_header_dcid_len =
                                    connections.lock().get(&remote_addr).map(|engine| {
                                        println!("🔧 Found existing engine for {}", remote_addr);
                                        engine.get_state().local_connection_id.bytes.len()
                                    });

                                // CCEK: Try to decrypt Initial packets before deserializing
                                let decrypted_data = Self::try_decrypt_initial_packet(packet_data);
                                let data_to_parse = decrypted_data.as_deref().unwrap_or(packet_data);

                                match deserialize_decoded_packet_with_dcid_len(
                                    data_to_parse,
                                    short_header_dcid_len,
                                ) {
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
                                                    }; // Dummy
                                                    let remote_conn_id = ConnectionId {
                                                        bytes: vec![8, 7, 6, 5, 4, 3, 2, 1],
                                                    }; // Dummy

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
                                                            ConnectionState::Handshaking, // Initial state
                                                    };
                                                    println!("🔧 Creating QuicEngine, context keys: {:?}", ctx.keys());
                                                    Arc::new(QuicEngine::new(
                                                        Role::Server,
                                                        initial_state,
                                                        socket.clone(),
                                                        remote_addr,
                                                        vec![0; 32], // Dummy private key
                                                        ctx.clone(),
                                                    ))
                                                })
                                                .clone()
                                        };

                                        // Process the packet with the engine
                                        match engine_arc.process_decoded_packet(decoded_packet).await {
                                            Ok(()) => {
                                                // If the packet contained a StreamFrame, echo the data back
                                                for frame in received_packet.frames.iter() {
                                                    println!("🎞️ Frame type: {:?}", frame);
                                                    if let QuicFrame::Stream(stream_frame) = frame {
                                                        // Visual QA - Simple file serving
                                                        let data_str = String::from_utf8_lossy(&stream_frame.data);
                                                        println!("📄 Server received request on stream {}: {}", stream_frame.stream_id, data_str);
                                                        
                                                        let response_data = if data_str.contains("index.css") {
                                                            match std::fs::read("index.css") {
                                                                Ok(d) => d,
                                                                Err(e) => {
                                                                    println!("❌ Failed to read index.css: {}", e);
                                                                    b"/* css not found */".to_vec()
                                                                }
                                                            }
                                                        } else if data_str.contains("bw_test_pattern.png") {
                                                            match std::fs::read("bw_test_pattern.png") {
                                                                Ok(d) => {
                                                                    println!("🖼️ Serving bw_test_pattern.png ({} bytes)", d.len());
                                                                    d
                                                                }
                                                                Err(e) => {
                                                                    println!("❌ Failed to read bw_test_pattern.png: {}", e);
                                                                    b"image not found".to_vec()
                                                                }
                                                            }
                                                        } else {
                                                            // For anything else (like the initial GET /), serve index.html
                                                            match std::fs::read("index.html") {
                                                                Ok(d) => d,
                                                                Err(e) => {
                                                                    println!("❌ Failed to read index.html: {}", e);
                                                                    b"<html><body><h1>QUIC VQA ERROR</h1></body></html>".to_vec()
                                                                }
                                                            }
                                                        };

                                                        // Chunk the data to avoid "Message too long" (UDP limit is 65k, but path MTU is typically ~1350)
                                                        let chunk_size = 1200;
                                                        let total_len = response_data.len();
                                                        let mut offset = 0;
                                                        let mut success = true;
                                                        
                                                        while offset < total_len {
                                                            let end = (offset + chunk_size).min(total_len);
                                                            let chunk = response_data[offset..end].to_vec();
                                                            if let Err(e) = engine_arc.send_stream_data(stream_frame.stream_id, chunk).await {
                                                                println!("❌ Failed to send VQA stream chunk at offset {}: {}", offset, e);
                                                                success = false;
                                                                break;
                                                            }
                                                            offset = end;
                                                        }

                                                        if success {
                                                            println!("✅ Sent response ({} bytes) in {} chunks on stream {}", 
                                                                total_len, 
                                                                (total_len + chunk_size - 1) / chunk_size, 
                                                                stream_frame.stream_id
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                qfail::log_error(
                                                    "server",
                                                    "process_packet",
                                                    &e,
                                                    serde_json::json!({"remote": remote_addr, "len": len}),
                                                );
                                                tracing::error!("Error processing packet: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        println!("❌ Deserialize error: {:?}", e);
                                        let quic_err = QuicError::Protocol(e);
                                        qfail::log_error(
                                            "server",
                                            "deserialize",
                                            &quic_err,
                                            serde_json::json!({"remote": remote_addr, "len": len}),
                                        );
                                        tracing::error!("Failed to deserialize packet");
                                    }
                                }
                            }
                            other => {
                                tracing::debug!(target = "rb", ?other, "RbCursive server preflight non-accept signal for packet from {}", remote_addr);
                                // Drop packet if not accepted by RbCursive
                            }
                        }
                    }
                    Err(e) => {
                        let quic_err = QuicError::Io(e);
                        qfail::log_error("server", "recv_from", &quic_err, serde_json::json!({}));
                        tracing::error!("UDP socket receive error");
                    }
                }
            }
        });
        Ok(())
    }

    pub fn local_addr(&self) -> Result<SocketAddr, QuicError> {
        self.socket.local_addr().map_err(QuicError::Io)
    }

    pub async fn accept(&self) -> Option<Arc<QuicEngine>> {
        // This method needs to be re-thought for a multi-connection server.
        // For now, it will just return None, as connections are managed internally.
        // A real accept would block until a new connection is established and return it.
        None
    }

    /// Close endpoint gracefully
    pub async fn close(&self) {
        // Iterate through all active engines and close them
        let connections_guard = self.connections.lock();
        for (_, engine) in connections_guard.iter() {
            engine.close().await;
        }
        // In a real implementation, close the UDP socket
    }
}
