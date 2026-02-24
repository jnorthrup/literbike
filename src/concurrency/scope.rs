//! Simplified Structured concurrency scopes

use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio::sync::oneshot;
use anyhow::Result;
use parking_lot::RwLock;

/// Job state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    New,
    Active,
    Completed,
    Cancelled,
    Failed,
}

/// Job interface for cancellable computations
pub trait Job: Send + Sync {
    fn cancel(&self);
    fn is_active(&self) -> bool;
    fn is_completed(&self) -> bool;
    fn is_cancelled(&self) -> bool;
}

/// Concrete job implementation
pub struct CoroutineJob {
    state: Arc<RwLock<JobState>>,
    cancel_tx: Arc<RwLock<Option<oneshot::Sender<()>>>>,
}

impl CoroutineJob {
    pub fn new(cancel_tx: oneshot::Sender<()>) -> Self {
        Self {
            state: Arc::new(RwLock::new(JobState::New)),
            cancel_tx: Arc::new(RwLock::new(Some(cancel_tx))),
        }
    }
    
    pub fn mark_active(&self) {
        *self.state.write() = JobState::Active;
    }
    
    pub fn mark_completed(&self) {
        *self.state.write() = JobState::Completed;
    }
    
    pub fn mark_failed(&self) {
        *self.state.write() = JobState::Failed;
    }
}

impl Job for CoroutineJob {
    fn cancel(&self) {
        let mut state = self.state.write();
        if *state == JobState::Active || *state == JobState::New {
            *state = JobState::Cancelled;
            if let Some(tx) = self.cancel_tx.write().take() {
                let _ = tx.send(());
            }
        }
    }
    
    fn is_active(&self) -> bool {
        *self.state.read() == JobState::Active
    }
    
    fn is_completed(&self) -> bool {
        let state = *self.state.read();
        state == JobState::Completed || state == JobState::Failed
    }
    
    fn is_cancelled(&self) -> bool {
        *self.state.read() == JobState::Cancelled
    }
}

/// CoroutineScope - structured concurrency scope
pub struct CoroutineScope {
    cancelled: Arc<RwLock<bool>>,
}

impl CoroutineScope {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Spawn a child coroutine
    pub fn spawn<F, T>(&self, f: F) -> JoinHandle<Result<T>>
    where
        F: futures::Future<Output = Result<T>> + Send + 'static,
        T: Send + 'static,
    {
        tokio::spawn(async move { f.await })
    }
    
    /// Check if scope is cancelled
    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.read()
    }
    
    /// Cancel the scope
    pub fn cancel(&self) {
        *self.cancelled.write() = true;
    }
}

impl Default for CoroutineScope {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for CoroutineScope {
    fn clone(&self) -> Self {
        Self {
            cancelled: self.cancelled.clone(),
        }
    }
}

/// SupervisorScope - supervisor that doesn't fail on child errors
pub struct SupervisorScope {
    cancelled: Arc<RwLock<bool>>,
}

impl SupervisorScope {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Spawn a child coroutine (errors don't propagate)
    pub fn spawn<F, T>(&self, f: F) -> JoinHandle<Result<T>>
    where
        F: futures::Future<Output = Result<T>> + Send + 'static,
        T: Send + 'static,
    {
        tokio::spawn(async move {
            let result = f.await;
            // Don't propagate error
            result
        })
    }
    
    /// Launch a fire-and-forget coroutine
    pub fn launch<F>(&self, f: F) -> Arc<CoroutineJob>
    where
        F: futures::Future<Output = Result<()>> + Send + 'static,
    {
        let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();
        let job = Arc::new(CoroutineJob::new(cancel_tx));
        job.mark_active();
        
        let job_clone = job.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = &mut cancel_rx => {
                    job_clone.mark_completed();
                }
                result = f => {
                    if result.is_ok() {
                        job_clone.mark_completed();
                    } else {
                        job_clone.mark_failed();
                    }
                }
            }
        });
        
        job
    }
    
    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.read()
    }
    
    pub fn cancel(&self) {
        *self.cancelled.write() = true;
    }
}

impl Default for SupervisorScope {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SupervisorScope {
    fn clone(&self) -> Self {
        Self {
            cancelled: self.cancelled.clone(),
        }
    }
}

/// Spawn a coroutine in the current scope
pub fn spawn<F, T>(scope: &CoroutineScope, f: F) -> JoinHandle<Result<T>>
where
    F: futures::Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    scope.spawn(f)
}

/// Launch a fire-and-forget coroutine
pub fn launch<F>(scope: &SupervisorScope, f: F) -> Arc<CoroutineJob>
where
    F: futures::Future<Output = Result<()>> + Send + 'static,
{
    scope.launch(f)
}

/// High-level coroutine scope function
pub async fn coroutine_scope<F, T>(f: F) -> Result<T>
where
    F: futures::Future<Output = Result<T>> + Send,
    T: Send,
{
    f.await
}

/// High-level supervisor scope function
pub async fn supervisor_scope<F, T>(f: F) -> Result<T>
where
    F: futures::Future<Output = Result<T>> + Send,
    T: Send,
{
    f.await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_coroutine_scope() {
        let result = coroutine_scope(async {
            Ok(42)
        }).await;
        
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_supervisor_scope() {
        let result = supervisor_scope(async {
            Ok(42)
        }).await;
        
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_job_cancel() {
        let scope = SupervisorScope::new();
        
        let job = scope.launch(async {
            sleep(Duration::from_secs(10)).await;
            Ok(())
        });
        
        assert!(job.is_active());
        
        job.cancel();
        
        assert!(job.is_cancelled());
    }
}
