# CCEK Pattern Skill

## Overview

CCEK (CoroutineContext Element Key) is a Rust pattern for composing libraries under a security boundary, based on Kotlin's CoroutineContext.

## Architecture

```
┌─────────────────────────────────────┐
│         CcekContext                 │
│     (Security Boundary)             │
│                                     │
│  Element + Key + Trait              │
│                                     │
└─────────────────────────────────────┘
```

## Three Components

### 1. Keys (static const, weight 100)

Keys are compile-time singleton identifiers. Each Key is a `static const` that provides its Element.

```rust
pub struct HtxKey;

impl HtxKey {
    pub const ELEMENT: HtxElement = HtxElement::new();
}

impl CcekKey for HtxKey {
    type Element = HtxElement;
}
```

### 2. Elements (CoroutineContext.Element, weight 100)

Elements are the actual library implementations. They hold state and implement traits.

```rust
pub struct HtxElement {
    pub connections: u32,
}

impl HtxElement {
    pub const fn new() -> Self {
        Self { connections: 0 }
    }
}

impl CcekElement for HtxElement {
    fn key(&self) -> &'static str { "HtxElement" }
}
```

### 3. Traits (declarations on Elements)

Traits define interfaces that Elements implement.

```rust
pub trait HtxVerifier {
    fn verify(&self, input: &[u8]) -> bool;
}

impl HtxVerifier for HtxElement {
    fn verify(&self, _input: &[u8]) -> bool {
        true
    }
}
```

## Context Pattern

Context composes Elements via the `+` operator (like Kotlin):

```rust
let ctx = EmptyContext
    + HtxKey::ELEMENT
    + QuicKey::ELEMENT
    + NioKey::element(1024);
```

## Usage

```rust
use literbike::ccek_sdk::{CcekContext, CcekElement, CcekKey, EmptyContext};
use literbike::ccek_sdk::elements::{HtxElement, HtxKey, QuicElement, QuicKey};
use literbike::ccek_sdk::traits::HtxVerifier;

// Create context
let ctx = EmptyContext + HtxKey::ELEMENT;

// Get element
if let Some(htx) = ctx.get::<HtxKey>() {
    htx.verify(data);
}
```

## Key Catalog

| Key | Element | Trait |
|-----|---------|-------|
| `HtxKey` | `HtxElement` | `HtxVerifier` |
| `QuicKey` | `QuicElement` | `QuicEngine` |
| `NioKey` | `NioElement` | `NioReactor` |
| `HttpKey` | `HttpElement` | `HttpHandler` |
| `SctpKey` | `SctpElement` | `SctpHandler` |

## Files

- `src/ccek_sdk/context.rs` - Context, CcekKey, CcekElement traits
- `src/ccek_sdk/elements.rs` - Element implementations + companion Keys
- `src/ccek_sdk/keys.rs` - Key exports
- `src/ccek_sdk/traits.rs` - Trait definitions and implementations
- `src/ccek_sdk/channels.rs` - Channel types for tributary flow
