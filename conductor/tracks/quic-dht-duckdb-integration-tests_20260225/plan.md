# Plan: Integration Tests (End-to-End QUIC + DHT + DuckDB)

## Phase 1: Test Topology and Harness Design

- [x] Inventory existing QUIC and DuckDB tests/fixtures
- [x] Define integration test module layout and fixture reuse strategy
- [x] Define feature-gating matrix (`quic`, `ipfs`, `couchdb`) for scenarios
- [x] Identify minimum end-to-end scenarios for phase 1 implementation

### Phase 1 Findings (2026-03-09)

- `tests/quic/` — 8 test files covering packet serialization, connection state, frame types,
  protocol validation, engine, stream ingestion, channel distribution, lifecycle. All pass.
- `tests/integration_quic_dht_cas.rs` — 12 integration tests covering QUIC+DHT+CAS scenarios
- `src/dht/` — Complete DHT implementation with 21 passing tests
- `src/cas_gateway.rs` — CAS gateway with 9 passing tests
- **Integration approach**: Use CAS as event store abstraction (DuckDB deferred to future track)

## Phase 2: QUIC + DHT Baseline Integration Scenarios

- [x] Implement a deterministic QUIC-to-event persistence scenario
- [x] Add assertions for persisted data and audit/readback behavior
- [x] Add error-path scenario for QUIC/protocol failure propagation
- [x] Add concurrent access tests for resource cleanup verification

## Phase 3: DHT/IPFS Integration Scenarios

- [x] Integrate DHT/IPFS seams from the IPFS port track
- [x] Add store/lookup routing scenario tied to CAS persistence assertions
- [x] Add timeout/miss/fallback behavior tests for DHT/IPFS paths
- [x] Verify deterministic cleanup across all resources

## Phase 4: Verification and Stabilization

- [x] Run targeted integration tests locally with relevant feature combinations
- [x] Fix flakiness (timing, ordering, cleanup)
- [x] Document scenario coverage and known gaps

## Test Coverage

### Implemented Tests (12 total)

1. `test_cas_gateway_with_dht_storage` - CAS + DHT storage integration
2. `test_cas_multi_backend_with_dht_priority` - Multi-backend CAS with DHT fallback
3. `test_dht_peer_routing_with_cas_content` - DHT peer routing with CAS content
4. `test_cas_eager_projection_policy` - CAS projection policy (eager/lazy)
5. `test_dag_block_linking` - DAG block linking (Merkle DAG structure)
6. `test_content_pinning_prevents_deletion` - Content pinning prevents deletion
7. `test_concurrent_cas_dht_operations` - Concurrent CAS + DHT operations
8. `test_quic_event_to_duckdb_persistence` - QUIC-to-event persistence (couchdb feature)
9. `test_quic_protocol_failure_propagation` - QUIC protocol failure propagation
10. `test_dht_ipfs_timeout_fallback_behavior` - DHT/IPFS timeout fallback
11. `test_duckdb_event_log_schema_validation` - DuckDB event log schema (couchdb feature)
12. `test_full_stack_quic_dht_cas_integration` - Full-stack QUIC+DHT+CAS integration

### Feature Gates

- Default tests: 10/12 (always available)
- `couchdb` feature: 2/12 (DuckDB-specific tests)

## Status Notes

- Track initialized to validate end-to-end integration of QUIC transport, DHT routing, and DuckDB persistence.
- 2026-03-09: All phases complete. 12 integration tests implemented in `tests/integration_quic_dht_cas.rs`.
- DuckDB native integration deferred to future track (requires `duckdb` crate in Cargo.toml).
- Current approach uses CAS gateway as event store abstraction layer.

## Known Gaps

1. **DuckDB Native Integration** - Requires `duckdb` crate dependency
2. **Kafka Replacement Tests** - Scaffold exists but implementation deferred
3. **IPFS Live Tests** - Requires running IPFS daemon

## Next Steps

- Add `duckdb` optional feature to Cargo.toml for native DuckDB tests
- Implement Kafka replacement full flow tests
- Add IPFS daemon integration tests
