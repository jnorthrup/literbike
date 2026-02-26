//! Reactor foundation modules (portable baseline + compatibility stub).

pub mod channel;
pub mod context;
pub mod handler;
pub mod operation;
pub mod platform;
pub mod reactor;
pub mod selector;
pub mod simple_reactor;
pub mod timer;

pub use reactor::{Reactor, ReactorTickResult};
pub use simple_reactor::SimpleReactor;
