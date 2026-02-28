# Qwen Agent: Reactor Implementation Team

## Assignment
Implement event-driven reactor pattern from Betanet Kotlin spec.

## Branches
- `reactor/p0-event-loop` - epoll/kqueue/io_uring abstraction
- `reactor/p0-handler-registration` - Event handler trait, dispatch
- `reactor/p0-timer-wheel` - Timeout management

## Priority
**P0 - Critical** (Required for high-performance I/O)

---

## Task 1: Event Loop

**Branch:** `reactor/p0-event-loop`

**Current state:**
- `SimpleReactor` stub (6 lines)
- No I/O multiplexing

**Implementation:**
```rust
// In src/reactor/event_loop.rs
pub trait IoBackend: Send + Sync {
    fn register(&mut self, fd: RawFd, events: Events) -> Result<()>;
    fn poll(&mut self, timeout: Duration) -> Result<Vec<Event>>;
}

// Platform-specific implementations
#[cfg(target_os = "linux")]
pub struct IoUringBackend { /* ... */ }

#[cfg(target_os = "macos")]
pub struct KqueueBackend { /* ... */ }

#[cfg(target_os = "linux")]
pub struct EpollBackend { /* ... */ }

pub struct EventLoop {
    backend: Box<dyn IoBackend>,
    handlers: HashMap<RawFd, Box<dyn EventHandler>>,
}

impl EventLoop {
    pub fn run(&mut self) -> Result<()> {
        loop {
            let events = self.backend.poll(Duration::from_millis(100))?;
            for event in events {
                self.dispatch(event);
            }
        }
    }
}
```

**Test:**
```bash
cargo test --features quic reactor_event_loop
```

---

## Task 2: Handler Registration

**Branch:** `reactor/p0-handler-registration`

**Implementation:**
```rust
// In src/reactor/handler.rs
pub trait EventHandler: Send + Sync {
    fn on_readable(&mut self, fd: RawFd) -> Result<()>;
    fn on_writable(&mut self, fd: RawFd) -> Result<()>;
    fn on_error(&mut self, fd: RawFd, error: Error);
}

pub struct HandlerRegistry {
    handlers: HashMap<RawFd, Box<dyn EventHandler>>,
}

impl HandlerRegistry {
    pub fn register(&mut self, fd: RawFd, handler: Box<dyn EventHandler>);
    pub fn unregister(&mut self, fd: RawFd);
    pub fn dispatch(&mut self, event: Event);
}
```

**Test:**
```bash
cargo test --features quic reactor_handler
```

---

## Task 3: Timer Wheel

**Implementation:**
```rust
// In src/reactor/timer.rs
pub struct TimerWheel {
    wheels: Vec<Vec<TimerEntry>>,
    current_tick: u64,
    tick_duration: Duration,
}

impl TimerWheel {
    pub fn schedule(&mut self, timeout: Duration, callback: TimerCallback);
    pub fn cancel(&mut self, timer_id: TimerId);
    pub fn tick(&mut self) -> Vec<TimerCallback>;
}

pub struct TimeoutManager {
    wheel: TimerWheel,
    pending_timeouts: HashMap<ConnectionId, TimerId>,
}
```

**Test:**
```bash
cargo test --features quic reactor_timer
```

---

## Success Criteria

- [ ] Event loop runs on Linux (epoll or io_uring)
- [ ] Event loop runs on macOS (kqueue)
- [ ] Handlers can be registered/unregistered
- [ ] Timer wheel manages timeouts efficiently
- [ ] Integration with QUIC engine
- [ ] No memory leaks under load

---

## Merge Order

1. `reactor/p0-event-loop` → master
2. `reactor/p0-handler-registration` → master (depends on event loop)
3. `reactor/p0-timer-wheel` → master (depends on event loop)

---

## Dependencies

- libc crate for syscalls
- Cleanup branches (for consistent code style)

---

**Created:** 2026-02-24  
**Status:** Ready to start
