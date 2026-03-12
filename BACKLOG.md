# Literbike Development Backlog

**Generated:** 2026-02-24  
**Status:** Active

---

## Priority Legend

| Priority | Meaning | Timeline |
|----------|---------|----------|
| P0 | Critical blocker | This week |
| P1 | High priority | This month |
| P2 | Medium priority | This quarter |
| P3 | Low priority | Backlog |

---

## P0: Critical Blockers

### QUIC Implementation Gaps
**Goal:** Make QUIC streams actually work for production traffic

- [ ] **QUIC-001:** Implement TLS 1.3 handshake
  - Current: Dummy connection IDs
  - Needed: Real crypto with rustls or ring
  - Effort: 40h

- [ ] **QUIC-002:** Add ACK frame management
  - Current: Generated but not tracked
  - Needed: ACK ranges, retransmission logic
  - Effort: 20h

- [ ] **QUIC-003:** Implement congestion control
  - Current: No flow control enforcement
  - Needed: CUBIC or BBR algorithm
  - Effort: 30h

- [ ] **QUIC-004:** Add loss recovery
  - Current: No retransmission
  - Needed: PTO, FACK, packet number spaces
  - Effort: 25h

- [ ] **QUIC-005:** Write integration tests
  - Current: Zero QUIC tests
  - Needed: Client-server handshake, stream I/O
  - Effort: 15h

### Reactor Implementation
**Goal:** Complete event-driven architecture from reference spec

- [ ] **REACTOR-001:** Implement event loop
  - Current: `SimpleReactor` stub (6 lines)
  - Needed: epoll/kqueue/io_uring abstraction
  - Effort: 30h

- [ ] **REACTOR-002:** Add handler registration
  - Current: None
  - Needed: Event handler trait, dispatch table
  - Effort: 15h

- [ ] **REACTOR-003:** Implement timer wheel
  - Current: None
  - Needed: Timeout management, wheel expiration
  - Effort: 20h

---

## P1: High Priority

### Kafka Replacement Completion
**Goal:** Production-ready event log

- [ ] **KAFKA-001:** Install DuckDB native library
  - Blocked: Native lib not installed on macOS
  - Needed: `brew install duckdb` or static linking
  - Effort: 2h

- [ ] **KAFKA-002:** Run smoke tests
  - Blocked: DuckDB dependency
  - Tests: 7 tests in `kafka_replacement_smoke.rs`
  - Effort: 4h

- [ ] **KAFKA-003:** Deploy test mesh
  - Needed: 1 event log, 3 ingest nodes, 5 agents
  - Effort: 8h

### Code Cleanup

- [ ] **CLEAN-001:** Rename `patterns.rs` → `p2p_patterns.rs`
  - Reason: Scrub project-specific references
  - Effort: 1h

- [ ] **CLEAN-002:** Rename types
  - `LegacyCID` → `ContentId`
  - `LegacyBlock` → `ContentBlock`
  - `LegacyDHTService` → `DHTService`
  - Effort: 2h

- [ ] **CLEAN-003:** Delete legacy extraction report file
  - Or move to `docs/archive/`
  - Effort: 0.5h

- [ ] **CLEAN-004:** Update `.claude/agents/` configs
  - Remove project-specific references
  - Effort: 1h

### Testing Infrastructure

- [ ] **TEST-001:** Create `litecurl` binary
  - HTTP client for testing endpoints
  - Uses reqwest, supports SOCKS5
  - Effort: 8h

- [ ] **TEST-002:** Create `ipfs_client` binary
  - Standalone IPFS test client
  - Commands: add, get, pin, ls, stats
  - Effort: 8h

- [ ] **TEST-003:** Create Qwen agent configs
  - `.qwen/agents/literbike-tester.md`
  - `.qwen/agents/http-tester.md`
  - Effort: 4h

---

## P2: Medium Priority

### HTTP/3 Support

- [ ] **H3-001:** Add QPACK implementation
  - Needed for HTTP/3 header compression
  - Effort: 20h

- [ ] **H3-002:** Implement HTTP/3 framing
  - DATA, HEADERS, SETTINGS frames
  - Effort: 15h

- [ ] **H3-003:** Add web transport support
  - For browser-based agents
  - Effort: 12h

### DHT Implementation

- [ ] **DHT-001:** Complete Kademlia routing
  - Current: `RoutingTable` exists
  - Needed: FIND_NODE, GET_PROVIDERS, PUT_VALUE
  - Effort: 25h

- [ ] **DHT-002:** Add peer discovery
  - Bootstrap nodes, DHT bootstrap
  - Effort: 10h

- [ ] **DHT-003:** Implement content routing
  - Provider records, announcement
  - Effort: 15h

### IPFS Integration

- [ ] **IPFS-001:** Test existing IPFS manager
  - `src/couchdb/ipfs.rs` (501 lines)
  - Needs: Running IPFS node
  - Effort: 4h

- [ ] **IPFS-002:** Add IPFS to event log
  - Store large blocks in IPFS
  - DuckDB stores metadata only
  - Effort: 8h

---

## P3: Low Priority / Future

### Performance Optimization

- [ ] **PERF-001:** Zero-copy packet parsing
  - Use `bytes::Bytes` throughout
  - Effort: 10h

- [ ] **PERF-002:** SIMD protocol detection
  - RBCursive SIMD optimizations
  - Effort: 15h

- [ ] **PERF-003:** io_uring integration (Linux only)
  - Replace tokio with io_uring on Linux
  - Effort: 40h

### WAM Engine

- [ ] **WAM-001:** Implement WAM predicate engine
  - From `quic_wam.rs` (928 lines, mostly comments)
  - Effort: 60h

- [ ] **WAM-002:** Port strategies to WAM
  - trading strategies as WAM predicates
  - Effort: 80h

### Documentation

- [ ] **DOC-001:** API documentation
  - rustdoc for all public APIs
  - Effort: 8h

- [ ] **DOC-002:** Architecture diagrams
  - Mermaid diagrams for data flow
  - Effort: 4h

- [ ] **DOC-003:** Deployment guide
  - VPS setup, Docker, monitoring
  - Effort: 6h

---

## Completed (Reference)

### 2026-02-24

- [x] **CONC-001:** Implement CCEK context composition
  - From Kotlin patterns
  - 27 passing tests

- [x] **CONC-002:** Add channel/flow/scope modules
  - async-channel, tokio-stream integration
  - Bridge layer for Tokio interop

- [x] **CONC-003:** Port generic patterns
  - NetworkEvent, CID, DHT types
  - VectorClock, CRDT traits
  - 5 passing tests

- [x] **KAFKA-DESIGN-001:** Design Kafka replacement
  - DuckDB event log
  - QUIC stream ingest
  - Channelized distribution
  - Documentation: `KAFKA_REPLACEMENT_SMOKE_TEST.md`

- [x] **FIX-001:** Fix static scanner false positives
  - Downgraded PK checks to notes
  - `failfast_persistence_backend.py` updated

- [x] **FIX-002:** Fix literbike compilation errors
  - 17+ Rust errors fixed
  - QUIC module now compiles

- [x] **FIX-003:** Fix rbcursive recursion warnings
  - `Join` trait implementations fixed

---

## Backlog Metrics

| Category | P0 | P1 | P2 | P3 | Total |
|----------|----|----|----|----|-------|
| QUIC | 5 | - | - | - | 5 |
| Reactor | 3 | - | - | - | 3 |
| Kafka | - | 3 | - | - | 3 |
| Cleanup | - | 4 | - | - | 4 |
| Testing | - | 3 | - | - | 3 |
| HTTP/3 | - | - | 3 | - | 3 |
| DHT | - | - | 3 | - | 3 |
| IPFS | - | - | 2 | - | 2 |
| Performance | - | - | - | 3 | 3 |
| WAM | - | - | - | 2 | 2 |
| Documentation | - | - | - | 3 | 3 |
| **Total** | **8** | **10** | **8** | **8** | **34** |

**Estimated Total Effort:** 548 hours

---

## Next Sprint (Week of 2026-02-24)

### Goals
1. Install DuckDB, run smoke tests
2. Start QUIC TLS implementation
3. Begin code cleanup (project-specific scrub)

### Tasks
- [ ] KAFKA-001: Install DuckDB (2h)
- [ ] KAFKA-002: Run smoke tests (4h)
- [ ] QUIC-001: Start TLS 1.3 (8h of 40h)
- [ ] CLEAN-001: Rename patterns module (1h)
- [ ] CLEAN-002: Rename types (2h)

**Total:** 17 hours

---

## Dependencies

### External
- DuckDB native library (brew/pip)
- IPFS daemon (for IPFS tests)
- trading bot instance (for integration tests)

### Internal
- QUIC TLS depends on ring/rustls
- Reactor depends on io_uring (Linux) or kqueue (macOS)
- HTTP/3 depends on QUIC completion

---

## Risk Register

| Risk | Impact | Mitigation |
|------|--------|------------|
| QUIC complexity underestimated | High | Use quinn crate as reference |
| DuckDB native linking issues | Medium | Static linking or Docker |
| Cleanup breaks imports | Low | Comprehensive tests first |
| Reactor io_uring Linux-only | Medium | Fallback to epoll/kqueue |

---

**Last Updated:** 2026-02-24  
**Next Review:** 2026-03-03
