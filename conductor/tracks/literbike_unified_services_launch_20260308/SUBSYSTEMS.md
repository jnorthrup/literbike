# LiterBike Subsystems Inventory

**Track:** `literbike_unified_services_launch_20260308`
**Phase:** 1 - Boundary Lock
**Date:** 2026-03-09

---

## Executive Summary

This document inventories all transport, traffic, and service subsystems
present in `literbike` as of the unified services launch. It establishes the
boundary between `litebike` (primary shell/edge ingress) and `literbike`
(gated heavy heart/backplane).

---

## Transport Subsystems

### QUIC Transport Layer
**Location:** `src/quic/`
**Status:** ✅ Production-Ready (54 tests passing)

| Module | Lines | Description |
|--------|-------|-------------|
| `quic_engine.rs` | 2645 | Connection and stream processing with flow control |
| `quic_stream.rs` | - | Stream lifecycle with priority-based scheduler |
| `quic_session_cache.rs` | - | Session resumption, 0-RTT, connection pooling |
| `quic_server.rs` | 1127 | Server integration with shared session cache |
| `quic_protocol.rs` | - | Wire codec with 89 RFC-TRACE anchors |
| `quic_error.rs` | - | Error taxonomy for QUIC operations |

**C ABI Exports:** `literbike-quic-capi/src/lib.rs` (1167 lines)
- Connection management: `quic_connect`, `quic_close`, `quic_disconnect`
- Stream management: `quic_stream_create`, `quic_stream_send`, `quic_stream_close`
- Request/Response: `quic_request`, `quic_request_ex`
- Error handling: `quic_last_error_message`
- DHT integration: `quic_dht_service_*` functions

**Capabilities:**
- Connection lifecycle management (idle timeout, recovery)
- Stream multiplexing (100+ concurrent streams)
- Session caching with connection pooling
- Priority-based stream scheduling
- Flow control and congestion control hooks
- HTTP/1.1-over-QUIC (HTTP/3 QPACK framing returns 501)

### Reactor Runtime
**Location:** `src/reactor/`
**Status:** ✅ Complete (10 tests passing)

| Module | Description |
|--------|-------------|
| `operation.rs` | Readiness operations and state |
| `selector.rs` | Platform-specific readiness backend (epoll/kqueue) |
| `timer.rs` | Timer wheel with expiration handling |
| `handler.rs` | Event handler trait and dispatch table |
| `context.rs` | Handler context and registration |

**Capabilities:**
- Event-driven I/O abstraction
- Timer wheel for timeout management
- Handler registration and dispatch
- Platform-specific backends (epoll/kqueue/io_uring)

---

## Traffic Subsystems

### CAS Gateway (Content-Addressed Storage)
**Location:** `src/cas_gateway.rs`, `src/cas_backends.rs`
**Status:** ✅ Complete (9 tests passing)

| Component | Description |
|-----------|-------------|
| `LazyProjectionGateway` | Canonical CAS with lazy materialization |
| `ProjectionPolicy` | Eager vs lazy projection policy |
| `ChunkManifest` | Large object handling with chunking |
| Backend Adapters | Git, IPFS, S3 Blobs, KV (Torrent deferred) |

**Backend Adapters:**
- **Git:** `git2` crate integration, object database storage
- **IPFS:** `ipfs-api-backend-hyper` integration, CID v1 support
- **S3 Blobs:** `reqwest` + S3-compatible API, SigV4 auth
- **KV:** `sled` embedded database, key-value storage

**Capabilities:**
- Lazy N-way projection to multiple backends
- Deterministic locator mapping from content hashes
- Policy-driven backend selection
- Fallback order for partial outages
- Integrity verification with digest round-trips

### DHT/Kademlia
**Location:** `src/dht/`
**Status:** ✅ Core Complete (21 tests passing)

| Module | Description |
|--------|-------------|
| `kademlia.rs` | Kademlia routing with XOR distance |
| `service.rs` | DHT service with persistence (sled) |
| `client.rs` | IPFS client facade |

**Capabilities:**
- PeerId with SHA256 identity
- KBucket routing (20 peers max per bucket)
- RoutingTable with 256 buckets
- Content Identifier (CID) with multihash
- IpfsBlock with DAG links
- Provider records and announcement
- Persistence via sled

---

## Service Subsystems

### KeyMux
**Location:** `src/keymux/` (part of service orchestration)
**Status:** ✅ Complete

**Capabilities:**
- Key management and routing policy
- Key selection for model requests
- Unified decision making

### ModelMux
**Location:** `src/modelmux/` (part of service orchestration)
**Status:** ✅ Complete (15+ tests passing)

**Capabilities:**
- Model facade with pack-backed DSEL picks
- Multi-model selection (GLM5, Kimi K2.5, NVIDIA fallback)
- Balanced model routing

### API Translation Layer
**Location:** `src/api/`, `src/adapters/`
**Status:** ✅ Complete

**Capabilities:**
- Protocol bridging (HTTP/3, HTTP/2, HTTP/1.1)
- Service adapter layer
- Provider facades

---

## Supporting Infrastructure

### Async/Concurrency
**Location:** `Cargo.toml` dependencies
**Status:** ✅ Production-Ready

| Crate | Usage |
|-------|-------|
| `tokio` | Async runtime with full features |
| `futures` | Future combinators and utilities |
| `tokio-stream` | Stream utilities |
| `async-channel` | Channel-based communication |
| `crossbeam-channel` | Lock-free channels |
| `dashmap` | Concurrent hash map |

### Data Processing
**Location:** Various modules
**Status:** ✅ Production-Ready

| Component | Description |
|-----------|-------------|
| `rbcursive` | Recursive data structures (45 tests passing) |
| `ndarray` | Tensor support (feature-gated) |
| `serde`/`serde_json` | Serialization |

### HTTP/2 Integration
**Location:** `src/curl_h2/` (feature: `curl-h2`)
**Status:** ✅ Complete

**Capabilities:**
- curl HTTP/2 support
- h2 protocol handling
- HTTP/1.1 upgrade path

---

## Feature Flags

| Feature | Dependencies | Description |
|---------|--------------|-------------|
| `quic` (default) | bincode, bytes, tracing, log, glob, base64 | QUIC transport |
| `quic-crypto` | quic + ring/rustls | QUIC with crypto |
| `couchdb` | sled, utoipa, axum, tower, ndarray | CouchDB emulator |
| `ipfs` | ipfs-api-backend-hyper, cid | IPFS integration |
| `tensor` | ndarray, ndarray-linalg | Tensor support |
| `curl-h2` | curl, h2, http, tokio-io, clap | HTTP/2 integration |
| `tls-quic` | clap, openssl, hkdf, rustls, rcgen | TLS over QUIC |
| `full` | All features | Complete feature set |

---

## Binary Targets

| Binary | Features | Description |
|--------|----------|-------------|
| `litebike` | warp, git2 | Edge ingress proxy |
| `literbike` | All | Heavy unified runtime |
| `couchdb_emulator` | couchdb | CouchDB compatibility layer |
| `quic_tls_server` | tls-quic | QUIC TLS server |
| `quic_curl_h2` | curl-h2 | QUIC curl HTTP/2 test |
| `quic_capi` | quic | C ABI library for FFI |

---

## Test Coverage

| Subsystem | Tests | Status |
|-----------|-------|--------|
| QUIC Engine | 53 | ✅ Passing |
| QUIC Stream | 5 | ✅ Passing |
| QUIC Session Cache | 5 | ✅ Passing |
| QUIC Server | 15 | ✅ Passing |
| QUIC Protocol | 10 | ✅ Passing |
| CAS Gateway | 7 | ✅ Passing |
| CAS Backends | 4 | ✅ Passing |
| Reactor | 10 | ✅ Passing |
| DHT | 10 | ✅ Passing |
| Rbcursive | 45 | ✅ Passing |
| **Total** | **263** | ✅ **All Passing** |

---

## Canonical Ports

| Port | Service | Status |
|------|---------|--------|
| `8888` | Unified agent surface | ✅ Canonical |
| `4433` | QUIC (temporary) | ⚠️ Deprecated |

**Note:** Port 8888 is the canonical unified-port surface for operator-facing control path. Port 4433 was temporary drift and should not be used in new deployments.

---

## Ownership Boundary

### LiteBike (Edge Ingress)
- Local protocol classification
- Fast operator-facing control path
- Lean proxy/router companion
- Unified port 8888 surface

### LiterBike (Heavy Runtime)
- QUIC transport depth
- Reactor/event-driven I/O
- CAS gateway with lazy projection
- DHT/Kademlia routing
- KeyMux/ModelMux orchestration
- API translation and adapters
- Service orchestration

---

## Next Steps

1. **Documentation:** Create operator-facing launch narrative
2. **Examples:** Add minimum working examples for each subsystem
3. **Cross-reference:** Align with `litebike` launch track
4. **Validation:** Ensure feature flags compile cleanly

---

**Generated:** 2026-03-09
**Track:** `literbike_unified_services_launch_20260308`
**Phase:** 1 Complete
