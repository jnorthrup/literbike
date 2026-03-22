# CCEK Pattern Skill

## Overview

CCEK (CoroutineContext Element Key) is a Rust pattern for composing async libraries under a security boundary, based on Kotlin's CoroutineContext.

**Key insight**: Elements ARE Coroutines. Context hosts Coroutine[Contexts]. This guides the compiler through explicit performant locality.

## Architecture

```
┌─────────────────────────────────────┐
│         CcekContext                 │
│     (Host of Coroutine[Contexts])    │
│                                     │
│  Element = Coroutine (async fn)      │
│  Key = static const factory         │
│                                     │
└─────────────────────────────────────┘
```

## Three Components

### 1. Keys (static const factory, weight 100)

Keys are compile-time const factories that create Coroutine Elements.

```rust
pub struct HtxKey;

impl HtxKey {
    pub const fn create() -> HtxElement {
        HtxElement::new()
    }
}

impl CcekKey for HtxKey {
    type Element = HtxElement;
}
```

### 2. Elements (Coroutine, weight 100 for access)

Elements ARE Coroutines (async fn returning Self). They implement `Future`.

```rust
pub struct HtxElement;

impl HtxElement {
    pub const fn new() -> Self {
        Self
    }
}

impl Future for HtxElement {
    type Output = Self;
    fn poll(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        Poll::Ready(self.get_mut().clone())
    }
}

impl CcekElement for HtxElement {
    fn key(&self) -> &'static str { "HtxElement" }
}
```

### 3. Context (host of Coroutine[Contexts])

Context hosts Coroutine[Contexts] for explicit compiler locality.

```rust
let ctx = EmptyContext
    + HtxKey::create()           // Coroutine[Context]
    + QuicKey::create()           // Coroutine[Context]
    + NioKey::create(1024);      // Coroutine[Context]
```

## Why This Matters

When Elements are Coroutines and Context hosts Coroutine[Contexts]:

1. **Compiler locality** - async/await chains are explicit
2. **No heap allocation** - static dispatch where possible
3. **Stack coroutines** - cooperative multitasking without boxing
4. **Security boundary** - Context isolates library composition

## Key Catalog

| Key | Element (Coroutine) | Factory |
|-----|-------------------|--------|
| `HtxKey` | `HtxElement` | `HtxKey::create()` |
| `QuicKey` | `QuicElement` | `QuicKey::create()` |
| `NioKey` | `NioElement` | `NioKey::create(max_fds)` |
| `HttpKey` | `HttpElement` | `HttpKey::create()` |
| `SctpKey` | `SctpElement` | `SctpKey::create()` |

## Files

- `src/ccek_sdk/context.rs` - Context, CcekKey, CcekElement, EmptyContext
- `src/ccek_sdk/elements.rs` - Element = Coroutine implementations
- `src/ccek_sdk/keys.rs` - Key = static const factories
- `src/ccek_sdk/traits.rs` - Trait definitions
- `src/ccek_sdk/channels.rs` - Channel types

## Pattern in Kotlin

```kotlin
// Kotlin CoroutineContext.Element pattern
return EmptyCoroutineContext +
    dhtService +
    protocolDetector +
    crdtStorage +
    crdtNetwork +
    conflictResolver
```

## Pattern in Rust (CCEK)

```rust
// Rust CCEK - Elements ARE Coroutines
let ctx = EmptyContext
    + HtxKey::create()
    + QuicKey::create()
    + NioKey::create(1024);
```
