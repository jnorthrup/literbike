//! CCEK - Kotlin CoroutineContext Exact Mirror
//!
//! Kotlin source (kotlinx-coroutines):
//! ```kotlin
//! public interface CoroutineContext {
//!     public operator fun <E : Element> get(key: Key<E>): E?
//!     public fun minusKey(key: Key<*>): CoroutineContext
//!     public operator fun plus(context: CoroutineContext): CoroutineContext
//! }
//!
//! public interface Element {
//!     public val key: Key<*>
//! }
//!
//! public interface Key<E : Element>
//!
//! public interface Job : Element, Coroutine {
//!     public val isActive: Boolean
//!     public val isCompleted: Boolean
//!     public suspend fun join()
//!     public fun cancel()
//!     public companion object Key : Key<Job>
//! }
//!
//! public interface CoroutineScope {
//!     public val coroutineContext: CoroutineContext
//! }
//!
//! public interface Flow<out T> {
//!     public suspend fun collect(collector: FlowCollector<T>)
//! }
//!
//! public interface FlowCollector<in T> {
//!     public suspend fun emit(value: T)
//! }
//!
//! public fun <E> Channel(capacity: Int): Channel<E>
//! ```

use std::any::{Any, TypeId};

// CoroutineContext -----------------------------------------------------------

pub trait CcekContext {
    fn get<E: CcekElement + 'static>(&self) -> Option<&E>;
    fn minus_key(&self, key: TypeId) -> Self;
}

impl std::ops::Add for dyn CcekContext {
    type Output = Box<dyn CcekContext>;
    fn add(self, rhs: Self) -> Self::Output {
        todo!("compound context")
    }
}

// Element --------------------------------------------------------------------

pub trait CcekElement: Send + Sync + 'static {
    fn key(&self) -> TypeId;
    fn as_any(&self) -> &dyn Any;
}

// Key -----------------------------------------------------------------------

pub trait CcekKey: 'static {
    type Element: CcekElement;
}

// Job -----------------------------------------------------------------------

pub trait CcekJob: CcekElement {
    fn is_active(&self) -> bool;
    fn is_completed(&self) -> bool;
    fn cancel(&self);
    fn join(&self);
}

// CoroutineScope ------------------------------------------------------------

pub trait CcekCoroutineScope {
    fn coroutine_context(&self) -> &dyn CcekContext;
}

// Flow ----------------------------------------------------------------------

pub trait CcekFlow<T>: Send + Sync {
    fn collect<C>(&self, collector: C)
    where
        C: CcekFlowCollector<T>;
}

pub trait CcekFlowCollector<T>: Send {
    fn emit(&mut self, value: T);
}

// Channel -------------------------------------------------------------------

pub struct CcekChannel<T> {
    _marker: std::marker::PhantomData<T>,
}

pub fn ccek_channel<T>(capacity: usize) -> CcekChannel<T> {
    let _ = capacity;
    CcekChannel {
        _marker: std::marker::PhantomData,
    }
}

pub trait CcekSendChannel<T>: Send {
    fn send(&self, element: T);
    fn try_send(&self, element: T);
    fn close(&self);
}

pub trait CcekReceiveChannel<T>: Send {
    fn receive(&self) -> T;
    fn try_receive(&self) -> Option<T>;
}
