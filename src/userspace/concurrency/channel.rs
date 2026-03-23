//! Channel primitives using CCEK pattern (no tokio)
//!
//! Minimal channel implementation for message passing between jobs.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll, Waker};

use crate::concurrency::CancellationError;

/// Send error types
#[derive(Debug, Clone)]
pub enum SendError<T> {
    Closed(T),
    Full(T),
}

impl<T> SendError<T> {
    pub fn into_inner(self) -> T {
        match self {
            Self::Closed(v) | Self::Full(v) => v,
        }
    }
}

/// Receive error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecvError {
    Empty,
    Closed,
}

/// Channel capacity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelCapacity {
    Unbounded,
    Buffered(usize),
    Rendezvous,
}

/// Simple channel implementation
pub struct Channel<T> {
    queue: Arc<RwLock<VecDeque<T>>>,
    capacity: ChannelCapacity,
    senders: Arc<RwLock<usize>>,
    receivers: Arc<RwLock<usize>>,
    closed: Arc<RwLock<bool>>,
    wakers: Arc<RwLock<Vec<Waker>>>,
}

impl<T> Channel<T> {
    pub fn new(capacity: ChannelCapacity) -> Self {
        Self {
            queue: Arc::new(RwLock::new(VecDeque::new())),
            capacity,
            senders: Arc::new(RwLock::new(1)),
            receivers: Arc::new(RwLock::new(1)),
            closed: Arc::new(RwLock::new(false)),
            wakers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn rendezvous() -> Self {
        Self::new(ChannelCapacity::Rendezvous)
    }

    pub fn buffered(capacity: usize) -> Self {
        Self::new(ChannelCapacity::Buffered(capacity))
    }

    pub fn unbounded() -> Self {
        Self::new(ChannelCapacity::Unbounded)
    }

    pub fn is_closed(&self) -> bool {
        *self.closed.read().unwrap()
    }

    pub fn close(&self) {
        *self.closed.write().unwrap() = true;
        self.wake_all();
    }

    fn wake_all(&self) {
        let wakers = self.wakers.write().unwrap();
        for waker in wakers.iter() {
            waker.wake_by_ref();
        }
    }

    fn wake_one(&self) {
        let mut wakers = self.wakers.write().unwrap();
        if let Some(waker) = wakers.pop() {
            waker.wake();
        }
    }
}

impl<T: Clone> Channel<T> {
    pub fn send(&self, value: T) -> ChannelSendFuture<T> {
        ChannelSendFuture {
            channel: self,
            value: Some(value),
        }
    }

    pub fn recv(&self) -> ChannelRecvFuture<T> {
        ChannelRecvFuture { channel: self }
    }
}

pub struct ChannelSendFuture<'a, T> {
    channel: &'a Channel<T>,
    value: Option<T>,
}

impl<T: Clone> Unpin for ChannelSendFuture<'_, T> {}

impl<T: Clone> Future for ChannelSendFuture<'_, T> {
    type Output = Result<(), SendError<T>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.channel.is_closed() {
            return Poll::Ready(Err(SendError::Closed(self.value.take().unwrap())));
        }

        let value = self.value.take().unwrap();
        let mut queue = self.channel.queue.write().unwrap();

        match &self.channel.capacity {
            ChannelCapacity::Unbounded => {
                queue.push_back(value);
                drop(queue);
                self.channel.wake_one();
                Poll::Ready(Ok(()))
            }
            ChannelCapacity::Buffered(n) => {
                if queue.len() >= *n {
                    let mut wakers = self.channel.wakers.write().unwrap();
                    wakers.push(cx.waker().clone());
                    drop(wakers);
                    self.value = Some(value);
                    Poll::Pending
                } else {
                    queue.push_back(value);
                    drop(queue);
                    self.channel.wake_one();
                    Poll::Ready(Ok(()))
                }
            }
            ChannelCapacity::Rendezvous => {
                drop(queue);
                let mut queue = self.channel.queue.write().unwrap();
                queue.push_back(value);
                drop(queue);
                self.channel.wake_one();
                let mut wakers = self.channel.wakers.write().unwrap();
                wakers.push(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

pub struct ChannelRecvFuture<'a, T> {
    channel: &'a Channel<T>,
}

impl<T: Clone> Future for ChannelRecvFuture<'_, T> {
    type Output = Result<T, RecvError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.channel.is_closed() {
            return Poll::Ready(Err(RecvError::Closed));
        }

        let mut queue = self.channel.queue.write().unwrap();

        if let Some(value) = queue.pop_front() {
            drop(queue);
            self.channel.wake_one();
            Poll::Ready(Ok(value))
        } else {
            drop(queue);
            let mut wakers = self.channel.wakers.write().unwrap();
            wakers.push(cx.waker().clone());
            Poll::Pending
        }
    }
}

/// Create a channel pair (sender and receiver)
pub fn channel<T>() -> (Channel<T>, Channel<T>) {
    let ch = Channel::rendezvous();
    (ch.clone(), ch)
}

impl<T> Clone for Channel<T> {
    fn clone(&self) -> Self {
        *self.senders.write().unwrap() += 1;
        *self.receivers.write().unwrap() += 1;
        Self {
            queue: Arc::clone(&self.queue),
            capacity: self.capacity,
            senders: Arc::clone(&self.senders),
            receivers: Arc::clone(&self.receivers),
            closed: Arc::clone(&self.closed),
            wakers: Arc::clone(&self.wakers),
        }
    }
}

impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        let mut senders = self.senders.write().unwrap();
        *senders -= 1;
        if *senders == 0 {
            self.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_rendezvous() {
        let (tx, rx) = channel::<i32>();
        assert!(!tx.is_closed());
        assert!(!rx.is_closed());
    }

    #[test]
    fn test_channel_buffered() {
        let ch = Channel::buffered(2);
        ch.send(1);
        ch.send(2);
        // Buffered channel can hold 2 items
    }
}
