//! Userspace selector backend for literbike reactor.
//!
//! This module provides a SelectorBackend implementation that wraps
//! userspace's NIO reactor, allowing literbike to leverage userspace's
//! io_uring and kernel bypass capabilities.

use crate::reactor::operation::InterestSet;
use crate::reactor::selector::{ReadyEvent, SelectorBackend};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::os::fd::RawFd;
use std::time::Duration;
use userspace::kernel::nio::{NioChannel, Reactor as _, SimpleReactor};

pub struct UserspaceSelector {
    registrations: HashMap<RawFd, InterestSet>,
    ready_queue: VecDeque<ReadyEvent>,
    closed: bool,
    last_timeout: Option<Duration>,
    userspace_reactor: SimpleReactor,
    fd_to_id: HashMap<RawFd, usize>,
}

impl UserspaceSelector {
    pub fn new() -> Self {
        Self {
            registrations: HashMap::new(),
            ready_queue: VecDeque::new(),
            closed: false,
            last_timeout: None,
            userspace_reactor: userspace::kernel::nio::SimpleReactor::new(),
            fd_to_id: HashMap::new(),
        }
    }

    pub fn with_userspace_reactor(reactor: SimpleReactor) -> Self {
        Self {
            registrations: HashMap::new(),
            ready_queue: VecDeque::new(),
            closed: false,
            last_timeout: None,
            userspace_reactor: reactor,
            fd_to_id: HashMap::new(),
        }
    }

    pub fn registered_count(&self) -> usize {
        self.registrations.len()
    }

    pub fn last_timeout(&self) -> Option<Duration> {
        self.last_timeout
    }
}

impl Default for UserspaceSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectorBackend for UserspaceSelector {
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

        self.userspace_reactor
            .register(UserspaceNioAdapter::new(fd))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

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

        if let Some(id) = self.fd_to_id.get(&fd) {
            self.userspace_reactor
                .unregister(*id)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            self.fd_to_id.remove(&fd);
        }

        Ok(())
    }

    fn select(&mut self, timeout: Option<Duration>) -> io::Result<Vec<ReadyEvent>> {
        if self.closed {
            return Ok(Vec::new());
        }
        self.last_timeout = timeout;

        let ready_count = self
            .userspace_reactor
            .tick(timeout)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        if ready_count > 0 {
            for (&fd, &interests) in &self.registrations {
                self.ready_queue.push_back(ReadyEvent::new(fd, interests));
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
        // Userspace reactor handles wakeup internally
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

impl UserspaceSelector {
    fn lookup_fd_id(&self) -> &HashMap<RawFd, usize> {
        &self.fd_to_id
    }
}

struct UserspaceSelectorInner {
    fd_to_id: HashMap<RawFd, usize>,
}

struct UserspaceNioAdapter {
    fd: RawFd,
}

impl UserspaceNioAdapter {
    fn new(fd: RawFd) -> Self {
        Self { fd }
    }
}

impl NioChannel for UserspaceNioAdapter {
    fn poll_readable(&self, timeout: Option<Duration>) -> io::Result<bool> {
        let mut pollfd = libc::pollfd {
            fd: self.fd,
            events: libc::POLLIN,
            revents: 0,
        };

        let timeout_ms = timeout.map(|d| d.as_millis() as i32).unwrap_or(-1);

        unsafe {
            let ret = libc::poll(&mut pollfd, 1, timeout_ms);
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok((pollfd.revents & libc::POLLIN) != 0)
        }
    }

    fn poll_writable(&self, timeout: Option<Duration>) -> io::Result<bool> {
        let mut pollfd = libc::pollfd {
            fd: self.fd,
            events: libc::POLLOUT,
            revents: 0,
        };

        let timeout_ms = timeout.map(|d| d.as_millis() as i32).unwrap_or(-1);

        unsafe {
            let ret = libc::poll(&mut pollfd, 1, timeout_ms);
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok((pollfd.revents & libc::POLLOUT) != 0)
        }
    }

    fn try_read(&self, buf: &mut [u8]) -> io::Result<usize> {
        use libc::read;

        unsafe {
            let ret = read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len());
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(ret as usize)
        }
    }

    fn try_write(&self, buf: &[u8]) -> io::Result<usize> {
        use libc::write;

        unsafe {
            let ret = write(self.fd, buf.as_ptr() as *const libc::c_void, buf.len());
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(ret as usize)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::io::{AsRawFd, FromRawFd};
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn userspace_selector_registers_and_selects() {
        let (a, b) = UnixStream::pair().unwrap();
        let fd = a.as_raw_fd();

        let mut selector = UserspaceSelector::new();
        selector.register(fd, InterestSet::READ).unwrap();

        assert_eq!(selector.registered_count(), 1);

        // Write to the other end to make fd readable
        let mut b = b;
        b.write_all(b"hello").unwrap();
        drop(b);

        let events = selector.select(Some(Duration::from_millis(100))).unwrap();
        assert!(!events.is_empty());
    }
}
