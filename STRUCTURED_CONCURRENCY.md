# Structured Concurrency Solution for Literbike

## Overview

Based on Kotlin Coroutines patterns, this implementation provides:

1. **CCEK (CoroutineContext Element Key)** - Composable execution context
2. **Channels** - Type-safe message passing  
3. **Flows** - Reactive streams with functional operators
4. **Scopes** - Structured concurrency (coroutineScope, supervisorScope)

## Core Pattern: CCEK Bundling

### Kotlin Original (from BetanetIntegrationDemo.kt)

```kotlin
private fun setupServices(): CoroutineContext {
    return EmptyCoroutineContext +
        dhtService +
        protocolDetector +
        crdtStorage +
        crdtNetwork +
        conflictResolver
}
```

### Rust Equivalent

```rust
use literbike::concurrency::*;

let ctx = EmptyContext
    + Arc::new(ProtocolDetector::new()) as Arc<dyn ContextElement>
    + Arc::new(DHTService::new("node-1"))
    + Arc::new(CRDTStorage::new("/tmp/crdt"))
    + Arc::new(CRDTNetwork::new("peer-1"))
    + Arc::new(ConflictResolver::default());

// Access services by key
let detector = ctx.get_typed::<ProtocolDetector>("ProtocolDetector");
let dht = ctx.get_typed::<DHTService>("DHTService");
```

## Architecture

### 1. CCEK Module (`ccek.rs`)

**Key Components:**
- `ContextElement` trait - Like Kotlin's `CoroutineContext.Element`
- `CoroutineContext` - Composite context holding multiple elements
- `EmptyContext` - Like `EmptyCoroutineContext`
- Service implementations:
  - `ProtocolDetector` - Protocol detection (HTTP, QUIC, TLS)
  - `DHTService` - Distributed Hash Table service
  - `CRDTStorage` - CRDT persistence
  - `CRDTNetwork` - CRDT networking
  - `ConflictResolver` - Conflict resolution strategies

**Design Pattern:**
```rust
pub trait ContextElement: Send + Sync + 'static {
    fn key(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
}

// Services implement the trait
impl ContextElement for ProtocolDetector {
    fn key(&self) -> &'static str { "ProtocolDetector" }
    fn as_any(&self) -> &dyn Any { self }
}
```

### 2. Channel Module (`channel.rs`)

**Pattern from Kotlin:**
```kotlin
val channel = Channel<Int>(capacity = 64)
channel.send(42)
val value = channel.receive()
```

**Rust Implementation:**
```rust
let (tx, mut rx) = channel::<i32>(64);
tx.send(42).await?;
let value = rx.recv().await.unwrap();
```

**Features:**
- Async send/recv with backpressure
- Try-send for non-blocking operations
- Scope integration for structured concurrency

### 3. Flow Module (`flow.rs`)

**Pattern from Kotlin:**
```kotlin
val flow = flowOf(1, 2, 3, 4, 5)
    .filter { it % 2 == 0 }
    .map { it * 10 }
    .take(3)
flow.collect { println(it) }  // Prints 20, 40, 60
```

**Rust Implementation:**
```rust
let flow = Flow::from_iter(vec![1, 2, 3, 4, 5])
    .filter(|x| x % 2 == 0)
    .map(|x| x * 10)
    .take(3);
let values = flow.to_vec();  // [20, 40, 60]
```

**Operators:**
- `map` - Transform elements
- `filter` - Keep matching elements
- `take` - Limit to first N elements
- `reduce` - Accumulate values

### 4. Scope Module (`scope.rs`)

**Pattern from Kotlin:**
```kotlin
// coroutineScope - waits for all children
coroutineScope {
    launch { doWork() }
    launch { doMoreWork() }
}  // Waits for all children

// supervisorScope - children run independently
supervisorScope {
    launch { 
        try { doRiskyWork() } 
        catch (e: Exception) { handle(e) }
    }
}  // Failures don't propagate
```

**Rust Implementation:**
```rust
// Coroutine scope
let result = coroutine_scope(async {
    spawn(&scope, async { do_work().await });
    Ok(42)
}).await?;

// Supervisor scope
let scope = SupervisorScope::new();
let job = scope.launch(async {
    do_risky_work().await?;
    Ok(())
});

job.cancel();  // Cancel if needed
```

**Job Interface:**
```rust
pub trait Job: Send + Sync {
    fn cancel(&self);
    fn is_active(&self) -> bool;
    fn is_completed(&self) -> bool;
    fn is_cancelled(&self) -> bool;
}
```

## Integration with QUIC

The structured concurrency module integrates with QUIC for the Kafka replacement architecture:

```rust
use literbike::concurrency::*;
use literbike::quic::*;

// Create context with QUIC + services
let ctx = EmptyContext
    + Arc::new(QuicEngine::new())
    + Arc::new(ProtocolDetector::new())
    + Arc::new(DHTService::new("node-1"));

// Create channel for alpha signals
let scope = SupervisorScope::new();
let (tx, mut rx) = channel_with_scope(&scope, 1024);

// Spawn QUIC stream processor
scope.spawn(async move {
    while let Some(signal) = rx.recv().await {
        // Process alpha signal
    }
    Ok(())
});
```

## Test Results

```
running 15 tests
test concurrency::ccek::tests::test_context_merge ... ok
test concurrency::ccek::tests::test_context_keys ... ok
test concurrency::ccek::tests::test_context_with_element ... ok
test concurrency::ccek::tests::test_context_get_typed ... ok
test concurrency::ccek::tests::test_empty_context ... ok
test concurrency::ccek::tests::test_protocol_detection ... ok
test concurrency::ccek::tests::test_context_composition ... ok
test concurrency::scope::tests::test_supervisor_scope ... ok
test concurrency::tests::test_ccek_context_composition ... ok
test concurrency::scope::tests::test_coroutine_scope ... ok
test concurrency::tests::test_channel_send_recv ... ok
test concurrency::channel::tests::test_channel_multiple_sends ... ok
test concurrency::scope::tests::test_job_cancel ... ok
test concurrency::tests::test_coroutine_scope ... ok
test concurrency::channel::tests::test_channel_send_recv ... ok

test result: ok. 15 passed; 0 failed
```

## Usage Examples

### Example 1: Multi-Bot Alpha Processing

```rust
use literbike::concurrency::*;

async fn process_alpha_signals() -> Result<()> {
    let scope = SupervisorScope::new();
    
    // Create channels for each bot
    let (bot1_tx, mut bot1_rx) = channel_with_scope(&scope, 100);
    let (bot2_tx, mut bot2_rx) = channel_with_scope(&scope, 100);
    
    // Spawn processors
    scope.spawn(async move {
        while let Some(signal) = bot1_rx.recv().await {
            process_signal(signal).await?;
        }
        Ok(())
    });
    
    scope.spawn(async move {
        while let Some(signal) = bot2_rx.recv().await {
            process_signal(signal).await?;
        }
        Ok(())
    });
    
    // Send signals
    bot1_tx.send(AlphaSignal::new("bot1", 0.05)).await?;
    bot2_tx.send(AlphaSignal::new("bot2", 0.03)).await?;
    
    Ok(())
}
```

### Example 2: Flow-Based Data Pipeline

```rust
use literbike::concurrency::*;

async fn build_data_pipeline() -> Result<()> {
    let flow = Flow::from_iter(vec![
        MarketTick::new("BTC", 45000.0),
        MarketTick::new("ETH", 3200.0),
        MarketTick::new("BTC", 45100.0),
    ])
    .filter(|tick| tick.symbol == "BTC")
    .map(|tick| MarketData {
        price: tick.price * 1.001,  // Adjust for fees
        ..tick
    })
    .take(100);
    
    let btc_ticks = flow.to_vec();
    println!("Processed {} BTC ticks", btc_ticks.len());
    
    Ok(())
}
```

### Example 3: Context-Aware Service

```rust
use literbike::concurrency::*;

struct AlphaProcessor {
    context: CoroutineContext,
}

impl AlphaProcessor {
    fn new(context: CoroutineContext) -> Self {
        Self { context }
    }
    
    async fn process(&self, signal: AlphaSignal) -> Result<()> {
        // Get protocol detector from context
        if let Some(detector) = self.context.get_typed::<ProtocolDetector>("ProtocolDetector") {
            let protocol = detector.detect_protocol(&signal.data);
            println!("Detected protocol: {:?}", protocol);
        }
        
        // Get DHT service from context
        if let Some(dht) = self.context.get_typed::<DHTService>("DHTService") {
            dht.announce(signal.id).await?;
        }
        
        Ok(())
    }
}
```

## Comparison: Kotlin vs Rust

| Feature | Kotlin Coroutines | Literbike Concurrency |
|---------|------------------|----------------------|
| Context Composition | `EmptyCoroutineContext + service` | `EmptyContext + Arc::new(service)` |
| Coroutine Scope | `coroutineScope { }` | `coroutine_scope(async { })` |
| Supervisor Scope | `supervisorScope { }` | `SupervisorScope::new()` |
| Launch | `launch { }` | `scope.launch(async { })` |
| Channel | `Channel<T>(capacity)` | `channel::<T>(capacity)` |
| Flow | `flowOf(1, 2, 3)` | `Flow::from_iter(vec![1, 2, 3])` |
| Map | `flow.map { it * 2 }` | `flow.map(|x| x * 2)` |
| Filter | `flow.filter { it > 0 }` | `flow.filter(|x| *x > 0)` |
| Job Cancel | `job.cancel()` | `job.cancel()` |

## Next Steps

1. **Integration Tests** - Test with real QUIC streams
2. **Performance Benchmarks** - Compare with tokio channels
3. **Additional Operators** - Add more Flow operators (zip, combine, etc.)
4. **Context Propagation** - Add trace/correlation ID propagation
5. **Error Handling** - Add supervisor strategies (restart, resume, stop)

## References

- Betanet Kotlin Sources: `/Users/jim/work/betanet/betanet-enhanced-*/src/commonMain/kotlin/`
- Kotlin Coroutines Guide: https://kotlinlang.org/docs/coroutines-guide.html
- Kotlin Flow Guide: https://kotlinlang.org/docs/flow.html
