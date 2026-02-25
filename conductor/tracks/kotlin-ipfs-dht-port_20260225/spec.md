# Spec: Port Kotlin IPFS (Complete DHT Client from Trikeshed)

## Overview

`literbike` already contains Kademlia-oriented DHT primitives in
`/Users/jim/work/literbike/src/dht/`, but it does not yet expose a coherent IPFS
client/core layer comparable to Trikeshed's Kotlin implementation. This track
ports the IPFS client/core functionality into a `literbike`-native module layout.

## Problem

- No first-class IPFS client/core module exists in current `literbike` sources.
- Existing DHT code (`src/dht/kademlia.rs`) lacks a higher-level IPFS-facing API.
- Future integration testing (QUIC + DHT + DuckDB) needs a stable IPFS/DHT seam.

## Source Material (Kotlin / Trikeshed)

- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/ipfs/IpfsCore.kt`
- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/ipfs/IpfsClient.kt`
- `/Users/jim/work/superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/ipfs/IpfsPubSubService.kt`

## Current Rust Context

- `/Users/jim/work/literbike/src/dht/mod.rs`
- `/Users/jim/work/literbike/src/dht/kademlia.rs`
- `/Users/jim/work/literbike/src/couchdb/ipfs.rs` (integration reference only)
- `/Users/jim/work/literbike/src/lib.rs` exposes `ipfs_integration` behind `feature = "ipfs"` but no equivalent `src/ipfs/` tree exists today.

## Target Modules (Expected)

- `/Users/jim/work/literbike/src/ipfs/` (new module tree, feature-gated)
- or `/Users/jim/work/literbike/src/ipfs_integration.rs` + submodules (if chosen)
- `/Users/jim/work/literbike/src/lib.rs` (module exports)
- `/Users/jim/work/literbike/tests/` (new IPFS/DHT tests)

## Functional Requirements

- Port Trikeshed `IpfsCore` concepts into a Rust IPFS core module layered on the
  existing Kademlia DHT primitives.
- Port an IPFS client API sufficient for local content operations / DHT lookup
  workflows used by `literbike` integrations.
- Define a pubsub seam (can be minimal/stubbed) derived from
  `IpfsPubSubService.kt`, with explicit scope and limitations.
- Feature-gate the IPFS client/core path behind the existing `ipfs` feature.
- Keep DHT core logic reusable independently of the IPFS client facade.

## Non-Functional Requirements

- Brownfield-safe: do not break existing `dht` consumers.
- No full external IPFS network compatibility guarantee in this track unless
  proven by tests.
- Avoid pulling in heavy runtime/network dependencies unless required.

## Acceptance Criteria

1. `literbike` has a coherent IPFS core/client module path gated by `feature = "ipfs"`.
2. Existing DHT primitives remain intact and are used by the new layer.
3. Tests cover at least core DHT/IPFS workflows (store/lookup or routing ops).
4. Module boundaries and limitations are documented in the track artifacts/code.

## Out of Scope

- Full production-grade IPFS node parity
- QUIC transport integration for IPFS traffic (separate integration track)
- DuckDB persistence integration (separate integration track)

