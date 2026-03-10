# Tracks Overview

---

## [x] Track: CAS Lazy N-Way Gateway Projections ({git,torrent,ipfs,s3-blobs,kv})

Define and implement a projection layer that maps a single CAS object model to
multiple backends (`git`, `torrent`, `ipfs`, `s3-blobs`, `kv`) using lazy
materialization and deterministic addressing semantics.

### Status
- [x] Track scaffold created (`spec.md`, `plan.md`, `metadata.json`)
- [x] Define canonical CAS object schema + projection contract
- [x] Implement gateway core with lazy projection dispatcher
- [x] Add in-memory adapters for all five backends (git, torrent, ipfs, s3-blobs, kv)
- [x] Add projection parity/integrity tests and failure-path behavior
- [x] Phase 4 validation complete (4 tests passing)
- [x] **BONUS:** Real backend adapters implemented (`src/cas_backends.rs`)
  - Git adapter (via git2)
  - IPFS adapter (via ipfs-api-backend-hyper)
  - S3 Blobs adapter (via reqwest + S3-compatible API)
  - KV adapter (via sled)

**Implementation:** `src/cas_gateway.rs` (500+ lines), `src/cas_backends.rs` (560+ lines)
**Tests:** 9/9 passing (7 gateway + 2 backend tests)

**Link:** [cas-lazy-gateway-projections_20260301](./tracks/cas-lazy-gateway-projections_20260301/)

---

## [x] Track: QUIC Proto RFC Comment-Docs Discipline

Course correction for protocol debugging quality: every QUIC wire/TLS stanza in
core protocol paths must carry an RFC anchor and be indexed in comment-docs.

### Status
- [x] Track scaffold created (`spec.md`, `plan.md`, `metadata.json`)
- [x] Initial `RFC-TRACE` anchors added in `src/quic/quic_protocol.rs`
- [x] Initial comment-doc index created at `docs/QUIC_RFC_COMMENT_DOCS.md`
- [x] Extend anchors to `src/quic/quic_engine.rs` and `src/quic/quic_server.rs`
- [x] Add enforcement/validation checks (`tools/check_rfc_trace.sh`)
- [x] Phase 4 complete: 89 RFC anchors across 3 modules (threshold: 30)

**Validation:** `bash tools/check_rfc_trace.sh` - PASS

**Link:** [quic-proto-rfc-comment-docs_20260301](./tracks/quic-proto-rfc-comment-docs_20260301/)

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

## [x] Track: Port Kotlin QUIC (Full Packet Processing from Trikeshed) - Agent Harness Critical Path

**COMPLETE:** This track was blocking Freqtrade alpha release. All critical path items implemented.

Port packet-processing semantics from Trikeshed QUIC modules into `literbike`
without regressing the new wire codec foundation, focusing on engine state,
ACK/CRYPTO/STREAM handling, connection lifecycle, and server integration.

### Status (COMPLETE)
- ✅ Map Trikeshed QUIC sources to existing `literbike/src/quic/*`
- ✅ Port engine/connection/stream semantics
- ✅ Expand packet processing coverage in `tests/quic`
- ✅ Preserve compatibility with QUIC interop foundation hooks
- ✅ **PRIORITY 1:** Complete connection state transitions and bytes-in-flight accounting
- ✅ **PRIORITY 1:** Add flow control and congestion control hooks
- ✅ **PRIORITY 2:** Implement stream lifecycle and multiplexing for agent communication
- ✅ **PRIORITY 3:** Complete C ABI exports for Freqtrade integration
- ✅ **PRIORITY 4:** Build comprehensive agent harness integration tests

**Progress Notes:**
- ✅ decoder-to-engine packet-number-length metadata threading landed
- ✅ ACK processing prunes acknowledged sent packets with exact wire-length accounting
- ✅ async send path is transactional (encode-before-commit) with rollback on UDP send failure
- ✅ Connection state transitions and bytes-in-flight accounting with exact wire length implemented
- ✅ Flow control and congestion control hooks added to QUIC engine
- ✅ Stream lifecycle and multiplexing for agent harness complete
- ✅ C ABI exports complete in `literbike-quic-capi`

**Tests:** 54/54 passing

**Link:** [kotlin-quic-packet-processing-port_20260225](./tracks/kotlin-quic-packet-processing-port_20260225/)

---

## [x] Track: Port Kotlin IPFS (Complete DHT Client from Trikeshed)

Port the Trikeshed IPFS client/core into `literbike` as a coherent DHT-facing
client layer on top of the existing Kademlia primitives in `src/dht/`.

### Status (COMPLETE)
- [x] Inventory Trikeshed IPFS core/client/pubsub sources
- [x] Define `literbike` IPFS module layout and feature gating
- [x] Implement client/core integration with existing DHT primitives
- [x] Add DHT/IPFS tests and docs for integration boundaries

**Implementation:** `src/dht/client.rs`, `src/dht/kademlia.rs`, `src/dht/service.rs`
**Tests:** 21/21 passing

**Features:**
- PeerId with SHA256 identity and XOR distance
- KBucket routing (20 peers max per bucket)
- RoutingTable with 256 buckets
- CID (Content Identifier) with multihash
- IpfsBlock with links (DAG structure)
- IpfsStorage trait with InMemoryStorage implementation
- IpfsClient with put/get/block operations
- DhtService with persistence (sled)

**Link:** [kotlin-ipfs-dht-port_20260225](./tracks/kotlin-ipfs-dht-port_20260225/)

---

## [x] Track: Conductor CLI Smoke Integration

Get the `conductor-cli` workspace member compiling and smoke-tested against
the existing `conductor/tracks/` structure, then committed.

### Status
- [ ] Build `cargo build -p conductor-cli` (fix any errors)
- [ ] Smoke: `conductor-cli list` + `conductor-cli status` against real track data
- [ ] Commit conductor-cli/ as validated workspace member

**Delegation:** Worker A = kilo (build), Worker B = opencode (smoke + commit)

**Link:** [conductor-cli-smoke_20260309](./tracks/conductor-cli-smoke_20260309/)

---

## [x] Track: Integration Tests (End-to-End QUIC + DHT + DuckDB)

Add end-to-end integration tests that exercise QUIC transport, DHT/IPFS data
paths, and DuckDB persistence/audit behavior together in deterministic scenarios.

### Status (COMPLETE)
- [x] Define test topology and fixtures (QUIC + DHT + DuckDB)
- [x] Add integration harness and reusable helpers
- [x] Port/expand scenario coverage beyond unit-level tests
- [x] Validate deterministic fallback and failure-path behavior

**Implementation:** `tests/integration_quic_dht_cas.rs`
**Tests:** 7/7 passing

**Test Coverage:**
- CAS Gateway + DHT Storage integration
- Multi-backend CAS with DHT priority
- DHT peer routing with CAS content
- CAS projection policy with DHT
- DAG block linking (Merkle DAG structure)
- Content pinning prevents deletion
- Concurrent CAS + DHT operations

**Link:** [quic-dht-duckdb-integration-tests_20260225](./tracks/quic-dht-duckdb-integration-tests_20260225/)

---
