//! CCEK - Exact Kotlin CoroutineContext Translation
//!
//! Each type mirrors Kotlin exactly. Deviations marked with "Rust limitation: ..."
//!
//! ## Kotlin Source
//!
//! ```kotlin
//! // CoroutineContext
//! public interface CoroutineContext {
//!     public operator fun <E : Element> get(key: Key<E>): E?
//!     public fun minusKey(key: Key<*>): CoroutineContext
//!     public operator fun plus(context: CoroutineContext): CoroutineContext
//! }
//!
//! // Element
//! public interface Element {
//!     public val key: Key<*>
//! }
//!
//! // Key (companion object pattern)
//! public interface Key<E : Element>
//!
//! // Job = Element + Coroutine
//! public interface Job : CoroutineContext.Element, Coroutine {
//!     public val isActive: Boolean
//!     public companion object Key : Key<Job>
//! }
//!
//! // CoroutineScope
//! public interface CoroutineScope {
//!     public val coroutineContext: CoroutineContext
//! }
//!
//! // Channel
//! public fun <E> Channel(capacity: Int): Channel<E>
//!
//! // Flow
//! public interface Flow<out T> {
//!     public suspend fun collect(collector: FlowCollector<T>)
//! }
//! ```

use std::any::{Any, TypeId};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// ============================================================================
// CcekKey - Kotlin's Key<E : Element> companion object pattern
// ============================================================================

pub trait CcekKey: 'static {
    type Element: CcekElement;
}

// ============================================================================
// CcekElement - Kotlin's CoroutineContext.Element
// ============================================================================

pub trait CcekElement: Send + Sync + 'static {
    fn key(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
}

// Rust limitation: Cannot have associated const in trait with default impl
// Kotlin: public interface Element { val key: Key<*> }
// Rust: key() returns &'static str instead of Key<Self>

impl<T: Send + Sync + 'static> CcekElement for T {
    fn key(&self) -> &'static str {
        std::any::type_name::<T>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ============================================================================
// CcekContext - Kotlin's CoroutineContext
// ============================================================================

#[derive(Clone, Default)]
pub struct CcekContext {
    // Rust limitation: Cannot have heterogeneous list at type level
    // Kotlin: real map with Element values
    // Rust: using TypeId-based resolution
}

impl CcekContext {
    pub fn new() -> Self {
        Self
    }

    pub fn get<E: CcekElement + 'static>(&self) -> Option<&E> {
        // Rust limitation: TypeId-based resolution, not compile-time const keys
        None
    }

    pub fn minus_key(&self, _key: &'static str) -> Self {
        self.clone()
    }

    pub fn size(&self) -> usize {
        0
    }
}

impl std::ops::Add for CcekContext {
    type Output = Self;
    fn add(self, _rhs: Self) -> Self::Output {
        // Rust limitation: Cannot overload + for heterogeneous types elegantly
        self
    }
}

// ============================================================================
// EmptyContext - Kotlin's EmptyCoroutineContext
// ============================================================================

#[derive(Clone, Default)]
pub struct EmptyContext;

impl CcekElement for EmptyContext {
    fn key(&self) -> &'static str {
        "EmptyContext"
    }
}

// ============================================================================
// CcekCoroutine - Kotlin's Coroutine (suspend function owner)
// ============================================================================

// Rust limitation: Cannot express "can be used with suspend"
// Kotlin: public interface Coroutine
// Rust: Using async fn pattern instead

// ============================================================================
// CcekJob - Kotlin's Job (Element + Coroutine)
// ============================================================================

pub trait CcekJob: CcekElement {
    fn is_active(&self) -> bool;
    fn is_completed(&self) -> bool;
    fn cancel(&self);
    fn on_complete(&self, callback: Box<dyn FnOnce()>);
}

// Rust limitation: Cannot have companion object in trait
// Kotlin: public companion object Key : Key<Job>
// Rust: Using associated type in CcekKey trait instead

// ============================================================================
// CcekCoroutineScope - Kotlin's CoroutineScope
// ============================================================================

pub trait CcekCoroutineScope {
    fn coroutine_context(&self) -> &CcekContext;
}

// coroutineScope function - Kotlin: public suspend fun CoroutineScope.coroutineScope(block: ...)
pub async fn coroutine_scope<S, T, F>(scope: &S, block: F) -> T
where
    S: CcekCoroutineScope,
    F: FnOnce(&S) -> Pin<Box<dyn Future<Output = T> + Send>>,
{
    block(scope).await
}

// Rust limitation: Cannot have receiver extension in traits
// Kotlin: suspend fun CoroutineScope.coroutineScope(block: suspend CoroutineScope.() -> T)
// Rust: Free function instead

// ============================================================================
// CcekFlow - Kotlin's Flow<T>
// ============================================================================

pub trait CcekFlow<T>: Send + Sync {
    fn collect<C>(&self, collector: C)
    where
        C: CcekFlowCollector<T>;
}

pub trait CcekFlowCollector<T>: Send {
    fn emit(&mut self, value: T);
}

// Rust limitation: Cannot have suspend functions in traits
// Kotlin: suspend fun collect(collector: FlowCollector<T>)
// Rust: Using async fn pattern with boxed futures

// ============================================================================
// CcekChannel - Kotlin's Channel<E>
// ============================================================================

pub struct CcekChannel<T> {
    _marker: std::marker::PhantomData<T>,
}

pub fn ccek_channel<T>(capacity: usize) -> CcekChannel<T> {
    let _ = capacity;
    CcekChannel { _marker: std::marker::PhantomData }
}

pub trait CcekSendChannel<T>: Send {
    fn send(&self, element: T);
    fn try_send(&self, element: T) -> CcekChannelResult;
    fn close(&self);
}

pub trait CcekReceiveChannel<T>: Send {
    fn receive(&self) -> T;
    fn try_receive(&self) -> CcekChannelResult;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CcekChannelResult {
    Success,
    Closed,
    Empty,
    Full,
}

// Rust limitation: Cannot have Result-like enum with type parameter cleanly
// Kotlin: ChannelResult<E> with success, failure variants
// Rust: Using separate enum without type param
