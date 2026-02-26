# Plan: Port Kotlin Reactor (Trikeshed Event-Driven I/O)

## Phase 1: Mapping and Interface Design

- [x] Read Trikeshed reactor source files and map responsibilities
- [x] Inspect existing `literbike` reactor modules (`simple_reactor`, `timer`, `handler`, `context`)
- [x] Define Rust module layout (`reactor`, `selector`, `channel`, `operation`, `platform`)
- [x] Define trait shapes for selectable/readable/writable channels

## Phase 2: Core Reactor Port

- [x] Implement `operation.rs` (interest/operation semantics)
- [x] Implement `channel.rs` traits and basic registration metadata
- [x] Implement `selector.rs` baseline readiness backend
- [x] Implement `reactor.rs` event loop (register/poll/dispatch/shutdown)
- [x] Integrate timer scheduling into poll timeout calculation

## Phase 3: Export and Compatibility Integration

- [x] Update `/Users/jim/work/literbike/src/reactor/mod.rs` exports
- [x] Retain `SimpleReactor` compatibility path
- [x] Wire handler/context modules into the new reactor path where applicable
- [x] Add docs/comments for module roles and boundaries

## Phase 4: Verification

- [x] Add unit tests for registration/readiness dispatch
- [x] Add timer integration tests
- [x] Add shutdown/cleanup tests
- [x] Run focused tests for reactor modules

## Validation Notes

- Implemented a portable baseline reactor using a deterministic `ManualSelector`
  backend (`src/reactor/selector.rs`) to provide real registration/readiness
  semantics without OS-specific polling in this track.
- `src/reactor/reactor.rs` integrates `TimerWheel` into poll timeout
  calculation and supports registration, dispatch, and shutdown cleanup.
- Focused validation run:
  `cargo test -p literbike --lib reactor::`
  (reactor module tests passed, including readiness dispatch/timer/shutdown).
