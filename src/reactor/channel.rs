// Selectable channel abstraction for reactor registration (port of SelectableChannel.kt)

use crate::reactor::operation::Interest;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub trait SelectableChannel: Send + Sync {
    fn set_interest(&self, interest: Interest) -> std::io::Result<()>;
    fn as_raw_fd(&self) -> Option<i32> { None }
    fn is_open(&self) -> bool { true }
}

/// In-memory channel for testing/stub use
pub struct MemoryChannel {
    open: Arc<AtomicBool>,
}

impl MemoryChannel {
    pub fn new() -> Self {
        Self { open: Arc::new(AtomicBool::new(true)) }
    }
    pub fn close(&self) {
        self.open.store(false, Ordering::SeqCst);
    }
}

impl SelectableChannel for MemoryChannel {
    fn set_interest(&self, _interest: Interest) -> std::io::Result<()> { Ok(()) }
    fn is_open(&self) -> bool { self.open.load(Ordering::SeqCst) }
}

impl Default for MemoryChannel {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn open_close() {
        let ch = MemoryChannel::new();
        assert!(ch.is_open());
        ch.close();
        assert!(!ch.is_open());
    }
}
