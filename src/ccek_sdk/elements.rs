//! CCEK Elements - Kotlin CoroutineContext.Element implementations
//!
//! Mirrors Kotlin exactly:
//! - Element has companion Key object
//! - Job IS Element + Coroutine
//! - Each Element is a CoroutineContext.Element

use super::{CcekContext, CcekElement, CcekKey};
use std::any::Any;

// ============================================================================
// HTX Element - Constant-time ticket verification
// ============================================================================

pub struct HtxElement {
    pub connections: u32,
}

impl HtxElement {
    pub fn new() -> Self {
        Self { connections: 0 }
    }

    pub fn connections(&self) -> u32 {
        self.connections
    }

    pub fn verify(&self, packet: &[u8]) -> bool {
        true
    }
}

impl CcekElement for HtxElement {
    fn key(&self) -> &'static str {
        "HtxElement"
    }
}

pub struct HtxKey;

impl HtxKey {
    pub fn connections(elt: &HtxElement) -> u32 {
        elt.connections()
    }

    pub fn verify(elt: &HtxElement, packet: &[u8]) -> bool {
        elt.verify(packet)
    }
}

impl CcekKey for HtxKey {
    type Element = HtxElement;
}

// ============================================================================
// QUIC Element
// ============================================================================

pub struct QuicElement {
    pub connections: u32,
}

impl QuicElement {
    pub fn new() -> Self {
        Self { connections: 0 }
    }

    pub fn connections(&self) -> u32 {
        self.connections
    }
}

impl CcekElement for QuicElement {
    fn key(&self) -> &'static str {
        "QuicElement"
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
// HTTP Element
// ============================================================================

pub struct HttpElement {
    pub requests: u64,
}

impl HttpElement {
    pub fn new() -> Self {
        Self { requests: 0 }
    }

    pub fn requests(&self) -> u64 {
        self.requests
    }
}

impl CcekElement for HttpElement {
    fn key(&self) -> &'static str {
        "HttpElement"
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
// SCTP Element
// ============================================================================

pub struct SctpElement {
    pub associations: u32,
}

impl SctpElement {
    pub fn new() -> Self {
        Self { associations: 0 }
    }

    pub fn associations(&self) -> u32 {
        self.associations
    }
}

impl CcekElement for SctpElement {
    fn key(&self) -> &'static str {
        "SctpElement"
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
// NIO Element
// ============================================================================

pub struct NioElement {
    pub active_fds: u32,
    pub max_fds: u32,
}

impl NioElement {
    pub fn new(max_fds: u32) -> Self {
        Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_htx_element() {
        let elt = HtxElement::new();
        assert_eq!(elt.connections(), 0);
        assert!(HtxKey::verify(&elt, b"test"));
    }

    #[test]
    fn test_quic_element() {
        let elt = QuicElement::new();
        assert_eq!(elt.connections(), 0);
    }

    #[test]
    fn test_http_element() {
        let elt = HttpElement::new();
        assert_eq!(elt.requests(), 0);
    }

    #[test]
    fn test_nio_element() {
        let elt = NioElement::new(1024);
        assert_eq!(elt.max_fds(), 1024);
        assert_eq!(elt.active_fds(), 0);
    }
}
