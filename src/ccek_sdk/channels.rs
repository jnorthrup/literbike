//! CCEK Channels - Compile-time channelized tributaries
//!
//! Channels are the rivers connecting protocol tributaries.
//! Bound at compile time through CCEK.

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

/// Transmit end of a CCEK channel
pub struct ChannelTx<T> {
    queue: Arc<RwLock<VecDeque<T>>>,
    capacity: usize,
}

/// Receive end of a CCEK channel  
pub struct ChannelRx<T> {
    queue: Arc<RwLock<VecDeque<T>>>,
    capacity: usize,
}

/// CCEK Channel pair - compile-time bound
pub struct Channel<T> {
    tx: ChannelTx<T>,
    rx: ChannelRx<T>,
}

impl<T> Channel<T> {
    pub fn new(capacity: usize) -> Self {
        let queue = Arc::new(RwLock::new(VecDeque::with_capacity(capacity)));
        Self {
            tx: ChannelTx {
                queue: Arc::clone(&queue),
                capacity,
            },
            rx: ChannelRx { queue, capacity },
        }
    }

    pub fn split(self) -> (ChannelTx<T>, ChannelRx<T>) {
        (self.tx, self.rx)
    }
}

impl<T> ChannelTx<T> {
    pub fn send(&self, value: T) -> Result<(), ChannelError<T>> {
        let mut queue = self.queue.write().map_err(|_| ChannelError::Closed)?;
        if queue.len() >= self.capacity {
            return Err(ChannelError::Full(value));
        }
        queue.push_back(value);
        Ok(())
    }

    pub fn try_send(&self, value: T) -> Result<(), ChannelError<T>> {
        self.send(value)
    }
}

impl<T> ChannelRx<T> {
    pub fn recv(&self) -> Option<T> {
        let queue = self.queue.read().ok()?;
        queue.pop_front()
    }

    pub fn try_recv(&self) -> Option<T> {
        self.recv()
    }
}

impl<T> Clone for ChannelTx<T> {
    fn clone(&self) -> Self {
        Self {
            queue: Arc::clone(&self.queue),
            capacity: self.capacity,
        }
    }
}

pub enum ChannelError<T> {
    Full(T),
    Closed,
}
