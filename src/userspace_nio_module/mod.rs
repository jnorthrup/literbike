//! Unified NIO (Non-Blocking I/O) SPI Facade Layer
//!
//! This module provides a public Service Provider Interface (SPI) facade
//! over platform-specific NIO backends (io_uring, kqueue, epoll).

pub mod backend;
pub mod endgame;
pub mod epoll_backend;
pub mod kqueue_backend;
pub mod nio_uring;
pub mod reactor;
pub mod session_island;
pub mod suspend_resume;

// Re-export key types
pub use backend::{
    create_buffer, create_mmap_buffer, create_socket, detect_backend, get_provider, init_default,
    register_provider, BackendConfig, BackendFactory, BufferFactory, Completion, CompletionFactory,
    Interest, MmapBuffer, NioBuffer, NioObject, NioProvider, NioSocket, OpType, PlatformBackend,
    SocketDomain, SocketFactory, SocketType, Token,
};
pub use endgame::{
    CqEntry, EndgameCapabilities, OpCode, ProcessingPath, SimdLevel, SqEntry, UringFacade,
};
pub use reactor::{
    Reactor, ReadFuture, ReadableFuture, RegistrationHandle, WritableFuture, WriteFuture,
};
pub use session_island::{
    execute_key_graph, CancellationElement, CancellationToken, CcekContext, ChannelHandle,
    ChannelRegistry, ContextElement, ContextElementKey, KeyGraph, ProtocolTransition,
    SessionElement, SessionIsland, StateKey, TransitionMap,
};
pub use suspend_resume::{
    ContinuationScheduler, ReactorContinuation, SuspendFuture, SuspendToken, SuspensionState,
};
