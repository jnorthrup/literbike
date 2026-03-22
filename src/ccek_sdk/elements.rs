//! CCEK Elements - State holders with delta structure
//!
//! Separation of concerns:
//! - Key = singleton, may transform state (has functions)
//! - Element = state holder, enables stateful methods
//! - Delta = structure within Element (inlets/tributaries/outflows)
//! - Context = sum of Keys + Elements + local state

use super::delta::{Delta, Inlet, Outflow, Tributary};
use super::{CcekElement, CcekKey};
use std::any::Any;

#[derive(Clone)]
pub struct NetPacket {
    pub data: Vec<u8>,
    pub source: String,
    pub destination: String,
}

impl NetPacket {
    pub fn new(data: Vec<u8>, source: &str, destination: &str) -> Self {
        Self {
            data,
            source: source.to_string(),
            destination: destination.to_string(),
        }
    }
}

// ============================================================================
// HTX Element - Constant-time ticket verification
// ============================================================================

pub struct HtxElement {
    pub delta: Delta<NetPacket>,
    connections: u32,
}

impl HtxElement {
    pub fn new() -> Self {
        Self {
            delta: Delta::new()
                .add_inlet("ticket", 64)
                .add_inlet("challenge", 64)
                .add_tributary("verified", 128)
                .add_tributary("rejected", 64)
                .add_outflow("response", 64),
            connections: 0,
        }
    }

    pub fn connections(&self) -> u32 {
        self.connections
    }

    pub fn ticket_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("ticket")
    }

    pub fn verified_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("verified")
    }

    pub fn rejected_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("rejected")
    }

    pub fn response_outflow(&self) -> Option<Outflow<NetPacket>> {
        self.delta.outflow("response")
    }
}

impl Default for HtxElement {
    fn default() -> Self {
        Self::new()
    }
}

impl CcekElement for HtxElement {
    fn key(&self) -> &'static str {
        "HtxElement"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct HtxKey;

impl HtxKey {
    pub fn connections(elt: &HtxElement) -> u32 {
        elt.connections()
    }

    pub fn verify(_elt: &mut HtxElement, _packet: NetPacket) -> bool {
        true
    }
}

impl CcekKey for HtxKey {
    type Element = HtxElement;
}

// ============================================================================
// QUIC Element - QUIC protocol with streams
// ============================================================================

pub struct QuicElement {
    pub delta: Delta<NetPacket>,
    connections: u32,
}

impl QuicElement {
    pub fn new() -> Self {
        Self {
            delta: Delta::new()
                .add_inlet("packet", 256)
                .add_inlet("stream_init", 64)
                .add_tributary("stream_0", 128)
                .add_tributary("stream_data", 256)
                .add_tributary("stream_close", 64)
                .add_outflow("packet_out", 256)
                .add_outflow("stream_out", 128),
            connections: 0,
        }
    }

    pub fn connections(&self) -> u32 {
        self.connections
    }
}

impl Default for QuicElement {
    fn default() -> Self {
        Self::new()
    }
}

impl CcekElement for QuicElement {
    fn key(&self) -> &'static str {
        "QuicElement"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct QuicKey;

impl QuicKey {
    pub fn connections(elt: &QuicElement) -> u32 {
        elt.connections()
    }
}

impl CcekKey for QuicKey {
    type Element = QuicElement;
}

// ============================================================================
// HTTP Element - HTTP with headers, body, trailers
// ============================================================================

pub struct HttpElement {
    pub delta: Delta<NetPacket>,
    requests: u64,
}

impl HttpElement {
    pub fn new() -> Self {
        Self {
            delta: Delta::new()
                .add_inlet("request_head", 128)
                .add_inlet("request_body", 256)
                .add_inlet("request_trailer", 64)
                .add_tributary("header_parse", 128)
                .add_tributary("body_chunk", 256)
                .add_tributary("trailer_parse", 64)
                .add_outflow("response_head", 128)
                .add_outflow("response_body", 256)
                .add_outflow("response_trailer", 64),
            requests: 0,
        }
    }

    pub fn requests(&self) -> u64 {
        self.requests
    }
}

impl Default for HttpElement {
    fn default() -> Self {
        Self::new()
    }
}

impl CcekElement for HttpElement {
    fn key(&self) -> &'static str {
        "HttpElement"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct HttpKey;

impl HttpKey {
    pub fn requests(elt: &HttpElement) -> u64 {
        elt.requests()
    }
}

impl CcekKey for HttpKey {
    type Element = HttpElement;
}

// ============================================================================
// SCTP Element - SCTP with chunks, heartbeats, notifications
// ============================================================================

pub struct SctpElement {
    pub delta: Delta<NetPacket>,
    associations: u32,
}

impl SctpElement {
    pub fn new() -> Self {
        Self {
            delta: Delta::new()
                .add_inlet("chunk", 256)
                .add_inlet("heartbeat", 64)
                .add_inlet("init", 64)
                .add_tributary("data_chunk", 256)
                .add_tributary("sack_chunk", 128)
                .add_tributary("heartbeat_ack", 64)
                .add_tributary("error_chunk", 64)
                .add_outflow("chunk_out", 256)
                .add_outflow("notify", 128),
            associations: 0,
        }
    }

    pub fn associations(&self) -> u32 {
        self.associations
    }
}

impl Default for SctpElement {
    fn default() -> Self {
        Self::new()
    }
}

impl CcekElement for SctpElement {
    fn key(&self) -> &'static str {
        "SctpElement"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct SctpKey;

impl SctpKey {
    pub fn associations(elt: &SctpElement) -> u32 {
        elt.associations()
    }
}

impl CcekKey for SctpKey {
    type Element = SctpElement;
}

// ============================================================================
// NIO Element - Non-blocking I/O with read/write channels
// ============================================================================

pub struct NioElement {
    pub delta: Delta<NetPacket>,
    active_fds: u32,
    max_fds: u32,
}

impl NioElement {
    pub fn new(max_fds: u32) -> Self {
        Self {
            delta: Delta::new()
                .add_inlet("read", 512)
                .add_inlet("write", 512)
                .add_inlet("accept", 128)
                .add_inlet("connect", 128)
                .add_tributary("read_ready", 256)
                .add_tributary("write_ready", 256)
                .add_tributary("error", 64)
                .add_outflow("read_complete", 256)
                .add_outflow("write_complete", 256)
                .add_outflow("accept_complete", 128)
                .add_outflow("connect_complete", 128),
            active_fds: 0,
            max_fds,
        }
    }

    pub fn active_fds(&self) -> u32 {
        self.active_fds
    }

    pub fn max_fds(&self) -> u32 {
        self.max_fds
    }
}

impl CcekElement for NioElement {
    fn key(&self) -> &'static str {
        "NioElement"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct NioKey;

impl NioKey {
    pub fn active_fds(elt: &NioElement) -> u32 {
        elt.active_fds()
    }

    pub fn max_fds(elt: &NioElement) -> u32 {
        elt.max_fds()
    }
}

impl CcekKey for NioKey {
    type Element = NioElement;
}
