//! Portable selector abstraction.
//!
//! The baseline implementation (`ManualSelector`) is deterministic and testable:
//! callers inject readiness events explicitly rather than relying on OS polling.

use crate::reactor::operation::InterestSet;
use std::collections::{HashMap, VecDeque};
use std::io;
use std::os::fd::RawFd;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadyEvent {
    pub fd: RawFd,
    pub ready: InterestSet,
}

impl ReadyEvent {
    pub fn new(fd: RawFd, ready: InterestSet) -> Self {
        Self { fd, ready }
    }
}

pub trait SelectorBackend {
    fn register(&mut self, fd: RawFd, interests: InterestSet) -> io::Result<()>;
    fn reregister(&mut self, fd: RawFd, interests: InterestSet) -> io::Result<()>;
    fn unregister(&mut self, fd: RawFd) -> io::Result<()>;
    fn select(&mut self, timeout: Option<Duration>) -> io::Result<Vec<ReadyEvent>>;
    fn wakeup(&mut self);
    fn close(&mut self) -> io::Result<()>;
    fn is_closed(&self) -> bool;
}

#[derive(Debug, Default)]
pub struct ManualSelector {
    registrations: HashMap<RawFd, InterestSet>,
    ready_queue: VecDeque<ReadyEvent>,
    closed: bool,
    last_timeout: Option<Duration>,
    wakeup_count: u64,
    sleep_on_empty: bool,
}

impl ManualSelector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_sleep_on_empty(mut self, enabled: bool) -> Self {
        self.sleep_on_empty = enabled;
        self
    }

    pub fn inject_ready(&mut self, fd: RawFd, ready: InterestSet) {
        self.ready_queue.push_back(ReadyEvent::new(fd, ready));
    }

    pub fn registered_count(&self) -> usize {
        self.registrations.len()
    }

    pub fn last_timeout(&self) -> Option<Duration> {
        self.last_timeout
    }

    pub fn wakeup_count(&self) -> u64 {
        self.wakeup_count
    }
}

impl SelectorBackend for ManualSelector {
    fn register(&mut self, fd: RawFd, interests: InterestSet) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "selector is closed",
            ));
        }
        if self.registrations.contains_key(&fd) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("fd {fd} already registered"),
            ));
        }
        self.registrations.insert(fd, interests);
        Ok(())
    }

    fn reregister(&mut self, fd: RawFd, interests: InterestSet) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "selector is closed",
            ));
        }
        let slot = self.registrations.get_mut(&fd).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("fd {fd} not registered"))
        })?;
        *slot = interests;
        Ok(())
    }

    fn unregister(&mut self, fd: RawFd) -> io::Result<()> {
        self.registrations.remove(&fd);
        Ok(())
    }

    fn select(&mut self, timeout: Option<Duration>) -> io::Result<Vec<ReadyEvent>> {
        if self.closed {
            return Ok(Vec::new());
        }
        self.last_timeout = timeout;

        if self.sleep_on_empty && self.ready_queue.is_empty() {
            if let Some(delay) = timeout {
                if !delay.is_zero() {
                    std::thread::sleep(delay);
                }
            }
        }

        let mut out = Vec::new();
        while let Some(event) = self.ready_queue.pop_front() {
            let Some(interests) = self.registrations.get(&event.fd).copied() else {
                continue;
            };
            let filtered = event.ready & interests;
            if !filtered.is_empty() {
                out.push(ReadyEvent::new(event.fd, filtered));
            }
        }
        Ok(out)
    }

    fn wakeup(&mut self) {
        self.wakeup_count += 1;
    }

    fn close(&mut self) -> io::Result<()> {
        self.closed = true;
        self.ready_queue.clear();
        self.registrations.clear();
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactor::operation::InterestSet;

    #[test]
    fn manual_selector_filters_to_registered_interests() {
        let mut selector = ManualSelector::new();
        selector.register(10, InterestSet::READ).unwrap();
        selector.inject_ready(10, InterestSet::READ | InterestSet::WRITE);

        let events = selector.select(Some(Duration::from_millis(5))).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].fd, 10);
        assert_eq!(events[0].ready, InterestSet::READ);
        assert_eq!(selector.last_timeout(), Some(Duration::from_millis(5)));
    }
}
