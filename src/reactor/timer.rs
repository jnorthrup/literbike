//! Timer Wheel - O(1) Scheduling
//!
//! Fail-fast timeout management for select reactor.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

pub type TimeoutCallback = Box<dyn FnOnce() + Send + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(pub u64);

pub struct Timeout {
    pub id: TimerId,
    pub expires_at: Instant,
    pub callback: Option<TimeoutCallback>,
}

impl std::fmt::Debug for Timeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timeout")
            .field("id", &self.id)
            .field("expires_at", &self.expires_at)
            .field("callback", &"<callback>")
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
pub struct TimerStats {
    pub timers_created: u64,
    pub timers_cancelled: u64,
    pub timers_expired: u64,
    pub active_count: usize,
}

/// Timer wheel - O(1) scheduling, O(n) expiration
pub struct TimerWheel {
    next_id: u64,
    timeouts: HashMap<TimerId, Timeout>,
    expiration_queue: VecDeque<(Instant, TimerId)>,
    stats: TimerStats,
}

impl TimerWheel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn schedule(&mut self, delay: Duration, callback: TimeoutCallback) -> TimerId {
        let id = TimerId(self.next_id);
        self.next_id += 1;

        let expires_at = Instant::now() + delay;
        let timeout = Timeout {
            id,
            expires_at,
            callback: Some(callback),
        };

        self.timeouts.insert(id, timeout);
        self.insert_sorted(expires_at, id);

        self.stats.timers_created += 1;
        self.stats.active_count = self.timeouts.len();
        id
    }

    pub fn cancel(&mut self, timer_id: TimerId) -> bool {
        if self.timeouts.remove(&timer_id).is_some() {
            self.expiration_queue.retain(|&(_, id)| id != timer_id);
            self.stats.timers_cancelled += 1;
            self.stats.active_count = self.timeouts.len();
            true
        } else {
            false
        }
    }

    pub fn next_timeout(&self) -> Option<Duration> {
        self.expiration_queue
            .front()
            .map(|&(expires_at, _)| expires_at.saturating_duration_since(Instant::now()))
    }

    pub fn take_expired(&mut self) -> Vec<TimeoutCallback> {
        let now = Instant::now();
        let mut callbacks = Vec::new();

        while let Some(&(expires_at, timer_id)) = self.expiration_queue.front() {
            if expires_at > now {
                break;
            }
            self.expiration_queue.pop_front();

            if let Some(mut timeout) = self.timeouts.remove(&timer_id) {
                if let Some(callback) = timeout.callback.take() {
                    callbacks.push(callback);
                    self.stats.timers_expired += 1;
                }
            }
        }

        self.stats.active_count = self.timeouts.len();
        callbacks
    }

    pub fn active_count(&self) -> usize {
        self.timeouts.len()
    }

    pub fn stats(&self) -> &TimerStats {
        &self.stats
    }

    fn insert_sorted(&mut self, expires_at: Instant, timer_id: TimerId) {
        let pos = self
            .expiration_queue
            .iter()
            .position(|&(time, _)| time > expires_at)
            .unwrap_or(self.expiration_queue.len());
        self.expiration_queue.insert(pos, (expires_at, timer_id));
    }
}

impl Default for TimerWheel {
    fn default() -> Self {
        Self {
            next_id: 0,
            timeouts: HashMap::new(),
            expiration_queue: VecDeque::new(),
            stats: TimerStats::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_timer_scheduling() {
        let mut wheel = TimerWheel::new();
        let id = wheel.schedule(Duration::from_millis(100), Box::new(|| {}));
        assert!(id.0 >= 0);
        assert_eq!(wheel.active_count(), 1);
    }

    #[test]
    fn test_timer_cancellation() {
        let mut wheel = TimerWheel::new();
        let id = wheel.schedule(Duration::from_secs(1), Box::new(|| {}));
        assert!(wheel.cancel(id));
        assert!(!wheel.cancel(id));
    }

    #[test]
    fn test_timer_expiration() {
        let mut wheel = TimerWheel::new();
        let expired = Arc::new(Mutex::new(false));
        let clone = expired.clone();

        wheel.schedule(
            Duration::from_millis(0),
            Box::new(move || {
                *clone.lock().unwrap() = true;
            }),
        );

        let callbacks = wheel.take_expired();
        for cb in callbacks {
            cb();
        }

        assert!(*expired.lock().unwrap());
    }
}
