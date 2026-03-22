//! Suspend/Resume primitives using futures and channels
//!
//! This module provides suspend/resume functionality without relying on
//! Rust coroutines (async/await). Instead, it uses futures polling and
//! channels to achieve similar behavior.
//!
//! Key concepts:
//! - `SuspendToken` — Token that can suspend/resume execution
//! - `SuspendFuture` — Future that wraps suspended state
//! - `StateChannel` — Channel for communicating suspension state

use std::collections::VecDeque;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use crate::concurrency::CancellationError;
use futures::FutureExt;

/// Suspension state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuspensionState<T> {
    Running,
    Suspended(T),
    Resumed(T),
    Completed(T),
    Cancelled,
}

/// Token for suspending and resuming tasks
pub struct SuspendToken<T> {
    state: Arc<SuspendState<T>>,
}

struct SuspendState<T> {
    state: Mutex<SuspensionState<T>>,
    wakers: Mutex<Vec<Waker>>,
    resume_requested: AtomicBool,
    resume_value: Mutex<Option<T>>,
}

impl<T> SuspendToken<T> {
    pub fn new(initial: T) -> Self {
        Self {
            state: Arc::new(SuspendState {
                state: Mutex::new(SuspensionState::Running),
                wakers: Mutex::new(Vec::new()),
                resume_requested: AtomicBool::new(false),
                resume_value: Mutex::new(None),
            }),
        }
    }

    pub fn is_suspended(&self) -> bool {
        matches!(
            *self.state.state.lock().unwrap(),
            SuspensionState::Suspended(_)
        )
    }

    pub fn is_completed(&self) -> bool {
        matches!(
            *self.state.state.lock().unwrap(),
            SuspensionState::Completed(_)
        )
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(
            *self.state.state.lock().unwrap(),
            SuspensionState::Cancelled
        )
    }

    pub fn suspend(&self, value: T) {
        {
            let mut state = self.state.state.lock().unwrap();
            *state = SuspensionState::Suspended(value);
        }

        let wakers = self.state.wakers.lock().unwrap();
        for waker in wakers.iter() {
            waker.wake_by_ref();
        }
    }

    pub fn resume(&self, value: T) {
        {
            let mut resume_value = self.state.resume_value.lock().unwrap();
            *resume_value = Some(value);
        }
        self.state.resume_requested.store(true, Ordering::SeqCst);

        let wakers = self.state.wakers.lock().unwrap();
        for waker in wakers.iter() {
            waker.wake_by_ref();
        }
    }

    pub fn complete(&self, value: T) {
        {
            let mut state = self.state.state.lock().unwrap();
            *state = SuspensionState::Completed(value);
        }

        let wakers = self.state.wakers.lock().unwrap();
        for waker in wakers.iter() {
            waker.wake_by_ref();
        }
    }

    pub fn cancel(&self) {
        {
            let mut state = self.state.state.lock().unwrap();
            *state = SuspensionState::Cancelled;
        }

        let wakers = self.state.wakers.lock().unwrap();
        for waker in wakers.iter() {
            waker.wake_by_ref();
        }
    }

    fn poll_suspend(&self, cx: &mut Context<'_>) -> Poll<SuspensionState<T>> {
        let mut state = self.state.state.lock().unwrap();
        let current = (*state).clone();

        match &current {
            SuspensionState::Suspended(_) | SuspensionState::Running => {
                if self.state.resume_requested.load(Ordering::SeqCst) {
                    let value = self.state.resume_value.lock().unwrap().take().unwrap();
                    self.state.resume_requested.store(false, Ordering::SeqCst);
                    *state = SuspensionState::Resumed(value);
                    Poll::Ready(SuspensionState::Resumed(
                        self.state.resume_value.lock().unwrap().clone().unwrap(),
                    ))
                } else {
                    let mut wakers = self.state.wakers.lock().unwrap();
                    wakers.push(cx.waker().clone());
                    Poll::Pending
                }
            }
            SuspensionState::Resumed(v) => {
                *state = SuspensionState::Running;
                Poll::Ready(SuspensionState::Running)
            }
            SuspensionState::Completed(v) => {
                Poll::Ready(SuspensionState::Completed(std::clone::Clone::clone(v)))
            }
            SuspensionState::Cancelled => Poll::Ready(SuspensionState::Cancelled),
        }
    }

    pub fn now_or_never(&self) -> Option<Result<T, CancellationError>> {
        use std::task::Poll;
        let mut cx = std::task::Context::from_waker(std::task::Waker::from_ref(
            &(NoopWaker {} as NoopWaker),
        ));
        match self.poll_suspend(&mut cx) {
            Poll::Ready(SuspensionState::Resumed(v)) => Some(Ok(v)),
            Poll::Ready(SuspensionState::Completed(v)) => Some(Ok(v)),
            Poll::Ready(SuspensionState::Cancelled) => {
                Some(Err(CancellationError::new("Suspended operation cancelled")))
            }
            Poll::Pending => None,
        }
    }
}

struct NoopWaker;
impl std::task::Wake for NoopWaker {
    fn wake(self: Arc<Self>) {}
}

impl<T: Clone> Clone for SuspendToken<T> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

/// Future for suspended operations
pub struct SuspendFuture<T> {
    token: SuspendToken<T>,
}

impl<T> SuspendFuture<T> {
    pub fn new(token: SuspendToken<T>) -> Self {
        Self { token }
    }

    pub fn suspend(&self, value: T) {
        self.token.suspend(value);
    }

    pub fn resume(&self, value: T) {
        self.token.resume(value);
    }

    pub fn cancel(&self) {
        self.token.cancel();
    }

    pub fn now_or_never(&self) -> Option<Result<T, CancellationError>> {
        self.token.now_or_never()
    }
}

impl<T: Clone> Future for SuspendFuture<T> {
    type Output = Result<T, CancellationError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.token.poll_suspend(cx) {
            SuspensionState::Resumed(v) => Poll::Ready(Ok(v)),
            SuspensionState::Completed(v) => Poll::Ready(Ok(v)),
            SuspensionState::Cancelled => {
                Poll::Ready(Err(CancellationError::new("Suspended operation cancelled")))
            }
            _ => Poll::Pending,
        }
    }
}

/// Continuation for suspended tasks
pub struct Continuation<T, R> {
    pub input: T,
    pub continuation: Box<dyn FnOnce(T) -> R + Send>,
}

impl<T, R> Continuation<T, R> {
    pub fn new(input: T, continuation: impl FnOnce(T) -> R + Send + 'static) -> Self {
        Self {
            input,
            continuation: Box::new(continuation),
        }
    }

    pub fn execute(self) -> R {
        (self.continuation)(self.input)
    }
}

/// Continuation scheduler for managing suspended task continuations
pub struct ContinuationScheduler<T, R> {
    pending: Mutex<VecDeque<Continuation<T, R>>>,
    waker: Mutex<Option<Waker>>,
}

impl<T, R> ContinuationScheduler<T, R> {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(VecDeque::new()),
            waker: Mutex::new(None),
        }
    }

    pub fn schedule(&self, cont: Continuation<T, R>) {
        {
            let mut pending = self.pending.lock().unwrap();
            pending.push_back(cont);
        }

        if let Some(waker) = self.waker.lock().unwrap().take() {
            waker.wake();
        }
    }

    pub fn poll_continuations(&self, cx: &mut Context<'_>) -> Poll<Option<R>> {
        let mut pending = self.pending.lock().unwrap();

        if let Some(cont) = pending.pop_front() {
            drop(pending);
            let result = cont.execute();
            Poll::Ready(Some(result))
        } else {
            *self.waker.lock().unwrap() = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pending.lock().unwrap().is_empty()
    }
}

impl<T, R> Default for ContinuationScheduler<T, R> {
    fn default() -> Self {
        Self::new()
    }
}

/// Reactor continuation that can be suspended and resumed
pub struct ReactorContinuation<T> {
    state: Arc<AtomicU64>,
    suspend_token: SuspendToken<T>,
    step_count: u64,
}

impl<T> ReactorContinuation<T> {
    pub fn new(initial: T) -> Self {
        Self {
            state: Arc::new(AtomicU64::new(0)),
            suspend_token: SuspendToken::new(initial),
            step_count: 0,
        }
    }

    pub fn step(&self) -> u64 {
        self.state.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn current_step(&self) -> u64 {
        self.state.load(Ordering::SeqCst)
    }

    pub fn suspend(&self, value: T) {
        self.suspend_token.suspend(value);
    }

    pub fn resume(&self, value: T) {
        self.suspend_token.resume(value);
    }

    pub fn token(&self) -> &SuspendToken<T> {
        &self.suspend_token
    }
}

impl<T> Clone for ReactorContinuation<T> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            suspend_token: self.suspend_token.clone(),
            step_count: self.step_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suspend_and_resume() {
        let token = SuspendToken::new(42);

        token.resume(100);

        let future = SuspendFuture::new(token);
        assert_eq!(future.now_or_never(), Some(Ok(100)));
    }

    #[test]
    fn test_suspend_complete() {
        let token = SuspendToken::new(42);
        token.complete(200);

        let future = SuspendFuture::new(token);
        assert_eq!(future.now_or_never(), Some(Ok(200)));
    }

    #[test]
    fn test_suspend_cancel() {
        let token = SuspendToken::new(42);
        token.cancel();

        let future = SuspendFuture::new(token);
        assert!(future.now_or_never().unwrap().is_err());
    }

    #[test]
    fn test_reactor_continuation() {
        let cont: ReactorContinuation<i32> = ReactorContinuation::new(0);

        let step1 = cont.step();
        let step2 = cont.step();

        assert_eq!(step1, 1);
        assert_eq!(step2, 2);
        assert_eq!(cont.current_step(), 2);
    }
}
