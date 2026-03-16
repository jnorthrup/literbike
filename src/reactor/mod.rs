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
pub mod userspace_selector;

pub use reactor::{Reactor, ReactorTickResult};
pub use simple_reactor::SimpleReactor;
pub use userspace_selector::UserspaceSelector;
