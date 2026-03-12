# Tech Stack

## Languages

- Rust 2021 (`/Users/jim/work/literbike/Cargo.toml`)

## Core

- `literbike` workspace members as the heavy heart/backplane layer
- `litebike` as the primary shell/operator entrypoint that gates `literbike`
  into the deployable surface
- Tokio async runtime and UDP sockets
- `parking_lot` mutexes for low-overhead shared state
- `serde` / `serde_json` for config and interchange

## QUIC-related current modules

- `src/quic/quic_protocol.rs` (wire codec foundation)
- `src/quic/quic_engine.rs` (connection/stream processing logic)
- `src/quic/quic_server.rs` (server path and packet decode integration)
- `src/quic/quic_error.rs` (error taxonomy)

## FFI surfaces

- Existing PyO3 crate: `literbike-ffi` (Python extension module)
- Planned separate C ABI crate: `literbike-quic-capi` (ctypes-friendly `cdylib`)

## Integration seam (external)

- `/Users/jim/work/external-bot/user_data/ops/literbike_quic_transport.py`

## Build/test commands (common)

- `cargo test --features quic --lib`
- `cargo build --features quic`
- `cargo test --features quic --lib quic::quic_protocol::tests`

## Notes

- Canonical product split: `litebike` owns the shell/operator surface;
  `literbike` owns the heavier transport/model/service heart.
- Canonical composed ingress: `litebike` `agent8888` on port `8888`; when
  `literbike` is present, that single surface subsumes both repos.
- Direct `literbike` launches remain valid for backplane validation or focused
  service work, but they do not supersede `litebike` as the front door.
- Current module and binary naming can still look mixed; repo-local
  `/conductor/` truth should prefer the shell/heart split until the code graph
  is fully reconciled.
