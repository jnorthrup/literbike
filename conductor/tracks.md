# Tracks Overview

---

## [x] Track: QUIC Interop Foundation (Packet Numbers, Crypto Hooks, C ABI)

Advance QUIC interoperability foundations by adding packet number reconstruction
and header-protection hooks, starting a feature-gated handshake/crypto path, and
adding a ctypes-friendly C ABI crate for the `freqtrade` integration seam.

### Status
- [x] Lock interfaces and scope for engine hooks / crypto path / C ABI
- [x] Add packet number reconstruction + header protection hooks in `quic_engine.rs`
- [x] Start feature-gated handshake/crypto integration path
- [x] Add separate `cdylib` crate with QUIC C ABI exports
- [x] Run focused validation (Rust tests + FFI smoke/error paths)

**Link:** [quic-interop-foundation_20260225](./tracks/quic-interop-foundation_20260225/)

---

## [x] Track: Port Kotlin Reactor (Trikeshed Event-Driven I/O)

Port the Trikeshed reactor foundation into `literbike` so QUIC and other
transport code can use a real event-driven I/O runtime abstraction instead of
the current stub-only reactor export.

### Status
- [x] Map Trikeshed reactor sources to `literbike/src/reactor/*`
- [x] Add reactor/channel/operation/platform modules and exports
- [x] Integrate timer and handler/context seams
- [x] Add readiness/timer/shutdown tests

**Link:** [kotlin-reactor-port_20260225](./tracks/kotlin-reactor-port_20260225/)

---

## [~] Track: Port Kotlin QUIC (Full Packet Processing from Trikeshed)

Port packet-processing semantics from Trikeshed QUIC modules into `literbike`
without regressing the new wire codec foundation, focusing on engine state,
ACK/CRYPTO/STREAM handling, connection lifecycle, and server integration.

### Status
- [ ] Map Trikeshed QUIC sources to existing `literbike/src/quic/*`
- [ ] Port engine/connection/stream semantics
- [ ] Expand packet processing coverage in `tests/quic`
- [ ] Preserve compatibility with QUIC interop foundation hooks
- In progress: decoder-to-engine packet-number-length metadata threading landed
  (`quic_protocol` -> `quic_engine` -> `quic_server`/`literbike-quic-capi`)
- In progress: ACK processing now prunes acknowledged sent packets and uses
  exact wire-length accounting (no fixed 1350-byte estimate)
- In progress: async send path is transactional (encode-before-commit) with
  best-effort rollback on UDP send failure to avoid state/accounting drift

**Link:** [kotlin-quic-packet-processing-port_20260225](./tracks/kotlin-quic-packet-processing-port_20260225/)

---

## [ ] Track: Port Kotlin IPFS (Complete DHT Client from Trikeshed)

Port the Trikeshed IPFS client/core into `literbike` as a coherent DHT-facing
client layer on top of the existing Kademlia primitives in `src/dht/`.

### Status
- [ ] Inventory Trikeshed IPFS core/client/pubsub sources
- [ ] Define `literbike` IPFS module layout and feature gating
- [ ] Implement client/core integration with existing DHT primitives
- [ ] Add DHT/IPFS tests and docs for integration boundaries

**Link:** [kotlin-ipfs-dht-port_20260225](./tracks/kotlin-ipfs-dht-port_20260225/)

---

## [ ] Track: Integration Tests (End-to-End QUIC + DHT + DuckDB)

Add end-to-end integration tests that exercise QUIC transport, DHT/IPFS data
paths, and DuckDB persistence/audit behavior together in deterministic scenarios.

### Status
- [ ] Define test topology and fixtures (QUIC + DHT + DuckDB)
- [ ] Add integration harness and reusable helpers
- [ ] Port/expand scenario coverage beyond unit-level tests
- [ ] Validate deterministic fallback and failure-path behavior

**Link:** [quic-dht-duckdb-integration-tests_20260225](./tracks/quic-dht-duckdb-integration-tests_20260225/)

---
