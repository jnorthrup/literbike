# Plan: Port Kotlin QUIC (Full Packet Processing from Trikeshed)

## Phase 1: Source-to-Target Mapping

- [ ] Read Trikeshed QUIC engine/connection/stream/session/config/error sources
- [ ] Inventory current `literbike` QUIC modules and note gaps vs Trikeshed behavior
- [ ] Identify overlap with existing QUIC interop foundation track (hooks/crypto seams)
- [ ] Define the port sequence to minimize churn (engine -> stream/session -> server -> tests)

## Phase 2: Engine and Connection Semantics Port

- [x] Refactor `/Users/jim/work/literbike/src/quic/quic_engine.rs` packet processing flow
- [ ] Port/align connection state transitions and bytes-in-flight accounting
- [x] Improve ACK generation/processing semantics
- [x] Route CRYPTO frame processing through the current/feature-gated crypto seam
- [x] Preserve async socket send path and lock-scope safety

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

## Progress Notes

- Added `DecodedQuicPacket` in `/Users/jim/work/literbike/src/quic/quic_protocol.rs`
  so wire decode can carry `encoded_packet_number_len` into engine processing.
- Added `QuicEngine::process_decoded_packet(...)` and refactored inbound packet
  processing to use explicit encoded packet-number length when available (falls
  back to inference only for legacy callers).
- Updated `/Users/jim/work/literbike/src/quic/quic_server.rs` and
  `/Users/jim/work/literbike/literbike-quic-capi/src/lib.rs` to use the decoded
  metadata path.
- Added engine test covering header-protection hook PN-length propagation from
  decoded packet metadata.
- Improved ACK processing in `/Users/jim/work/literbike/src/quic/quic_engine.rs`
  to prune acknowledged sent packets and subtract exact wire lengths (instead of
  fixed-size estimates), and deduplicate pending ACK packet numbers before range
  generation.
- `send_stream_data(...)` now accounts actual serialized packet wire length in
  `bytes_in_flight`.
- `send_stream_data(...)` now commits stream/connection state only after packet
  encode succeeds and performs best-effort rollback of `sent_packets`,
  `bytes_in_flight`, and the appended stream send-buffer tail if UDP send fails.
- Confirmed `CRYPTO` frame handling remains routed through
  `crypto_provider.on_crypto_frame(...)` in the engine path (feature-gated seam
  from the QUIC interop foundation track).

## Validation Notes

- `cargo test -p literbike --features 'quic quic-crypto' --lib quic::quic_engine::tests`
  ✅
- `cargo test -p literbike --features quic --lib quic::quic_protocol::tests` ✅
- `cargo test -p literbike-quic-capi --lib` ✅
- Re-ran after ACK/accounting changes:
  `cargo test -p literbike --features 'quic quic-crypto' --lib quic::quic_engine::tests` ✅
  and `cargo test -p literbike-quic-capi --lib` ✅
- Re-ran after async send rollback changes:
  `cargo test -p literbike --features 'quic quic-crypto' --lib quic::quic_engine::tests` ✅
