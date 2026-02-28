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

## [~] Track: Port Kotlin QUIC (Full Packet Processing from Trikeshed) - Agent Harness Critical Path

**URGENT:** This track is blocking Freqtrade alpha release and agent harness robustness. Completion required for QUIC transport stability in trading applications.

Port packet-processing semantics from Trikeshed QUIC modules into `literbike`
without regressing the new wire codec foundation, focusing on engine state,
ACK/CRYPTO/STREAM handling, connection lifecycle, and server integration.

### Status (Updated for Agent Harness Priority)
- ✅ Map Trikeshed QUIC sources to existing `literbike/src/quic/*`
- ✅ Port engine/connection/stream semantics (in progress)
- ✅ Expand packet processing coverage in `tests/quic`
- ✅ Preserve compatibility with QUIC interop foundation hooks
- ✅ **PRIORITY 1:** Complete connection state transitions and bytes-in-flight accounting
- ✅ **PRIORITY 1:** Add flow control and congestion control hooks
- 🔄 **PRIORITY 2:** Implement stream lifecycle and multiplexing for agent communication
- 🔄 **PRIORITY 3:** Complete C ABI exports for Freqtrade integration
- 🔄 **PRIORITY 4:** Build comprehensive agent harness integration tests

**Progress Notes:**
- ✅ decoder-to-engine packet-number-length metadata threading landed (`quic_protocol` -> `quic_engine` -> `quic_server`/`literbike-quic-capi`)
- ✅ ACK processing now prunes acknowledged sent packets and uses exact wire-length accounting (no fixed 1350-byte estimate)
- ✅ async send path is transactional (encode-before-commit) with best-effort rollback on UDP send failure to avoid state/accounting drift
- ✅ **PRIORITY 1 COMPLETE:** Connection state transitions and bytes-in-flight accounting with exact wire length implemented
- ✅ **PRIORITY 1 COMPLETE:** Flow control and congestion control hooks added to QUIC engine
- 🔄 **PRIORITY 2:** Stream lifecycle and multiplexing for agent harness
- 🔄 **BLOCKING:** Connection lifecycle management and stream multiplexing for agent harness

**Impact:** 
- ✅ **Agent Harness:** Requires QUIC transport stability
- ✅ **Freqtrade Alpha:** QUIC transport is critical path
- ✅ **Trading Performance:** Sub-millisecond latency requirements

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
