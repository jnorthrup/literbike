use super::quic_engine::{QuicEngine, Role};
use super::quic_error::QuicError;
use super::quic_failure_log as qfail;
use super::quic_protocol::{
    deserialize_decoded_packet_with_dcid_len, ConnectionId, ConnectionState, QuicConnectionState,
    QuicFrame, StreamFrame, TransportParameters,
};
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

    // CCEK: Attempt Initial packet decryption - returns None to skip decryption
    // Full implementation would need proper header protection removal and AEAD decryption
    fn try_decrypt_initial_packet(&self, packet_data: &[u8]) -> Option<Vec<u8>> {
        if let Some(dcid) = Self::extract_dcid_from_long_header(packet_data) {
            // CCEK: Derive Initial secrets from client DCID
            let (_, server_initial_secret) = derive_initial_secrets(&dcid);
            let (key, iv, hp_key) = derive_packet_protection_keys(&server_initial_secret);

            println!("🔓 CCEK: DCID {:02x?} -> keys derived (key={}, iv={}, hp={})",
                &dcid[..dcid.len().min(8)],
                hex::encode(&key[..4]),
                hex::encode(&iv[..4]),
                hex::encode(&hp_key[..4]));
        }

        // Return None - packet is encrypted, skip decryption for now
        None
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

                                // CCEK: Log key derivation for this connection
                                // (in full implementation, decryption would happen here)
                                let _ = Self::extract_dcid_from_long_header(packet_data).map(|dcid| {
                                    let (_, server_secret) = derive_initial_secrets(&dcid);
                                    let (key, iv, hp) = derive_packet_protection_keys(&server_secret);
                                    println!("🔓 CCEK: Keys derived for DCID {:02x?}", &dcid[..dcid.len().min(8)]);
                                });

                                let short_header_dcid_len =
                                    connections.lock().get(&remote_addr).map(|engine| {
                                        println!("🔧 Found existing engine for {}", remote_addr);
                                        engine.get_state().local_connection_id.bytes.len()
                                    });

                                match deserialize_decoded_packet_with_dcid_len(
                                    packet_data,
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