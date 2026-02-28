//! Event Handler Trait
//!
//! Minimal handler interface for reactor events.

use std::io;
use std::os::fd::RawFd;

/// Event handler trait - implement for I/O event handling
pub trait EventHandler: Send + Sync {
    fn on_readable(&mut self, fd: RawFd);
    fn on_writable(&mut self, fd: RawFd);
    fn on_error(&mut self, fd: RawFd, error: io::Error);
}

/// Null handler for testing
pub struct NullHandler;
impl EventHandler for NullHandler {
    fn on_readable(&mut self, _fd: RawFd) {}
    fn on_writable(&mut self, _fd: RawFd) {}
    fn on_error(&mut self, _fd: RawFd, _error: io::Error) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_null_handler() {
        let mut handler = NullHandler;
        handler.on_readable(42);
        handler.on_writable(42);
        handler.on_error(42, io::Error::new(io::ErrorKind::Other, "test"));
        // Should not panic
    }
}
