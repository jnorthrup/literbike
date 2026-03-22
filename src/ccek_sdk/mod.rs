//! CCEK SDK - Compile-Time Channelized Protocol Bindings
//!
//! Architecture:
//! - Keys: Linkage points between components (one per channel)
//! - Elements: Hold session state for each SDK
//! - Traits: Many-to-one bindings into Context
//!
//! ```text
//! Protocol Tributaries (many Keys)
//!        │
//!        ▼
//! ┌─────────────────────────────────────┐
//! │     CcekContext (single context)     │
//! │                                     │
//! │  [HtxElement] ──► HTX session       │
//! │  [QuicElement] ──► QUIC session     │
//! │  [NioElement] ───► NIO session     │
//! │                                     │
//! └─────────────────────────────────────┘
//!        │
//!        ▼ (all flow into)
//! ┌─────────────────────────────────────┐
//! │         ENDGAME Reactor              │
//! └─────────────────────────────────────┘
//! ```

pub mod context;
pub mod channels;
pub mod keys;
pub mod elements;
pub mod traits;

pub use context::{CcekContext, CcekKey, CcekElement};
pub use channels::{Channel, ChannelRx, ChannelTx};

// SDK trait bindings (many-to-one into context)
#[cfg(feature = "htx")]
pub use traits::htx_verifier;

#[cfg(feature = "quic")]
pub use traits::quic_engine;

#[cfg(feature = "userspace-nio")]
pub use traits::nio_reactor;

#[cfg(feature = "http")]
pub use traits::http_handler;
