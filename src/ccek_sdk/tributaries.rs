//! Protocol Tributaries - CCEK channelized protocol components
//!
//! Each tributary is a compile-time bound channel connecting
//! a protocol component to the ENDGAME processing engine.

use crate::ccek_sdk::{CcekContext, CcekElement, CcekKey, Channel, ChannelRx, ChannelTx};

/// Key for HTX verification tributary
pub struct HtxVerifierKey;

impl CcekKey for HtxVerifierKey {
    type Element = HtxTributary;
}

/// HTX ticket verification tributary
pub struct HtxTributary {
    pub input: ChannelTx<Vec<u8>>,
    pub output: ChannelRx<bool>,
}

impl HtxTributary {
    pub fn new(capacity: usize) -> Self {
        let channel = Channel::new(capacity);
        let (input, output) = channel.split();
        Self { input, output }
    }
}

/// Key for QUIC engine tributary  
pub struct QuicEngineKey;

impl CcekKey for QuicEngineKey {
    type Element = QuicTributary;
}

/// QUIC protocol tributary
pub struct QuicTributary {
    pub packets_in: ChannelRx<Vec<u8>>,
    pub packets_out: ChannelTx<Vec<u8>>,
}

impl QuicTributary {
    pub fn new(capacity: usize) -> Self {
        let channel = Channel::new(capacity);
        let (tx, rx) = channel.split();
        Self {
            packets_in: rx,
            packets_out: tx,
        }
    }
}

/// Protocol tributary trait - all tributaries implement this
pub trait ProtocolTributary: Send + Sync {
    fn name(&self) -> &'static str;
    fn channel_count(&self) -> usize;
}

/// Key for NIO reactor tributary
pub struct NioReactorKey;

impl CcekKey for NioReactorKey {
    type Element = NioTributary;
}

/// NIO reactor tributary flowing into ENDGAME
pub struct NioTributary {
    pub read_ready: ChannelRx<u32>,
    pub write_ready: ChannelRx<u32>,
    pub submitted: ChannelTx<u64>,
}

impl NioTributary {
    pub fn new(capacity: usize) -> Self {
        let channel = Channel::new(capacity);
        let (tx, rx) = channel.split();
        Self {
            read_ready: rx.clone(),
            write_ready: rx,
            submitted: tx,
        }
    }
}

/// Compile-time bindings for tributaries
#[cfg(feature = "htx")]
pub fn htx_verifier(ctx: CcekContext) -> CcekContext {
    ctx.with::<HtxVerifierKey>(HtxTributary::new(1024))
}

#[cfg(feature = "quic")]
pub fn quic_engine(ctx: CcekContext) -> CcekContext {
    ctx.with::<QuicEngineKey>(QuicTributary::new(1024))
}

#[cfg(feature = "userspace-nio")]
pub fn nio_reactor(ctx: CcekContext) -> CcekContext {
    ctx.with::<NioReactorKey>(NioTributary::new(1024))
}
