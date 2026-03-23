//! Cross-platform NIO backend trait
//!
//! Provides a unified interface for different platform-specific NIO backends:
//! - Linux: io_uring (primary), epoll (fallback)
//! - macOS/BSD: kqueue
//! - Windows: IOCP (future)
//!
//! This module defines the core abstractions for the NIO subsystem,
//! enabling platform-specific optimizations while maintaining a consistent API.
//!
//! # SPI Hierarchy
//!
//! The NIO system follows a factory-based SPI pattern:
//!
//! ```text
//! NioProvider (root factory)
//! ├── SocketFactory (creates NioSocket)
//! ├── BufferFactory (creates NioBuffer)  
//! ├── BackendFactory (creates PlatformBackend)
//! └── CompletionFactory (creates Completion)
//! ```

use std::io;
use std::os::unix::io::RawFd;
use std::sync::Arc;
use std::task::Waker;

/// Operation types for NIO backends
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpType {
    Read,
    Write,
    Accept,
    Connect,
    PollAdd,
    PollRemove,
    Nop,
}

/// A completion event from the NIO backend
#[derive(Debug, Clone)]
pub struct Completion {
    pub user_data: u64,
    pub result: io::Result<usize>,
    pub op_type: OpType,
}

/// Registration token for a monitored file descriptor
pub type Token = u64;

/// Interest flags for file descriptor monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interest {
    pub readable: bool,
    pub writable: bool,
}

impl Interest {
    pub const READABLE: Self = Self {
        readable: true,
        writable: false,
    };
    pub const WRITABLE: Self = Self {
        readable: false,
        writable: true,
    };
    pub const READ_WRITE: Self = Self {
        readable: true,
        writable: true,
    };
}

/// NIO object trait - all NIO objects implement this
pub trait NioObject: Send + Sync {
    fn as_raw_fd(&self) -> Option<RawFd>;
    fn is_open(&self) -> bool;
}

/// NIO socket handle
#[derive(Debug)]
pub struct NioSocket {
    fd: RawFd,
    domain: SocketDomain,
    socket_type: SocketType,
}

impl NioSocket {
    pub fn new(fd: RawFd, domain: SocketDomain, socket_type: SocketType) -> Self {
        Self {
            fd,
            domain,
            socket_type,
        }
    }
    pub fn fd(&self) -> RawFd {
        self.fd
    }
    pub fn domain(&self) -> SocketDomain {
        self.domain
    }
    pub fn socket_type(&self) -> SocketType {
        self.socket_type
    }
}

impl NioObject for NioSocket {
    fn as_raw_fd(&self) -> Option<RawFd> {
        Some(self.fd)
    }
    fn is_open(&self) -> bool {
        self.fd >= 0
    }
}

impl Drop for NioSocket {
    fn drop(&mut self) {
        if self.fd >= 0 {
            unsafe { libc::close(self.fd) };
        }
    }
}

/// Socket domain
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketDomain {
    Inet,
    Inet6,
    Unix,
}

/// Socket type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Stream,
    Dgram,
}

/// NIO buffer for zero-copy operations
pub trait NioBuffer: NioObject {
    fn as_ptr(&self) -> *mut u8;
    fn len(&self) -> usize;
    fn clear(&mut self);
}

/// Memory-mapped buffer
#[derive(Debug)]
pub struct MmapBuffer {
    ptr: *mut u8,
    size: usize,
}

impl MmapBuffer {
    pub fn new(ptr: *mut u8, size: usize) -> Self {
        Self { ptr, size }
    }
}

impl NioObject for MmapBuffer {
    fn as_raw_fd(&self) -> Option<RawFd> {
        None
    }
    fn is_open(&self) -> bool {
        !self.ptr.is_null()
    }
}

impl NioBuffer for MmapBuffer {
    fn as_ptr(&self) -> *mut u8 {
        self.ptr
    }
    fn len(&self) -> usize {
        self.size
    }
    fn clear(&mut self) {
        if !self.ptr.is_null() {
            unsafe { std::ptr::write_bytes(self.ptr, 0, self.size) };
        }
    }
}

impl Drop for MmapBuffer {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { libc::munmap(self.ptr as *mut libc::c_void, self.size) };
        }
    }
}

// Safety: MmapBuffer is Send + Sync because we ensure exclusive access via mmap
// and the pointer is only used for read/write operations that are controlled
unsafe impl Send for MmapBuffer {}
unsafe impl Sync for MmapBuffer {}

// ============================================================================
// FACTORY TRAITS - SPI Pattern
// ============================================================================

/// Root NIO provider factory - all other factories derive from this
pub trait NioProvider: Send + Sync {
    fn socket_factory(&self) -> &dyn SocketFactory;
    fn buffer_factory(&self) -> &dyn BufferFactory;
    fn backend_factory(&self) -> &dyn BackendFactory;
    fn completion_factory(&self) -> &dyn CompletionFactory;

    fn name(&self) -> &'static str;
    fn priority(&self) -> u32;
}

/// Socket factory - creates NioSocket instances
pub trait SocketFactory: Send + Sync {
    fn create_socket(&self, domain: SocketDomain, socket_type: SocketType)
        -> io::Result<NioSocket>;
    fn create_pair(
        &self,
        domain: SocketDomain,
        socket_type: SocketType,
    ) -> io::Result<(NioSocket, NioSocket)>;
    fn set_nonblocking(&self, socket: &NioSocket, nonblocking: bool) -> io::Result<()>;
    fn bind(&self, socket: &NioSocket, addr: &[u8]) -> io::Result<()>;
    fn listen(&self, socket: &NioSocket, backlog: i32) -> io::Result<()>;
    fn connect(&self, socket: &NioSocket, addr: &[u8]) -> io::Result<()>;
    fn accept(&self, socket: &NioSocket) -> io::Result<NioSocket>;
}

/// Buffer factory - creates NioBuffer instances
pub trait BufferFactory: Send + Sync {
    fn create_mmap_buffer(&self, size: usize) -> io::Result<Box<MmapBuffer>>;
    fn create_buffer(&self, size: usize) -> io::Result<Box<dyn NioBuffer>>;
    fn create_aligned_buffer(&self, size: usize, align: usize) -> io::Result<Box<dyn NioBuffer>>;
}

/// Backend factory - creates PlatformBackend instances
pub trait BackendFactory: Send + Sync {
    fn create_backend(&self, config: &BackendConfig) -> io::Result<Box<dyn PlatformBackend>>;
    fn is_available(&self) -> bool;
}

/// Completion factory - creates Completion instances
pub trait CompletionFactory: Send + Sync {
    fn create_completion(
        &self,
        user_data: u64,
        result: io::Result<usize>,
        op_type: OpType,
    ) -> Completion;
    fn create_completion_vec(&self, capacity: usize) -> Vec<Completion>;
}

/// Platform-agnostic NIO backend trait
///
/// Each platform implements this trait to provide non-blocking I/O:
/// - Linux: io_uring or epoll
/// - macOS/BSD: kqueue
///
/// The trait is designed for zero-allocation hot paths and
/// supports batching operations for maximum throughput.
pub trait PlatformBackend: Send + Sync {
    fn register(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()>;
    fn reregister(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()>;
    fn unregister(&self, fd: RawFd) -> io::Result<()>;
    fn submit_read(&self, fd: RawFd, buf: &mut [u8], user_data: u64) -> io::Result<()>;
    fn submit_write(&self, fd: RawFd, buf: &[u8], user_data: u64) -> io::Result<()>;
    fn submit_read_at(
        &self,
        fd: RawFd,
        offset: u64,
        buf: &mut [u8],
        user_data: u64,
    ) -> io::Result<()>;
    fn submit_write_at(&self, fd: RawFd, offset: u64, buf: &[u8], user_data: u64)
        -> io::Result<()>;
    fn submit_poll(&self, fd: RawFd, interest: Interest, user_data: u64) -> io::Result<()>;
    fn submit_nop(&self, user_data: u64) -> io::Result<()>;
    fn submit(&self) -> io::Result<u64>;
    fn wait(&self, min: u32) -> io::Result<u64>;
    fn peek(&self) -> io::Result<u64>;
    fn poll_completion(&self) -> io::Result<Option<Completion>>;
    fn poll_completions(&self, completions: &mut [Completion]) -> io::Result<usize>;
    fn as_raw_fd(&self) -> Option<RawFd> {
        None
    }
}

/// Backend configuration
#[derive(Debug, Clone)]
pub struct BackendConfig {
    pub entries: u32,
    pub sqpoll: bool,
    pub iopoll: bool,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            entries: 256,
            sqpoll: false,
            iopoll: false,
        }
    }
}

/// Default completion factory
pub struct DefaultCompletionFactory;

impl CompletionFactory for DefaultCompletionFactory {
    fn create_completion(
        &self,
        user_data: u64,
        result: io::Result<usize>,
        op_type: OpType,
    ) -> Completion {
        Completion {
            user_data,
            result,
            op_type,
        }
    }

    fn create_completion_vec(&self, capacity: usize) -> Vec<Completion> {
        Vec::with_capacity(capacity)
    }
}

/// Default buffer factory
pub struct DefaultBufferFactory;

impl BufferFactory for DefaultBufferFactory {
    fn create_mmap_buffer(&self, size: usize) -> io::Result<Box<MmapBuffer>> {
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            Err(io::Error::last_os_error())
        } else {
            Ok(Box::new(MmapBuffer::new(ptr as *mut u8, size)))
        }
    }

    fn create_buffer(&self, size: usize) -> io::Result<Box<dyn NioBuffer>> {
        self.create_mmap_buffer(size)
            .map(|b| b as Box<dyn NioBuffer>)
    }

    fn create_aligned_buffer(&self, size: usize, align: usize) -> io::Result<Box<dyn NioBuffer>> {
        self.create_buffer(size)
    }
}

/// Default socket factory using libc
pub struct DefaultSocketFactory;

impl SocketFactory for DefaultSocketFactory {
    fn create_socket(
        &self,
        domain: SocketDomain,
        socket_type: SocketType,
    ) -> io::Result<NioSocket> {
        let (domain_libc, socket_type_libc) = match (domain, socket_type) {
            (SocketDomain::Inet, SocketType::Stream) => (libc::AF_INET, libc::SOCK_STREAM),
            (SocketDomain::Inet, SocketType::Dgram) => (libc::AF_INET, libc::SOCK_DGRAM),
            (SocketDomain::Inet6, SocketType::Stream) => (libc::AF_INET6, libc::SOCK_STREAM),
            (SocketDomain::Inet6, SocketType::Dgram) => (libc::AF_INET6, libc::SOCK_DGRAM),
            (SocketDomain::Unix, SocketType::Stream) => (libc::AF_UNIX, libc::SOCK_STREAM),
            (SocketDomain::Unix, SocketType::Dgram) => (libc::AF_UNIX, libc::SOCK_DGRAM),
        };

        let fd = unsafe { libc::socket(domain_libc, socket_type_libc, 0) };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(NioSocket::new(fd, domain, socket_type))
        }
    }

    fn create_pair(
        &self,
        domain: SocketDomain,
        socket_type: SocketType,
    ) -> io::Result<(NioSocket, NioSocket)> {
        let mut fds = [0i32, 0i32];
        let socket_type_libc = match socket_type {
            SocketType::Stream => libc::SOCK_STREAM,
            SocketType::Dgram => libc::SOCK_DGRAM,
        };
        let domain_libc = match domain {
            SocketDomain::Inet => libc::AF_INET,
            SocketDomain::Inet6 => libc::AF_INET6,
            SocketDomain::Unix => libc::AF_UNIX,
        };

        let ret = unsafe { libc::socketpair(domain_libc, socket_type_libc, 0, fds.as_mut_ptr()) };
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok((
                NioSocket::new(fds[0], domain, socket_type),
                NioSocket::new(fds[1], domain, socket_type),
            ))
        }
    }

    fn set_nonblocking(&self, socket: &NioSocket, nonblocking: bool) -> io::Result<()> {
        let flags = unsafe { libc::fcntl(socket.fd(), libc::F_GETFL, 0) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        let new_flags = if nonblocking {
            flags | libc::O_NONBLOCK
        } else {
            flags & !libc::O_NONBLOCK
        };
        let ret = unsafe { libc::fcntl(socket.fd(), libc::F_SETFL, new_flags) };
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn bind(&self, socket: &NioSocket, addr: &[u8]) -> io::Result<()> {
        let ret = unsafe {
            libc::bind(
                socket.fd(),
                addr.as_ptr() as *const libc::sockaddr,
                addr.len() as libc::socklen_t,
            )
        };
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn listen(&self, socket: &NioSocket, backlog: i32) -> io::Result<()> {
        let ret = unsafe { libc::listen(socket.fd(), backlog) };
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn connect(&self, socket: &NioSocket, addr: &[u8]) -> io::Result<()> {
        let ret = unsafe {
            libc::connect(
                socket.fd(),
                addr.as_ptr() as *const libc::sockaddr,
                addr.len() as libc::socklen_t,
            )
        };
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn accept(&self, socket: &NioSocket) -> io::Result<NioSocket> {
        let fd = unsafe { libc::accept(socket.fd(), std::ptr::null_mut(), std::ptr::null_mut()) };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(NioSocket::new(fd, socket.domain(), socket.socket_type()))
        }
    }
}

/// Full NIO provider combining all factories
pub struct NioProviderImpl {
    pub socket_factory: DefaultSocketFactory,
    pub buffer_factory: DefaultBufferFactory,
    pub completion_factory: DefaultCompletionFactory,
}

impl NioProvider for NioProviderImpl {
    fn socket_factory(&self) -> &dyn SocketFactory {
        &self.socket_factory
    }
    fn buffer_factory(&self) -> &dyn BufferFactory {
        &self.buffer_factory
    }
    fn completion_factory(&self) -> &dyn CompletionFactory {
        &self.completion_factory
    }
    fn backend_factory(&self) -> &dyn BackendFactory {
        &DefaultBackendFactory
    }
    fn name(&self) -> &'static str {
        "default"
    }
    fn priority(&self) -> u32 {
        0
    }
}

/// Default backend factory
pub struct DefaultBackendFactory;

impl BackendFactory for DefaultBackendFactory {
    fn create_backend(&self, config: &BackendConfig) -> io::Result<Box<dyn PlatformBackend>> {
        detect_backend(config)
    }

    fn is_available(&self) -> bool {
        true
    }
}

/// Detect the best available backend for the current platform
pub fn detect_backend(config: &BackendConfig) -> io::Result<Box<dyn PlatformBackend>> {
    #[cfg(target_os = "linux")]
    {
        match crate::nio::nio_uring::UringPlatformBackend::new(config) {
            Ok(backend) => return Ok(Box::new(backend)),
            Err(e) => {
                log::warn!("io_uring not available: {}, falling back to epoll", e);
            }
        }
        crate::nio::epoll_backend::EpollPlatformBackend::new(config)
            .map(|b| Box::new(b) as Box<dyn PlatformBackend>)
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    {
        crate::nio::kqueue_backend::KqueuePlatformBackend::new(config)
            .map(|b| Box::new(b) as Box<dyn PlatformBackend>)
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    )))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "No NIO backend available for this platform",
        ))
    }
}

/// Global provider registry
static PROVIDER_REGISTRY: std::sync::RwLock<Vec<Arc<dyn NioProvider>>> =
    std::sync::RwLock::new(Vec::new());

/// Register a NIO provider
pub fn register_provider(provider: Arc<dyn NioProvider>) {
    if let Ok(mut providers) = PROVIDER_REGISTRY.write() {
        providers.push(provider);
        providers.sort_by(|a, b| b.priority().cmp(&a.priority()));
    }
}

/// Get the best available provider
pub fn get_provider() -> Option<Arc<dyn NioProvider>> {
    PROVIDER_REGISTRY.read().ok()?.first().cloned()
}

/// Create a NioSocket using the best available provider
pub fn create_socket(domain: SocketDomain, socket_type: SocketType) -> io::Result<NioSocket> {
    get_provider()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No NIO provider registered"))?
        .socket_factory()
        .create_socket(domain, socket_type)
}

/// Create a buffer using the best available provider
pub fn create_buffer(size: usize) -> io::Result<Box<dyn NioBuffer>> {
    get_provider()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No NIO provider registered"))?
        .buffer_factory()
        .create_buffer(size)
}

/// Create a mmap buffer using the best available provider
pub fn create_mmap_buffer(size: usize) -> io::Result<Box<MmapBuffer>> {
    get_provider()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No NIO provider registered"))?
        .buffer_factory()
        .create_mmap_buffer(size)
}

/// Initialize with default provider
pub fn init_default() {
    register_provider(Arc::new(NioProviderImpl {
        socket_factory: DefaultSocketFactory,
        buffer_factory: DefaultBufferFactory,
        completion_factory: DefaultCompletionFactory,
    }));
}
