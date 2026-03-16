//! Syscall network operations from userspace crate.
//!
//! This module re-exports network operations from the userspace crate
//! for direct syscall-based networking with kernel bypass capabilities.

pub use userspace::kernel::posix_sockets::{PosixSocket, SocketPair};
pub use userspace::kernel::syscall_net::{
    get_default_gateway, get_default_local_ipv4, NetworkInterface, SocketOps,
};
