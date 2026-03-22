//! Syscall network operations - inlined from userspace.
//!
//! This module re-exports network operations from the userspace_kernel module
//! for direct syscall-based networking with kernel bypass capabilities.

#[cfg(feature = "userspace-kernel")]
pub use crate::userspace_kernel::posix_sockets::{PosixSocket, SocketPair};

#[cfg(feature = "userspace-kernel")]
pub use crate::userspace_kernel::syscall_net::{
    get_default_gateway, get_default_local_ipv4, NetworkInterface, SocketOps,
};
