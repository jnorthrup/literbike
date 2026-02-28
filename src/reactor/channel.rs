//! Channel traits for the portable reactor abstraction.

use crate::reactor::operation::InterestSet;
use std::io;
use std::os::fd::RawFd;

pub trait SelectableChannel: Send {
    fn raw_fd(&self) -> RawFd;
    fn is_open(&self) -> bool;
    fn close(&mut self) -> io::Result<()>;
}

pub trait ReadableChannel: SelectableChannel {}

pub trait WritableChannel: SelectableChannel {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelRegistration {
    pub fd: RawFd,
    pub interests: InterestSet,
}

impl ChannelRegistration {
    pub fn new(fd: RawFd, interests: InterestSet) -> Self {
        Self { fd, interests }
    }
}
