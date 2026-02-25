<!-- tokens: T002 -->

# Plan: QUIC Interop Foundation (Packet Numbers, Crypto Hooks, C ABI)

## Phase 1: Interface Lock and Baseline Analysis

- [x] Review current packet processing path in `/Users/jim/work/literbike/src/quic/quic_engine.rs`
- [x] Identify where truncated packet number is currently parsed/used and define reconstruction inputs
- [x] Define header-protection hook interface shape (trait/functions/module boundaries)
- [x] Define feature flag name and module layout for handshake/crypto integration
- [x] Confirm C ABI function signatures against `/Users/jim/work/freqtrade/user_data/ops/literbike_quic_transport.py`
- [x] Choose new crate name and output library name strategy (`libliterbike_quic.*` compatibility)

## Phase 2: Packet Number Reconstruction + Header Protection Hooks

- [x] Implement packet number reconstruction helper(s) in `/Users/jim/work/literbike/src/quic/quic_engine.rs`
- [x] Add engine state inputs required for reconstruction (expected packet number, pn length, etc.)
- [x] Add header-protection hook interfaces and wire them into inbound packet processing
- [x] Add outbound header-protection hook call sites (stub/no-op when feature disabled)
- [x] Add/extend unit tests for packet number reconstruction and hook invocation/error paths

## Phase 3: Feature-Gated Handshake/Crypto Integration Path

- [x] Add new feature flag in `/Users/jim/work/literbike/Cargo.toml` for handshake/crypto path
- [x] Create handshake/crypto module(s) and public integration seam in `/Users/jim/work/literbike/src/quic/`
- [x] Define coarse handshake state model and readiness checks used by engine/server
- [x] Wire CRYPTO frame handling through the new seam (feature-on path)
- [x] Keep feature-off behavior compiling and preserve current foundational tests
- [x] Add compile/smoke tests for feature-on and feature-off builds

## Phase 4: C ABI QUIC `cdylib` Crate for `freqtrade`

- [x] Add a new workspace member crate (separate from `/Users/jim/work/literbike/literbike-ffi`)
- [x] Configure crate as `cdylib` with ctypes-friendly exported symbols
- [x] Implement opaque connection handle lifecycle (`quic_connect` / `quic_close`)
- [x] Implement request/response surface (`quic_request` + response accessors/free)
- [x] Implement thread-local or equivalent last-error message accessor
- [x] Add smoke/error tests for invalid handles and request failures

## Phase 5: Validation and Integration Readiness

- [x] Run targeted Rust tests for QUIC protocol/engine changes
- [x] Build the new C ABI crate and verify exported symbols are present
- [ ] Run a minimal Python `ctypes` smoke path (connect/request error path is acceptable initially)
- [ ] Verify existing `literbike-ffi` PyO3 crate remains buildable/unbroken
- [x] Document limitations (not full QUIC/TLS interop yet) in track notes or code comments
