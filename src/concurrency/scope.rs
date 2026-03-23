//! Simplified Structured concurrency scopes - no tokio
//!
//! Uses std futures and userspace executor.

use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
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
}

impl CoroutineJob {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(JobState::New)),
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
        *self.state.write() = JobState::Cancelled;
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

impl Default for CoroutineJob {
    fn default() -> Self {
        Self::new()
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
    
    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.read()
    }
    
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
    
    /// Launch a fire-and-forget coroutine
    pub fn launch<F>(&self, f: F) -> Arc<CoroutineJob>
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        let job = Arc::new(CoroutineJob::new());
        job.mark_active();
        // TODO: actually spawn and manage the future
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

/// High-level coroutine scope function
pub async fn coroutine_scope<F, T>(f: F) -> Result<T>
where
    F: Future<Output = Result<T>> + Send,
    T: Send,
{
    f.await
}

/// High-level supervisor scope function
pub async fn supervisor_scope<F, T>(f: F) -> Result<T>
where
    F: Future<Output = Result<T>> + Send,
    T: Send,
{
    f.await
}
