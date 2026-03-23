//! Uring - liburing facade
//! 
//! Zero-allocation liburing facade with userspace fallback.

pub mod uring_facade;
pub mod liburing_facade;

pub use uring_facade::{UringFacade, SqEntry, CqEntry, OpCode};
pub use liburing_facade::{LibUringFacade, OpResult};
