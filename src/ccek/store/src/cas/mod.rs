//! CAS (Content-Addressed Storage) module
//!
//! Provides content-addressed storage with gateway and backend adapters.
//! Based on original implementation from src/cas_storage.rs, src/cas_gateway.rs, src/cas_backends.rs.

pub mod storage;
pub mod gateway;
pub mod backends;

pub use storage::*;
pub use gateway::*;
pub use backends::*;
