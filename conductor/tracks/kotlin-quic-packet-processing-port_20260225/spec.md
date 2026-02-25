# Spec: Port Kotlin QUIC (Full Packet Processing from Trikeshed)

## Overview

This track ports the higher-level QUIC packet-processing behavior from Trikeshed
into `literbike`, focusing on connection state transitions, stream lifecycle,
ACK/CRYPTO/STREAM frame handling, packet builders, and server integration.

This track builds on the existing Rust wire codec foundation already added to
`/Users/jim/work/literbike/src/quic/quic_protocol.rs` and must not regress it.

## Problem

- `literbike` has a partial QUIC engine with simplified packet processing and
  handshake placeholders.
- Trikeshed contains richer QUIC engine/connection/session semantics that can be
  ported to strengthen behavior and test coverage.
- Without this port, the server/engine path remains too incomplete for robust
  interop and higher-level integration work.

## Source Material (Kotlin / Trikeshed)

- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/net/quic/QuicEngine.kt`
- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/net/quic/QuicConnection.kt`
- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/net/quic/QuicProtocol.kt`
- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/net/quic/QuicPacketBuilder.kt`
- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/net/quic/QuicStream.kt`
- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/net/quic/QuicSessionCache.kt`
- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/net/quic/QuicConfig.kt`
- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/net/quic/QuicError.kt`
- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/net/quic/SecureQuicEngine.kt` (reference for follow-on seams)

## Target Modules (Expected)

- `/Users/jim/work/literbike/src/quic/quic_engine.rs`
- `/Users/jim/work/literbike/src/quic/quic_server.rs`
- `/Users/jim/work/literbike/src/quic/quic_stream.rs`
- `/Users/jim/work/literbike/src/quic/quic_session_cache.rs`
- `/Users/jim/work/literbike/src/quic/quic_config.rs`
- `/Users/jim/work/literbike/src/quic/quic_error.rs`
- `/Users/jim/work/literbike/src/quic/quic_protocol.rs` (integration only; avoid format regression)
- `/Users/jim/work/literbike/tests/quic/*`

## Functional Requirements

- Port connection/engine packet-processing semantics from Trikeshed to Rust:
  - inbound packet processing flow
  - ACK generation and ACK handling
  - STREAM frame ingestion and stream state updates
  - CRYPTO frame routing to the active crypto seam (or placeholder seam)
  - connection lifecycle state transitions
- Improve stream ID allocation and stream lifecycle semantics where Trikeshed
  behavior is clearer/more complete than current `literbike`.
- Port packet builder concepts where they reduce duplicated packet construction
  logic in engine/server paths.
- Preserve and use the Rust wire codec (no `bincode` reintroduction).
- Align with the packet-number reconstruction/header-protection hook track
  rather than bypassing those seams.

## Non-Functional Requirements

- Brownfield-safe refactor: preserve compileability and existing exports.
- Keep full TLS/header protection correctness out of scope for this track.
- Prefer test-backed behavioral parity over line-by-line source translation.

## Acceptance Criteria

1. `quic_engine.rs` and related modules incorporate richer packet-processing and
   connection/stream semantics ported from Trikeshed.
2. Existing QUIC tests in `/Users/jim/work/literbike/tests/quic/` continue to
   pass or are updated with stronger expected behavior.
3. New/expanded tests cover ACK, STREAM, CRYPTO, and lifecycle behavior.
4. `quic_protocol.rs` wire codec remains the active serialization path.

## Out of Scope

- Full QUIC/TLS cryptographic correctness
- `io_uring` backend acceleration
- C ABI export crate (tracked separately)

