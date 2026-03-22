//! Bridge between CCEK and userspace NIO ecosystem
//! 
//! This module provides integration between our CCEK context system
//! and the userspace kernel emulation (ENDGAME).

use crate::concurrency::CoroutineContext;
use std::sync::Arc;

/// CCEK-aware userspace runtime wrapper
pub struct CcekRuntime {
    context: CoroutineContext,
}

impl CcekRuntime {
    pub fn new(context: CoroutineContext) -> Self {
        Self { context }
    }
    
    pub fn context(&self) -> &CoroutineContext {
        &self.context
    }
}

/// Create a channel bound to CCEK context
pub fn ccek_channel<T: Send + 'static>(buffer: usize) -> (CcekSender<T>, CcekReceiver<T>) {
    let (tx, rx) = crate::concurrency::channel::channel(buffer);
    (CcekSender(tx), CcekReceiver(rx))
}

pub struct CcekSender<T>(pub(crate) crate::concurrency::ChannelSender<T>);

impl<T: Send + 'static> CcekSender<T> {
    pub async fn send(&self, value: T) -> crate::concurrency::CoroutineResult<()> {
        self.0.send(value).await
    }
}

impl<T: Send + 'static> Clone for CcekSender<T> {
    fn clone(&self) -> Self {
        CcekSender(self.0.clone())
    }
}

pub struct CcekReceiver<T>(pub(crate) crate::concurrency::ChannelReceiver<T>);

impl<T: Send + 'static> CcekReceiver<T> {
    pub async fn recv(&mut self) -> Option<T> {
        self.0.recv().await
    }
}
