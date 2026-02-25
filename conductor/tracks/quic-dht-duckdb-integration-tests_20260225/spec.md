# Spec: Integration Tests (End-to-End QUIC + DHT + DuckDB)

## Overview

This track adds integration tests that exercise cross-module behavior rather than
unit-only correctness: QUIC transport handling, DHT/IPFS operations, and DuckDB
persistence/audit behavior in combined scenarios.

## Problem

- `literbike` currently has unit/feature-level tests in:
  - `/Users/jim/work/literbike/tests/quic/`
  - `/Users/jim/work/literbike/tests/duckdb/`
- There is no end-to-end harness validating interactions across QUIC, DHT/IPFS,
  and DuckDB persistence semantics.
- Kafka-removal/local-persistence architecture goals need deterministic proof at
  integration-test level.

## Current Test Context

- `/Users/jim/work/literbike/tests/quic/test_engine.rs`
- `/Users/jim/work/literbike/tests/quic/test_stream_ingestion.rs`
- `/Users/jim/work/literbike/tests/duckdb/test_basic_operations.rs`
- `/Users/jim/work/literbike/tests/duckdb/test_kafka_compatibility.rs`

## Target Scope

- Add a new integration harness (e.g. `/Users/jim/work/literbike/tests/integration/`)
  or equivalent test module layout.
- Reuse existing QUIC and DuckDB fixtures where possible.
- Integrate DHT/IPFS tests as the IPFS port track lands (feature-gated where needed).

## Functional Requirements

- Add deterministic end-to-end test scenarios covering:
  - QUIC packet/stream ingestion -> application event creation
  - DHT/IPFS lookup/store routing path (or mocked seam if IPFS track incomplete)
  - DuckDB persistence/audit append/read verification
- Cover failure-path behavior:
  - QUIC parse/protocol errors
  - DHT/IPFS misses/timeouts (or mocked equivalents)
  - DuckDB write/read failure propagation
- Add explicit fallback behavior assertions where applicable (e.g. transport mode
  or mocked fallback path semantics).
- Keep tests runnable in local dev without external infrastructure.

## Non-Functional Requirements

- Deterministic and CI-friendly (no external network dependency by default)
- Feature-gated where modules are optional (`quic`, `ipfs`)
- Avoid flaky timing assumptions; use bounded waits and explicit synchronization

## Acceptance Criteria

1. A new integration-test harness exists and runs alongside existing tests.
2. At least one passing end-to-end scenario covers QUIC + DuckDB, with a clear
   extension point for DHT/IPFS.
3. Feature-gated scenarios cover QUIC + DHT/IPFS + DuckDB once the IPFS track is available.
4. Failure-path tests verify error propagation and deterministic cleanup.

## Out of Scope

- Benchmark/performance testing
- External multi-node network/system tests
- `io_uring`-specific validation

