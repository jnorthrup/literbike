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

## Four Concepts (NOT Three)

### 1. Key = singleton, may transform state

Stateless. Globally accessible. Has functions that may transform state.

```rust
pub struct HtxKey;

impl HtxKey {
    pub fn verify(elt: &mut HtxElement, packet: NetPacket) -> bool { ... }
    pub fn connections(elt: &HtxElement) -> u32 { ... }
}
```

### 2. Element = state holder, enables stateful methods

Holds state (including Delta). Element has Delta, not IS Delta.

```rust
pub struct HtxElement {
    pub delta: Delta<NetPacket>,  // delta is PART of element
    connections: u32,            // local state
}
```

### 3. Delta = structure within Element

Inlets/tributaries/outflows are the **structure** of state flow.

```rust
delta: Delta::new()
    .add_inlet("ticket", 64)
    .add_tributary("verified", 128)
    .add_outflow("response", 64)
```

### 4. Context = sum of Keys + Elements + local state

```rust
pub struct CcekContext {
    elements: HashMap<&'static str, Box<dyn CcekElement>>,
    // local state lives here
}
```

## Separation of Concerns

| Concept | Role | Stateful? |
|---------|------|-----------|
| Key | Singleton lookup, may transform | No |
| Element | State holder | Yes |
| Delta | Structure within Element | Yes (structure) |
| Context | Sum of Keys + Elements + state | Yes |

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
