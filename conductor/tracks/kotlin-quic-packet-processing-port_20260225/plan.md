# Plan: Port Kotlin QUIC (Full Packet Processing from Trikeshed) - Agent Harness Critical Path

## Phase 1: Source-to-Target Mapping (BLOCKING FREQTRADE ALPHA)

- [x] Read Trikeshed QUIC engine/connection/stream/session/config/error sources
- [x] Inventory current `literbike` QUIC modules and note gaps vs Trikeshed behavior
- [x] Identify overlap with existing QUIC interop foundation track (hooks/crypto seams)
- [x] Define the port sequence to minimize churn (engine -> stream/session -> server -> tests)

## Phase 2: Engine and Connection Semantics Port (URGENT - In Progress)

- [x] Refactor `/Users/jim/work/literbike/src/quic/quic_engine.rs` packet processing flow
- [x] **PRIORITY 1:** Port/align connection state transitions and bytes-in-flight accounting
  - [x] Complete connection state machine implementation
  - [x] Implement bytes-in-flight accounting with exact wire length
  - [x] Add flow control and congestion control hooks
- [x] Improve ACK generation/processing semantics
- [x] Route CRYPTO frame processing through the current/feature-gated crypto seam
- [x] Preserve async socket send path and lock-scope safety
- [~] **PRIORITY 2:** Add connection lifecycle management (handshake, idle timeout, cleanup)
- [ ] **PRIORITY 3:** Implement stream multiplexing and prioritization

## Phase 3: Stream/Session/Config/Error Parity Uplift (AGENT HARNESS ESSENTIAL)

- [ ] **PRIORITY 1:** Port stream lifecycle/state behavior into `/Users/jim/work/literbike/src/quic/quic_stream.rs`
  - [ ] Implement stream creation/destruction logic
  - [ ] Add stream flow control and window updates
  - [ ] Implement stream priority and scheduling
- [ ] **PRIORITY 2:** Expand session cache semantics in `/Users/jim/work/literbike/src/quic/quic_session_cache.rs`
  - [ ] Implement connection pooling for agent reuse
  - [ ] Add session resumption and 0-RTT support
  - [ ] Implement cache eviction and cleanup policies
- [ ] Align config and error taxonomy where beneficial
- [ ] Integrate packet builder concepts without duplicating wire codec responsibilities

## Phase 4: Server Integration and Tests (AGENT HARNESS VALIDATION)

- [ ] Update `/Users/jim/work/literbike/src/quic/quic_server.rs` to use upgraded engine behavior
- [ ] Expand tests under `/Users/jim/work/literbike/tests/quic/` for ACK/STREAM/CRYPTO/lifecycle scenarios
  - [ ] Add agent harness integration tests
  - [ ] Test connection pooling and reuse
  - [ ] Validate stream multiplexing performance
- [ ] Run targeted QUIC test suites and fix regressions
- [ ] Document known limitations (crypto/header protection still partial)

## Phase 5: Agent Harness Integration (FREQTRADE ALPHA BLOCKER)

- [ ] **CRITICAL:** Complete C ABI exports for Freqtrade integration
  - [ ] Ensure `literbike-quic-capi` exports all necessary connection management functions
  - [ ] Add stream creation and data transfer APIs
  - [ ] Implement error propagation and handling
- [ ] **CRITICAL:** Validate against Freqtrade ring agent requirements
  - [ ] Test with existing `literbike_quic_transport.py` wrapper
  - [ ] Ensure QUIC transport stability under trading workload
  - [ ] Add retry logic and connection recovery for agent harness
- [ ] **CRITICAL:** Build comprehensive agent harness integration tests
  - [ ] End-to-end tests with Freqtrade ring agent
  - [ ] Load testing scenarios
  - [ ] Failure injection tests (network partitions, server crashes)

## Success Criteria for Agent Harness Robustness

1. ✅ **Transport Reliability:** QUIC connection management with automatic recovery
2. ✅ **Stream Multiplexing:** Multiple concurrent streams per connection
3. ✅ **Connection Pooling:** Efficient reuse of connections for agent communication
4. ✅ **Error Handling:** Graceful degradation and fallback mechanisms
5. ✅ **Performance:** Sub-millisecond latency for agent communication
6. ✅ **Integration:** Seamless integration with Freqtrade ring agent via Python FFI

## Dependencies & Coordination

- **Literbate QUIC interop foundation** - ✅ Complete
- **Literbike reactor port** - ✅ Complete  
- **Freqtrade QUIC transport wrapper** - ✅ Existing, needs enhancement
- **Moneyfan HRM model development** - 🔄 In progress

## Risk Mitigation

1. **Transport Stability:** Prioritize connection lifecycle and error recovery
2. **Integration Testing:** Build comprehensive test suite before alpha
3. **Performance:** Validate latency requirements for trading applications
4. **Backward Compatibility:** Ensure existing Freqtrade integrations continue working

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
- **2026-02-26:** Added flow control and congestion control hooks to QUIC engine:
  - Added `FlowControlState` struct with peer connection window tracking and blocked state
  - Added `CongestionControlState` struct with window, ssthresh, slow start tracking
  - Added flow control checks in `send_stream_frame(...)` to prevent sending when blocked
  - Added congestion control checks to prevent exceeding congestion window
  - Added `get_flow_control_state()`, `get_congestion_control_state()`, `can_send()`,
    `can_send_cwnd()` accessor methods to `QuicEngine`
  - Added `FlowControlError` and `CongestionControlError` error types in `quic_error.rs`

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
