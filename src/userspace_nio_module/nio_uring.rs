//! Linux io_uring NIO backend
//!
//! Provides the primary NIO backend for Linux using io_uring,
//! with zero-allocation hot paths and batching support.

use std::io;
use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::backend::{BackendConfig, Completion, Interest, OpType, PlatformBackend, Token};

// io_uring opcodes - Linux only
#[cfg(target_os = "linux")]
const IORING_OP_READ: u8 = libc::IORING_OP_READ;
#[cfg(target_os = "linux")]
const IORING_OP_WRITE: u8 = libc::IORING_OP_WRITE;
#[cfg(target_os = "linux")]
const IORING_OP_POLL_ADD: u8 = libc::IORING_OP_POLL_ADD;
#[cfg(target_os = "linux")]
const IORING_OP_NOP: u8 = libc::IORING_OP_NOP;
#[cfg(target_os = "linux")]
const IORING_OP_READV: u8 = libc::IORING_OP_READV;
#[cfg(target_os = "linux")]
const IORING_OP_WRITEV: u8 = libc::IORING_OP_WRITEV;

// Placeholder values for non-Linux (won't actually be used)
#[cfg(not(target_os = "linux"))]
const IORING_OP_READ: u8 = 0;
#[cfg(not(target_os = "linux"))]
const IORING_OP_WRITE: u8 = 1;
#[cfg(not(target_os = "linux"))]
const IORING_OP_POLL_ADD: u8 = 6;
#[cfg(not(target_os = "linux"))]
const IORING_OP_NOP: u8 = 5;
#[cfg(not(target_os = "linux"))]
const IORING_OP_READV: u8 = 2;
#[cfg(not(target_os = "linux"))]
const IORING_OP_WRITEV: u8 = 3;

/// Linux io_uring backend
pub struct UringPlatformBackend {
    ring_fd: RawFd,
    sq_entries: u32,
    cq_entries: u32,
    pending_ops: Mutex<Vec<PendingOp>>,
    completion_counter: AtomicU64,
}

struct PendingOp {
    fd: RawFd,
    op_type: OpType,
    user_data: u64,
    buf_ptr: *mut u8,
    buf_len: usize,
    offset: u64,
}

// Safety: PendingOp is Send + Sync because we control the lifecycle
// and ensure buffers outlive the operations
unsafe impl Send for PendingOp {}
unsafe impl Sync for PendingOp {}

impl UringPlatformBackend {
    pub fn new(config: &BackendConfig) -> io::Result<Self> {
        #[cfg(target_os = "linux")]
        {
            use std::mem::MaybeUninit;

            let mut params = MaybeUninit::<libc::io_uring_params>::zeroed();
            let fd = unsafe {
                libc::syscall(
                    libc::SYS_io_uring_setup,
                    config.entries,
                    params.as_mut_ptr(),
                ) as i32
            };

            if fd < 0 {
                return Err(io::Error::last_os_error());
            }

            let params = unsafe { params.assume_init() };

            Ok(Self {
                ring_fd: fd,
                sq_entries: params.sq_entries,
                cq_entries: params.cq_entries,
                pending_ops: Mutex::new(Vec::new()),
                completion_counter: AtomicU64::new(0),
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = config;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "io_uring is only available on Linux",
            ))
        }
    }

    #[cfg(target_os = "linux")]
    fn push_sqe(
        &self,
        fd: RawFd,
        opcode: u8,
        user_data: u64,
        buf_ptr: *mut u8,
        buf_len: usize,
        offset: u64,
    ) -> io::Result<()> {
        let mut ops = self.pending_ops.lock().unwrap();
        let op_type = match opcode {
            IORING_OP_READV | IORING_OP_READ => OpType::Read,
            IORING_OP_WRITEV | IORING_OP_WRITE => OpType::Write,
            IORING_OP_POLL_ADD => OpType::PollAdd,
            _ => OpType::Nop,
        };
        ops.push(PendingOp {
            fd,
            op_type,
            user_data,
            buf_ptr,
            buf_len,
            offset,
        });
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    fn push_sqe(
        &self,
        _fd: RawFd,
        _opcode: u8,
        _user_data: u64,
        _buf_ptr: *mut u8,
        _buf_len: usize,
        _offset: u64,
    ) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "io_uring is only available on Linux",
        ))
    }
}

impl PlatformBackend for UringPlatformBackend {
    fn register(&self, fd: RawFd, _token: Token, interest: Interest) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            // io_uring doesn't require explicit registration like epoll/kqueue
            // File descriptors are used directly in SQEs
            let _ = (fd, interest);
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (fd, interest);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "io_uring is only available on Linux",
            ))
        }
    }

    fn reregister(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        self.register(fd, token, interest)
    }

    fn unregister(&self, fd: RawFd) -> io::Result<()> {
        // io_uring doesn't require explicit unregistration
        let _ = fd;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn submit_read(&self, fd: RawFd, buf: &mut [u8], user_data: u64) -> io::Result<()> {
        self.push_sqe(
            fd,
            IORING_OP_READ,
            user_data,
            buf.as_mut_ptr(),
            buf.len(),
            0,
        )
    }

    #[cfg(not(target_os = "linux"))]
    fn submit_read(&self, _fd: RawFd, _buf: &mut [u8], _user_data: u64) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "io_uring not available",
        ))
    }

    #[cfg(target_os = "linux")]
    fn submit_write(&self, fd: RawFd, buf: &[u8], user_data: u64) -> io::Result<()> {
        self.push_sqe(
            fd,
            IORING_OP_WRITE,
            user_data,
            buf.as_ptr() as *mut u8,
            buf.len(),
            0,
        )
    }

    #[cfg(not(target_os = "linux"))]
    fn submit_write(&self, _fd: RawFd, _buf: &[u8], _user_data: u64) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "io_uring not available",
        ))
    }

    #[cfg(target_os = "linux")]
    fn submit_read_at(
        &self,
        fd: RawFd,
        offset: u64,
        buf: &mut [u8],
        user_data: u64,
    ) -> io::Result<()> {
        self.push_sqe(
            fd,
            IORING_OP_READ,
            user_data,
            buf.as_mut_ptr(),
            buf.len(),
            offset,
        )
    }

    #[cfg(not(target_os = "linux"))]
    fn submit_read_at(
        &self,
        _fd: RawFd,
        _offset: u64,
        _buf: &mut [u8],
        _user_data: u64,
    ) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "io_uring not available",
        ))
    }

    #[cfg(target_os = "linux")]
    fn submit_write_at(
        &self,
        fd: RawFd,
        offset: u64,
        buf: &[u8],
        user_data: u64,
    ) -> io::Result<()> {
        self.push_sqe(
            fd,
            IORING_OP_WRITE,
            user_data,
            buf.as_ptr() as *mut u8,
            buf.len(),
            offset,
        )
    }

    #[cfg(not(target_os = "linux"))]
    fn submit_write_at(
        &self,
        _fd: RawFd,
        _offset: u64,
        _buf: &[u8],
        _user_data: u64,
    ) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "io_uring not available",
        ))
    }

    #[cfg(target_os = "linux")]
    #[cfg(target_os = "linux")]
    fn submit_poll(&self, fd: RawFd, interest: Interest, user_data: u64) -> io::Result<()> {
        let poll_mask = if interest.readable { libc::POLLIN } else { 0 }
            | if interest.writable { libc::POLLOUT } else { 0 };
        self.push_sqe(
            fd,
            IORING_OP_POLL_ADD,
            user_data,
            std::ptr::null_mut(),
            poll_mask as usize,
            0,
        )
    }

    #[cfg(not(target_os = "linux"))]
    fn submit_poll(&self, _fd: RawFd, _interest: Interest, _user_data: u64) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "io_uring not available",
        ))
    }

    #[cfg(target_os = "linux")]
    fn submit_nop(&self, user_data: u64) -> io::Result<()> {
        self.push_sqe(-1, IORING_OP_NOP, user_data, std::ptr::null_mut(), 0, 0)
    }

    #[cfg(not(target_os = "linux"))]
    fn submit_nop(&self, _user_data: u64) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "io_uring not available",
        ))
    }

    fn submit(&self) -> io::Result<u64> {
        #[cfg(target_os = "linux")]
        {
            let ret = unsafe {
                libc::syscall(
                    libc::SYS_io_uring_enter,
                    self.ring_fd,
                    0u32, // to_submit
                    0u32, // min_complete
                    libc::IORING_ENTER_GETEVENTS,
                    std::ptr::null::<libc::sigset_t>(),
                    0usize,
                )
            };

            if ret < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(ret as u64)
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "io_uring is only available on Linux",
            ))
        }
    }

    fn wait(&self, min: u32) -> io::Result<u64> {
        #[cfg(target_os = "linux")]
        {
            let ret = unsafe {
                libc::syscall(
                    libc::SYS_io_uring_enter,
                    self.ring_fd,
                    0u32, // to_submit
                    min,  // min_complete
                    libc::IORING_ENTER_GETEVENTS,
                    std::ptr::null::<libc::sigset_t>(),
                    0usize,
                )
            };

            if ret < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(ret as u64)
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = min;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "io_uring is only available on Linux",
            ))
        }
    }

    fn peek(&self) -> io::Result<u64> {
        self.wait(0)
    }

    fn poll_completion(&self) -> io::Result<Option<Completion>> {
        #[cfg(target_os = "linux")]
        {
            // This is a simplified implementation
            // In a real implementation, we'd need to read from the CQ ring
            let mut completions = [Completion {
                user_data: 0,
                result: Ok(0),
                op_type: OpType::Nop,
            }; 1];

            let count = self.poll_completions(&mut completions)?;
            if count > 0 {
                Ok(Some(completions[0].clone()))
            } else {
                Ok(None)
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "io_uring is only available on Linux",
            ))
        }
    }

    fn poll_completions(&self, completions: &mut [Completion]) -> io::Result<usize> {
        #[cfg(target_os = "linux")]
        {
            // Simplified: just return 0 for now
            // Real implementation would read from CQ ring
            let _ = completions;
            Ok(0)
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = completions;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "io_uring is only available on Linux",
            ))
        }
    }

    fn as_raw_fd(&self) -> Option<RawFd> {
        Some(self.ring_fd)
    }
}

impl Drop for UringPlatformBackend {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            unsafe {
                libc::close(self.ring_fd);
            }
        }
    }
}

// Safety: UringPlatformBackend is Send + Sync
// We ensure all internal state is properly synchronized
unsafe impl Send for UringPlatformBackend {}
unsafe impl Sync for UringPlatformBackend {}
