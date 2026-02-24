use crate::quic::quic_error::QuicError;
use crate::rbcursive::{RbCursor, NetTuple, Protocol as RbProtocol, Signal as RbSignal};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use bytes::Bytes;
use std::sync::Arc;
use std::net::SocketAddr;
use crate::quic::quic_engine::QuicEngine; // Import QuicEngine

// Global RbCursive scanner for observational classification on stream payloads
static RB_STREAM_SCANNER: Lazy<Mutex<RbCursor>> = Lazy::new(|| Mutex::new(RbCursor::new()));

pub struct QuicStream {
    pub stream_id: u64,
    pub send_buffer: Vec<u8>,
    pub recv_buffer: Vec<u8>,
    engine: Arc<QuicEngine>,
    remote_addr: SocketAddr,
}

impl QuicStream {
    // Constructor for QuicStream
    pub fn new(stream_id: u64, engine: Arc<QuicEngine>, remote_addr: SocketAddr) -> Self {
        QuicStream {
            stream_id,
            send_buffer: Vec::new(),
            recv_buffer: Vec::new(),
            engine,
            remote_addr,
        }
    }

    pub async fn write(&mut self, data: &[u8]) -> Result<(), QuicError> {
        // RbCursive: classify first bytes of payload (observational)
        let hint_len = data.len().min(64);
        if hint_len > 0 {
            let tuple = NetTuple::from_socket_addr(self.remote_addr, RbProtocol::HtxQuic);
            let signal = RB_STREAM_SCANNER.lock().recognize(tuple, &data[..hint_len]);
            match signal {
                RbSignal::Accept(proto) => tracing::debug!(target = "rb", ?proto, "RbCursive stream TX classification accepted"),
                other => tracing::debug!(target = "rb", ?other, "RbCursive stream TX classification non-accept"),
            }
        }
        // Send data via the QuicEngine
        self.engine.send_stream_data(self.stream_id, data.to_vec()).await?;
        Ok(())
    }

    pub async fn finish(&mut self) -> Result<(), QuicError> {
        // In a real implementation, this would signal the QuicEngine that the send stream is finished
        Ok(())
    }

    /// Read up to `max` bytes as a chunk, classifying with RbCursive on first receipt.
    pub async fn read_chunk(&mut self, max: usize) -> Result<Option<Bytes>, QuicError> {
        // In a real implementation, this would read from the QuicEngine's receive buffer for this stream
        // For now, simulate reading from internal buffer
        if self.recv_buffer.is_empty() {
            return Ok(None);
        }

        let bytes_to_read = self.recv_buffer.len().min(max);
        let chunk = self.recv_buffer.drain(..bytes_to_read).collect::<Vec<u8>>();

        if !chunk.is_empty() {
            let b = Bytes::from(chunk);
            let hint_len = b.len().min(64);
            if hint_len > 0 {
                let tuple = NetTuple::from_socket_addr(self.remote_addr, RbProtocol::HtxQuic);
                let signal = RB_STREAM_SCANNER.lock().recognize(tuple, &b[..hint_len]);
                match signal {
                    RbSignal::Accept(proto) => tracing::debug!(target = "rb", ?proto, "RbCursive stream RX classification accepted"),
                    other => tracing::debug!(target = "rb", ?other, "RbCursive stream RX classification non-accept"),
                }
            }
            return Ok(Some(b));
        }
        Ok(None)
    }
}
