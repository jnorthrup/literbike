# Track: Userspace Channel Fixes for Bun JSON Confluence

## Objective

Fix pre-existing compilation errors in the `userspace` crate that are blocking Phase 2 of the Bun JSON Rust Confluence track.

## Status: [~] In Progress

## Context

The Bun JSON Rust Confluence Phase 1 is complete, but Phase 2 (FFI integration) is blocked by compilation errors in the `userspace` crate. These errors prevent `cargo check` from passing, which is required to proceed with FFI bindings.

## Scope

Fix the following compilation errors in `/Users/jim/work/userspace/src/concurrency/channels/channel.rs`:

1. **Missing Clone implementation** - `RendezvousChannel` needs to implement `Clone` for the `channel()` constructor
2. **Missing trait methods** - `UnboundedChannel` doesn't implement required `try_send` and `try_recv` methods from the `Channel` trait
3. **Type mismatches** - `SendFuture` and `RecvFuture` expect `Arc<dyn Channel<_>>` but receive `&dyn Channel<T>`

## Tasks

- [ ] 1.1 Add `#[derive(Clone)]` or manual `Clone` impl for `RendezvousChannel<T>`
- [ ] 1.2 Implement `try_send()` and `try_recv()` in `Channel<T>` trait impl for `UnboundedChannel<T>`
- [ ] 1.3 Fix type mismatches in `SendFuture` and `RecvFuture` (change `&*self.0` to `self.0.clone()`)
- [ ] 1.4 Verify compilation passes with `cargo check --lib --features json`

## Verification

```bash
# Build should pass without errors
cargo check --lib --features json

# All channel tests should pass
cargo test --lib channels
```

## Deliverables

- Fixed `userspace/src/concurrency/channels/channel.rs`
- Unblocked Bun JSON Confluence Phase 2
- All tests passing

## Dependencies

- None (all fixes are local to channel.rs)

## Risks

- Breaking existing channel behavior if trait implementations change
- Performance regression from Arc cloning in hot path

## Mitigation

- The existing methods already have the correct logic, just need to be moved to the trait impl
- Arc cloning is cheap (atomic increment) and necessary for sharing across futures
- All existing tests must pass after changes
