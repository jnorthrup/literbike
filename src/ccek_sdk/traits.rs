//! CCEK Traits - Many-to-one SDK bindings
//!
//! Traits represent SDK capabilities that can be bound many-to-one
//! into a CcekContext. Each trait maps to one Element.

use super::elements::*;
use super::keys::*;
use super::{CcekContext, CcekElement, CcekKey};

/// HTX verifier trait - bound many-to-one
pub trait HtxVerifier: Send + Sync {
    fn verify_ticket(&self, ticket: &[u8]) -> bool;
    fn tickets_processed(&self) -> u64;
    fn verified_count(&self) -> u64;
    fn failed_count(&self) -> u64;
}

/// QUIC engine trait - bound many-to-one
pub trait QuicEngine: Send + Sync {
    fn send_packet(&self, packet: &[u8]) -> Result<(), ()>;
    fn recv_packet(&self) -> Result<Option<Vec<u8>>, ()>;
    fn packets_sent(&self) -> u64;
    fn packets_received(&self) -> u64;
    fn connections(&self) -> u32;
    fn new_connection(&self);
}

/// NIO reactor trait - bound many-to-one
pub trait NioReactor: Send + Sync {
    fn submit_read(&self, fd: u32, buf: &mut [u8]) -> Result<u64, ()>;
    fn submit_write(&self, fd: u32, buf: &[u8]) -> Result<u64, ()>;
    fn active_fds(&self) -> u32;
    fn submitted_ops(&self) -> u64;
    fn completed_ops(&self) -> u64;
    fn register_fd(&self);
    fn unregister_fd(&self);
}

/// HTTP handler trait - bound many-to-one
pub trait HttpHandler: Send + Sync {
    fn handle_request(&self, req: &[u8]) -> Result<Vec<u8>, ()>;
    fn requests_handled(&self) -> u64;
    fn responses_sent(&self) -> u64;
    fn active_connections(&self) -> u32;
}

/// SCTP handler trait - bound many-to-one
pub trait SctpHandler: Send + Sync {
    fn send_chunk(&self, chunk: &[u8]) -> Result<(), ()>;
    fn recv_chunk(&self) -> Result<Option<Vec<u8>>, ()>;
    fn chunks_sent(&self) -> u64;
    fn chunks_received(&self) -> u64;
    fn associations(&self) -> u32;
}

// ============================================================================
// Trait Implementations bound to Elements via Keys (many-to-one)
// ============================================================================

impl HtxVerifier for HtxElement {
    fn verify_ticket(&self, _ticket: &[u8]) -> bool {
        self.increment_verified();
        true
    }

    fn tickets_processed(&self) -> u64 {
        *self.tickets_processed.read().unwrap()
    }

    fn verified_count(&self) -> u64 {
        *self.verified_count.read().unwrap()
    }

    fn failed_count(&self) -> u64 {
        *self.failed_count.read().unwrap()
    }
}

impl QuicEngine for QuicElement {
    fn send_packet(&self, _packet: &[u8]) -> Result<(), ()> {
        self.increment_out(1);
        Ok(())
    }

    fn recv_packet(&self) -> Result<Option<Vec<u8>>, ()> {
        Ok(None)
    }

    fn packets_sent(&self) -> u64 {
        *self.packets_out.read().unwrap()
    }

    fn packets_received(&self) -> u64 {
        *self.packets_in.read().unwrap()
    }

    fn connections(&self) -> u32 {
        *self.connections.read().unwrap()
    }

    fn new_connection(&self) {
        self.new_connection();
    }
}

impl NioReactor for NioElement {
    fn submit_read(&self, _fd: u32, _buf: &mut [u8]) -> Result<u64, ()> {
        self.submit_op();
        Ok(0)
    }

    fn submit_write(&self, _fd: u32, _buf: &[u8]) -> Result<u64, ()> {
        self.submit_op();
        Ok(0)
    }

    fn active_fds(&self) -> u32 {
        *self.active_fds.read().unwrap()
    }

    fn submitted_ops(&self) -> u64 {
        *self.submitted_ops.read().unwrap()
    }

    fn completed_ops(&self) -> u64 {
        *self.completed_ops.read().unwrap()
    }

    fn register_fd(&self) {
        self.register_fd();
    }

    fn unregister_fd(&self) {
        self.unregister_fd();
    }
}

impl HttpHandler for HttpElement {
    fn handle_request(&self, _req: &[u8]) -> Result<Vec<u8>, ()> {
        self.increment_requests();
        self.increment_responses();
        Ok(Vec::new())
    }

    fn requests_handled(&self) -> u64 {
        *self.requests.read().unwrap()
    }

    fn responses_sent(&self) -> u64 {
        *self.responses.read().unwrap()
    }

    fn active_connections(&self) -> u32 {
        *self.active_connections.read().unwrap()
    }
}

impl SctpHandler for SctpElement {
    fn send_chunk(&self, _chunk: &[u8]) -> Result<(), ()> {
        self.increment_out(1);
        Ok(())
    }

    fn recv_chunk(&self) -> Result<Option<Vec<u8>>, ()> {
        Ok(None)
    }

    fn chunks_sent(&self) -> u64 {
        *self.chunks_out.read().unwrap()
    }

    fn chunks_received(&self) -> u64 {
        *self.chunks_in.read().unwrap()
    }

    fn associations(&self) -> u32 {
        *self.associations.read().unwrap()
    }
}

// ============================================================================
// Context Builder Functions (convenience)
// ============================================================================

/// Add HTX verification to context (many-to-one binding)
#[cfg(feature = "htx")]
pub fn htx_verifier(ctx: CcekContext) -> CcekContext {
    ctx.with::<HtxInputKey>(HtxElement::new())
        .with::<HtxOutputKey>(HtxElement::new())
        .with::<HtxSessionKey>(HtxElement::new())
}

/// Add QUIC engine to context (many-to-one binding)
#[cfg(feature = "quic")]
pub fn quic_engine(ctx: CcekContext) -> CcekContext {
    ctx.with::<QuicPacketsInKey>(QuicElement::new())
        .with::<QuicPacketsOutKey>(QuicElement::new())
        .with::<QuicSessionKey>(QuicElement::new())
        .with::<QuicConnectionKey>(QuicElement::new())
}

/// Add NIO reactor to context (many-to-one binding)
#[cfg(feature = "userspace-nio")]
pub fn nio_reactor(ctx: CcekContext) -> CcekContext {
    ctx.with::<NioReadReadyKey>(NioElement::new())
        .with::<NioWriteReadyKey>(NioElement::new())
        .with::<NioSubmittedKey>(NioElement::new())
        .with::<NioCompletedKey>(NioElement::new())
        .with::<NioSessionKey>(NioElement::new())
}

/// Add HTTP handler to context (many-to-one binding)
#[cfg(feature = "http")]
pub fn http_handler(ctx: CcekContext) -> CcekContext {
    ctx.with::<HttpRequestKey>(HttpElement::new())
        .with::<HttpResponseKey>(HttpElement::new())
        .with::<HttpSessionKey>(HttpElement::new())
}
