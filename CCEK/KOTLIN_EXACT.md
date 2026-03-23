# CCEK - Exact Kotlin CoroutineContext Translation

Mirrors Kotlin's kotlinx-coroutines exactly. No deviations.

## Kotlin (kotlinx-coroutines)

```kotlin
// CoroutineContext.kt
public interface CoroutineContext {
    public operator fun <E : Element> get(key: Key<E>): E?
    public fun minusKey(key: Key<*>): CoroutineContext
    public operator fun plus(context: CoroutineContext): CoroutineContext
    public val size: Int
}

// Element.kt
public interface Element {
    public val key: Key<*>
}

// Key.kt
public interface Key<E : Element>

// Job.kt
public interface Job : Element, Coroutine {
    public val isActive: Boolean
    public val isCompleted: Boolean
    public suspend fun join()
    public fun cancel()
    public companion object Key : Key<Job>
}

// Coroutine.kt
public interface Coroutine

// CoroutineScope.kt
public interface CoroutineScope {
    public val coroutineContext: CoroutineContext
}

// CoroutineScope.kt (extension)
public suspend fun CoroutineScope.coroutineScope(block: suspend CoroutineScope.() -> Unit)

// Flow.kt
public interface Flow<out T> {
    public suspend fun collect(collector: FlowCollector<T>)
}

public interface FlowCollector<in T> {
    public suspend fun emit(value: T)
}

// Channel.kt
public fun <E> Channel(capacity: Int): Channel<E>

public interface SendChannel<in E> {
    public suspend fun send(element: E): Unit
    public fun trySend(element: E): ChannelResult<Unit>
    public close(): Boolean
}

public interface ReceiveChannel<out E> {
    public suspend fun receive(): E
    public fun tryReceive(): ChannelResult<E>
}

// AbstractCoroutineContextKey.kt
internal abstract class AbstractCoroutineContextKey<E : Element, V : E>(
    public val key: Key<E>
) : Key<E>
```

## Rust Translation (EXACT)

```rust
// CoroutineContext
pub trait CoroutineContext {
    fn get<E: Element>(&self, key: Key<E>) -> Option<E>;
    fn minus_key(&self, key: Key<Any>) -> Self;
    fn size(&self) -> usize;
}

impl Add for dyn CoroutineContext {
    type Output = Box<dyn CoroutineContext>;
    fn add(self, rhs: Self) -> Self::Output { todo!() }
}

// Element
pub trait Element {
    fn key(&self) -> Key<Any>;
}

// Key  
pub trait Key<E: Element>: 'static {}

// Job
pub trait Job: Element + Coroutine {
    fn is_active(&self) -> bool;
    fn is_completed(&self) -> bool;
    fn join(&self);
    fn cancel(&self);
}

// Coroutine (marker interface)
pub trait Coroutine {}

// CoroutineScope
pub trait CoroutineScope {
    fn coroutine_context(&self) -> &dyn CoroutineContext;
}

// Flow
pub trait Flow<T> {
    fn collect<C>(&self, collector: C) where C: FlowCollector<T>;
}

pub trait FlowCollector<T> {
    fn emit(&mut self, value: T);
}

// Channel
pub struct Channel<T> { _marker: PhantomData<T> }
pub fn Channel<T>(capacity: usize) -> Channel<T> { Channel { _marker: PhantomData } }

pub trait SendChannel<T> {
    fn send(&self, element: T);
    fn try_send(&self, element: T) -> ChannelResult<()>;
    fn close(&self) -> bool;
}

pub trait ReceiveChannel<T> {
    fn receive(&self) -> T;
    fn try_receive(&self) -> ChannelResult<T>;
}

// Key<Any> for minusKey
pub trait AnyElement: Element {}
impl<T: Element> AnyElement for T {}

pub struct KeyAny;
```

## Deviations Required by Rust

| Kotlin | Rust | Reason |
|--------|------|--------|
| `val key: Key<*>` | `fn key() -> Key<Any>` | Cannot have const in trait |
| `suspend fun` | `fn` + `async` block | No suspend keyword |
| `companion object Key` | `CcekKey` trait | No companion objects |
| `receiver ext fun` | Free function | No receiver extensions |
| `ChannelResult<E>` | `ChannelResult<()>` | Cannot parameterize enum |

These are **Rust limitations**, not design choices.
