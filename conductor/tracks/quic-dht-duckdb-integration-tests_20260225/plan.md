# Plan: Integration Tests (End-to-End QUIC + DHT + DuckDB)

## Phase 1: Test Topology and Harness Design

- [ ] Inventory existing QUIC and DuckDB tests/fixtures
- [ ] Define integration test module layout and fixture reuse strategy
- [ ] Define feature-gating matrix (`quic`, `ipfs`) for scenarios
- [ ] Identify minimum end-to-end scenarios for phase 1 implementation

## Phase 2: QUIC + DuckDB Baseline Integration Scenarios

- [ ] Implement a deterministic QUIC-to-event-to-DuckDB scenario
- [ ] Add assertions for persisted data and audit/readback behavior
- [ ] Add error-path scenario for QUIC/protocol failure propagation
- [ ] Add error-path scenario for DuckDB persistence failure handling (mock or controlled fault)

## Phase 3: DHT/IPFS Integration Scenarios

- [ ] Integrate DHT/IPFS seams from the IPFS port track (feature-gated)
- [ ] Add store/lookup routing scenario tied to DuckDB persistence assertions
- [ ] Add timeout/miss/fallback behavior tests for DHT/IPFS paths
- [ ] Verify deterministic cleanup across all resources

## Phase 4: Verification and Stabilization

- [ ] Run targeted integration tests locally with relevant feature combinations
- [ ] Fix flakiness (timing, ordering, cleanup)
- [ ] Document scenario coverage and known gaps

