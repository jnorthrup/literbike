//! Network abstractions and protocol adapters
//!
//! This module provides unified network protocol handling with adapters
//! for HTTP, QUIC, SSH, and other protocols.

pub mod adapters;
pub mod channels;
pub mod protocols;

pub use adapters::{AdapterType, NetworkAdapter};
pub use channels::{Channel, ChannelProvider};
pub use protocols::{Protocol, ProtocolDetector};
