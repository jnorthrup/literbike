//! CCEK Key - 1:1 with Element, passive SDK provider
//!
//! Keys are hierarchical in source code to match runtime hierarchy:
//! ```text
//! src/htxke/key/
//! ├── htx/
//! │   ├── mod.rs      (HtxKey + HtxElement 1:1)
//! │   ├── crypto.rs   (HtxCryptoKey - nested)
//! │   └── packet.rs   (HtxPacketKey - nested)
//! ├── quic/
//! │   ├── mod.rs      (QuicKey + QuicElement 1:1)
//! │   └── ...
//! ```
//!
//! Kotlin: `interface Key<E : Element>`

pub mod htx;
pub mod quic;
pub mod http;
pub mod sctp;
pub mod nio;

pub use htx::{HtxKey, HtxElement};
pub use quic::{QuicKey, QuicElement};
pub use http::{HttpKey, HttpElement};
pub use sctp::{SctpKey, SctpElement};
pub use nio::{NioKey, NioElement};

/// Key trait - 1:1 with Element
pub trait Key: 'static {
    type Element: super::Element;
    const FACTORY: fn() -> Self::Element;
}
