//! CCEK Elements - Coroutines with explicit locality
//!
//! Elements ARE Coroutines. Each Element is an async fn returning Self.
//! Keys are static const factories for these Coroutines.
//!
//! Pattern:
//! ```rust
//! async fn htx_element() -> HtxElement { HtxElement::new() }
//! const HtxKey = HtxFactory { create: htx_element };
//! ```

use super::{CcekContext, CcekElement, CcekKey};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};

// ============================================================================
// HTX Element - async Coroutine
// ============================================================================

pub struct HtxElement;

impl HtxElement {
    pub const fn new() -> Self {
        Self
    }
}

impl CcekElement for HtxElement {
    fn key(&self) -> &'static str {
        "HtxElement"
    }
}

impl Future for HtxElement {
    type Output = Self;
    fn poll(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        Poll::Ready(self.get_mut().clone())
    }
}

impl Clone for HtxElement {
    fn clone(&self) -> Self {
        Self
    }
}

pub struct HtxKey;

impl HtxKey {
    pub const fn create() -> HtxElement {
        HtxElement::new()
    }
}

impl CcekKey for HtxKey {
    type Element = HtxElement;
}

// ============================================================================
// QUIC Element - async Coroutine
// ============================================================================

pub struct QuicElement {
    pub connections: u32,
}

impl QuicElement {
    pub const fn new() -> Self {
        Self { connections: 0 }
    }
}

impl CcekElement for QuicElement {
    fn key(&self) -> &'static str {
        "QuicElement"
    }
}

impl Future for QuicElement {
    type Output = Self;
    fn poll(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        Poll::Ready(self.get_mut().clone())
    }
}

impl Clone for QuicElement {
    fn clone(&self) -> Self {
        Self {
            connections: self.connections,
        }
    }
}

pub struct QuicKey;

impl QuicKey {
    pub const fn create() -> QuicElement {
        QuicElement::new()
    }
}

impl CcekKey for QuicKey {
    type Element = QuicElement;
}

// ============================================================================
// NIO Element - async Coroutine
// ============================================================================

pub struct NioElement {
    pub active_fds: u32,
    pub max_fds: u32,
}

impl NioElement {
    pub const fn new(max_fds: u32) -> Self {
        Self {
            active_fds: 0,
            max_fds,
        }
    }
}

impl CcekElement for NioElement {
    fn key(&self) -> &'static str {
        "NioElement"
    }
}

impl Future for NioElement {
    type Output = Self;
    fn poll(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        Poll::Ready(self.get_mut().clone())
    }
}

impl Clone for NioElement {
    fn clone(&self) -> Self {
        Self {
            active_fds: self.active_fds,
            max_fds: self.max_fds,
        }
    }
}

pub struct NioKey;

impl NioKey {
    pub const fn create(max_fds: u32) -> NioElement {
        NioElement::new(max_fds)
    }
}

impl CcekKey for NioKey {
    type Element = NioElement;
}

// ============================================================================
// HTTP Element - async Coroutine
// ============================================================================

pub struct HttpElement {
    pub requests: u64,
}

impl HttpElement {
    pub const fn new() -> Self {
        Self { requests: 0 }
    }
}

impl CcekElement for HttpElement {
    fn key(&self) -> &'static str {
        "HttpElement"
    }
}

impl Future for HttpElement {
    type Output = Self;
    fn poll(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        Poll::Ready(self.get_mut().clone())
    }
}

impl Clone for HttpElement {
    fn clone(&self) -> Self {
        Self {
            requests: self.requests,
        }
    }
}

pub struct HttpKey;

impl HttpKey {
    pub const fn create() -> HttpElement {
        HttpElement::new()
    }
}

impl CcekKey for HttpKey {
    type Element = HttpElement;
}

// ============================================================================
// SCTP Element - async Coroutine
// ============================================================================

pub struct SctpElement {
    pub associations: u32,
}

impl SctpElement {
    pub const fn new() -> Self {
        Self { associations: 0 }
    }
}

impl CcekElement for SctpElement {
    fn key(&self) -> &'static str {
        "SctpElement"
    }
}

impl Future for SctpElement {
    type Output = Self;
    fn poll(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        Poll::Ready(self.get_mut().clone())
    }
}

impl Clone for SctpElement {
    fn clone(&self) -> Self {
        Self {
            associations: self.associations,
        }
    }
}

pub struct SctpKey;

impl SctpKey {
    pub const fn create() -> SctpElement {
        SctpElement::new()
    }
}

impl CcekKey for SctpKey {
    type Element = SctpElement;
}
