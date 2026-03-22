//! CCEK SDK - CoroutineContext Element Key pattern
//!
//! Elements ARE Coroutines. Context hosts Coroutine[Contexts].
//!
//! ## Kotlin River Mapping
//!
//! | Kotlin | Literbike River | Description |
//! |--------|-----------------|-------------|
//! | CoroutineContext | CcekContext | Compile-time optimized map |
//! | CoroutineContext.Element | CcekElement | Protocol element |
//! | CoroutineContext.Key | CcekKey | Const singleton factory |
//! | Channel | Channel<T> | River connecting tributaries |
//! | Flow | RiverFlow<T> | Cold async stream |
//! | Job | ProtocolJob | Cancellable task handle |
//! | CoroutineScope | CcekScope | Structured concurrency scope |

pub mod channels;
pub mod context;
pub mod elements;
pub mod keys;
pub mod scope;
pub mod traits;

pub use channels::{Channel, ChannelRx, ChannelTx, ChannelError};
pub use context::{CcekContext, CcekElement, CcekKey, EmptyContext};
pub use elements::{HtxElement, QuicElement, HttpElement, SctpElement, NioElement};
pub use elements::{HtxKey, QuicKey, HttpKey, SctpKey, NioKey};
pub use keys::*;
pub use scope::{CcekScope, CcekScopeHandle, CcekScopeRef, CcekElementAdd, CcekLocal, ScopeExt};
