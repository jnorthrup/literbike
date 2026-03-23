//! CCEK Elements - Kotlin CoroutineContext.Element implementations

use super::{AnyElement, Coroutine, Element, Job, Key, KeyAny};
use std::any::TypeId;

// HTX -----------------------------------------------------------------------

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
    pub fn verify(&self, _packet: &[u8]) -> bool {
        true
    }
}

impl Element for HtxElement {
    fn key(&self) -> KeyAny {
        HtxKey
    }
}

pub struct HtxKey;

impl Key<HtxElement> for HtxKey {}

impl Job for HtxElement {
    fn is_active(&self) -> bool {
        true
    }
    fn is_completed(&self) -> bool {
        false
    }
    fn join(&self) {
        loop {}
    }
    fn cancel(&self) {}
}

impl Coroutine for HtxElement {}

// QUIC ----------------------------------------------------------------------

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

impl Element for QuicElement {
    fn key(&self) -> KeyAny {
        QuicKey
    }
}

pub struct QuicKey;

impl Key<QuicElement> for QuicKey {}

impl Job for QuicElement {
    fn is_active(&self) -> bool {
        true
    }
    fn is_completed(&self) -> bool {
        false
    }
    fn join(&self) {
        loop {}
    }
    fn cancel(&self) {}
}

impl Coroutine for QuicElement {}

// HTTP ----------------------------------------------------------------------

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

impl Element for HttpElement {
    fn key(&self) -> KeyAny {
        HttpKey
    }
}

pub struct HttpKey;

impl Key<HttpElement> for HttpKey {}

impl Job for HttpElement {
    fn is_active(&self) -> bool {
        true
    }
    fn is_completed(&self) -> bool {
        false
    }
    fn join(&self) {
        loop {}
    }
    fn cancel(&self) {}
}

impl Coroutine for HttpElement {}

// SCTP ----------------------------------------------------------------------

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

impl Element for SctpElement {
    fn key(&self) -> KeyAny {
        SctpKey
    }
}

pub struct SctpKey;

impl Key<SctpElement> for SctpKey {}

impl Job for SctpElement {
    fn is_active(&self) -> bool {
        true
    }
    fn is_completed(&self) -> bool {
        false
    }
    fn join(&self) {
        loop {}
    }
    fn cancel(&self) {}
}

impl Coroutine for SctpElement {}

// NIO ----------------------------------------------------------------------

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

impl Element for NioElement {
    fn key(&self) -> KeyAny {
        NioKey
    }
}

pub struct NioKey;

impl Key<NioElement> for NioKey {}

impl Job for NioElement {
    fn is_active(&self) -> bool {
        true
    }
    fn is_completed(&self) -> bool {
        false
    }
    fn join(&self) {
        loop {}
    }
    fn cancel(&self) {}
}

impl Coroutine for NioElement {}
