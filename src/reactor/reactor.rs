// Core reactor event loop (port of Trikeshed Reactor.kt)

use crate::reactor::operation::Interest;
use crate::reactor::selector::{ChannelId, ManualSelector};
use crate::reactor::timer::TimerWheel;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactorTickResult {
    Continue,
    Shutdown,
}

pub struct Reactor {
    selector: Arc<ManualSelector>,
    timer_wheel: Arc<TimerWheel>,
    running: Arc<AtomicBool>,
}

impl Reactor {
    pub fn new() -> Self {
        Self {
            selector: Arc::new(ManualSelector::new()),
            timer_wheel: Arc::new(TimerWheel::new()),
            running: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn register(&self, id: ChannelId, interest: Interest) {
        self.selector.register(id, interest);
    }
    pub fn unregister(&self, id: ChannelId) {
        self.selector.unregister(id);
    }
    pub fn timer_wheel(&self) -> Arc<TimerWheel> {
        self.timer_wheel.clone()
    }
    pub fn tick(&self, timeout_ms: u64) -> ReactorTickResult {
        self.timer_wheel.fire_ready();
        let poll_timeout = self.timer_wheel.next_timeout_ms()
            .map(|t| t.min(timeout_ms))
            .unwrap_or(timeout_ms);
        let _ = self.selector.poll(poll_timeout);
        if self.running.load(Ordering::SeqCst) {
            ReactorTickResult::Continue
        } else {
            ReactorTickResult::Shutdown
        }
    }
    pub fn start(&self) {
        self.running.store(true, Ordering::SeqCst);
    }
    pub fn shutdown(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Default for Reactor {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shutdown_stops_loop() {
        let r = Reactor::new();
        r.shutdown();
        assert_eq!(r.tick(0), ReactorTickResult::Shutdown);
    }
    #[test]
    fn running_continues() {
        let r = Reactor::new();
        r.start();
        assert_eq!(r.tick(0), ReactorTickResult::Continue);
    }
}
