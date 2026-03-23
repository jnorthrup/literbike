//! HTXKE - Exact Kotlin kotlinx-coroutines translation (LEGACY)
//!
//! This is the original CCEK concept. The new implementation is in src/ccek/

pub mod channels;
// pub mod context; // TODO: stub out missing modules
pub mod elements;
pub mod kotlin_mirror;
pub mod scope;
pub mod traits;

pub use kotlin_mirror::{
    CoroutineContext, Element, Key, Job, Coroutine, CoroutineScope,
    Flow, FlowCollector, SendChannel, ReceiveChannel, ChannelResult,
    EmptyCoroutineContext, coroutine_scope, AnyElement, KeyAny,
};
pub use elements::{HtxElement, HtxKey, QuicElement, QuicKey, HttpElement, HttpKey, SctpElement, SctpKey, NioElement, NioKey};
pub use scope::Scope;
pub use channels::{Channel, ChannelRx, ChannelTx, ChannelError};
