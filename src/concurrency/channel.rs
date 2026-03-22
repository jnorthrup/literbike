//! Simplified Channel module - no tokio dependency
//! Channel-based communication for structured concurrency

use anyhow::Result;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll, Waker};

/// Channel sender
pub struct ChannelSender<T: Send + 'static> {
    queue: Arc<RwLock<VecDeque<T>>>,
    capacity: usize,
    senders: Arc<RwLock<usize>>,
    receivers: Arc<RwLock<usize>>,
    wakers: Arc<RwLock<Vec<Waker>>>,
    closed: Arc<RwLock<bool>>,
}

impl<T: Send + 'static> ChannelSender<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Arc::new(RwLock::new(VecDeque::new())),
            capacity,
            senders: Arc::new(RwLock::new(1)),
            receivers: Arc::new(RwLock::new(1)),
            wakers: Arc::new(RwLock::new(Vec::new())),
            closed: Arc::new(RwLock::new(false)),
        }
    }
    
    pub async fn send(&self, value: T) -> Result<()> {
        let mut queue = self.queue.write().unwrap();
        if *self.closed.read().unwrap() {
            return Err(anyhow::anyhow!("Channel closed"));
        }
        if queue.len() >= self.capacity {
            drop(queue);
            let mut wakers = self.wakers.write().unwrap();
            wakers.push(Context::from_waker(&Waker::clone(&futures::task::noop_waker_ref())));
            return Err(anyhow::anyhow!("Channel full"));
        }
        queue.push_back(value);
        drop(queue);
        self.wake_receiver();
        Ok(())
    }
    
    pub fn try_send(&self, value: T) -> Result<()> {
        let mut queue = self.queue.write().unwrap();
        if *self.closed.read().unwrap() {
            return Err(anyhow::anyhow!("Channel closed"));
        }
        if queue.len() >= self.capacity {
            return Err(anyhow::anyhow!("Channel full"));
        }
        queue.push_back(value);
        drop(queue);
        self.wake_receiver();
        Ok(())
    }
    
    fn wake_receiver(&self) {
        let wakers = self.wakers.write().unwrap();
        for waker in wakers.iter() {
            waker.wake_by_ref();
        }
    }
}

impl<T: Send + 'static> Clone for ChannelSender<T> {
    fn clone(&self) -> Self {
        *self.senders.write().unwrap() += 1;
        Self {
            queue: Arc::clone(&self.queue),
            capacity: self.capacity,
            senders: Arc::clone(&self.senders),
            receivers: Arc::clone(&self.receivers),
            wakers: Arc::clone(&self.wakers),
            closed: Arc::clone(&self.closed),
        }
    }
}

/// Channel receiver  
pub struct ChannelReceiver<T: Send + 'static> {
    queue: Arc<RwLock<VecDeque<T>>>,
    capacity: usize,
    senders: Arc<RwLock<usize>>,
    receivers: Arc<RwLock<usize>>,
    wakers: Arc<RwLock<Vec<Waker>>>,
    closed: Arc<RwLock<bool>>,
}

impl<T: Send + 'static> ChannelReceiver<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Arc::new(RwLock::new(VecDeque::new())),
            capacity,
            senders: Arc::new(RwLock::new(1)),
            receivers: Arc::new(RwLock::new(1)),
            wakers: Arc::new(RwLock::new(Vec::new())),
            closed: Arc::new(RwLock::new(false)),
        }
    }
    
    pub async fn recv(&mut self) -> Option<T> {
        loop {
            let queue = self.queue.read().unwrap();
            if let Some(value) = queue.pop_front() {
                drop(queue);
                self.wake_sender();
                return Some(value);
            }
            if *self.closed.read().unwrap() {
                return None;
            }
            drop(queue);
            
            let waker = Context::from_waker(&Waker::clone(&futures::task::noop_waker_ref()));
            let mut cx = Context::from_waker(&waker);
            
            futures::future::pending().await;
        }
    }
    
    pub fn try_recv(&mut self) -> Option<T> {
        let mut queue = self.queue.write().unwrap();
        queue.pop_front()
    }
    
    fn wake_sender(&self) {
        let wakers = self.wakers.write().unwrap();
        for waker in wakers.iter() {
            waker.wake_by_ref();
        }
    }
}

/// Create a channel pair
pub fn channel<T: Send + 'static>(buffer_size: usize) -> (ChannelSender<T>, ChannelReceiver<T>) {
    let tx = ChannelSender::new(buffer_size);
    let rx = ChannelReceiver::new(buffer_size);
    (tx, rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_try_send_recv() {
        let (tx, mut rx) = channel::<i32>(10);
        tx.try_send(42).unwrap();
        let value = rx.try_recv();
        assert_eq!(value, Some(42));
    }
}
