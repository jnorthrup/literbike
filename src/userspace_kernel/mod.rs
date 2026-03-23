//! Unified Kernel Abstractions
//!
//! Provides high-performance, zero-overhead interfaces to:
//! - io_uring for kernel I/O
//! - eBPF JIT compilation  
//! - Memory-mapped I/O
//! - Kernel bypass techniques

// Kernel feature detection and capabilities
pub mod kernel_capabilities;

// Core kernel interface modules
#[cfg(target_os = "linux")]
pub mod io_uring;

pub mod nio;

pub mod syscall_net;

pub mod posix_sockets;

// Performance and optimization modules
pub mod ebpf;

pub mod ebpf_mmap;

pub mod densified_ops;

pub mod endgame_bypass;

pub mod knox_proxy;

pub mod tethering_bypass;

pub mod syscall;

pub mod uring;

// Re-exports
pub use endgame_bypass::{DensifiedKernel, IoUringParams};
pub use kernel_capabilities::SystemCapabilities;
pub use nio::{NioChannel, SimpleReactor};
pub use posix_sockets::{PosixSocket, SocketPair};
pub use syscall_net::{NetworkInterface, SocketOps};
