use std::sync::Arc;
use std::net::SocketAddr;
use super::quic_error::QuicError;
use super::quic_engine::{QuicEngine, Role};
use super::quic_protocol::{QuicConnectionState, ConnectionId, TransportParameters, deserialize_packet, QuicFrame, ConnectionState};
use crate::rbcursive::{RbCursor, NetTuple, Protocol as RbProtocol, Signal as RbSignal};
use parking_lot::Mutex;
use super::quic_failure_log as qfail;
use tokio::net::UdpSocket as TokioUdpSocket;
use std::collections::HashMap;
use socket2::{Socket, Domain, Type};

pub struct QuicServer {
    socket: Arc<TokioUdpSocket>,
    rb: Arc<Mutex<RbCursor>>,
    connections: Arc<Mutex<HashMap<SocketAddr, Arc<QuicEngine>>>>,
}

impl QuicServer {
    pub async fn bind(addr: SocketAddr) -> Result<Self, QuicError> {
        let socket = Socket::new(Domain::for_address(addr), Type::DGRAM, None)
            .map_err(QuicError::Io)?;
        
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
        })
    }

    pub async fn start(&self) -> Result<(), QuicError> {
        let socket = self.socket.clone();
    let connections = self.connections.clone(); // Clone for the spawned task
    let rb_cursor = self.rb.clone();

    // Spawn the UDP receive loop locally — the captured `RbCursor` is not
    // `Send` (contains raw pointers), so we use a local task. Callers must
    // run `start()` inside a `tokio::task::LocalSet` or equivalent.
    tokio::task::spawn_local(async move {
            let mut buf = vec![0u8; 65536]; // Max UDP packet size
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, remote_addr)) => {
                        let packet_data = &buf[..len];

                        // RbCursive preflight
                        let tuple = NetTuple::from_socket_addr(remote_addr, RbProtocol::HtxQuic);
                        let hint = if packet_data.len() > 0 {
                            vec![packet_data[0]]
                        } else {
                            vec![]
                        };
                        let signal = rb_cursor.lock().recognize(tuple, &hint);

                        match signal {
                            RbSignal::Accept(proto) => {
                                tracing::debug!(target = "rb", ?proto, "RbCursive server preflight accepted protocol");
                                tracing::info!("Received packet from {}: {} bytes", remote_addr, len);

                                match deserialize_packet(packet_data) {
                                    Ok(received_packet) => {
                                        let engine_arc = {
                                            let mut connections_guard = connections.lock();
                                            connections_guard.entry(remote_addr).or_insert_with(|| {
                                                // Create a new engine for this connection
                                                let local_conn_id = ConnectionId { bytes: vec![1, 2, 3, 4, 5, 6, 7, 8] }; // Dummy
                                                let remote_conn_id = ConnectionId { bytes: vec![8, 7, 6, 5, 4, 3, 2, 1] }; // Dummy

                                                let initial_state = QuicConnectionState {
                                                    local_connection_id: local_conn_id,
                                                    remote_connection_id: remote_conn_id,
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
                                                    connection_state: ConnectionState::Handshaking, // Initial state
                                                };
                                                Arc::new(QuicEngine::new(
                                                    Role::Server,
                                                    initial_state,
                                                    socket.clone(),
                                                    remote_addr,
                                                    vec![0; 32], // Dummy private key
                                                ))
                                            }).clone()
                                        };

                                        // Process the packet with the engine
                                        match engine_arc.process_packet(received_packet.clone()).await {
                                            Ok(()) => {
                                                // If the packet contained a StreamFrame, echo the data back
                                                for frame in received_packet.frames.iter() {
                                                    if let QuicFrame::Stream(stream_frame) = frame {
                                                        tracing::info!("Server received stream data on stream {}: {:?}", stream_frame.stream_id, stream_frame.data);
                                                        // Echo data back on the same stream
                                                        if let Err(e) = engine_arc.send_stream_data(stream_frame.stream_id, stream_frame.data.clone()).await {
                                                            tracing::error!("Failed to echo stream data: {}", e);
                                                        }
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                qfail::log_error("server","process_packet", &e, serde_json::json!({"remote": remote_addr, "len": len}));
                                                tracing::error!("Error processing packet: {}", e);
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        qfail::log_error("server","deserialize", &e, serde_json::json!({"remote": remote_addr, "len": len}));
                                        tracing::error!("Failed to deserialize packet: {}", e);
                                    }
                                }
                            }
                            other => {
                                tracing::debug!(target = "rb", ?other, "RbCursive server preflight non-accept signal for packet from {}", remote_addr);
                                // Drop packet if not accepted by RbCursive
                            }
                        }
                    },
                    Err(e) => {
                        qfail::log_error("server","recv_from", &e, serde_json::json!({}));
                        tracing::error!("UDP socket receive error: {}", e);
                    }
                }
            }
        });
        Ok(())
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