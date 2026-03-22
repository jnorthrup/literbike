//! Unified protocol detection and handling
//!
//! This module provides protocol detection and routing using the userspace
//! kernel emulation network abstractions.

pub mod detector;

// Re-export from userspace
pub use crate::userspace_network::protocols::{Protocol, ProtocolDetector};

pub use detector::{detect_protocol, UnifiedDetector};

// Protocol handler traits
pub use detector::{HandlerResult, ProtocolHandler};
