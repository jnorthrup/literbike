use crate::quic::quic_engine::QuicEngine;
use crate::quic::quic_error::QuicError;
use crate::quic::quic_protocol::StreamPriority;
use crate::rbcursive::{NetTuple, Protocol as RbProtocol, RbCursor, Signal as RbSignal};
use bytes::Bytes;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::net::SocketAddr;
use std::sync::Arc; // Import QuicEngine

// Global RbCursive scanner for observational classification on stream payloads
static RB_STREAM_SCANNER: Lazy<Mutex<RbCursor>> = Lazy::new(|| Mutex::new(RbCursor::new()));

pub struct QuicStream {
    pub stream_id: u64,
    pub send_buffer: Vec<u8>,
    engine: Arc<QuicEngine>,
    remote_addr: SocketAddr,
    /// Priority level for stream scheduling
    pub priority: StreamPriority,
}

impl QuicStream {
    // Constructor for QuicStream with default priority
    pub fn new(stream_id: u64, engine: Arc<QuicEngine>, remote_addr: SocketAddr) -> Self {
        QuicStream {
            stream_id,
            send_buffer: Vec::new(),
            engine,
            remote_addr,
            priority: StreamPriority::Normal,
        }
    }

    /// Create a new stream with explicit priority
    pub fn with_priority(
        stream_id: u64,
        engine: Arc<QuicEngine>,
        remote_addr: SocketAddr,
        priority: StreamPriority,
    ) -> Self {
        QuicStream {
            stream_id,
            send_buffer: Vec::new(),
            engine,
            remote_addr,
            priority,
        }
    }

    /// Set the priority of this stream
    pub fn set_priority(&mut self, priority: StreamPriority) {
        self.priority = priority;
        // Update priority in engine's stream state
        self.engine.set_stream_priority(self.stream_id, priority);
    }

    /// Get the current priority of this stream
    pub fn priority(&self) -> StreamPriority {
        self.priority
    }

    pub async fn write(&mut self, data: &[u8]) -> Result<(), QuicError> {
        // RbCursive: classify first bytes of payload (observational)
        let hint_len = data.len().min(64);
        if hint_len > 0 {
            let tuple = NetTuple::from_socket_addr(self.remote_addr, RbProtocol::CustomQuic);
            let signal = RB_STREAM_SCANNER.lock().recognize(tuple, &data[..hint_len]);
            match signal {
                RbSignal::Accept(proto) => tracing::debug!(
                    target = "rb",
                    ?proto,
                    "RbCursive stream TX classification accepted"
                ),
                other => tracing::debug!(
                    target = "rb",
                    ?other,
                    "RbCursive stream TX classification non-accept"
                ),
            }
        }
        // Send data via the QuicEngine with priority awareness
        self.engine
            .send_stream_data_priority(self.stream_id, data.to_vec(), self.priority)
            .await?;
        Ok(())
    }

    pub async fn finish(&mut self) -> Result<(), QuicError> {
        self.engine.send_stream_fin(self.stream_id).await?;
        Ok(())
    }

    /// Read up to `max` bytes as a chunk, classifying with RbCursive on first receipt.
    pub async fn read_chunk(&mut self, max: usize) -> Result<Option<Bytes>, QuicError> {
        let chunk = self.engine.drain_stream_recv(self.stream_id, max);

        if !chunk.is_empty() {
            let b = Bytes::from(chunk);
            let hint_len = b.len().min(64);
            if hint_len > 0 {
                let tuple = NetTuple::from_socket_addr(self.remote_addr, RbProtocol::CustomQuic);
                let signal = RB_STREAM_SCANNER.lock().recognize(tuple, &b[..hint_len]);
                match signal {
                    RbSignal::Accept(proto) => tracing::debug!(
                        target = "rb",
                        ?proto,
                        "RbCursive stream RX classification accepted"
                    ),
                    other => tracing::debug!(
                        target = "rb",
                        ?other,
                        "RbCursive stream RX classification non-accept"
                    ),
                }
            }
            return Ok(Some(b));
        }
        Ok(None)
    }

    /// Get stream statistics
    pub fn stats(&self) -> Option<StreamStats> {
        self.engine.get_stream_stats(self.stream_id)
    }
}

/// Statistics for a QUIC stream
#[derive(Debug, Clone, Default)]
pub struct StreamStats {
    pub stream_id: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub send_offset: u64,
    pub receive_offset: u64,
    pub state: crate::quic::quic_protocol::StreamState,
    pub priority: StreamPriority,
}

/// Pending write entry in the stream scheduler queue.
#[derive(Debug, Clone)]
pub struct ScheduledWrite {
    pub stream_id: u64,
    pub data: Vec<u8>,
    pub priority: StreamPriority,
}

/// Priority-aware stream multiplexer scheduler.
///
/// Callers enqueue writes with `push`; the scheduler drains them in
/// descending priority order (Critical > High > Normal > Background),
/// with stable FIFO ordering within the same priority tier.
pub struct StreamScheduler {
    queue: Vec<ScheduledWrite>,
}

impl Default for StreamScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamScheduler {
    pub fn new() -> Self {
        Self { queue: Vec::new() }
    }

    /// Enqueue a pending write.
    pub fn push(&mut self, stream_id: u64, data: Vec<u8>, priority: StreamPriority) {
        self.queue.push(ScheduledWrite { stream_id, data, priority });
    }

    /// Drain up to `limit` writes in priority order (highest first, FIFO within tier).
    /// Returns the drained entries; remaining entries stay queued.
    pub fn drain_next(&mut self, limit: usize) -> Vec<ScheduledWrite> {
        // Stable sort: highest priority_value first, FIFO within same tier preserved by stable sort.
        self.queue.sort_by(|a, b| {
            b.priority.as_u8().cmp(&a.priority.as_u8())
        });
        let count = limit.min(self.queue.len());
        self.queue.drain(..count).collect()
    }

    /// Number of pending writes.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quic::quic_protocol::StreamPriority;

    #[test]
    fn scheduler_drains_highest_priority_first() {
        let mut sched = StreamScheduler::new();
        sched.push(1, b"bg".to_vec(), StreamPriority::Low);
        sched.push(2, b"normal".to_vec(), StreamPriority::Normal);
        sched.push(3, b"critical".to_vec(), StreamPriority::Critical);
        sched.push(4, b"high".to_vec(), StreamPriority::High);

        let batch = sched.drain_next(4);
        assert_eq!(batch[0].stream_id, 3, "Critical must come first");
        assert_eq!(batch[1].stream_id, 4, "High must come second");
        assert_eq!(batch[2].stream_id, 2, "Normal must come third");
        assert_eq!(batch[3].stream_id, 1, "Low must come last");
    }

    #[test]
    fn scheduler_fifo_within_same_priority_tier() {
        let mut sched = StreamScheduler::new();
        sched.push(10, b"first".to_vec(), StreamPriority::High);
        sched.push(20, b"second".to_vec(), StreamPriority::High);
        sched.push(30, b"third".to_vec(), StreamPriority::High);

        let batch = sched.drain_next(3);
        // Stable sort preserves insertion order within same tier
        let ids: Vec<u64> = batch.iter().map(|w| w.stream_id).collect();
        assert_eq!(ids, vec![10, 20, 30], "FIFO order within same priority tier");
    }

    #[test]
    fn scheduler_partial_drain_leaves_remainder() {
        let mut sched = StreamScheduler::new();
        for i in 0..5u64 {
            sched.push(i, vec![], StreamPriority::Normal);
        }
        let batch = sched.drain_next(3);
        assert_eq!(batch.len(), 3);
        assert_eq!(sched.len(), 2, "2 entries should remain after draining 3 of 5");
    }

    #[test]
    fn scheduler_empty_drain_returns_empty() {
        let mut sched = StreamScheduler::new();
        assert!(sched.drain_next(10).is_empty());
        assert!(sched.is_empty());
    }
}
