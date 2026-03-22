//! CCEK Elements - Session state per SDK
//!
//! Each element holds the session state for its respective SDK.
//! Elements are stored in the CcekContext and accessed via Keys.

use super::CcekElement;
use std::sync::{Arc, RwLock};

/// HTX session element - holds verification state
pub struct HtxElement {
    pub tickets_processed: Arc<RwLock<u64>>,
    pub current_hour: Arc<RwLock<u64>>,
    pub verified_count: Arc<RwLock<u64>>,
    pub failed_count: Arc<RwLock<u64>>,
}

impl HtxElement {
    pub fn new() -> Self {
        Self {
            tickets_processed: Arc::new(RwLock::new(0)),
            current_hour: Arc::new(RwLock::new(0)),
            verified_count: Arc::new(RwLock::new(0)),
            failed_count: Arc::new(RwLock::new(0)),
        }
    }

    pub fn increment_verified(&self) {
        *self.tickets_processed.write().unwrap() += 1;
        *self.verified_count.write().unwrap() += 1;
    }

    pub fn increment_failed(&self) {
        *self.failed_count.write().unwrap() += 1;
    }
}

impl CcekElement for HtxElement {
    fn key(&self) -> &'static str {
        "HtxElement"
    }
}

impl Default for HtxElement {
    fn default() -> Self {
        Self::new()
    }
}

/// QUIC session element - holds connection state
pub struct QuicElement {
    pub packets_in: Arc<RwLock<u64>>,
    pub packets_out: Arc<RwLock<u64>>,
    pub connections: Arc<RwLock<u32>>,
    pub streams: Arc<RwLock<u64>>,
}

impl QuicElement {
    pub fn new() -> Self {
        Self {
            packets_in: Arc::new(RwLock::new(0)),
            packets_out: Arc::new(RwLock::new(0)),
            connections: Arc::new(RwLock::new(0)),
            streams: Arc::new(RwLock::new(0)),
        }
    }

    pub fn increment_in(&self, n: u64) {
        *self.packets_in.write().unwrap() += n;
    }

    pub fn increment_out(&self, n: u64) {
        *self.packets_out.write().unwrap() += n;
    }

    pub fn new_connection(&self) {
        *self.connections.write().unwrap() += 1;
    }
}

impl CcekElement for QuicElement {
    fn key(&self) -> &'static str {
        "QuicElement"
    }
}

impl Default for QuicElement {
    fn default() -> Self {
        Self::new()
    }
}

/// NIO session element - holds reactor state
pub struct NioElement {
    pub active_fds: Arc<RwLock<u32>>,
    pub submitted_ops: Arc<RwLock<u64>>,
    pub completed_ops: Arc<RwLock<u64>>,
    pub read_events: Arc<RwLock<u64>>,
    pub write_events: Arc<RwLock<u64>>,
}

impl NioElement {
    pub fn new() -> Self {
        Self {
            active_fds: Arc::new(RwLock::new(0)),
            submitted_ops: Arc::new(RwLock::new(0)),
            completed_ops: Arc::new(RwLock::new(0)),
            read_events: Arc::new(RwLock::new(0)),
            write_events: Arc::new(RwLock::new(0)),
        }
    }

    pub fn register_fd(&self) {
        *self.active_fds.write().unwrap() += 1;
    }

    pub fn unregister_fd(&self) {
        *self.active_fds.write().unwrap() -= 1;
    }

    pub fn submit_op(&self) {
        *self.submitted_ops.write().unwrap() += 1;
    }

    pub fn complete_op(&self) {
        *self.completed_ops.write().unwrap() += 1;
    }
}

impl CcekElement for NioElement {
    fn key(&self) -> &'static str {
        "NioElement"
    }
}

impl Default for NioElement {
    fn default() -> Self {
        Self::new()
    }
}

/// HTTP session element
pub struct HttpElement {
    pub requests: Arc<RwLock<u64>>,
    pub responses: Arc<RwLock<u64>>,
    pub active_connections: Arc<RwLock<u32>>,
}

impl HttpElement {
    pub fn new() -> Self {
        Self {
            requests: Arc::new(RwLock::new(0)),
            responses: Arc::new(RwLock::new(0)),
            active_connections: Arc::new(RwLock::new(0)),
        }
    }

    pub fn increment_requests(&self) {
        *self.requests.write().unwrap() += 1;
    }

    pub fn increment_responses(&self) {
        *self.responses.write().unwrap() += 1;
    }
}

impl CcekElement for HttpElement {
    fn key(&self) -> &'static str {
        "HttpElement"
    }
}

impl Default for HttpElement {
    fn default() -> Self {
        Self::new()
    }
}

/// SCTP session element
pub struct SctpElement {
    pub chunks_in: Arc<RwLock<u64>>,
    pub chunks_out: Arc<RwLock<u64>>,
    pub associations: Arc<RwLock<u32>>,
}

impl SctpElement {
    pub fn new() -> Self {
        Self {
            chunks_in: Arc::new(RwLock::new(0)),
            chunks_out: Arc::new(RwLock::new(0)),
            associations: Arc::new(RwLock::new(0)),
        }
    }

    pub fn increment_in(&self, n: u64) {
        *self.chunks_in.write().unwrap() += n;
    }

    pub fn increment_out(&self, n: u64) {
        *self.chunks_out.write().unwrap() += n;
    }
}

impl CcekElement for SctpElement {
    fn key(&self) -> &'static str {
        "SctpElement"
    }
}

impl Default for SctpElement {
    fn default() -> Self {
        Self::new()
    }
}
