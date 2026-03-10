# Plan: CAS Lazy N-Way Gateway Projections

## Phase 1: Canonical CAS Contract

- [x] Define canonical object identity and metadata envelope
- [x] Define chunk/manifest strategy and small-object fast path
- [x] Define projection API (`put`, `project`, `get`, integrity verify)

## Phase 2: Gateway Core and Routing

- [x] Add backend registry and lazy projection dispatcher
- [x] Implement deterministic backend-handle mapping from canonical IDs
- [x] Add policy hooks for projection trigger and fallback order

## Phase 3: Backend Adapters ({git,torrent,ipfs,s3-blobs,kv})

- [x] Implement `git` projection adapter (via git2)
- [ ] Implement `torrent` projection adapter (deferred)
- [x] Implement `ipfs` projection adapter (via ipfs-api-backend-hyper)
- [x] Implement `s3-blobs` projection adapter (via reqwest + S3-compatible API)
- [x] Implement `kv` projection adapter (via sled)

## Phase 4: Verification and Failure Semantics

- [x] Add parity fixtures and digest round-trip tests per backend
- [x] Add lazy-write verification tests (no eager materialization)
- [x] Add partial-outage and retry behavior tests
- [x] Document residual gaps and operational constraints

## Status Notes

- Track initialized from request: "new track {git,torrent,ipfs,s3-blobs,kv} lazy n-way gateway projections of CAS".
- Implementation slice landed in `src/cas_gateway.rs` with canonical CAS envelope,
  lazy `put/project/get`, deterministic locator mapping, and in-memory adapter
  stubs for all five backends.
- 2026-03-09: Phase 1, 2, 4 closed. ChunkManifest + ProjectionPolicy + 4 Phase-4 tests added to cas_gateway.rs.
- 2026-03-09: Phase 3 complete for git, ipfs, s3-blobs, kv adapters in `src/cas_backends.rs` (564 lines).
  Torrent adapter deferred to future track.
