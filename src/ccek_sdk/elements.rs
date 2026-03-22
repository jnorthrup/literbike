//! CCEK Elements - Kotlin CoroutineContext.Element implementations

use super::{CcekContext, CcekElement, CcekKey};
use std::any::TypeId;

// ============================================================================
// HTX Element
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
    fn key(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

pub struct HtxKey;

impl CcekKey for HtxKey {
    type Element = HtxElement;
}

impl HtxKey {
    pub fn connections(elt: &HtxElement) -> u32 {
        elt.connections()
    }
    pub fn verify(elt: &HtxElement, packet: &[u8]) -> bool {
        elt.verify(packet)
    }
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
    fn key(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

pub struct QuicKey;

impl CcekKey for QuicKey {
    type Element = QuicElement;
}

impl QuicKey {
    pub fn connections(elt: &QuicElement) -> u32 {
        elt.connections()
    }
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
    fn key(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

pub struct HttpKey;

impl CcekKey for HttpKey {
    type Element = HttpElement;
}

impl HttpKey {
    pub fn requests(elt: &HttpElement) -> u64 {
        elt.requests()
    }
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
    fn key(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

pub struct SctpKey;

impl CcekKey for SctpKey {
    type Element = SctpElement;
}

impl SctpKey {
    pub fn associations(elt: &SctpElement) -> u32 {
        elt.associations()
    }
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
    fn key(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

pub struct NioKey;

impl CcekKey for NioKey {
    type Element = NioElement;
}

impl NioKey {
    pub fn active_fds(elt: &NioElement) -> u32 {
        elt.active_fds()
    }
    pub fn max_fds(elt: &NioElement) -> u32 {
        elt.max_fds()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_htx() {
        let e = HtxElement::new();
        assert_eq!(e.connections(), 0);
        assert!(HtxKey::verify(&e, b"test"));
    }

    #[test]
    fn test_nio() {
        let e = NioElement::new(1024);
        assert_eq!(e.max_fds(), 1024);
    }
}
