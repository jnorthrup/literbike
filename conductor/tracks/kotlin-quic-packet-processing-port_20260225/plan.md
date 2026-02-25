# Plan: Port Kotlin QUIC (Full Packet Processing from Trikeshed)

## Phase 1: Source-to-Target Mapping

- [ ] Read Trikeshed QUIC engine/connection/stream/session/config/error sources
- [ ] Inventory current `literbike` QUIC modules and note gaps vs Trikeshed behavior
- [ ] Identify overlap with existing QUIC interop foundation track (hooks/crypto seams)
- [ ] Define the port sequence to minimize churn (engine -> stream/session -> server -> tests)

## Phase 2: Engine and Connection Semantics Port

- [ ] Refactor `/Users/jim/work/literbike/src/quic/quic_engine.rs` packet processing flow
- [ ] Port/align connection state transitions and bytes-in-flight accounting
- [ ] Improve ACK generation/processing semantics
- [ ] Route CRYPTO frame processing through the current/feature-gated crypto seam
- [ ] Preserve async socket send path and lock-scope safety

## Phase 3: Stream/Session/Config/Error Parity Uplift

- [ ] Port useful stream lifecycle/state behavior into `/Users/jim/work/literbike/src/quic/quic_stream.rs`
- [ ] Expand session cache semantics in `/Users/jim/work/literbike/src/quic/quic_session_cache.rs`
- [ ] Align config and error taxonomy where beneficial
- [ ] Integrate packet builder concepts without duplicating wire codec responsibilities

## Phase 4: Server Integration and Tests

- [ ] Update `/Users/jim/work/literbike/src/quic/quic_server.rs` to use upgraded engine behavior
- [ ] Expand tests under `/Users/jim/work/literbike/tests/quic/` for ACK/STREAM/CRYPTO/lifecycle scenarios
- [ ] Run targeted QUIC test suites and fix regressions
- [ ] Document known limitations (crypto/header protection still partial)

