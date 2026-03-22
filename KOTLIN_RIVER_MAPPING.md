# Kotlin Coroutine → Literbike River Mapping

## Core Insight

Kotlin coroutines are **sequential by default**. The `suspend` keyword just means "can yield". No event loops, no dispatch tables - just sequential async code that composes naturally.

```kotlin
// Kotlin: sequential suspend pipeline (no dispatch tables)
suspend fun pipeline(ctx: CoroutineContext) {
    val data = source()          // suspend - yields here
    val processed = transform(ctx, data)  // suspend
    sink(processed)              // suspend
}
```

```rust
// Rust: same sequential pattern (no tokio, no dispatch)
async fn pipeline(ctx: impl CcekScope) {
    let data = source().await;           // sequential await
    let processed = ctx.transform(data).await;  // sequential await
    sink(processed).await;               // sequential await
}
```

## Kotlin Abstractions → Literbike River

| Kotlin | Literbike | Pattern |
|--------|-----------|---------|
| `CoroutineContext` | `CcekContext` | Compile-time optimized map |
| `CoroutineContext.Element` | `CcekElement` | Protocol-specific element |
| `CoroutineContext.Key` | `CcekKey` | Const singleton factory |
| `Channel` | `Channel<T>` | River connecting tributaries |
| `Flow` | `RiverFlow<T>` | Cold async stream (tributary) |
| `Job` | `ProtocolJob` | Cancellable task handle |
| `CoroutineScope` | `CcekScope` | Implicit context provider |

## Sequential Composition (NOT dispatch tables)

### Kotlin
```kotlin
// NO match/if dispatch tables - just sequential suspend
suspend fun processHTX(ctx: CoroutineContext, packet: Packet): Result<Packet> =
    ctx[HTX].verify(packet)

suspend fun processAll(ctx: CoroutineContext, packets: List<Packet>) {
    packets.forEach { packet ->
        val result = processHTX(ctx, packet)
        ctx[Channel].send(result)
    }
}
```

### Rust (Literbike)
```rust
// NO match/if dispatch tables - just sequential async
async fn process_htx(ctx: &CcekScope, packet: Packet) -> Result<Packet> {
    ctx.get::<HtxElement>().verify(packet).await
}

async fn process_all(ctx: &CcekScope, packets: Vec<Packet>) {
    for packet in packets {
        let result = process_htx(ctx, packet).await;
        ctx.channel().send(result).await;
    }
}
```

## River Delta Architecture

Each protocol is a **river delta** with multiple inlets, tributaries, and outflows:

```
                    DELTA
    ┌──────────────────────────────────────┐
    │           HTTP PROTOCOL               │
    │                                      │
    │  INLETS         TRIBUTARIES    OUTFLOWS
    │  ┌─────┐       ┌─────┐       ┌─────┐  │
    │  │req_h│──┬────│chunk│──┬────│res_h│  │
    │  │req_b│  │    │body │  │    │res_b│  │
    │  └─────┘  │    └─────┘  │    └─────┘  │
    │            │             │             │
    │       ┌────▼────┐  ┌─────▼─────┐      │
    │       │ header  │  │  body     │       │
    │       │ stream  │  │  stream   │       │
    │       └─────────┘  └───────────┘       │
    └──────────────────────────────────────┘
```

### Flow Through Delta

```
INLETS ──► TRIBUTARIES ──► OUTFLOWS

request_head ──► header_parse ──► response_head
     │                                 ▲
     ▼                                 │
request_body ──► body_chunk ──────────┘
     │                                 ▲
     ▼                                 │
request_trailer ──► trailer_parse ────┘
```

## CCEK SDK Structure

```rust
// src/ccek_sdk/
mod context;    // CcekContext, CcekKey, CcekElement, EmptyContext
mod elements;    // HtxElement, QuicElement, NioElement, etc.
mod keys;       // Key exports
mod channels;   // Channel<T>, ChannelTx<T>, ChannelRx<T>
mod scope;      // CcekScope, CcekScopeHandle, CcekScopeRef
mod traits;     // Traits on Elements

pub use context::*;
pub use elements::*;
pub use channels::*;
```

## Usage

```rust
use crate::ccek_sdk::{CcekContext, CcekScope, Channel};

// Sequential pipeline - no dispatch tables
async fn process_packets(ctx: &CcekScope, packets: Vec<Packet>) -> Vec<Result> {
    let mut results = Vec::new();
    for packet in packets {
        let result = process_one(ctx, packet).await;
        results.push(result);
    }
    results
}

async fn process_one(ctx: &CcekScope, packet: Packet) -> Result {
    ctx.with_element(HtxElement::new(), |ctx| async {
        let verified = ctx.verify(packet).await?;
        ctx.send(verified).await
    }).await
}
```

## What NOT To Do

1. **NO dispatch tables**: `match`, `if let`, `select!` as primary composition
2. **NO thread::spawn everywhere**: Flat Rust, not structured concurrency
3. **NO Box::pin chains**: Oppressive type ceremony
4. **YES sequential await**: Natural async composition
5. **YES implicit context via Scope**: Like Kotlin's CoroutineScope

## Next Steps

1. [x] Create minimal CCEK SDK with scope-based context
2. [x] Remove complex combinators (jobs.rs, river.rs, etc.)
3. [ ] Wire HTX, QUIC elements through channels
4. [ ] Test sequential async composition
5. [ ] Verify compilation
