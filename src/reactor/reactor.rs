//! Core reactor event loop with registration, readiness dispatch, and timers.

use crate::reactor::channel::SelectableChannel;
use crate::reactor::context::ReactorConfig;
use crate::reactor::handler::EventHandler;
use crate::reactor::operation::InterestSet;
use crate::reactor::selector::{ManualSelector, ReadyEvent, SelectorBackend};
use crate::reactor::timer::{TimeoutCallback, TimerId, TimerWheel};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::os::fd::RawFd;
use std::time::Duration;

struct PendingRegistration {
    channel: Box<dyn SelectableChannel>,
    interests: InterestSet,
    handler: Box<dyn EventHandler>,
}

#[derive(Debug, Clone, Default)]
pub struct ReactorStats {
    pub registrations_applied: u64,
    pub select_calls: u64,
    pub dispatch_callbacks: u64,
    pub timer_callbacks: u64,
    pub handler_errors: u64,
    pub shutdowns: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReactorTickResult {
    pub registrations_applied: usize,
    pub ready_events: usize,
    pub handler_callbacks: usize,
    pub timer_callbacks: usize,
}

pub struct Reactor<S: SelectorBackend = ManualSelector> {
    selector: S,
    timer_wheel: TimerWheel,
    pending_registrations: VecDeque<PendingRegistration>,
    handlers: HashMap<RawFd, Box<dyn EventHandler>>,
    channels: HashMap<RawFd, Box<dyn SelectableChannel>>,
    default_select_timeout: Duration,
    running: bool,
    stats: ReactorStats,
}

impl<S: SelectorBackend> Reactor<S> {
    pub fn new(selector: S, config: ReactorConfig) -> Self {
        Self {
            selector,
            timer_wheel: TimerWheel::new(),
            pending_registrations: VecDeque::new(),
            handlers: HashMap::new(),
            channels: HashMap::new(),
            default_select_timeout: Duration::from_millis(config.select_timeout_ms.max(1)),
            running: true,
            stats: ReactorStats::default(),
        }
    }

    pub fn with_default_config(selector: S) -> Self {
        Self::new(selector, ReactorConfig::default())
    }

    pub fn is_active(&self) -> bool {
        self.running
    }

    pub fn stats(&self) -> &ReactorStats {
        &self.stats
    }

    pub fn registered_channels(&self) -> usize {
        self.channels.len()
    }

    pub fn schedule_timeout(&mut self, delay: Duration, callback: TimeoutCallback) -> TimerId {
        self.timer_wheel.schedule(delay, callback)
    }

    pub fn cancel_timeout(&mut self, timer_id: TimerId) -> bool {
        self.timer_wheel.cancel(timer_id)
    }

    pub fn register_channel<C, H>(
        &mut self,
        channel: C,
        interests: InterestSet,
        handler: H,
    ) -> io::Result<RawFd>
    where
        C: SelectableChannel + 'static,
        H: EventHandler + 'static,
    {
        let fd = channel.raw_fd();
        if fd < 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "negative fd"));
        }
        self.pending_registrations.push_back(PendingRegistration {
            channel: Box::new(channel),
            interests,
            handler: Box::new(handler),
        });
        self.selector.wakeup();
        Ok(fd)
    }

    pub fn run_once(&mut self) -> io::Result<ReactorTickResult> {
        if !self.running {
            return Ok(ReactorTickResult::default());
        }

        let registrations_applied = self.apply_pending_registrations();
        let timeout = self.compute_poll_timeout();

        self.stats.select_calls += 1;
        let events = self.selector.select(Some(timeout))?;

        let mut callbacks = 0usize;
        for event in &events {
            callbacks += self.dispatch_event(*event);
        }

        let timer_callbacks = self.run_expired_timers();

        Ok(ReactorTickResult {
            registrations_applied,
            ready_events: events.len(),
            handler_callbacks: callbacks,
            timer_callbacks,
        })
    }

    pub fn shutdown(&mut self) -> io::Result<()> {
        if !self.running {
            return Ok(());
        }
        self.running = false;
        self.stats.shutdowns += 1;

        while let Some(mut pending) = self.pending_registrations.pop_front() {
            let _ = pending.channel.close();
        }

        for (_, mut channel) in self.channels.drain() {
            let _ = channel.close();
        }
        self.handlers.clear();

        self.selector.close()
    }

    pub fn selector(&self) -> &S {
        &self.selector
    }

    pub fn selector_mut(&mut self) -> &mut S {
        &mut self.selector
    }

    fn compute_poll_timeout(&self) -> Duration {
        match self.timer_wheel.next_timeout() {
            Some(next) => next.min(self.default_select_timeout),
            None => self.default_select_timeout,
        }
    }

    fn apply_pending_registrations(&mut self) -> usize {
        let mut applied = 0usize;
        while let Some(mut pending) = self.pending_registrations.pop_front() {
            let fd = pending.channel.raw_fd();
            match self.selector.register(fd, pending.interests) {
                Ok(()) => {
                    self.channels.insert(fd, pending.channel);
                    self.handlers.insert(fd, pending.handler);
                    self.stats.registrations_applied += 1;
                    applied += 1;
                }
                Err(err) => {
                    let kind = err.kind();
                    let msg = err.to_string();
                    pending.handler.on_error(fd, io::Error::new(kind, msg));
                    self.stats.handler_errors += 1;
                    let _ = pending.channel.close();
                }
            }
        }
        applied
    }

    fn dispatch_event(&mut self, event: ReadyEvent) -> usize {
        let Some(handler) = self.handlers.get_mut(&event.fd) else {
            return 0;
        };

        let mut callbacks = 0usize;
        if event.ready.contains(InterestSet::READ) || event.ready.contains(InterestSet::ACCEPT) {
            handler.on_readable(event.fd);
            callbacks += 1;
            self.stats.dispatch_callbacks += 1;
        }
        if event.ready.contains(InterestSet::WRITE) || event.ready.contains(InterestSet::CONNECT) {
            handler.on_writable(event.fd);
            callbacks += 1;
            self.stats.dispatch_callbacks += 1;
        }
        if event.ready.contains(InterestSet::ERROR) {
            handler.on_error(event.fd, io::Error::other("reactor readiness error"));
            callbacks += 1;
            self.stats.dispatch_callbacks += 1;
            self.stats.handler_errors += 1;
        }
        callbacks
    }

    fn run_expired_timers(&mut self) -> usize {
        let callbacks = self.timer_wheel.take_expired();
        let count = callbacks.len();
        for cb in callbacks {
            cb();
        }
        self.stats.timer_callbacks += count as u64;
        count
    }
}

impl Reactor<ManualSelector> {
    pub fn manual(config: ReactorConfig) -> Self {
        Self::new(ManualSelector::new(), config)
    }

    pub fn inject_ready(&mut self, fd: RawFd, ready: InterestSet) {
        self.selector.inject_ready(fd, ready);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactor::channel::SelectableChannel;
    use crate::reactor::handler::EventHandler;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    struct MockChannel {
        fd: RawFd,
        open: bool,
        closed_flag: Arc<AtomicBool>,
    }

    impl MockChannel {
        fn new(fd: RawFd, closed_flag: Arc<AtomicBool>) -> Self {
            Self {
                fd,
                open: true,
                closed_flag,
            }
        }
    }

    impl SelectableChannel for MockChannel {
        fn raw_fd(&self) -> RawFd {
            self.fd
        }

        fn is_open(&self) -> bool {
            self.open
        }

        fn close(&mut self) -> io::Result<()> {
            self.open = false;
            self.closed_flag.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    struct RecordingHandler {
        events: Arc<Mutex<Vec<String>>>,
    }

    impl RecordingHandler {
        fn new(events: Arc<Mutex<Vec<String>>>) -> Self {
            Self { events }
        }
    }

    impl EventHandler for RecordingHandler {
        fn on_readable(&mut self, fd: RawFd) {
            self.events.lock().unwrap().push(format!("read:{fd}"));
        }

        fn on_writable(&mut self, fd: RawFd) {
            self.events.lock().unwrap().push(format!("write:{fd}"));
        }

        fn on_error(&mut self, fd: RawFd, error: io::Error) {
            self.events
                .lock()
                .unwrap()
                .push(format!("error:{fd}:{}", error.kind() as u8));
        }
    }

    #[test]
    fn registration_and_readiness_dispatch_work() {
        let mut reactor = Reactor::manual(ReactorConfig {
            select_timeout_ms: 50,
            stats_enabled: true,
        });
        let log = Arc::new(Mutex::new(Vec::new()));
        let closed = Arc::new(AtomicBool::new(false));

        let fd = reactor
            .register_channel(
                MockChannel::new(42, closed),
                InterestSet::READ | InterestSet::WRITE,
                RecordingHandler::new(log.clone()),
            )
            .unwrap();
        assert_eq!(fd, 42);

        let first = reactor.run_once().unwrap();
        assert_eq!(first.registrations_applied, 1);
        assert_eq!(reactor.selector().registered_count(), 1);

        reactor.inject_ready(42, InterestSet::READ | InterestSet::WRITE);
        let tick = reactor.run_once().unwrap();
        assert_eq!(tick.ready_events, 1);
        assert_eq!(tick.handler_callbacks, 2);

        let events = log.lock().unwrap().clone();
        assert!(events.iter().any(|e| e == "read:42"));
        assert!(events.iter().any(|e| e == "write:42"));
    }

    #[test]
    fn timer_is_integrated_into_poll_timeout_and_fires() {
        let mut reactor = Reactor::manual(ReactorConfig {
            select_timeout_ms: 250,
            stats_enabled: true,
        });

        let fired = Arc::new(AtomicUsize::new(0));
        let fired_clone = fired.clone();
        reactor.schedule_timeout(
            Duration::from_millis(0),
            Box::new(move || {
                fired_clone.fetch_add(1, Ordering::SeqCst);
            }),
        );

        let tick = reactor.run_once().unwrap();
        assert_eq!(tick.timer_callbacks, 1);
        assert_eq!(fired.load(Ordering::SeqCst), 1);

        let last_timeout = reactor.selector().last_timeout().unwrap();
        assert_eq!(last_timeout, Duration::from_millis(0));
    }

    #[test]
    fn shutdown_closes_channels_and_selector() {
        let mut reactor = Reactor::manual(ReactorConfig::default());
        let closed = Arc::new(AtomicBool::new(false));

        reactor
            .register_channel(
                MockChannel::new(7, closed.clone()),
                InterestSet::READ,
                RecordingHandler::new(Arc::new(Mutex::new(Vec::new()))),
            )
            .unwrap();
        reactor.run_once().unwrap();

        reactor.shutdown().unwrap();
        assert!(!reactor.is_active());
        assert!(closed.load(Ordering::SeqCst));
        assert!(reactor.selector().is_closed());
        assert_eq!(reactor.registered_channels(), 0);
    }
}
