# Tech Stack

## Core

- Rust 2021 (`/Users/jim/work/literbike/Cargo.toml`)
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

- `/Users/jim/work/freqtrade/user_data/ops/literbike_quic_transport.py`

## Build/test commands (common)

- `cargo test --features quic --lib`
- `cargo build --features quic`
- `cargo test --features quic --lib quic::quic_protocol::tests`

