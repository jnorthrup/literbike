# Plan: Port Kotlin Reactor (Trikeshed Event-Driven I/O)

## Phase 1: Mapping and Interface Design

- [ ] Read Trikeshed reactor source files and map responsibilities
- [ ] Inspect existing `literbike` reactor modules (`simple_reactor`, `timer`, `handler`, `context`)
- [ ] Define Rust module layout (`reactor`, `selector`, `channel`, `operation`, `platform`)
- [ ] Define trait shapes for selectable/readable/writable channels

## Phase 2: Core Reactor Port

- [ ] Implement `operation.rs` (interest/operation semantics)
- [ ] Implement `channel.rs` traits and basic registration metadata
- [ ] Implement `selector.rs` baseline readiness backend
- [ ] Implement `reactor.rs` event loop (register/poll/dispatch/shutdown)
- [ ] Integrate timer scheduling into poll timeout calculation

## Phase 3: Export and Compatibility Integration

- [ ] Update `/Users/jim/work/literbike/src/reactor/mod.rs` exports
- [ ] Retain `SimpleReactor` compatibility path
- [ ] Wire handler/context modules into the new reactor path where applicable
- [ ] Add docs/comments for module roles and boundaries

## Phase 4: Verification

- [ ] Add unit tests for registration/readiness dispatch
- [ ] Add timer integration tests
- [ ] Add shutdown/cleanup tests
- [ ] Run focused tests for reactor modules

