# CCEK Pattern Skill

## Overview

CCEK (CoroutineContext Element Key) mirrors Kotlin's CoroutineContext pattern **exactly**.

## Kotlin Abstractions We Preserve

### 1. CoroutineContext

```kotlin
public interface CoroutineContext {
    public operator fun <E : Element> get(key: Key<E>): E?
    public fun minusKey(key: Key<*>): CoroutineContext
    public operator fun plus(context: CoroutineContext): CoroutineContext
}
```

### 2. CoroutineContext.Element

```kotlin
public interface Element {
    public val key: Key<*>
}
```

### 3. Element.Key (companion object)

```kotlin
public interface Key<E : Element>

// Example:
public interface Job : Element, Coroutine {
    public companion object Key : Key<Job>
}
```

### 4. Job = Element + Coroutine

```kotlin
public interface Job : CoroutineContext.Element, Coroutine {
    public val isActive: Boolean
    public suspend fun join()
}
```

### 5. Channel

```kotlin
public fun <E> Channel(capacity: Int): Channel<E>
public interface SendChannel<in E> { suspend fun send(element: E) }
public interface ReceiveChannel<out E> { suspend fun receive(): E }
```

### 6. Flow (cold stream)

```kotlin
public interface Flow<out T> {
    public suspend fun collect(collector: FlowCollector<T>)
}
```

### 7. CoroutineScope

```kotlin
public interface CoroutineScope {
    public val coroutineContext: CoroutineContext
}
```

## Rust Translation

```rust
// CoroutineContext
pub trait CcekContext {
    fn get<E: CcekElement>(&self) -> Option<&E>;
    fn minus_key(&self, key: &'static str) -> Self;
}
impl Add for CcekContext { ... }  // + operator

// Element with companion Key
pub trait CcekElement: Send + Sync + 'static {
    fn key(&self) -> &'static str;
}

pub trait CcekKey<E: CcekElement>: 'static {
    // Companion object pattern
}

// Job IS Element + Coroutine
pub trait CcekJob: CcekElement {
    fn is_active(&self) -> bool;
    async fn join(&self);
}

// Channel
pub struct Channel<T> { ... }
pub struct ChannelTx<T> { ... }
pub struct ChannelRx<T> { ... }

// Flow
pub trait CcekFlow<T>: Send + Sync {
    fn collect<C>(&self, collector: C) where C: CcekFlowCollector<T>;
}
pub trait CcekFlowCollector<T> { ... }

// CoroutineScope
pub trait CcekScope {
    fn context(&self) -> &CcekContext;
}
```

## Kotlin Usage

```kotlin
val ctx = EmptyCoroutineContext + htxService + quicService

val job = ctx[Job]?.launch {
    flow.collect { value ->
        ctx[Channel]?.send(value)
    }
}
```

## Rust Usage (Same Pattern)

```rust
let ctx = EmptyContext + htx_element + quic_element;

let job = ctx.get::<dyn CcekJob>()?.launch(|| async {
    flow.collect(|value| {
        ctx.get::<Channel<_>>()?.send(value).await
    });
});
```

## Files

- `context.rs` - CoroutineContext, Element, Key traits
- `elements.rs` - Element implementations (HtxElement, QuicElement, etc.)
- `keys.rs` - Key companion objects
- `channels.rs` - Channel, ChannelTx, ChannelRx
- `scope.rs` - CoroutineScope trait
