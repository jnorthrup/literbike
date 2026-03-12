<!-- tokens: T001 -->

# Spec: QUIC Interop Foundation (Packet Numbers, Crypto Hooks, C ABI)

## Overview

This track advances `literbike` from a foundational QUIC wire codec toward an
interop-ready transport core. The scope is intentionally limited to foundational
work that unblocks future "Google-compatible" behavior without pretending full
QUIC/TLS interoperability is complete in this track.

The track covers three linked deliverables:

1. Packet number reconstruction and header-protection hook points in
   `/Users/jim/work/literbike/src/quic/quic_engine.rs`
2. A real handshake/crypto integration path that is feature-gated (off by
   default unless explicitly enabled)
3. A separate ctypes-friendly C ABI `cdylib` crate for the `external-bot` wrapper
   in `/Users/jim/work/external-bot/user_data/ops/literbike_quic_transport.py`

## Problem

- The current engine lacks packet number reconstruction and header-protection
  extension points, which are required for real QUIC packet processing.
- Handshake/crypto behavior is placeholder-level and not structured for an
  incremental upgrade path.
- `external-bot` expects C ABI functions (`quic_connect`, `quic_request`,
  `quic_close`) but `literbike` currently has only a PyO3-oriented crate
  (`literbike-ffi`) and no stable ctypes-facing exports.

## Goals

- Establish explicit engine-level hooks for packet number reconstruction and
  header protection without breaking current foundational packet flow.
- Introduce a feature-gated crypto/handshake integration seam (traits/modules +
  state hooks) that can grow toward real QUIC/TLS handling.
- Provide a separate `cdylib` crate with stable C ABI exports aligned to the
  `external-bot` ctypes wrapper expectations.

## Functional Requirements

### 1. Packet Number Reconstruction + Header Protection Hooks

- Add packet-number reconstruction logic (or equivalent helper APIs) in
  `/Users/jim/work/literbike/src/quic/quic_engine.rs` that can compute the full
  packet number from the truncated on-wire packet number and connection state.
- Add header-protection hook points that allow a future implementation to:
  - unmask short/long header protected fields
  - recover encoded packet number length before reconstruction
  - apply outbound protection before transmit
- Hook design must be explicit (traits/functions/modules), not implicit TODOs.
- Default behavior may be no-op or stubbed when crypto feature is disabled, but
  the engine path must compile and run.

### 2. Feature-Gated Handshake/Crypto Integration Path

- Introduce a feature-gated module path (e.g. `quic-crypto` or equivalent) that
  wires handshake/crypto responsibilities into the QUIC engine/server flow.
- The initial path must define:
  - handshake state transitions (at least coarse phases)
  - crypto frame handling seam (ingress/egress handoff)
  - key availability / protection readiness checks
  - clear error propagation for unsupported/incomplete states
- The feature-off path must remain the default and keep current tests/builds
  working.
- The feature-on path can be partial, but it must compile and exercise real code
  paths beyond placeholders.

### 3. Separate C ABI QUIC Exports (`cdylib`) for `external-bot`

- Add a new workspace member crate (separate from `/Users/jim/work/literbike/literbike-ffi`)
  for ctypes-compatible QUIC exports.
- The library should produce a platform C dynamic library with a stable symbol
  surface usable from Python `ctypes` (prefer library name compatibility with
  the existing wrapper search path, e.g. `libliterbike_quic.*`).
- Required exported functions (minimum):
  - `quic_connect`
  - `quic_request`
  - `quic_close`
- Strongly recommended exported helpers for response/error ownership:
  - response status/body accessors
  - response free function
  - last-error message accessor
- Ownership semantics must be explicit (opaque handles + free functions).
- Existing `literbike-ffi` PyO3 crate must remain intact and non-breaking.

## Non-Functional Requirements

- Brownfield-safe, additive changes preferred.
- Keep `io_uring` / Linux-native acceleration out of this track.
- Avoid regressions in current `quic` feature builds/tests.
- Keep API surface documented enough for `external-bot` wrapper integration.

## Acceptance Criteria

1. `/Users/jim/work/literbike/src/quic/quic_engine.rs` contains concrete packet
   number reconstruction logic and explicit header-protection hook interfaces
   used by packet-processing paths.
2. A feature-gated crypto/handshake module path exists, is integrated into the
   engine/server flow, and compiles both with the feature disabled and enabled.
3. `/Users/jim/work/literbike/Cargo.toml` workspace includes a new C ABI crate,
   and the crate builds a `cdylib` exposing the required QUIC symbols.
4. The C ABI surface is compatible with the expected call shape used in
   `/Users/jim/work/external-bot/user_data/ops/literbike_quic_transport.py`
   (connect/request/close, plus clear error behavior).
5. Focused tests cover:
   - packet number reconstruction behavior
   - header-protection hook integration/error paths
   - feature-gated handshake/crypto compile/runtime smoke
   - C ABI handle/error smoke path

## Out of Scope

- Full QUIC/TLS interoperability (complete TLS 1.3 handshake, key schedule,
  packet protection correctness, header protection correctness)
- Full HTTP/3/QPACK implementation
- `io_uring`/quiche backend acceleration
- Replacing the existing Python HTTP fallback behavior in `external-bot`

## Impacted Files and Modules (Expected)

- `/Users/jim/work/literbike/src/quic/quic_engine.rs`
- `/Users/jim/work/literbike/src/quic/quic_server.rs`
- `/Users/jim/work/literbike/src/quic/mod.rs`
- `/Users/jim/work/literbike/Cargo.toml`
- `/Users/jim/work/literbike/literbike-ffi/*` (read-only compatibility reference)
- `/Users/jim/work/literbike/<new-capi-crate>/...`
- `/Users/jim/work/external-bot/user_data/ops/literbike_quic_transport.py` (consumer compatibility reference; implementation may remain unchanged in this track)

