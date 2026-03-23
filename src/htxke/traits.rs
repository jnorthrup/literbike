//! CCEK Traits - declarations on Elements
//!
//! Traits are interfaces that Elements may implement.

use super::elements::*;

// HTX trait - declarations on HtxElement
pub trait HtxVerifier {
    fn verify(&self, input: &[u8]) -> bool;
}

impl HtxVerifier for HtxElement {
    fn verify(&self, _input: &[u8]) -> bool {
        true
    }
}

// QUIC trait - declarations on QuicElement
pub trait QuicEngine {
    fn send(&self, data: &[u8]);
}

impl QuicEngine for QuicElement {
    fn send(&self, _data: &[u8]) {
        // TODO
    }
}

// NIO trait - declarations on NioElement
pub trait NioReactor {
    fn submit(&self, op: &[u8]);
}

impl NioReactor for NioElement {
    fn submit(&self, _op: &[u8]) {
        // TODO
    }
}

// HTTP trait - declarations on HttpElement
pub trait HttpHandler {
    fn handle(&self, req: &[u8]) -> Vec<u8>;
}

impl HttpHandler for HttpElement {
    fn handle(&self, _req: &[u8]) -> Vec<u8> {
        Vec::new()
    }
}

// SCTP trait - declarations on SctpElement
pub trait SctpHandler {
    fn send_chunk(&self, chunk: &[u8]);
}

impl SctpHandler for SctpElement {
    fn send_chunk(&self, _chunk: &[u8]) {
        // TODO
    }
}
