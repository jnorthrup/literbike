# Plan: CAS Lazy N-Way Gateway Projections

## Phase 1: Canonical CAS Contract

- [x] Define canonical object identity and metadata envelope
- [ ] Define chunk/manifest strategy and small-object fast path
- [x] Define projection API (`put`, `project`, `get`, integrity verify)

## Phase 2: Gateway Core and Routing

- [x] Add backend registry and lazy projection dispatcher
- [x] Implement deterministic backend-handle mapping from canonical IDs
- [ ] Add policy hooks for projection trigger and fallback order

## Phase 3: Backend Adapters ({git,torrent,ipfs,s3-blobs,kv})

- [~] Implement `git` projection adapter
- [~] Implement `torrent` projection adapter
- [~] Implement `ipfs` projection adapter
- [~] Implement `s3-blobs` projection adapter
- [~] Implement `kv` projection adapter

## Phase 4: Verification and Failure Semantics

- [ ] Add parity fixtures and digest round-trip tests per backend
- [ ] Add lazy-write verification tests (no eager materialization)
- [ ] Add partial-outage and retry behavior tests
- [ ] Document residual gaps and operational constraints

## Status Notes

- Track initialized from request: "new track {git,torrent,ipfs,s3-blobs,kv} lazy n-way gateway projections of CAS".
- Implementation slice landed in `src/cas_gateway.rs` with canonical CAS envelope,
  lazy `put/project/get`, deterministic locator mapping, and in-memory adapter
  stubs for all five backends.
