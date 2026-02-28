# Qwen Agent: Kafka Replacement Team

## Assignment
Complete Kafka replacement with DuckDB event log.

## Branches
- `kafka/p1-duckdb-install` - Native library setup
- `kafka/p1-smoke-tests` - Run 7 smoke tests
- `kafka/p1-test-mesh` - Deploy test mesh

## Priority
**P1 - High** (Enables distributed bot coordination)

---

## Task 1: DuckDB Installation

**Branch:** `kafka/p1-duckdb-install`

**Actions:**

**macOS:**
```bash
brew install duckdb
# Verify
duckdb --version
```

**Or use Python bindings (includes native lib):**
```bash
pip install duckdb
```

**Or static linking:**
```toml
# Cargo.toml
[dependencies]
duckdb = { version = "1.0", features = ["bundled"] }
```

**Test:**
```bash
# Run smoke test
cargo test --features quic test_duckdb_event_log
```

---

## Task 2: Smoke Tests

**Branch:** `kafka/p1-smoke-tests`

**Tests to pass:**
1. `test_duckdb_event_log` - Basic append/read
2. `test_quic_stream_ingest` - Producer + broadcast
3. `test_channelized_distributor` - Multi-channel broadcast
4. `test_pandas_edge_agent` - Consumer processing
5. `test_kafka_replacement_full_flow` - End-to-end
6. `test_replay_from_offset` - Kafka consumer seek equivalent
7. `test_duckdb_query_filter` - Kafka Streams filtering

**Run all:**
```bash
cargo test --features quic kafka_replacement_smoke
```

**Expected output:**
```
test result: ok. 7 passed; 0 failed
```

---

## Task 3: Test Mesh Deployment

**Branch:** `kafka/p1-test-mesh`

**Deployment:**
```bash
# Terminal 1: Event log
./target/debug/kafka_event_log --db user_data/market_ticks.duckdb

# Terminal 2-4: Ingest nodes
./target/debug/kafka_ingest --addr 127.0.0.1:9001
./target/debug/kafka_ingest --addr 127.0.0.1:9002
./target/debug/kafka_ingest --addr 127.0.0.1:9003

# Terminal 5-9: Edge agents
./target/debug/pandas_agent --id agent-1
./target/debug/pandas_agent --id agent-2
./target/debug/pandas_agent --id agent-3
./target/debug/pandas_agent --id agent-4
./target/debug/pandas_agent --id agent-5
```

**Verify:**
```bash
# Check event log
duckdb user_data/market_ticks.duckdb "SELECT COUNT(*) FROM market_ticks"

# Check agent processing
curl http://localhost:8081/agent-1/stats
```

---

## Success Criteria

- [ ] DuckDB native library installed
- [ ] All 7 smoke tests pass
- [ ] Test mesh runs with 3 ingest nodes + 5 agents
- [ ] Event replay works from any offset
- [ ] Backpressure prevents memory exhaustion

---

## Merge Order

1. `kafka/p1-duckdb-install` → master
2. `kafka/p1-smoke-tests` → master (depends on duckdb-install)
3. `kafka/p1-test-mesh` → master (depends on smoke-tests)

---

## Dependencies

- DuckDB native library
- Cleanup branches (for consistent imports)
- QUIC TLS branch (for secure transport)

---

**Created:** 2026-02-24  
**Status:** Ready to start (after DuckDB install)
