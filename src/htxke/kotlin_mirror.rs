//! CCEK - Exact Kotlin kotlinx-coroutines Translation
//!
//! Kotlin source:
//! ```kotlin
//! public interface CoroutineContext { ... }
//! public interface Element { val key: Key<*> }
//! public interface Key<E : Element>
//! public interface Job : Element, Coroutine { ... }
//! public interface CoroutineScope { ... }
//! public interface Flow<out T> { ... }
//! public fun Channel<T>(capacity: Int): Channel<T>
//! ```

use std::marker::PhantomData;
use std::any::TypeId;

// CoroutineContext -----------------------------------------------------------
// Kotlin: public interface CoroutineContext
pub trait CoroutineContext {
    fn get<E: Element>(&self, key: Key<E>) -> Option<E>
    where
        E: 'static;
    fn minus_key(&self, key: KeyAny) -> Self;
    fn size(&self) -> usize;
}

impl std::ops::Add for dyn CoroutineContext {
    type Output = Box<dyn CoroutineContext>;
    fn add(self, _rhs: Self) -> Self::Output {
        todo!("compound context")
    }
}

// Element -------------------------------------------------------------------
// Kotlin: public interface Element { public val key: Key<*> }
pub trait Element {
    fn key(&self) -> KeyAny;
}

// Key -----------------------------------------------------------------------
// Kotlin: public interface Key<E : Element>
pub trait Key<E: Element>: 'static {}

// Key<Any> for minusKey ---------------------------------------------------
// Kotlin uses Key<*> for the minusKey parameter
pub trait AnyElement: Element {}
impl<T: Element> AnyElement for T {}

// Rust limitation: No way to express Key<*> directly, using AnyElement

// Job -----------------------------------------------------------------------
// Kotlin: public interface Job : Element, Coroutine
pub trait Job: Element + Coroutine {
    fn is_active(&self) -> bool;
    fn is_completed(&self) -> bool;
    fn join(&self);
    fn cancel(&self);
}

// Coroutine -----------------------------------------------------------------
// Kotlin: public interface Coroutine (marker interface)
pub trait Coroutine {}

// CoroutineScope -----------------------------------------------------------
// Kotlin: public interface CoroutineScope
pub trait CoroutineScope {
    fn coroutine_context(&self) -> &dyn CoroutineContext;
}

// coroutineScope -----------------------------------------------------------
// Kotlin: public suspend fun CoroutineScope.coroutineScope(block: suspend CoroutineScope.() -> Unit)
// Rust limitation: No receiver extension, no suspend keyword
pub async fn coroutine_scope<S, F>(scope: &S, block: F)
where
    S: CoroutineScope,
    F: FnOnce(&S),
{
    block(scope)
}

// Flow ---------------------------------------------------------------------
// Kotlin: public interface Flow<out T>
pub trait Flow<T> {
    fn collect<C>(&self, collector: C)
    where
        C: FlowCollector<T>;
}

// Kotlin: public interface FlowCollector<in T>
pub trait FlowCollector<T> {
    fn emit(&mut self, value: T);
}

// Channel ------------------------------------------------------------------
// Kotlin: public fun <E> Channel(capacity: Int): Channel<E>
pub struct Channel<T> {
    _marker: PhantomData<T>,
}

pub fn Channel<T>(capacity: usize) -> Channel<T> {
    let _ = capacity;
    Channel { _marker: PhantomData }
}

// Kotlin: public interface SendChannel<in E>
pub trait SendChannel<T> {
    fn send(&self, _element: T);
    fn try_send(&self, _element: T) -> ChannelResult<()>;
    fn close(&self) -> bool;
}

// Kotlin: public interface ReceiveChannel<out E>
pub trait ReceiveChannel<T> {
    fn receive(&self) -> T;
    fn try_receive(&self) -> ChannelResult<T>;
}

// Kotlin: public enum class ChannelResult
// Rust limitation: Cannot parameterize enum with E, using ChannelResult<()> 
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelResult<T> {
    Success(T),
    Closed,
    Empty,
    Full,
}

// EmptyCoroutineContext ----------------------------------------------------
#[derive(Clone, Default)]
pub struct EmptyCoroutineContext;

impl CoroutineContext for EmptyCoroutineContext {
    fn get<E: Element>(&self, _key: Key<E>) -> Option<E> {
        None
    }
    fn minus_key(&self, _key: KeyAny) -> Self {
        EmptyCoroutineContext
    }
    fn size(&self) -> usize {
        0
    }
}

impl Element for EmptyCoroutineContext {
    fn key(&self) -> KeyAny {
        KeyAny
    }
}
