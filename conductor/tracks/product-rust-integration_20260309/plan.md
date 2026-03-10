# Plan: Product Rust Integration — Untracked Code Validation & Commit

## Phase 1: Fix Integration Test

- [x] Fix `tests/integration_quic_dht_cas.rs:372` — changed to
  `QuicError::Connection(ConnectionError::ConnectionClosed)`; added import
- [x] `cargo test --test integration_quic_dht_cas` — 10/10 passed

## Phase 2: Commit Product Code

- [x] Committed untracked product files (commit 0502269):
  - `src/cas_backends.rs`
  - `src/bin/carrier_bypass.rs`
  - `tests/integration_quic_dht_cas.rs`
  - `tools/check_rfc_trace.sh`
  - `tools/build_macos_control_plane_app.sh`
  - `macos/LiterbikeControlPlane/`
  - All modified `src/` files

## Progress Notes

- 2026-03-09: One compile error in integration tests (QuicError::ConnectionClosed → needs
  QuicError::Connection(ConnectionError::ConnectionClosed)). All other Rust code compiles.
  Delegation: Worker A=kilo (fix+test), Worker B=opencode (commit).
