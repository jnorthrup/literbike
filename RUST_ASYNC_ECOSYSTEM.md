# Rust Async Ecosystem for Kotlin-style Concurrency

## Existing Crates for Structured Concurrency

The Rust ecosystem already has excellent crates that implement patterns similar to Kotlin coroutines. Here's how they map to our CCEK implementation:

---

## 1. Channels

### async-channel (2.5.0)
**Kotlin Equivalent:** `kotlinx.coroutines.channels.Channel`

```rust
use async_channel::{bounded, unbounded};

// Bounded channel (like Channel(capacity))
let (tx, rx) = bounded::<i32>(10);
tx.send(42).await?;
let value = rx.recv().await?;

// Unbounded channel (like Channel(Channel.UNLIMITED))
let (tx, rx) = unbounded::<String>();
```

**Features:**
- ✅ Multi-producer multi-consumer (MPMC)
- ✅ Bounded and unbounded variants
- ✅ Zero-copy message passing
- ✅ Backpressure support

**Comparison with our implementation:**
| Feature | Our Implementation | async-channel |
|---------|-------------------|---------------|
| MPMC | ✅ | ✅ |
| Bounded | ✅ | ✅ |
| Unbounded | ❌ | ✅ |
| Backpressure | ✅ | ✅ |
| Scope integration | ✅ | ❌ |

**Recommendation:** Use `async-channel` for production, keep ours for CCEK integration.

---

## 2. Streams/Flows

### tokio-stream (0.1.18)
**Kotlin Equivalent:** `kotlinx.coroutines.flow.Flow`

```rust
use tokio_stream::{StreamExt, StreamMap};
use tokio::time::{interval, Duration};

// Create interval stream (like flow.tick())
let mut intv = interval(Duration::from_millis(100));
let stream = tokio_stream::wrappers::IntervalStream::new(intv);

// Map, filter, etc.
let processed = stream
    .map(|_| get_data())
    .filter(|result| result.is_ok())
    .take(10);
```

**Features:**
- ✅ Full Stream trait implementation
- ✅ Interoperates with tokio
- ✅ Rich operator set (map, filter, fold, etc.)
- ✅ StreamMap for merging multiple streams

### async-stream (0.3.6)
**Kotlin Equivalent:** `flow { emit(value) }` builder

```rust
use async_stream::stream;
use futures_util::StreamExt;

let s = stream! {
    for i in 0..10 {
        yield i;
    }
};

// Consume the stream
s.for_each(|x| println!("{}", x)).await;
```

**Features:**
- ✅ Async generator syntax
- ✅ Zero-cost abstraction
- ✅ Integrates with futures

**Comparison with our Flow:**
| Feature | Our Flow | tokio-stream + async-stream |
|---------|----------|----------------------------|
| Builder syntax | ❌ | ✅ (`stream!{}`) |
| Operators | Basic | Full |
| Backpressure | ❌ | ✅ |
| Tokio integration | ✅ | ✅ |
| Standalone | ✅ | Requires tokio |

**Recommendation:** Use `tokio-stream` + `async-stream` for production flows.

---

## 3. Structured Concurrency

### tokio::spawn (built-in)
**Kotlin Equivalent:** `CoroutineScope.launch`

```rust
use tokio::task::JoinHandle;

let handle: JoinHandle<Result<(), Error>> = tokio::spawn(async {
    do_work().await
});

// Wait for completion
handle.await??;

// Cancel
handle.abort();
```

### tokio::select! (built-in)
**Kotlin Equivalent:** `coroutineScope { select { } }`

```rust
tokio::select! {
    result = task1() => { /* handle */ }
    result = task2() => { /* handle */ }
    _ = shutdown_signal() => { /* cleanup */ }
}
```

### asupersync (0.2.6)
**Kotlin Equivalent:** `supervisorScope`

```rust
use asupersync::supervisor;

supervisor(|scope| async {
    scope.spawn(async { risky_task().await });
    scope.spawn(async { another_task().await });
    // Failures don't propagate
}).await;
```

**Features:**
- ✅ True supervisor scope
- ✅ Cancel-safe
- ✅ Capability-secure

**Comparison with our scope:**
| Feature | Our Scope | asupersync | tokio |
|---------|-----------|------------|-------|
| coroutineScope | ✅ | ✅ | Manual |
| supervisorScope | ✅ | ✅ | Manual |
| Job tracking | Basic | Advanced | Basic |
| Cancel propagation | ✅ | ✅ | Manual |
| Maturity | New | Experimental | Production |

**Recommendation:** Use tokio for production, keep ours for CCEK integration.

---

## 4. Context/Dependency Injection

### No direct equivalent in Rust

Kotlin's `CoroutineContext.Element` pattern is unique. Our CCEK implementation fills this gap:

```kotlin
// Kotlin
val ctx = EmptyCoroutineContext + service1 + service2
```

```rust
// Rust with CCEK
let ctx = EmptyContext
    + Arc::new(service1) as Arc<dyn ContextElement>
    + Arc::new(service2);
```

**Alternatives:**
- **Arc<AppState>** - Common pattern but less composable
- **Thread-local storage** - Global state, not per-coroutine
- **Macros** - Compile-time only, not runtime composable

**Our CCEK advantage:** Runtime-composable, type-safe, Kotlin-equivalent pattern.

---

## 5. Job Management

### tokio::task::JoinHandle
**Kotlin Equivalent:** `Job`

```rust
let handle = tokio::spawn(async { work().await });

// Check status
if handle.is_finished() { /* ... */ }

// Cancel
handle.abort();

// Wait
handle.await??;
```

### tokio-console (0.5.0)
**Kotlin Equivalent:** `CoroutineDispatcher` monitoring

```rust
// Runtime telemetry
use tokio_metrics::RuntimeMonitor;

let monitor = RuntimeMonitor::new(&runtime);
for metrics in monitor.intervals() {
    println!("Tasks: {}", metrics.num_tasks);
}
```

---

## Recommended Integration Strategy

### Phase 1: Use Existing Crates (Production Ready)

```rust
// Cargo.toml
[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"
async-stream = "0.3"
async-channel = "2.5"
futures = "0.3"
```

```rust
// Use tokio for concurrency
use tokio::task::JoinHandle;
use tokio_stream::{Stream, StreamExt};
use async_channel::{bounded, Sender, Receiver};

// Our CCEK for context composition
use literbike::concurrency::{EmptyContext, ContextElement, CoroutineContext};
```

### Phase 2: Bridge CCEK with Tokio

```rust
use literbike::concurrency::*;
use tokio::runtime::Handle;

/// CCEK-aware Tokio runtime wrapper
pub struct CcekRuntime {
    context: CoroutineContext,
    runtime_handle: Handle,
}

impl CcekRuntime {
    pub fn spawn<F, T>(&self, f: F) -> JoinHandle<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let ctx = self.context.clone();
        self.runtime_handle.spawn(async move {
            // Context is available in coroutine
            f.await
        })
    }
    
    pub fn channel<T>(&self, buffer: usize) -> (Sender<T>, Receiver<T>) {
        bounded(buffer)
    }
}
```

### Phase 3: Replace with Tokio Equivalents

| Our Module | Tokio Equivalent | Migration Path |
|------------|-----------------|----------------|
| `ChannelSender/Receiver` | `async_channel::Sender/Receiver` | Drop-in replacement |
| `Flow` | `tokio_stream::Stream` | Implement `Stream` trait |
| `CoroutineScope` | `tokio::spawn` | Wrap with context |
| `SupervisorScope` | `tokio::spawn` + error handling | Add error handler |
| `CCEK` | Keep (no equivalent) | Bridge to tokio |

---

## Updated Cargo.toml for Literbike

```toml
[dependencies]
# Core async runtime
tokio = { version = "1.41", features = ["full"] }
tokio-stream = "0.1"
async-stream = "0.3"

# Channels
async-channel = "2.5"
async-broadcast = "0.7"  # For pub/sub patterns

# Futures utilities
futures = "0.3"
futures-core = "0.3"

# Our CCEK (keep for context composition)
# No external dependency needed - it's in the crate

# Existing dependencies
parking_lot = "0.12"
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

---

## Code Migration Example

### Before (Our Implementation)

```rust
use literbike::concurrency::*;

async fn process_signals() -> Result<()> {
    let scope = SupervisorScope::new();
    let (tx, mut rx) = channel_with_scope(&scope, 100);
    
    let flow = Flow::from_iter(vec![1, 2, 3])
        .map(|x| x * 2)
        .filter(|x| *x > 2);
    
    scope.spawn(async move {
        while let Some(value) = rx.recv().await {
            process(value).await?;
        }
        Ok(())
    });
    
    Ok(())
}
```

### After (Tokio + async-channel)

```rust
use tokio_stream::StreamExt;
use async_channel::bounded;
use literbike::concurrency::{EmptyContext, ContextElement};

async fn process_signals() -> Result<()> {
    let (tx, rx) = bounded(100);
    
    let stream = tokio_stream::iter(vec![1, 2, 3])
        .map(|x| x * 2)
        .filter(|x| futures::future::ready(*x > 2));
    
    tokio::spawn(async move {
        while let Some(value) = rx.recv().await.ok() {
            process(value).await?;
        }
        Ok::<_, anyhow::Error>(())
    });
    
    Ok(())
}
```

---

## Summary

### Keep from Our Implementation:
- ✅ **CCEK** - No Rust equivalent for Kotlin-style context composition
- ✅ **Service traits** - ProtocolDetector, DHTService, etc.
- ✅ **Integration patterns** - How to compose services

### Replace with Existing Crates:
- 🔄 **Channels** → `async-channel`
- 🔄 **Flows** → `tokio-stream` + `async-stream`
- 🔄 **Scopes** → `tokio::spawn` (with CCEK wrapper)
- 🔄 **Job** → `tokio::task::JoinHandle`

### Benefits:
1. **Production-ready** - Existing crates are battle-tested
2. **Better performance** - Optimized implementations
3. **Rich ecosystem** - More operators, adapters, utilities
4. **Maintained** - Active development and support
5. **Interoperability** - Works with other tokio-based crates

### Next Steps:
1. Add `async-channel` and `tokio-stream` to Cargo.toml
2. Create bridge layer between CCEK and tokio
3. Migrate channel/flow implementations to use existing crates
4. Keep CCEK as the unique value proposition
