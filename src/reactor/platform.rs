// Platform-specific I/O abstraction (port of Trikeshed PlatformIO.kt)

use std::io;

#[derive(Debug, Clone)]
pub struct ReadinessEvent {
    pub fd: i32,
    pub readable: bool,
    pub writable: bool,
}

pub trait PlatformIO: Send + Sync {
    fn register(&self, fd: i32) -> io::Result<()>;
    fn unregister(&self, fd: i32) -> io::Result<()>;
    fn wait(&self, timeout_ms: u64) -> io::Result<Vec<ReadinessEvent>>;
}

/// Stub platform — always returns empty readiness (for portable baseline)
pub struct StubPlatformIO;

impl PlatformIO for StubPlatformIO {
    fn register(&self, _fd: i32) -> io::Result<()> { Ok(()) }
    fn unregister(&self, _fd: i32) -> io::Result<()> { Ok(()) }
    fn wait(&self, _timeout_ms: u64) -> io::Result<Vec<ReadinessEvent>> { Ok(vec![]) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stub_returns_empty() {
        let p = StubPlatformIO;
        assert!(p.wait(10).unwrap().is_empty());
    }
}
