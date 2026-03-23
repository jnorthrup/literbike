//! Unified NIO reactor using platform-specific backends
//!
//! This module provides a high-level reactor that abstracts over
//! different platform backends (io_uring, kqueue, epoll) and provides
//! a consistent interface for async I/O operations.

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll, Waker};

use super::backend::{BackendConfig, Completion, Interest, OpType, PlatformBackend, Token};

/// Shared state for reactor operations
struct ReactorState {
    /// Platform-specific backend
    backend: Box<dyn PlatformBackend>,
    /// Registered file descriptors
    registrations: Mutex<HashMap<RawFd, Registration>>,
    /// Pending operations keyed by user_data
    pending_ops: Mutex<HashMap<u64, PendingOperation>>,
    /// Wakers for completed operations
    wakers: Mutex<HashMap<u64, Waker>>,
    /// Next user_data value
    next_user_data: Mutex<u64>,
}

struct Registration {
    token: Token,
    interest: Interest,
}

struct PendingOperation {
    user_data: u64,
    op_type: OpType,
    fd: RawFd,
}

/// High-level reactor interface
pub struct Reactor {
    state: Arc<ReactorState>,
}

/// Registration handle for a file descriptor
pub struct RegistrationHandle {
    fd: RawFd,
    reactor: Weak<ReactorState>,
}

impl Reactor {
    /// Create a new reactor with the default backend configuration
    pub fn new() -> io::Result<Self> {
        Self::with_config(&BackendConfig::default())
    }

    /// Create a new reactor with the specified configuration
    pub fn with_config(config: &BackendConfig) -> io::Result<Self> {
        let backend = super::backend::detect_backend(config)?;

        Ok(Self {
            state: Arc::new(ReactorState {
                backend,
                registrations: Mutex::new(HashMap::with_capacity(config.entries as usize)),
                pending_ops: Mutex::new(HashMap::new()),
                wakers: Mutex::new(HashMap::new()),
                next_user_data: Mutex::new(1),
            }),
        })
    }

    /// Register a file descriptor for monitoring
    pub fn register(&self, fd: RawFd, interest: Interest) -> io::Result<RegistrationHandle> {
        let token = self.allocate_token()?;

        self.state.backend.register(fd, token, interest)?;

        let mut registrations = self.state.registrations.lock().unwrap();
        registrations.insert(fd, Registration { token, interest });

        Ok(RegistrationHandle {
            fd,
            reactor: Arc::downgrade(&self.state),
        })
    }

    /// Allocate a unique user_data token
    fn allocate_token(&self) -> io::Result<u64> {
        let mut next = self.state.next_user_data.lock().unwrap();
        let token = *next;
        *next = next.wrapping_add(1);
        if *next == 0 {
            *next = 1; // Skip 0
        }
        Ok(token)
    }

    /// Submit a read operation
    pub fn read(&self, fd: RawFd, buf: &mut [u8]) -> io::Result<ReadFuture> {
        let user_data = self.allocate_token()?;

        self.state.backend.submit_read(fd, buf, user_data)?;

        {
            let mut pending = self.state.pending_ops.lock().unwrap();
            pending.insert(
                user_data,
                PendingOperation {
                    user_data,
                    op_type: OpType::Read,
                    fd,
                },
            );
        }

        Ok(ReadFuture {
            state: Arc::downgrade(&self.state),
            user_data,
        })
    }

    /// Submit a write operation
    pub fn write(&self, fd: RawFd, buf: &[u8]) -> io::Result<WriteFuture> {
        let user_data = self.allocate_token()?;

        self.state.backend.submit_write(fd, buf, user_data)?;

        {
            let mut pending = self.state.pending_ops.lock().unwrap();
            pending.insert(
                user_data,
                PendingOperation {
                    user_data,
                    op_type: OpType::Write,
                    fd,
                },
            );
        }

        Ok(WriteFuture {
            state: Arc::downgrade(&self.state),
            user_data,
        })
    }

    /// Submit all pending operations
    pub fn submit(&self) -> io::Result<u64> {
        self.state.backend.submit()
    }

    /// Wait for completions
    pub fn wait(&self, min: u32) -> io::Result<u64> {
        self.state.backend.wait(min)
    }

    /// Process completions
    pub fn process_completions(&self, completions: &mut [Completion]) -> io::Result<usize> {
        let count = self.state.backend.poll_completions(completions)?;

        let mut wakers = self.state.wakers.lock().unwrap();
        let mut pending = self.state.pending_ops.lock().unwrap();

        for i in 0..count {
            let completion = &completions[i];

            // Remove from pending operations
            pending.remove(&completion.user_data);

            // Wake any futures waiting for this operation
            if let Some(waker) = wakers.remove(&completion.user_data) {
                waker.wake();
            }
        }

        Ok(count)
    }

    /// Run the reactor loop
    pub fn run_once(&self) -> io::Result<usize> {
        let mut completions = [Completion {
            user_data: 0,
            result: Ok(0),
            op_type: OpType::Nop,
        }; 64];

        self.process_completions(&mut completions)
    }
}

impl Drop for RegistrationHandle {
    fn drop(&mut self) {
        if let Some(state) = self.reactor.upgrade() {
            let _ = state.backend.unregister(self.fd);
            let mut registrations = state.registrations.lock().unwrap();
            registrations.remove(&self.fd);
        }
    }
}

/// Future for read operations
pub struct ReadFuture {
    state: Weak<ReactorState>,
    user_data: u64,
}

impl Future for ReadFuture {
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        if let Some(state) = this.state.upgrade() {
            // Check if we have a completion
            let mut pending = state.pending_ops.lock().unwrap();
            if let Some(_op) = pending.remove(&this.user_data) {
                // Operation still pending, register waker
                let mut wakers = state.wakers.lock().unwrap();
                wakers.insert(this.user_data, cx.waker().clone());
                Poll::Pending
            } else {
                // Operation completed - we need to get the result
                // For now, return Pending and let the reactor wake us
                let mut wakers = state.wakers.lock().unwrap();
                wakers.insert(this.user_data, cx.waker().clone());
                Poll::Pending
            }
        } else {
            Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, "Reactor dropped")))
        }
    }
}

/// Future for write operations
pub struct WriteFuture {
    state: Weak<ReactorState>,
    user_data: u64,
}

impl Future for WriteFuture {
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        if let Some(state) = this.state.upgrade() {
            // Check if we have a completion
            let mut pending = state.pending_ops.lock().unwrap();
            if let Some(_op) = pending.remove(&this.user_data) {
                // Operation still pending, register waker
                let mut wakers = state.wakers.lock().unwrap();
                wakers.insert(this.user_data, cx.waker().clone());
                Poll::Pending
            } else {
                // Operation completed - we need to get the result
                // For now, return Pending and let the reactor wake us
                let mut wakers = state.wakers.lock().unwrap();
                wakers.insert(this.user_data, cx.waker().clone());
                Poll::Pending
            }
        } else {
            Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, "Reactor dropped")))
        }
    }
}

/// Future that waits for a file descriptor to become readable
pub struct ReadableFuture {
    fd: RawFd,
    state: Weak<ReactorState>,
    registered: bool,
}

impl ReadableFuture {
    pub fn new(fd: RawFd, reactor: &Reactor) -> io::Result<Self> {
        Ok(Self {
            fd,
            state: Arc::downgrade(&reactor.state),
            registered: false,
        })
    }
}

impl Future for ReadableFuture {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.as_mut().get_unchecked_mut() };

        if !this.registered {
            if let Some(state) = this.state.upgrade() {
                let user_data = {
                    let mut next = state.next_user_data.lock().unwrap();
                    let token = *next;
                    *next = next.wrapping_add(1);
                    if *next == 0 {
                        *next = 1;
                    }
                    token
                };

                state
                    .backend
                    .submit_poll(this.fd, Interest::READABLE, user_data)?;

                let mut wakers = state.wakers.lock().unwrap();
                wakers.insert(user_data, cx.waker().clone());

                this.registered = true;
                return Poll::Pending;
            }
        }

        // For now, just wake again
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

/// Future that waits for a file descriptor to become writable
pub struct WritableFuture {
    fd: RawFd,
    state: Weak<ReactorState>,
    registered: bool,
}

impl WritableFuture {
    pub fn new(fd: RawFd, reactor: &Reactor) -> io::Result<Self> {
        Ok(Self {
            fd,
            state: Arc::downgrade(&reactor.state),
            registered: false,
        })
    }
}

impl Future for WritableFuture {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.as_mut().get_unchecked_mut() };

        if !this.registered {
            if let Some(state) = this.state.upgrade() {
                let user_data = {
                    let mut next = state.next_user_data.lock().unwrap();
                    let token = *next;
                    *next = next.wrapping_add(1);
                    if *next == 0 {
                        *next = 1;
                    }
                    token
                };

                state
                    .backend
                    .submit_poll(this.fd, Interest::WRITABLE, user_data)?;

                let mut wakers = state.wakers.lock().unwrap();
                wakers.insert(user_data, cx.waker().clone());

                this.registered = true;
                return Poll::Pending;
            }
        }

        // For now, just wake again
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reactor_creation() {
        let reactor = Reactor::new();
        assert!(reactor.is_ok(), "Reactor creation should succeed");
    }

    #[test]
    fn test_reactor_with_config() {
        let config = BackendConfig {
            entries: 128,
            sqpoll: false,
            iopoll: false,
        };

        let reactor = Reactor::with_config(&config);
        assert!(
            reactor.is_ok(),
            "Reactor creation with config should succeed"
        );
    }
}
