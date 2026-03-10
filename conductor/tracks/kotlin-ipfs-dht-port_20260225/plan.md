# Plan: Port Kotlin IPFS (Complete DHT Client from Trikeshed)

## Phase 1: Inventory and Module Design

- [x] Read Trikeshed IPFS core/client/pubsub source files
- [x] Inspect existing `literbike` DHT and CouchDB IPFS integration code
- [x] Choose Rust module layout (`src/dht/` tree)
- [x] Define feature-gated export strategy in `/Users/jim/work/literbike/src/lib.rs`

## Phase 2: Core Port on Top of DHT Primitives

- [x] Port `IpfsCore` concepts into Rust core module(s) (`src/dht/kademlia.rs`)
- [x] Integrate with `/Users/jim/work/literbike/src/dht/kademlia.rs`
- [x] Define content/routing interfaces and error types
- [x] Add minimal pubsub seam (explicitly scoped)

## Phase 3: Client API and Integration Seams

- [x] Port `IpfsClient`-style operations into an idiomatic Rust client facade (`src/dht/client.rs`)
- [x] Add feature-gated exports and module wiring
- [x] Reconcile overlap with `/Users/jim/work/literbike/src/couchdb/ipfs.rs`
- [x] Document integration boundaries (DHT-only vs IPFS facade)

## Phase 4: Verification

- [x] Add IPFS/DHT tests under `/Users/jim/work/literbike/tests/`
- [x] Run DHT/IPFS feature-gated tests
- [x] Document known gaps versus Trikeshed implementation

## Implementation Status

**Date:** 2026-03-09
**Status:** ✅ COMPLETE

### Implemented Modules

1. **`src/dht/kademlia.rs`** - Kademlia DHT routing
   - PeerId with SHA256 identity and XOR distance
   - KBucket routing (20 peers max per bucket)
   - RoutingTable with 256 buckets
   - FIND_NODE, GET_PROVIDERS, PUT_VALUE operations

2. **`src/dht/client.rs`** - IPFS client facade
   - CID (Content Identifier) with multihash
   - IpfsBlock with links (DAG structure)
   - IpfsStorage trait with InMemoryStorage implementation
   - IpfsClient with put/get/block operations

3. **`src/dht/service.rs`** - DHT service layer
   - DhtService with persistence (sled)
   - Peer discovery and routing
   - Provider records management

### Test Coverage

- **21 tests passing** across DHT modules
- PeerId identity and XOR distance
- KBucket operations and limits
- RoutingTable find_closest_peers
- CID encoding and decoding
- IpfsBlock put/get roundtrip
- Pin/unpin operations
- DAG link traversal

### Trikeshed Alignment

| Trikeshed Class | Literbike Rust Module | Status |
|-----------------|----------------------|--------|
| `IpfsCore.kt` | `src/dht/kademlia.rs` | ✅ Ported |
| `IpfsClient.kt` | `src/dht/client.rs` | ✅ Ported |
| `IpfsPubSubService.kt` | Deferred | ⏸️ Future track |

## Status Notes

- 2026-03-09: All phases complete. DHT/IPFS client ported from Trikeshed.
- Implementation in `src/dht/` with 3 modules (kademlia, client, service).
- 21 tests passing covering routing, storage, and client operations.
- PubSub service deferred to future track.

