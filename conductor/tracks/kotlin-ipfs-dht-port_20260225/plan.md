# Plan: Port Kotlin IPFS (Complete DHT Client from Trikeshed)

## Phase 1: Inventory and Module Design

- [ ] Read Trikeshed IPFS core/client/pubsub source files
- [ ] Inspect existing `literbike` DHT and CouchDB IPFS integration code
- [ ] Choose Rust module layout (`src/ipfs/` tree vs `ipfs_integration` split)
- [ ] Define feature-gated export strategy in `/Users/jim/work/literbike/src/lib.rs`

## Phase 2: Core Port on Top of DHT Primitives

- [ ] Port `IpfsCore` concepts into Rust core module(s)
- [ ] Integrate with `/Users/jim/work/literbike/src/dht/kademlia.rs`
- [ ] Define content/routing interfaces and error types
- [ ] Add minimal pubsub seam (explicitly scoped)

## Phase 3: Client API and Integration Seams

- [ ] Port `IpfsClient`-style operations into an idiomatic Rust client facade
- [ ] Add feature-gated exports and module wiring
- [ ] Reconcile overlap with `/Users/jim/work/literbike/src/couchdb/ipfs.rs`
- [ ] Document integration boundaries (DHT-only vs IPFS facade)

## Phase 4: Verification

- [ ] Add IPFS/DHT tests under `/Users/jim/work/literbike/tests/`
- [ ] Run DHT/IPFS feature-gated tests
- [ ] Document known gaps versus Trikeshed implementation

