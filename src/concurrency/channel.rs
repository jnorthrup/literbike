//! Simplified Channel module
//! Channel-based communication for structured concurrency

use tokio::sync::mpsc;
use anyhow::Result;
use super::scope::SupervisorScope;

/// Channel sender
pub struct ChannelSender<T: Send + 'static> {
    tx: mpsc::Sender<T>,
}

impl<T: Send + 'static> ChannelSender<T> {
    pub fn new(tx: mpsc::Sender<T>) -> Self {
        Self { tx }
    }
    
    /// Send a value to the channel
    pub async fn send(&self, value: T) -> Result<()> {
        self.tx.send(value).await
            .map_err(|e| anyhow::anyhow!("Channel send error: {}", e))?;
        Ok(())
    }
    
    /// Try to send without waiting
    pub fn try_send(&self, value: T) -> Result<()> {
        self.tx.try_send(value)
            .map_err(|e| anyhow::anyhow!("Channel try_send error: {}", e))?;
        Ok(())
    }
}

impl<T: Send + 'static> Clone for ChannelSender<T> {
    fn clone(&self) -> Self {
        Self { tx: self.tx.clone() }
    }
}

/// Channel receiver
pub struct ChannelReceiver<T: Send + 'static> {
    rx: mpsc::Receiver<T>,
}

impl<T: Send + 'static> ChannelReceiver<T> {
    pub fn new(rx: mpsc::Receiver<T>) -> Self {
        Self { rx }
    }
    
    /// Receive a value from the channel
    pub async fn recv(&mut self) -> Option<T> {
        self.rx.recv().await
    }
    
    /// Try to receive without waiting
    pub fn try_recv(&mut self) -> Option<T> {
        self.rx.try_recv().ok()
    }
}

/// Create a channel
pub fn channel<T: Send + 'static>(buffer_size: usize) -> (ChannelSender<T>, ChannelReceiver<T>) {
    let (tx, rx) = mpsc::channel(buffer_size);
    (ChannelSender::new(tx), ChannelReceiver::new(rx))
}

/// Create channel with scope integration
pub fn channel_with_scope<T: Send + 'static>(
    _scope: &SupervisorScope,
    buffer_size: usize,
) -> (ChannelSender<T>, ChannelReceiver<T>) {
    channel(buffer_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_channel_send_recv() {
        let (tx, mut rx) = channel::<i32>(10);
        
        tx.send(42).await.unwrap();
        let value = rx.recv().await.unwrap();
        
        assert_eq!(value, 42);
    }

    #[tokio::test]
    async fn test_channel_multiple_sends() {
        let (tx, mut rx) = channel::<i32>(10);
        
        for i in 0..5 {
            tx.send(i).await.unwrap();
        }
        
        for i in 0..5 {
            assert_eq!(rx.recv().await.unwrap(), i);
        }
    }
}
