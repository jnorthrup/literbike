// Timer wheel for reactor scheduled operations (port of Trikeshed TimerWheel.kt)

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AOrdering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use parking_lot::Mutex;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_millis() as u64
}

pub struct TimerEntry {
    pub id: u64,
    pub deadline: u64,
    pub callback: Arc<dyn Fn() + Send + Sync>,
}

impl Eq for TimerEntry {}
impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool { self.id == other.id }
}
impl Ord for TimerEntry {
    // min-heap: earliest deadline first
    fn cmp(&self, other: &Self) -> Ordering { other.deadline.cmp(&self.deadline) }
}
impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

pub struct TimerWheel {
    heap: Arc<Mutex<BinaryHeap<TimerEntry>>>,
    seq: AtomicU64,
}

impl TimerWheel {
    pub fn new() -> Self {
        Self { heap: Arc::new(Mutex::new(BinaryHeap::new())), seq: AtomicU64::new(1) }
    }
    pub fn schedule(&self, delay_ms: u64, cb: Arc<dyn Fn() + Send + Sync>) -> u64 {
        let id = self.seq.fetch_add(1, AOrdering::Relaxed);
        self.heap.lock().push(TimerEntry { id, deadline: now_ms() + delay_ms, callback: cb });
        id
    }
    pub fn fire_ready(&self) {
        let now = now_ms();
        let mut heap = self.heap.lock();
        while heap.peek().map(|e| e.deadline <= now).unwrap_or(false) {
            let entry = heap.pop().unwrap();
            (entry.callback)();
        }
    }
    pub fn next_timeout_ms(&self) -> Option<u64> {
        let now = now_ms();
        self.heap.lock().peek().map(|e| e.deadline.saturating_sub(now))
    }
}

impl Default for TimerWheel {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    #[test]
    fn fires_after_deadline() {
        let wheel = TimerWheel::new();
        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        wheel.schedule(0, Arc::new(move || { f.store(true, AOrdering::SeqCst); }));
        wheel.fire_ready();
        assert!(fired.load(AOrdering::SeqCst));
    }
}
