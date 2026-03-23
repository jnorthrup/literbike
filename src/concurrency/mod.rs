//! Structured Concurrency for Literbike
//! 
//! Based on Kotlin Coroutines patterns:
//! - CoroutineContext.Element composition (CCEK pattern)
//! - Channel-based communication
//! - Flow-based reactive streams  
//! - Structured concurrency scopes
//!
//! # Integration with Tokio Ecosystem
//! 
//! This module integrates with the Tokio async ecosystem:
//! - **Channels**: Use `async-channel` crate for production
//! - **Streams**: Use `tokio-stream` for Flow-like patterns
//! - **Scopes**: Use `tokio::spawn` with CCEK context
//!
//! ```rust,no_run
//! use literbike::concurrency::*;
//! use std::sync::Arc;
//!
//! // Create context with services
//! let ctx = EmptyContext
//!     + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>
//!     + Arc::new(DHTService::new("node-1"));
//!
//! // Spawn with context
//! let handle = ctx.spawn_task(async {
//!     do_work().await
//! });
//!
//! // Create channel
//! let (tx, rx) = ctx.create_channel::<Message>(100);
//! ```

pub mod ccek;
pub mod channel;
pub mod scope;
pub mod flow;
pub mod bridge;

pub use ccek::{
    ContextKey, ContextElement, EmptyContext, CoroutineContext,
    ProtocolDetector, DHTService, CRDTStorage, CRDTNetwork, ConflictResolver,
};
pub use channel::{ChannelSender, ChannelReceiver, channel, channel_with_scope};
pub use scope::{CoroutineScope, SupervisorScope, spawn, launch, coroutine_scope, supervisor_scope, Job, CoroutineJob};
pub use bridge::{CcekRuntime, CoroutineContextExt};
pub use flow::{Flow, FlowBuilder, FlowOperator};

/// Core result type for coroutine operations
pub type CoroutineResult<T> = anyhow::Result<T>;


