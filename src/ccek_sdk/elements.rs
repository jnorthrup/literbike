//! CCEK Elements - Protocol deltas with multiple inlets, tributaries, outflows
//!
//! Each protocol is a river delta with:
//! - INLETS: incoming data sources
//! - TRIBUTARIES: branching sub-streams  
//! - OUTFLOWS: outgoing data sinks

use super::delta::{Delta, Inlet, Outflow, Tributary};
use super::{CcekContext, CcekElement, CcekKey};

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
}

impl HtxElement {
    pub fn new() -> Self {
        Self {
            delta: Delta::new()
                .add_inlet("ticket", 64) // incoming tickets
                .add_inlet("challenge", 64) // challenge requests
                .add_tributary("verified", 128) // verified tickets branch
                .add_tributary("rejected", 64) // rejected tickets branch
                .add_outflow("response", 64), // verification responses
        }
    }

    pub fn ticket_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("ticket")
    }

    pub fn challenge_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("challenge")
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
}

pub struct HtxKey;

impl HtxKey {
    pub fn element() -> HtxElement {
        HtxElement::new()
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
}

impl QuicElement {
    pub fn new() -> Self {
        Self {
            delta: Delta::new()
                .add_inlet("packet", 256) // incoming packets
                .add_inlet("stream_init", 64) // stream initialization
                .add_tributary("stream_0", 128) // stream 0 (control)
                .add_tributary("stream_data", 256) // data streams
                .add_tributary("stream_close", 64) // stream close
                .add_outflow("packet_out", 256) // outgoing packets
                .add_outflow("stream_out", 128), // stream data out
        }
    }

    pub fn packet_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("packet")
    }

    pub fn stream_init_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("stream_init")
    }

    pub fn stream_0_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("stream_0")
    }

    pub fn stream_data_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("stream_data")
    }

    pub fn stream_close_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("stream_close")
    }

    pub fn packet_out_outflow(&self) -> Option<Outflow<NetPacket>> {
        self.delta.outflow("packet_out")
    }

    pub fn stream_out_outflow(&self) -> Option<Outflow<NetPacket>> {
        self.delta.outflow("stream_out")
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
}

pub struct QuicKey;

impl QuicKey {
    pub fn element() -> QuicElement {
        QuicElement::new()
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
}

impl HttpElement {
    pub fn new() -> Self {
        Self {
            delta: Delta::new()
                .add_inlet("request_head", 128) // request headers
                .add_inlet("request_body", 256) // request body
                .add_inlet("request_trailer", 64) // request trailers
                .add_tributary("header_parse", 128) // parsed headers
                .add_tributary("body_chunk", 256) // body chunks
                .add_tributary("trailer_parse", 64) // parsed trailers
                .add_outflow("response_head", 128) // response headers
                .add_outflow("response_body", 256) // response body
                .add_outflow("response_trailer", 64), // response trailers
        }
    }

    pub fn request_head_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("request_head")
    }

    pub fn request_body_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("request_body")
    }

    pub fn request_trailer_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("request_trailer")
    }

    pub fn header_parse_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("header_parse")
    }

    pub fn body_chunk_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("body_chunk")
    }

    pub fn trailer_parse_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("trailer_parse")
    }

    pub fn response_head_outflow(&self) -> Option<Outflow<NetPacket>> {
        self.delta.outflow("response_head")
    }

    pub fn response_body_outflow(&self) -> Option<Outflow<NetPacket>> {
        self.delta.outflow("response_body")
    }

    pub fn response_trailer_outflow(&self) -> Option<Outflow<NetPacket>> {
        self.delta.outflow("response_trailer")
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
}

pub struct HttpKey;

impl HttpKey {
    pub fn element() -> HttpElement {
        HttpElement::new()
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
}

impl SctpElement {
    pub fn new() -> Self {
        Self {
            delta: Delta::new()
                .add_inlet("chunk", 256) // incoming chunks
                .add_inlet("heartbeat", 64) // heartbeat requests
                .add_inlet("init", 64) // INIT chunks
                .add_tributary("data_chunk", 256) // DATA chunks
                .add_tributary("sack_chunk", 128) // SACK chunks
                .add_tributary("heartbeat_ack", 64) // heartbeat acks
                .add_tributary("error_chunk", 64) // ERROR chunks
                .add_outflow("chunk_out", 256) // outgoing chunks
                .add_outflow("notify", 128), // notifications
        }
    }

    pub fn chunk_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("chunk")
    }

    pub fn heartbeat_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("heartbeat")
    }

    pub fn init_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("init")
    }

    pub fn data_chunk_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("data_chunk")
    }

    pub fn sack_chunk_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("sack_chunk")
    }

    pub fn heartbeat_ack_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("heartbeat_ack")
    }

    pub fn error_chunk_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("error_chunk")
    }

    pub fn chunk_out_outflow(&self) -> Option<Outflow<NetPacket>> {
        self.delta.outflow("chunk_out")
    }

    pub fn notify_outflow(&self) -> Option<Outflow<NetPacket>> {
        self.delta.outflow("notify")
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
}

pub struct SctpKey;

impl SctpKey {
    pub fn element() -> SctpElement {
        SctpElement::new()
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
}

impl NioElement {
    pub fn new(max_fds: u32) -> Self {
        Self {
            delta: Delta::new()
                .add_inlet("read", 512) // read requests
                .add_inlet("write", 512) // write requests
                .add_inlet("accept", 128) // accept requests
                .add_inlet("connect", 128) // connect requests
                .add_tributary("read_ready", 256) // read ready fds
                .add_tributary("write_ready", 256) // write ready fds
                .add_tributary("error", 64) // error events
                .add_outflow("read_complete", 256) // read completions
                .add_outflow("write_complete", 256) // write completions
                .add_outflow("accept_complete", 128) // accept completions
                .add_outflow("connect_complete", 128), // connect completions
        }
    }

    pub fn read_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("read")
    }

    pub fn write_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("write")
    }

    pub fn accept_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("accept")
    }

    pub fn connect_inlet(&self) -> Option<Inlet<NetPacket>> {
        self.delta.inlet("connect")
    }

    pub fn read_ready_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("read_ready")
    }

    pub fn write_ready_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("write_ready")
    }

    pub fn error_tributary(&self) -> Option<Tributary<NetPacket>> {
        self.delta.tributary("error")
    }

    pub fn read_complete_outflow(&self) -> Option<Outflow<NetPacket>> {
        self.delta.outflow("read_complete")
    }

    pub fn write_complete_outflow(&self) -> Option<Outflow<NetPacket>> {
        self.delta.outflow("write_complete")
    }

    pub fn accept_complete_outflow(&self) -> Option<Outflow<NetPacket>> {
        self.delta.outflow("accept_complete")
    }

    pub fn connect_complete_outflow(&self) -> Option<Outflow<NetPacket>> {
        self.delta.outflow("connect_complete")
    }
}

impl CcekElement for NioElement {
    fn key(&self) -> &'static str {
        "NioElement"
    }
}

pub struct NioKey;

impl NioKey {
    pub fn element(max_fds: u32) -> NioElement {
        NioElement::new(max_fds)
    }
}

impl CcekKey for NioKey {
    type Element = NioElement;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_htx_delta() {
        let htx = HtxElement::new();
        assert!(htx.ticket_inlet().is_some());
        assert!(htx.verified_tributary().is_some());
        assert!(htx.response_outflow().is_some());
    }

    #[test]
    fn test_http_delta() {
        let http = HttpElement::new();
        assert!(http.request_head_inlet().is_some());
        assert!(http.request_body_inlet().is_some());
        assert!(http.response_body_outflow().is_some());
    }

    #[test]
    fn test_quic_delta() {
        let quic = QuicElement::new();
        assert!(quic.packet_inlet().is_some());
        assert!(quic.stream_data_tributary().is_some());
        assert!(quic.packet_out_outflow().is_some());
    }

    #[test]
    fn test_nio_delta() {
        let nio = NioElement::new(1024);
        assert!(nio.read_inlet().is_some());
        assert!(nio.write_inlet().is_some());
        assert!(nio.read_ready_tributary().is_some());
        assert!(nio.read_complete_outflow().is_some());
    }

    #[test]
    fn test_sctp_delta() {
        let sctp = SctpElement::new();
        assert!(sctp.chunk_inlet().is_some());
        assert!(sctp.data_chunk_tributary().is_some());
        assert!(sctp.notify_outflow().is_some());
    }
}
