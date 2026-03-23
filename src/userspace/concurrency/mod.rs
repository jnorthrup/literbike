//! Structured concurrency patterns using CCEK (CoroutineContext Element Key)
//!
//! This module provides structured concurrency patterns inspired by Kotlin coroutines
//! using the Trikeshed CCEK pattern. No tokio dependency.
//!
//! Key concepts:
//! - CCEK: CoroutineContext Element Key (trait-based keyed services)
//! - Jobs: Unit of structured work with cancellation
//! - Deferred: Suspended computation with result
//! - Channels: Message passing between jobs

use std::any::{Any, TypeId};
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};

pub mod channel;

pub use channel::*;

/// Result type for coroutine operations that can be cancelled
pub type CoroutineResult<T> = Result<T, CancellationError>;

/// Exception thrown when a coroutine is cancelled
#[derive(Debug, Clone, PartialEq)]
pub struct CancellationError {
    pub message: String,
}

impl CancellationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CancellationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CancellationError: {}", self.message)
    }
}

impl std::error::Error for CancellationError {}

/// CCEK - Coroutine Context Element Key
/// Mirrors Kotlin's CoroutineContext.Key pattern
pub trait CcekKey: Send + Sync + 'static {
    type Element: CcekElement;
}

/// CCEK Element - Coroutine Context Element
/// Mirrors Kotlin's CoroutineContext.Element
pub trait CcekElement: Send + Sync + Any {
    fn clone_element(&self) -> Box<dyn CcekElement>;
    fn as_any(&self) -> &dyn Any;
}

impl<T: Clone + Send + Sync + 'static> CcekElement for T {
    fn clone_element(&self) -> Box<dyn CcekElement> {
        Box::new(self.clone())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// CCEK Context - collection of keyed elements
/// Mirrors Kotlin's CoroutineContext
#[derive(Clone, Default)]
pub struct CcekContext {
    elements: Arc<RwLock<std::collections::HashMap<TypeId, Box<dyn CcekElement>>>>,
}

impl Debug for CcekContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CcekContext").finish()
    }
}

impl CcekContext {
    pub fn new() -> Self {
        Self {
            elements: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub fn with<K: CcekKey>(mut self, element: K::Element) -> Self {
        let type_id = TypeId::of::<K>();
        {
            let mut elements = self.elements.write().unwrap();
            elements.insert(type_id, element.clone_element());
        }
        self
    }

    pub fn get<K: CcekKey>(&self) -> Option<K::Element>
    where
        K::Element: Clone,
    {
        let type_id = TypeId::of::<K>();
        let guard = self.elements.read().unwrap();
        let ptr = &*guard as *const std::collections::HashMap<TypeId, Box<dyn CcekElement>>;
        drop(guard);
        unsafe {
            (*ptr)
                .get(&type_id)
                .and_then(|e| (*e).as_any().downcast_ref::<K::Element>().cloned())
        }
    }

    pub fn minus_key<K: CcekKey>(mut self) -> Self {
        let type_id = TypeId::of::<K>();
        {
            let mut elements = self.elements.write().unwrap();
            elements.remove(&type_id);
        }
        self
    }
}

/// CCEK Key for Job
pub struct JobKey;

impl CcekKey for JobKey {
    type Element = Arc<dyn Job>;
}

/// CCEK Key for Cancellation
pub struct CancellationKey;

impl CcekKey for CancellationKey {
    type Element = CancellationToken;
}

/// Cancellation token for structured cancellation
#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<RwLock<bool>>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(RwLock::new(false)),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.read().unwrap()
    }

    pub fn cancel(&self) {
        *self.cancelled.write().unwrap() = true;
    }
}

/// Job trait - unit of structured work
pub trait Job: Send + Sync {
    fn is_active(&self) -> bool;
    fn cancel(&self);
}

/// Suspend token for suspend/resume without coroutines
pub struct SuspendToken<T> {
    state: Arc<RwLock<SuspendState<T>>>,
}

enum SuspendState<T> {
    Running,
    Suspended(T),
    Resumed(T),
    Complete(T),
    Cancelled,
}

impl<T> SuspendToken<T> {
    pub fn new(initial: T) -> Self {
        Self {
            state: Arc::new(RwLock::new(SuspendState::Running)),
        }
    }

    pub fn is_suspended(&self) -> bool {
        matches!(*self.state.read().unwrap(), SuspendState::Suspended(_))
    }

    pub fn suspend(&self, value: T) {
        *self.state.write().unwrap() = SuspendState::Suspended(value);
    }

    pub fn resume(&self, value: T) {
        *self.state.write().unwrap() = SuspendState::Resumed(value);
    }

    pub fn complete(&self, value: T) {
        *self.state.write().unwrap() = SuspendState::Complete(value);
    }

    pub fn cancel(&self) {
        *self.state.write().unwrap() = SuspendState::Cancelled;
    }
}

impl<T: Clone> Future for SuspendToken<T> {
    type Output = Result<T, CancellationError>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let state = self.state.read().unwrap();
        match &*state {
            SuspendState::Resumed(v) => Poll::Ready(Ok(v.clone())),
            SuspendState::Complete(v) => Poll::Ready(Ok(v.clone())),
            SuspendState::Cancelled => Poll::Ready(Err(CancellationError::new("Cancelled"))),
            _ => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ccek_context() {
        let ctx = CcekContext::new();
        assert!(ctx.get::<JobKey>().is_none());

        let job: Arc<dyn Job> = Arc::new(MockJob::default());
        let ctx = ctx.with::<JobKey>(job);

        assert!(ctx.get::<JobKey>().is_some());
    }

    #[derive(Default)]
    struct MockJob {
        active: Arc<RwLock<bool>>,
    }

    impl Job for MockJob {
        fn is_active(&self) -> bool {
            *self.active.read().unwrap()
        }
        fn cancel(&self) {
            *self.active.write().unwrap() = false;
        }
    }

    #[test]
    fn test_cancellation_token() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_suspend_token() {
        let token: SuspendToken<i32> = SuspendToken::new(42);
        assert!(!token.is_suspended());

        token.suspend(100);
        assert!(token.is_suspended());

        token.complete(200);
        // Now future would resolve to 200
    }
}

#[derive(Debug, Clone, Default)]
pub struct LimitedDispatcher;

impl LimitedDispatcher {
    pub fn new() -> Self {
        Self
    }
}
