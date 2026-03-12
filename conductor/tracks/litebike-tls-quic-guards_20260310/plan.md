# Plan: Gate litebike QUIC TLS paths behind tls-quic

## Scope

Focused `src/bin/litebike.rs` repair for the next `cargo build --bin litebike --features warp,git2`
blockers after the stub-command track was completed.

## Phase 1: Repair compile blockers

- [x] `run_proxy_server` — gate `literbike::quic::tls` / `tls_ccek` setup behind `tls-quic` or provide a truthful no-feature fallback
- [x] `run_proxy_server` — build the CCEK context using the already-working `src/bin/quic_tls_server.rs` pattern
- [x] `run_quic_vqa` — gate `literbike::quic::tls` / `tls_ccek` setup behind `tls-quic` or provide a truthful no-feature fallback
- [x] `run_quic_vqa` — build the CCEK context using the already-working `src/bin/quic_tls_server.rs` pattern

## Phase 2: Verify

- [x] `cargo build --bin litebike --features warp,git2`
- [x] Evaluate the next litebike-only blockers, if any, after the TLS/CCEK slice is repaired
- [ ] Reconfirm `cargo test --lib` if the edit alters shared compile paths

## Progress Notes

- 2026-03-10: Follow-on build after the stub-command track now fails only in two
  `src/bin/litebike.rs` regions: `run_proxy_server` and `run_quic_vqa`.
  Errors are:
  - missing `literbike::quic::tls` without `tls-quic`
  - missing `literbike::quic::tls_ccek` without `tls-quic`
  - `CoroutineContext` mismatches on the local `EmptyContext + tls_ccek` expressions
- 2026-03-10: `src/bin/quic_tls_server.rs` already demonstrates the intended
  working pattern: construct `tls_ccek`, then bind `let ctx = EmptyContext + tls_ccek.clone() as Arc<dyn ContextElement>;`
  and run the TLS-specific server path only when the feature is present.
- 2026-03-10: `claude` completed the bounded repair with a valid rendezvous
  payload. Master verification confirms the final `src/bin/litebike.rs` diff is
  limited to `run_proxy_server` and `run_quic_vqa`, and
  `cargo build --bin litebike --features warp,git2` now passes. The build still
  emits 7 pre-existing warnings in `litebike.rs`, but there are no remaining
  litebike compile blockers from this slice.
