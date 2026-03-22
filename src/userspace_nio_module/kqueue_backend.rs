//! macOS/BSD kqueue NIO backend
//!
//! Provides NIO backend for macOS and BSD systems using kqueue,
//! which is the standard non-blocking I/O mechanism on these platforms.

use std::collections::HashMap;
use std::io;
use std::os::unix::io::RawFd;
use std::sync::Mutex;

use super::backend::{BackendConfig, Completion, Interest, OpType, PlatformBackend, Token};

/// kqueue backend for macOS/BSD
pub struct KqueuePlatformBackend {
    kqueue_fd: RawFd,
    registered_fds: Mutex<HashMap<RawFd, Token>>,
    pending_completions: Mutex<Vec<Completion>>,
}

impl KqueuePlatformBackend {
    pub fn new(config: &BackendConfig) -> io::Result<Self> {
        #[cfg(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        {
            let kqueue_fd = unsafe { libc::kqueue() };
            if kqueue_fd < 0 {
                return Err(io::Error::last_os_error());
            }

            Ok(Self {
                kqueue_fd,
                registered_fds: Mutex::new(HashMap::with_capacity(config.entries as usize)),
                pending_completions: Mutex::new(Vec::new()),
            })
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        )))]
        {
            let _ = config;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "kqueue is only available on macOS/BSD",
            ))
        }
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    fn kevent_add(&self, fd: RawFd, filter: i16, flags: u16) -> io::Result<()> {
        let mut kev = libc::kevent {
            ident: fd as libc::uintptr_t,
            filter,
            flags,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        };

        let ret = unsafe {
            libc::kevent(
                self.kqueue_fd,
                &kev as *const libc::kevent,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };

        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    fn kevent_delete(&self, fd: RawFd, filter: i16) -> io::Result<()> {
        let mut kev = libc::kevent {
            ident: fd as libc::uintptr_t,
            filter,
            flags: libc::EV_DELETE,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        };

        let ret = unsafe {
            libc::kevent(
                self.kqueue_fd,
                &kev as *const libc::kevent,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };

        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl PlatformBackend for KqueuePlatformBackend {
    fn register(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        #[cfg(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        {
            let mut fds = self.registered_fds.lock().unwrap();

            if interest.readable {
                self.kevent_add(fd, libc::EVFILT_READ, libc::EV_ADD | libc::EV_ENABLE)?;
            }

            if interest.writable {
                self.kevent_add(fd, libc::EVFILT_WRITE, libc::EV_ADD | libc::EV_ENABLE)?;
            }

            fds.insert(fd, token);
            Ok(())
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        )))]
        {
            let _ = (fd, token, interest);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "kqueue is only available on macOS/BSD",
            ))
        }
    }

    fn reregister(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        #[cfg(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        {
            // Remove existing registrations
            let _ = self.kevent_delete(fd, libc::EVFILT_READ);
            let _ = self.kevent_delete(fd, libc::EVFILT_WRITE);

            // Register with new interest
            self.register(fd, token, interest)
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        )))]
        {
            let _ = (fd, token, interest);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "kqueue is only available on macOS/BSD",
            ))
        }
    }

    fn unregister(&self, fd: RawFd) -> io::Result<()> {
        #[cfg(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        {
            let mut fds = self.registered_fds.lock().unwrap();

            // Try to delete both read and write filters
            let _ = self.kevent_delete(fd, libc::EVFILT_READ);
            let _ = self.kevent_delete(fd, libc::EVFILT_WRITE);

            fds.remove(&fd);
            Ok(())
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        )))]
        {
            let _ = fd;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "kqueue is only available on macOS/BSD",
            ))
        }
    }

    fn submit_read(&self, fd: RawFd, buf: &mut [u8], user_data: u64) -> io::Result<()> {
        // kqueue doesn't have a direct submit_read like io_uring
        // Instead, we read directly when the FD becomes readable
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };

        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::WouldBlock {
                // Not ready yet, queue for later
                let mut completions = self.pending_completions.lock().unwrap();
                completions.push(Completion {
                    user_data,
                    result: Err(err),
                    op_type: OpType::Read,
                });
                Ok(())
            } else {
                Err(err)
            }
        } else {
            let mut completions = self.pending_completions.lock().unwrap();
            completions.push(Completion {
                user_data,
                result: Ok(n as usize),
                op_type: OpType::Read,
            });
            Ok(())
        }
    }

    fn submit_write(&self, fd: RawFd, buf: &[u8], user_data: u64) -> io::Result<()> {
        let n = unsafe { libc::write(fd, buf.as_ptr() as *const libc::c_void, buf.len()) };

        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::WouldBlock {
                let mut completions = self.pending_completions.lock().unwrap();
                completions.push(Completion {
                    user_data,
                    result: Err(err),
                    op_type: OpType::Write,
                });
                Ok(())
            } else {
                Err(err)
            }
        } else {
            let mut completions = self.pending_completions.lock().unwrap();
            completions.push(Completion {
                user_data,
                result: Ok(n as usize),
                op_type: OpType::Write,
            });
            Ok(())
        }
    }

    fn submit_read_at(
        &self,
        fd: RawFd,
        offset: u64,
        buf: &mut [u8],
        user_data: u64,
    ) -> io::Result<()> {
        // Use pread for positioned reads
        let n = unsafe {
            libc::pread(
                fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
                offset as libc::off_t,
            )
        };

        if n < 0 {
            Err(io::Error::last_os_error())
        } else {
            let mut completions = self.pending_completions.lock().unwrap();
            completions.push(Completion {
                user_data,
                result: Ok(n as usize),
                op_type: OpType::Read,
            });
            Ok(())
        }
    }

    fn submit_write_at(
        &self,
        fd: RawFd,
        offset: u64,
        buf: &[u8],
        user_data: u64,
    ) -> io::Result<()> {
        // Use pwrite for positioned writes
        let n = unsafe {
            libc::pwrite(
                fd,
                buf.as_ptr() as *const libc::c_void,
                buf.len(),
                offset as libc::off_t,
            )
        };

        if n < 0 {
            Err(io::Error::last_os_error())
        } else {
            let mut completions = self.pending_completions.lock().unwrap();
            completions.push(Completion {
                user_data,
                result: Ok(n as usize),
                op_type: OpType::Write,
            });
            Ok(())
        }
    }

    fn submit_poll(&self, fd: RawFd, interest: Interest, user_data: u64) -> io::Result<()> {
        #[cfg(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        {
            if interest.readable {
                self.kevent_add(fd, libc::EVFILT_READ, libc::EV_ADD | libc::EV_ENABLE)?;
            }

            if interest.writable {
                self.kevent_add(fd, libc::EVFILT_WRITE, libc::EV_ADD | libc::EV_ENABLE)?;
            }

            // Store user_data mapping
            let mut completions = self.pending_completions.lock().unwrap();
            completions.push(Completion {
                user_data,
                result: Ok(0),
                op_type: OpType::PollAdd,
            });

            Ok(())
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        )))]
        {
            let _ = (fd, interest, user_data);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "kqueue is only available on macOS/BSD",
            ))
        }
    }

    fn submit_nop(&self, user_data: u64) -> io::Result<()> {
        let mut completions = self.pending_completions.lock().unwrap();
        completions.push(Completion {
            user_data,
            result: Ok(0),
            op_type: OpType::Nop,
        });
        Ok(())
    }

    fn submit(&self) -> io::Result<u64> {
        // kqueue doesn't have a separate submit step
        // Operations are active immediately
        let completions = self.pending_completions.lock().unwrap();
        Ok(completions.len() as u64)
    }

    fn wait(&self, min: u32) -> io::Result<u64> {
        #[cfg(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        {
            let mut events: [libc::kevent; 64] = unsafe { std::mem::zeroed() };
            let mut timeout = libc::timespec {
                tv_sec: 0,
                tv_nsec: 10_000_000, // 10ms
            };

            let n = unsafe {
                libc::kevent(
                    self.kqueue_fd,
                    std::ptr::null(),
                    0,
                    events.as_mut_ptr(),
                    events.len() as i32,
                    if min > 0 { &timeout } else { std::ptr::null() },
                )
            };

            if n < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(n as u64)
            }
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        )))]
        {
            let _ = min;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "kqueue is only available on macOS/BSD",
            ))
        }
    }

    fn peek(&self) -> io::Result<u64> {
        self.wait(0)
    }

    fn poll_completion(&self) -> io::Result<Option<Completion>> {
        let mut completions = self.pending_completions.lock().unwrap();
        Ok(completions.pop())
    }

    fn poll_completions(&self, completions: &mut [Completion]) -> io::Result<usize> {
        let mut pending = self.pending_completions.lock().unwrap();
        let count = std::cmp::min(completions.len(), pending.len());

        for i in 0..count {
            completions[i] = pending.remove(0);
        }

        Ok(count)
    }

    fn as_raw_fd(&self) -> Option<RawFd> {
        Some(self.kqueue_fd)
    }
}

impl Drop for KqueuePlatformBackend {
    fn drop(&mut self) {
        #[cfg(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        {
            unsafe {
                libc::close(self.kqueue_fd);
            }
        }
    }
}

// Safety: KqueuePlatformBackend is Send + Sync
unsafe impl Send for KqueuePlatformBackend {}
unsafe impl Sync for KqueuePlatformBackend {}
