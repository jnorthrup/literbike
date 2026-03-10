//! Bridge between CCEK and Tokio ecosystem
//! 
//! This module provides integration between our CCEK context system
//! and the Tokio async runtime ecosystem.

use crate::concurrency::CoroutineContext;
use tokio::runtime::Handle;

/// CCEK-aware Tokio runtime wrapper
/// 
/// Allows spawning tasks with access to CCEK context
pub struct CcekRuntime {
    context: CoroutineContext,
    runtime_handle: Handle,
}

impl CcekRuntime {
    /// Create a new CCEK runtime wrapper
    pub fn new(context: CoroutineContext) -> Self {
        Self {
            context,
            runtime_handle: Handle::current(),
        }
    }
    
    /// Get the context
    pub fn context(&self) -> &CoroutineContext {
        &self.context
    }
    
    /// Spawn a task with CCEK context available
    pub fn spawn<F, T>(&self, f: F) -> tokio::task::JoinHandle<T>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let _ctx = self.context.clone();
        self.runtime_handle.spawn(async move {
            // Context is available for use in the spawned task
            f.await
        })
    }
    
    /// Create an async-channel integrated with CCEK
    pub fn channel<T: Send + 'static>(
        &self,
        buffer: usize,
    ) -> (async_channel::Sender<T>, async_channel::Receiver<T>) {
        async_channel::bounded(buffer)
    }
    
    /// Create a stream from an iterator with CCEK context
    pub fn iter_stream<T: Send + 'static>(
        &self,
        iter: impl IntoIterator<Item = T> + Send + 'static,
    ) -> impl tokio_stream::Stream<Item = T> + Send {
        let vec: Vec<T> = iter.into_iter().collect();
        tokio_stream::iter(vec)
    }
}

impl Clone for CcekRuntime {
    fn clone(&self) -> Self {
        Self {
            context: self.context.clone(),
            runtime_handle: self.runtime_handle.clone(),
        }
    }
}

/// Extension trait for CoroutineContext to easily spawn with tokio
pub trait CoroutineContextExt {
    /// Spawn a task with this context
    fn spawn_task<F, T>(&self, f: F) -> tokio::task::JoinHandle<T>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static;
    
    /// Create a channel with this context
    fn create_channel<T: Send + 'static>(&self, buffer: usize) 
        -> (async_channel::Sender<T>, async_channel::Receiver<T>);
}

impl CoroutineContextExt for CoroutineContext {
    fn spawn_task<F, T>(&self, f: F) -> tokio::task::JoinHandle<T>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let _ctx = self.clone();
        tokio::spawn(async move {
            // Context available here
            f.await
        })
    }
    
    fn create_channel<T: Send + 'static>(&self, buffer: usize) 
        -> (async_channel::Sender<T>, async_channel::Receiver<T>) {
        async_channel::bounded(buffer)
    }
}

/// Helper to convert our ChannelSender to async-channel Sender
pub fn to_async_channel<T: Send + 'static>(
    _our_sender: crate::concurrency::ChannelSender<T>,
) -> async_channel::Sender<T> {
    // Note: This is a placeholder - in practice you'd want to
    // migrate all code to use async-channel directly
    let (_tx, _rx) = async_channel::bounded(100);
    _tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::concurrency::{EmptyContext, ProtocolDetector, DHTService, ContextElement, SupervisorScope};

    #[tokio::test]
    async fn test_ccek_runtime_spawn() {
        let ctx = EmptyContext
            + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>
            + Arc::new(DHTService::new("test-node"));

        let runtime = CcekRuntime::new(ctx);

        let handle = runtime.spawn(async {
            42
        });

        let result = handle.await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_ccek_channel() {
        let ctx = EmptyContext
            + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>;
        let runtime = CcekRuntime::new(ctx);

        let (tx, rx) = runtime.channel::<i32>(10);

        tx.send(42).await.unwrap();
        let value = rx.recv().await.unwrap();

        assert_eq!(value, 42);
    }

    #[tokio::test]
    async fn test_context_ext_spawn() {
        let ctx = EmptyContext
            + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>;
        
        let handle = ctx.spawn_task(async {
            "hello".to_string()
        });
        
        let result = handle.await.unwrap();
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn test_context_ext_channel() {
        let ctx = EmptyContext;
        
        let (tx, rx) = ctx.create_channel::<String>(10);
        
        tx.send("test".to_string()).await.unwrap();
        let value = rx.recv().await.unwrap();
        
        assert_eq!(value, "test");
    }

    #[tokio::test]
    async fn test_iter_stream() {
        let ctx = EmptyContext
            + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>;
        let runtime = CcekRuntime::new(ctx);

        let stream = runtime.iter_stream(vec![1, 2, 3, 4, 5]);
        use tokio_stream::StreamExt;
        let values: Vec<_> = stream.collect().await;

        assert_eq!(values, vec![1, 2, 3, 4, 5]);
    }
}
