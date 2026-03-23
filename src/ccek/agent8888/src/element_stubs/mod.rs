//! Element stubs - protocol Element/Key definitions
//!
//! Submodules define the CCEK protocol hierarchy:
//! - protocol: Core protocol detection (always enabled)
//! - io: I/O abstractions (feature-gated)
//! - matcher: Speculative parsing (feature-gated)
//! - listener: TCP listener (feature-gated)
//! - reactor: Event loop (feature-gated)
//! - timer: Timer wheel (feature-gated)
//! - handler: Protocol handlers (feature-gated)

pub mod protocol;

#[cfg(feature = "io")]
pub mod io;

#[cfg(feature = "matcher")]
pub mod matcher;

#[cfg(feature = "listener")]
pub mod listener;

#[cfg(feature = "reactor")]
pub mod reactor;

#[cfg(feature = "timer")]
pub mod timer;

#[cfg(feature = "handler")]
pub mod handler;
