//! CCEK SDK - CoroutineContext Element Key pattern
//!
//! Elements ARE Coroutines. Context hosts Coroutine[Contexts].
//! This guides the compiler through explicit performant locality.
//!
//! Pattern:
//! - Element = Coroutine (implements Future)
//! - Key = static const factory for Coroutine
//! - Context = host of Coroutine[Contexts]
//!
//! Usage:
//! ```rust
//! let ctx = EmptyContext
//!     + HtxKey::create()
//!     + QuicKey::create()
//!     + NioKey::create(1024);
//! ```

pub mod channels;
pub mod context;
pub mod elements;
pub mod keys;
pub mod traits;

pub use channels::{Channel, ChannelRx, ChannelTx};
pub use context::{CcekContext, CcekElement, CcekKey, EmptyContext};
pub use keys::*;
