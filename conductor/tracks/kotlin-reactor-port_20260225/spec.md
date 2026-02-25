# Spec: Port Kotlin Reactor (Trikeshed Event-Driven I/O)

## Overview

`literbike` currently exports only a stub/simple reactor via
`/Users/jim/work/literbike/src/reactor/mod.rs`, which is insufficient for the
planned QUIC and transport integrations. This track ports the core Trikeshed
reactor abstractions into idiomatic Rust modules while preserving brownfield
compatibility.

## Problem

- `reactor/mod.rs` only exports `simple_reactor`.
- There is no portable selector/readiness abstraction for transport modules.
- QUIC and future DHT/IPFS integration need a real event loop boundary, not
  ad hoc per-module loops.

## Source Material (Kotlin / Trikeshed)

- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/reactor/Reactor.kt`
- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/reactor/SelectableChannel.kt`
- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/reactor/IOOperation.kt`
- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/reactor/PlatformIO.kt`

## Target Modules (Expected)

- `/Users/jim/work/literbike/src/reactor/mod.rs` (exports)
- `/Users/jim/work/literbike/src/reactor/reactor.rs`
- `/Users/jim/work/literbike/src/reactor/selector.rs`
- `/Users/jim/work/literbike/src/reactor/channel.rs`
- `/Users/jim/work/literbike/src/reactor/operation.rs`
- `/Users/jim/work/literbike/src/reactor/platform.rs`
- Existing modules retained/integrated:
  - `/Users/jim/work/literbike/src/reactor/simple_reactor.rs`
  - `/Users/jim/work/literbike/src/reactor/timer.rs`
  - `/Users/jim/work/literbike/src/reactor/handler.rs`
  - `/Users/jim/work/literbike/src/reactor/context.rs`

## Functional Requirements

- Port core reactor semantics (registration, readiness dispatch, shutdown).
- Model selectable/readable/writable channel behavior as Rust traits.
- Port I/O operation semantics (read/write/connect/accept interest flags).
- Add a portable selector abstraction with a baseline implementation.
- Integrate timer scheduling with poll/select timeout calculation.
- Keep `SimpleReactor` available for compatibility (test/demo path).

## Non-Functional Requirements

- Brownfield-safe and additive: do not break current imports abruptly.
- Tokio compatibility: reactor abstractions can coexist with Tokio-driven code.
- Avoid Linux-only optimization scope in this track (`io_uring` out of scope).

## Acceptance Criteria

1. `/Users/jim/work/literbike/src/reactor/mod.rs` exports real reactor modules
   beyond `simple_reactor`.
2. A runnable reactor loop abstraction exists with channel registration and
   readiness dispatch.
3. Timer integration is wired into the reactor wait path.
4. Tests cover readiness dispatch, timer firing, and shutdown cleanup.

## Out of Scope

- QUIC-specific packet processing behavior (tracked separately)
- `io_uring`-specific reactor backend
- Full cross-platform backend optimization (epoll/kqueue tuning)

