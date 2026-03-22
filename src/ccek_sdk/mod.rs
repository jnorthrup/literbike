//! CCEK SDK - Compile-Time Channelized Protocol Bindings
//!
//! CCEK (CoroutineContext Element Key) provides:
//! - Compile-time bindings for services in the active context
//! - Channelized protocol tributaries flowing into ENDGAME
//! - Explicit SDK features that wire at compile time
//!
//! ## Architecture
//!
//! ```text
//! Protocol Tributaries (CCEK Channels)
//!        │
//!        ▼
//! ┌─────────────────────────────────────┐
//! │     Active Context (CoroutineContext) │
//! │                                     │
//! │  [DHTService] [ProtocolDetector]    │
//! │  [CRDTStorage] [HtxVerifier]      │
//! │  [QuicEngine] [HttpHandler]        │
//! └─────────────────────────────────────┘
//!        │
//!        ▼ (channelized flow)
//! ┌─────────────────────────────────────┐
//! │         ENDGAME Reactor              │
//! │    (Densification Processing Path)    │
//! └─────────────────────────────────────┘
//! ```

pub mod context;
pub mod channels;
pub mod tributaries;

pub use context::{CcekContext, CcekElement, CcekKey};
pub use channels::{Channel, ChannelRx, ChannelTx};
pub use tributaries::{ProtocolTributary, HtxTributary, QuicTributary};

// SDK feature bindings - these wire at compile time
#[cfg(feature = "htx")]
pub use tributaries::htx_verifier;

#[cfg(feature = "quic")]
pub use tributaries::quic_engine;

#[cfg(feature = "userspace-nio")]
pub use tributaries::nio_reactor;
