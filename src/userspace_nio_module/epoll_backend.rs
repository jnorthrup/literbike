//! Linux epoll NIO backend
//!
//! Provides the fallback NIO backend for Linux using epoll,
//! when io_uring is not available or not desired.

use std::collections::HashMap;
use std::io;
use std::os::unix::io::RawFd;
use std::sync::Mutex;

use super::backend::{BackendConfig, Completion, Interest, OpType, PlatformBackend, Token};

/// epoll backend for Linux
pub struct EpollPlatformBackend {
    epoll_fd: RawFd,
    registered_fds: Mutex<HashMap<RawFd, Token>>,
    pending_completions: Mutex<Vec<Completion>>,
}

impl EpollPlatformBackend {
    pub fn new(config: &BackendConfig) -> io::Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let epoll_fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
            if epoll_fd < 0 {
                return Err(io::Error::last_os_error());
            }

            Ok(Self {
                epoll_fd,
                registered_fds: Mutex::new(HashMap::with_capacity(config.entries as usize)),
                pending_completions: Mutex::new(Vec::new()),
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = config;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "epoll is only available on Linux",
            ))
        }
    }

    #[cfg(target_os = "linux")]
    fn epoll_ctl(&self, op: i32, fd: RawFd, event: &mut libc::epoll_event) -> io::Result<()> {
        let ret =
            unsafe { libc::epoll_ctl(self.epoll_fd, op, fd, event as *mut libc::epoll_event) };

        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl PlatformBackend for EpollPlatformBackend {
    fn register(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            let mut events = 0u32;
            if interest.readable {
                events |= libc::EPOLLIN;
            }
            if interest.writable {
                events |= libc::EPOLLOUT;
            }

            let mut event = libc::epoll_event { events, u64: token };

            self.epoll_ctl(libc::EPOLL_CTL_ADD, fd, &mut event)?;

            let mut fds = self.registered_fds.lock().unwrap();
            fds.insert(fd, token);

            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (fd, token, interest);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "epoll is only available on Linux",
            ))
        }
    }

    fn reregister(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            let mut events = 0u32;
            if interest.readable {
                events |= libc::EPOLLIN;
            }
            if interest.writable {
                events |= libc::EPOLLOUT;
            }

            let mut event = libc::epoll_event { events, u64: token };

            self.epoll_ctl(libc::EPOLL_CTL_MOD, fd, &mut event)?;

            let mut fds = self.registered_fds.lock().unwrap();
            fds.insert(fd, token);

            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (fd, token, interest);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "epoll is only available on Linux",
            ))
        }
    }

    fn unregister(&self, fd: RawFd) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            // epoll requires a non-null event pointer even for EPOLL_CTL_DEL
            let mut event = libc::epoll_event { events: 0, u64: 0 };

            self.epoll_ctl(libc::EPOLL_CTL_DEL, fd, &mut event)?;

            let mut fds = self.registered_fds.lock().unwrap();
            fds.remove(&fd);

            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = fd;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "epoll is only available on Linux",
            ))
        }
    }

    fn submit_read(&self, fd: RawFd, buf: &mut [u8], user_data: u64) -> io::Result<()> {
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };

        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::WouldBlock {
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
        #[cfg(target_os = "linux")]
        {
            let mut events = 0u32;
            if interest.readable {
                events |= libc::EPOLLIN;
            }
            if interest.writable {
                events |= libc::EPOLLOUT;
            }

            let mut event = libc::epoll_event {
                events,
                u64: user_data,
            };

            self.epoll_ctl(libc::EPOLL_CTL_MOD, fd, &mut event)?;

            let mut completions = self.pending_completions.lock().unwrap();
            completions.push(Completion {
                user_data,
                result: Ok(0),
                op_type: OpType::PollAdd,
            });

            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (fd, interest, user_data);
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "epoll is only available on Linux",
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
        let completions = self.pending_completions.lock().unwrap();
        Ok(completions.len() as u64)
    }

    fn wait(&self, min: u32) -> io::Result<u64> {
        #[cfg(target_os = "linux")]
        {
            let mut events: [libc::epoll_event; 64] = unsafe { std::mem::zeroed() };
            let timeout = if min > 0 { 10 } else { 0 }; // 10ms or non-blocking

            let n = unsafe {
                libc::epoll_wait(
                    self.epoll_fd,
                    events.as_mut_ptr(),
                    events.len() as i32,
                    timeout,
                )
            };

            if n < 0 {
                Err(io::Error::last_os_error())
            } else {
                // Process epoll events into completions
                let mut completions = self.pending_completions.lock().unwrap();
                let fds = self.registered_fds.lock().unwrap();

                for i in 0..(n as usize) {
                    let event = &events[i];
                    let token = event.u64;

                    // Determine event type based on epoll event flags
                    let (op_type, result) = if event.events & libc::EPOLLIN != 0 {
                        (OpType::Read, Ok(0))
                    } else if event.events & libc::EPOLLOUT != 0 {
                        (OpType::Write, Ok(0))
                    } else if event.events & libc::EPOLLERR != 0 {
                        (
                            OpType::Nop,
                            Err(io::Error::new(io::ErrorKind::Other, "epoll error")),
                        )
                    } else {
                        (OpType::Nop, Ok(0))
                    };

                    completions.push(Completion {
                        user_data: token,
                        result,
                        op_type,
                    });
                }

                Ok(n as u64)
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = min;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "epoll is only available on Linux",
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
        Some(self.epoll_fd)
    }
}

impl Drop for EpollPlatformBackend {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            unsafe {
                libc::close(self.epoll_fd);
            }
        }
    }
}

// Safety: EpollPlatformBackend is Send + Sync
unsafe impl Send for EpollPlatformBackend {}
unsafe impl Sync for EpollPlatformBackend {}
