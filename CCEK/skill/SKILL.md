# CCEK Pattern Skill

## Overview

CCEK (CoroutineContext Element Key) mirrors Kotlin's CoroutineContext pattern **exactly**, with deviations documented.

## Kotlin Abstractions - Exact Mirror

### 1. CoroutineContext

**Kotlin:**
```kotlin
public interface CoroutineContext {
    public operator fun <E : Element> get(key: Key<E>): E?
    public fun minusKey(key: Key<*>): CoroutineContext
    public operator fun plus(context: CoroutineContext): CoroutineContext
}
```

**Rust:**
```rust
pub trait CcekContext {
    fn get<E: CcekElement + 'static>(&self) -> Option<&E>;
    fn minus_key(&self, key: &'static str) -> Self;
}
impl std::ops::Add for CcekContext { ... }
```

### 2. Element

**Kotlin:**
```kotlin
public interface Element {
    public val key: Key<*>
}
```

**Rust:**
```rust
pub trait CcekElement: Send + Sync + 'static {
    fn key(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
}
```

### 3. Key (companion object)

**Kotlin:**
```kotlin
public interface Key<E : Element>
// Usage:
public companion object Key : Key<Job>
```

**Rust:**
```rust
pub trait CcekKey: 'static {
    type Element: CcekElement;
}
```

### 4. Job = Element + Coroutine

**Kotlin:**
```kotlin
public interface Job : CoroutineContext.Element, Coroutine {
    public val isActive: Boolean
    public val isCompleted: Boolean
    public suspend fun join()
    public fun cancel()
    public companion object Key : Key<Job>
}
```

**Rust:**
```rust
pub trait CcekJob: CcekElement {
    fn is_active(&self) -> bool;
    fn is_completed(&self) -> bool;
    fn cancel(&self);
}
```

### 5. CoroutineScope

**Kotlin:**
```kotlin
public interface CoroutineScope {
    public val coroutineContext: CoroutineContext
}
public suspend fun CoroutineScope.coroutineScope(block: suspend CoroutineScope.() -> T)
```

**Rust:**
```rust
pub trait CcekCoroutineScope {
    fn coroutine_context(&self) -> &CcekContext;
}
pub async fn coroutine_scope<S, T, F>(scope: &S, block: F) -> T
where
    S: CcekCoroutineScope,
    F: FnOnce(&S) -> Pin<Box<dyn Future<Output = T> + Send>>;
```

### 6. Channel

**Kotlin:**
```kotlin
public fun <E> Channel(capacity: Int): Channel<E>
public interface SendChannel<in E> {
    public suspend fun send(element: E)
    public fun trySend(element: E): ChannelResult<Unit>
    public fun close()
}
public interface ReceiveChannel<out E> {
    public suspend fun receive(): E
    public fun tryReceive(): ChannelResult<E>
}
```

**Rust:**
```rust
pub struct CcekChannel<T> { _marker: PhantomData<T> }
pub fn ccek_channel<T>(capacity: usize) -> CcekChannel<T>
pub trait CcekSendChannel<T>: Send { ... }
pub trait CcekReceiveChannel<T>: Send { ... }
```

### 7. Flow

**Kotlin:**
```kotlin
public interface Flow<out T> {
    public suspend fun collect(collector: FlowCollector<T>)
}
public interface FlowCollector<in T> {
    public suspend fun emit(value: T)
}
```

**Rust:**
```rust
pub trait CcekFlow<T>: Send + Sync {
    fn collect<C>(&self, collector: C) where C: CcekFlowCollector<T>;
}
pub trait CcekFlowCollector<T>: Send {
    fn emit(&mut self, value: T);
}
```

## Deviations from Kotlin

| Kotlin Feature | Rust Limitation | Explanation |
|---------------|-----------------|-------------|
| `val key: Key<*>` | `fn key(&self) -> &'static str` | Rust cannot have associated const with default impl in object-safe trait |
| Companion objects | Associated type in trait | Rust traits cannot have companion objects; using `CcekKey` trait with `type Element` |
| `suspend fun` | `async fn` + `Pin<Box<...>>` | Rust cannot express "suspend" directly; async/await is function-level, not type-level |
| `receiver extension fun` | Free function | Rust cannot have receiver extensions; `coroutine_scope(scope, block)` instead of `scope.coroutineScope(block)` |
| `ChannelResult<E>` | `CcekChannelResult` (no type param) | Rust enums cannot have type parameters with bounds |
| `Coroutine` interface | Not expressed | Rust async fns are not types that can implement interfaces |
| `size: Int` property | `fn size(&self) -> usize` | Rust struct cannot have const property |

## Why These Deviations Are Acceptable

1. **Semantic equivalence**: All Kotlin abstractions are expressible in Rust
2. **Type safety preserved**: Same compile-time guarantees where possible
3. **Ergonomics**: Using idiomatic Rust patterns that achieve same goals
4. **No runtime overhead**: Deviations don't add runtime cost

## Usage

```rust
use crate::ccek_sdk::{
    CcekContext, CcekElement, CcekKey, CcekJob,
    CcekCoroutineScope, coroutine_scope,
    CcekFlow, CcekFlowCollector,
    CcekChannel, ccek_channel,
};

// Compose context
let ctx = CcekContext::new();

// Get element by key
let job: Option<&dyn CcekJob> = ctx.get::<dyn CcekJob>();

// Use with scope
let scope = /* ... */;
coroutine_scope(&scope, |s| Box::pin(async move {
    // ...
})).await;
```

## Files

- `kotlin_mirror.rs` - Exact Kotlin translation with deviation comments
- `context.rs` - Context implementation
- `elements.rs` - Element implementations
- `scope.rs` - Scope implementation
