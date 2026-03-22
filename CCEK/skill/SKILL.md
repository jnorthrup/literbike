# CCEK Pattern Skill

## Overview

CCEK (CoroutineContext Element Key) mirrors Kotlin's CoroutineContext pattern with river delta semantics.

**Key insight**: Context is a compile-time optimized map with const keys. Each protocol is a **river delta** with multiple inlets, tributaries, and outflows.

## Kotlin CoroutineContext

```kotlin
// Kotlin CoroutineContext.Element pattern
return EmptyCoroutineContext +
    dhtService +
    protocolDetector +
    crdtStorage

// Where each service is a CoroutineContext.Element
// With a companion object Key for compile-time resolution
```

## Three Components

### 1. Context = Compile-time optimized map

Like Kotlin's `CoroutineContext` - specialized map with const key resolution.

```rust
#[derive(Clone, Default)]
pub struct CcekContext;
```

### 2. Keys = const compile-time singletons

Like Kotlin's `Element.Key` companion objects. **Stateless. Globally accessible. Nothing more.**

```rust
pub struct HtxKey;  // singleton - no state, just a type
```

A Key can have functions, but a Key is NOT required to be a factory.

### 3. Elements = river deltas (NOT flat structures)

Each protocol Element is a **river delta** with:
- **INLETS**: incoming data sources
- **TRIBUTARIES**: branching sub-streams
- **OUTFLOWS**: outgoing data sinks

```rust
pub struct HtxElement {
    pub delta: Delta<NetPacket>,
}

impl HtxElement {
    pub fn new() -> Self {
        Self {
            delta: Delta::new()
                .add_inlet("ticket", 64)           // incoming tickets
                .add_inlet("challenge", 64)         // challenge requests
                .add_tributary("verified", 128)     // verified tickets branch
                .add_tributary("rejected", 64)     // rejected tickets branch
                .add_outflow("response", 64),      // verification responses
        }
    }
}
```

Key and Element are **separate**. Key is the singleton lookup key. Element is the stateful river delta.

## Delta Architecture

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

## Protocol Delta Reference

### HTX (Constant-time Ticket Verification)
| Type | Name | Purpose |
|------|------|---------|
| Inlet | `ticket` | Incoming tickets |
| Inlet | `challenge` | Challenge requests |
| Tributary | `verified` | Verified tickets branch |
| Tributary | `rejected` | Rejected tickets branch |
| Outflow | `response` | Verification responses |

### QUIC (Stream-oriented)
| Type | Name | Purpose |
|------|------|---------|
| Inlet | `packet` | Incoming packets |
| Inlet | `stream_init` | Stream initialization |
| Tributary | `stream_0` | Stream 0 (control) |
| Tributary | `stream_data` | Data streams |
| Tributary | `stream_close` | Stream close |
| Outflow | `packet_out` | Outgoing packets |
| Outflow | `stream_out` | Stream data out |

### HTTP (Request/Response)
| Type | Name | Purpose |
|------|------|---------|
| Inlet | `request_head` | Request headers |
| Inlet | `request_body` | Request body |
| Inlet | `request_trailer` | Request trailers |
| Tributary | `header_parse` | Parsed headers |
| Tributary | `body_chunk` | Body chunks |
| Tributary | `trailer_parse` | Parsed trailers |
| Outflow | `response_head` | Response headers |
| Outflow | `response_body` | Response body |
| Outflow | `response_trailer` | Response trailers |

### SCTP (Chunk-oriented)
| Type | Name | Purpose |
|------|------|---------|
| Inlet | `chunk` | Incoming chunks |
| Inlet | `heartbeat` | Heartbeat requests |
| Inlet | `init` | INIT chunks |
| Tributary | `data_chunk` | DATA chunks |
| Tributary | `sack_chunk` | SACK chunks |
| Tributary | `heartbeat_ack` | Heartbeat acks |
| Tributary | `error_chunk` | ERROR chunks |
| Outflow | `chunk_out` | Outgoing chunks |
| Outflow | `notify` | Notifications |

### NIO (Non-blocking I/O)
| Type | Name | Purpose |
|------|------|---------|
| Inlet | `read` | Read requests |
| Inlet | `write` | Write requests |
| Inlet | `accept` | Accept requests |
| Inlet | `connect` | Connect requests |
| Tributary | `read_ready` | Read ready fds |
| Tributary | `write_ready` | Write ready fds |
| Tributary | `error` | Error events |
| Outflow | `read_complete` | Read completions |
| Outflow | `write_complete` | Write completions |
| Outflow | `accept_complete` | Accept completions |
| Outflow | `connect_complete` | Connect completions |

## Why Compile-time Keys Matter

1. **No runtime dispatch** - keys resolved at compile time
2. **No hashing** - direct TypeId lookup
3. **Stack allocation** - context can live on stack
4. **Security** - isolated library composition

## Usage

```rust
use crate::ccek_sdk::{CcekContext, HtxElement, HtxKey, QuicElement, QuicKey};

// Compose like Kotlin
let ctx = CcekContext::new()
    .with(HtxKey::element())
    .with(QuicKey::element());

// Access delta inlets/tributaries/outflows
let htx = ctx.get::<HtxElement>().unwrap();
htx.ticket_inlet().send(packet);
```

## Sequential Suspend (No Dispatch Tables)

Kotlin's `suspend` is sequential by default. Same in Literbike:

```rust
// Sequential suspend pipeline - NO match/if dispatch tables
async fn process_packet(ctx: &CcekScope, packet: NetPacket) -> Result {
    let verified = ctx.with_element(HtxElement::new(), |ctx| async {
        let inlet = ctx.ticket_inlet();
        inlet.send(packet).await
    }).await?;
    // ... sequential processing
}
```

## Files

- `src/ccek_sdk/context.rs` - Context, Key, Element traits
- `src/ccek_sdk/delta.rs` - Delta, Inlet, Outflow, Tributary
- `src/ccek_sdk/elements.rs` - Element implementations with deltas
- `src/ccek_sdk/channels.rs` - Channel types for inlets/outflows
- `src/ccek_sdk/scope.rs` - CcekScope for implicit context
