use super::quic_engine::{QuicEngine, Role};
use super::quic_error::QuicError;
use super::quic_failure_log as qfail;
use super::quic_protocol::{
    deserialize_decoded_packet_with_dcid_len, ConnectionId, ConnectionState, QuicConnectionState,
    QuicFrame, TransportParameters,
};
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
