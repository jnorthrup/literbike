# Plan: Export http Module and Fix Reactor API Mismatch

## Phase 1: Export (DONE)

- [x] Add `pub mod http;` to `src/lib.rs` (done by kilo)

## Phase 2: Fix Reactor API Mismatch in server.rs

- [x] `src/http/server.rs` reactor API mismatch resolved — see below
- [x] Rewrote `HttpEventHandler` to implement `EventHandler` (on_readable/on_writable/on_error)
  with sessions stored internally; removed Attachment/Interest generics
- [x] Fixed `try_parse_headers` to extract body bytes already in parser buffer into
  `body_buffer` (fixes infinite loop in combined header+body reads)

## Phase 3: Verify

- [x] `cargo build --example http_server` — compiles (commit includes lib.rs + server.rs)
- [x] `cargo test --lib http` — 23/23 passed, no hangs
- [x] `cargo test --lib` — 278 passed, 0 failed (13 new http tests now run)

## Progress Notes

- 2026-03-09: lib.rs export added. Deeper issue: server.rs uses relaxfactory-style
  generic reactor API that was never implemented. Delegation: Worker A=kilo (fix server.rs)
  Worker B=opencode (commit).

