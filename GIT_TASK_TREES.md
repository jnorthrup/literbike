# Literbike Git Task Tree Strategy

**Purpose:** Divide and conquer backlog items using parallel git branches

**Created:** 2026-02-24

---

## Branch Naming Convention

```
<category>/<priority>-<short-description>
```

**Categories:**
- `quic` - QUIC protocol implementation
- `reactor` - Event loop / reactor pattern
- `kafka` - Kafka replacement / event log
- `cleanup` - Code cleanup, renaming
- `test` - Testing infrastructure
- `http3` - HTTP/3 implementation
- `dht` - Distributed hash table
- `ipfs` - IPFS integration
- `perf` - Performance optimizations
- `wam` - WAM engine
- `docs` - Documentation

**Priority Markers:**
- `p0-` - Critical blocker
- `p1-` - High priority
- `p2-` - Medium priority
- `p3-` - Low priority

---

## Active Task Trees (P0/P1)

### QUIC Implementation

```
quic/p0-tls-handshake          # TLS 1.3 with rustls
quic/p0-ack-management         # ACK frames, retransmission
quic/p0-congestion-control     # CUBIC/BBR algorithm
quic/p0-loss-recovery          # PTO, FACK, packet spaces
quic/p0-integration-tests      # Client-server tests
```

**Base:** `master`  
**Dependencies:** None (parallel work possible)  
**Merge Order:** tls-handshake → ack-management → congestion-control → loss-recovery → integration-tests

### Reactor Implementation

```
reactor/p0-event-loop          # epoll/kqueue/io_uring abstraction
reactor/p0-handler-registration # Event handler trait, dispatch
reactor/p0-timer-wheel         # Timeout management
```

**Base:** `master`  
**Dependencies:** None  
**Merge Order:** event-loop → handler-registration → timer-wheel

### Kafka Replacement

```
kafka/p1-duckdb-install        # Native library setup
kafka/p1-smoke-tests           # Run 7 smoke tests
kafka/p1-test-mesh             # Deploy test mesh
```

**Base:** `master`  
**Dependencies:** duckdb-install → smoke-tests → test-mesh

### Code Cleanup

```
cleanup/p1-rename-patterns     # patterns.rs → p2p_patterns.rs (scrub project refs)
cleanup/p1-rename-types        # BetanetCID → ContentId, etc.
cleanup/p1-archive-docs        # Move extraction report
cleanup/p1-update-agents       # Update .claude/agents/ configs
```

**Base:** `master`  
**Dependencies:** None (can be done in parallel)  
**Note:** Should be done early to avoid conflicts

### Testing Infrastructure

```
test/p1-litecurl               # HTTP client binary
test/p1-ipfs-client            # IPFS client binary
test/p1-qwen-agents            # Qwen agent test configs
```

**Base:** `master`  
**Dependencies:** None

---

## Future Task Trees (P2/P3)

### HTTP/3

```
http3/p2-qpack                 # Header compression
http3/p2-framing               # DATA, HEADERS, SETTINGS
http3/p2-web-transport         # Browser support
```

**Base:** `quic/p0-integration-tests` (depends on QUIC completion)

### DHT

```
dht/p2-kademlia-routing        # FIND_NODE, GET_PROVIDERS
dht/p2-peer-discovery          # Bootstrap, DHT bootstrap
dht/p2-content-routing         # Provider records
```

**Base:** `cleanup/p1-rename-types` (uses renamed types)

### IPFS

```
ipfs/p2-test-manager           # Test existing IPFS manager
ipfs/p2-event-log-integration  # IPFS + DuckDB hybrid
```

**Base:** `test/p1-ipfs-client`

### Performance

```
perf/p3-zero-copy              # bytes::Bytes throughout
perf/p3-simd-detection         # RBCursive SIMD
perf/p3-io-uring               # Linux io_uring backend
```

**Base:** `master` (can be done anytime)

### WAM Engine

```
wam/p3-predicate-engine        # WAM implementation
wam/p3-strategy-port           # Port trading strategies
```

**Base:** `master` (independent)

### Documentation

```
docs/p3-api-docs               # rustdoc coverage
docs/p3-architecture             # Mermaid diagrams
docs/p3-deployment             # Deployment guide
```

**Base:** `master` (independent)

---

## Branch Creation Commands

### P0 Branches (Create Immediately)

```bash
# QUIC branches
git checkout -b quic/p0-tls-handshake master
git checkout -b quic/p0-ack-management master
git checkout -b quic/p0-congestion-control master
git checkout -b quic/p0-loss-recovery master
git checkout -b quic/p0-integration-tests master

# Reactor branches
git checkout -b reactor/p0-event-loop master
git checkout -b reactor/p0-handler-registration master
git checkout -b reactor/p0-timer-wheel master

# Cleanup branches (do first to avoid conflicts)
git checkout -b cleanup/p1-rename-patterns master
git checkout -b cleanup/p1-rename-types master
git checkout -b cleanup/p1-archive-docs master
git checkout -b cleanup/p1-update-agents master
```

### P1 Branches (Create After Cleanup)

```bash
# Kafka branches
git checkout -b kafka/p1-duckdb-install master
git checkout -b kafka/p1-smoke-tests kafka/p1-duckdb-install
git checkout -b kafka/p1-test-mesh kafka/p1-smoke-tests

# Testing branches
git checkout -b test/p1-litecurl master
git checkout -b test/p1-ipfs-client master
git checkout -b test/p1-qwen-agents master
```

---

## Merge Strategy

### Phase 1: Cleanup (Week 1)
```
cleanup/p1-rename-patterns     → master
cleanup/p1-rename-types        → master
cleanup/p1-archive-docs        → master
cleanup/p1-update-agents       → master
```

### Phase 2: Foundation (Week 2-3)
```
quic/p0-tls-handshake          → master
reactor/p0-event-loop          → master
kafka/p1-duckdb-install        → master
test/p1-litecurl               → master
test/p1-ipfs-client            → master
```

### Phase 3: Integration (Week 4-6)
```
quic/p0-ack-management         → master
quic/p0-congestion-control     → master
reactor/p0-handler-registration → master
reactor/p0-timer-wheel         → master
kafka/p1-smoke-tests           → master
test/p1-qwen-agents            → master
```

### Phase 4: Completion (Week 7-8)
```
quic/p0-loss-recovery          → master
quic/p0-integration-tests      → master
kafka/p1-test-mesh             → master
```

---

## Conflict Prevention

### High-Risk Files

| File | Affected Branches | Mitigation |
|------|------------------|------------|
| `src/lib.rs` | cleanup, quic, reactor | Cleanup merges first |
| `Cargo.toml` | all branches | Coordinate dependency additions |
| `src/concurrency/mod.rs` | cleanup, test | Cleanup merges first |
| `src/betanet_patterns.rs` | cleanup, dht, ipfs | Rename immediately |

### Merge Coordination

```bash
# Before starting work each day:
git checkout master
git pull origin master
git rebase master <your-branch>

# Before merging to master:
1. Ensure all tests pass
2. Rebase on latest master
3. Request review from team
4. Squash commits if needed
5. Merge with --no-ff for visibility
```

---

## Progress Tracking

### Branch Status Board

| Branch | Status | PR # | Assignee | ETA |
|--------|--------|------|----------|-----|
| cleanup/p1-rename-patterns | 🟢 Ready | - | - | 2026-02-24 |
| cleanup/p1-rename-types | 🟢 Ready | - | - | 2026-02-24 |
| cleanup/p1-archive-docs | 🟢 Ready | - | - | 2026-02-24 |
| cleanup/p1-update-agents | 🟢 Ready | - | - | 2026-02-24 |
| quic/p0-tls-handshake | ⚪ Pending | - | - | 2026-02-25 |
| quic/p0-ack-management | ⚪ Pending | - | - | 2026-02-26 |
| reactor/p0-event-loop | ⚪ Pending | - | - | 2026-02-25 |
| kafka/p1-duckdb-install | ⚪ Pending | - | - | 2026-02-24 |

**Legend:** 🟢 Ready | 🟡 In Progress | 🔴 Blocked | ⚪ Not Started

---

## Qwen Agent Delegation

### Agent Assignments

```yaml
agents:
  quic-team:
    branches:
      - quic/p0-tls-handshake
      - quic/p0-ack-management
      - quic/p0-congestion-control
    context: .qwen/agents/quic-impl.md
    
  reactor-team:
    branches:
      - reactor/p0-event-loop
      - reactor/p0-handler-registration
      - reactor/p0-timer-wheel
    context: .qwen/agents/reactor-impl.md
    
  cleanup-team:
    branches:
      - cleanup/p1-rename-patterns
      - cleanup/p1-rename-types
    context: .qwen/agents/code-cleanup.md
    
  test-team:
    branches:
      - test/p1-litecurl
      - test/p1-ipfs-client
      - test/p1-qwen-agents
    context: .qwen/agents/test-infra.md
    
  kafka-team:
    branches:
      - kafka/p1-duckdb-install
      - kafka/p1-smoke-tests
    context: .qwen/agents/kafka-replacement.md
```

### Agent Context Template

```markdown
# Agent Context: <branch-name>

## Goal
<One sentence description>

## Files to Modify
- `path/to/file1.rs`
- `path/to/file2.rs`

## Success Criteria
- [ ] Compiles without errors
- [ ] All tests pass
- [ ] No new clippy warnings
- [ ] Documentation updated

## Dependencies
- Depends on: <branch-name>
- Blocks: <branch-name>

## Test Commands
```bash
cargo test --features quic <test_name>
cargo clippy --features quic
```
```

---

## Automation

### CI/CD Integration

```yaml
# .github/workflows/task-tree-ci.yml
name: Task Tree CI

on:
  push:
    branches:
      - 'quic/p0-*'
      - 'reactor/p0-*'
      - 'kafka/p1-*'
      - 'cleanup/p1-*'
      - 'test/p1-*'

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run tests
        run: cargo test --features quic
      - name: Run clippy
        run: cargo clippy --features quic -- -D warnings
```

### Branch Status Script

```bash
#!/bin/bash
# scripts/branch-status.sh

echo "=== Active Task Trees ==="
for branch in $(git branch | grep -E 'quic|reactor|kafka|cleanup|test'); do
    echo ""
    echo "📍 $branch"
    git log --oneline -1 $branch
done

echo ""
echo "=== Branches Behind Master ==="
for branch in $(git branch | grep -v master); do
    behind=$(git rev-list --count master..$branch 2>/dev/null || echo 0)
    if [ "$behind" -gt 0 ]; then
        echo "⚠️  $branch is $behind commits behind master"
    fi
done
```

---

## Review Checklist

### Before Creating Branch

- [ ] Branch name follows convention
- [ ] Base branch is correct (usually master)
- [ ] No conflicting work in progress
- [ ] Agent context document created

### Before Merging to Master

- [ ] All tests pass
- [ ] Clippy warnings resolved
- [ ] Documentation updated
- [ ] BACKLOG.md updated
- [ ] Reviewed by team member
- [ ] Squashed commits (if appropriate)
- [ ] Merge commit message follows convention

---

## Rollback Plan

If a branch causes issues after merge:

```bash
# Quick revert
git revert <merge-commit-hash>
git push origin master

# Or reset to before merge (if no other work)
git reset --hard HEAD~1
git push --force origin master
```

**Critical branches to monitor:**
1. `cleanup/p1-rename-types` - Affects many files
2. `quic/p0-tls-handshake` - Core functionality
3. `kafka/p1-smoke-tests` - Adds DuckDB dependency

---

**Last Updated:** 2026-02-24  
**Next Review:** After cleanup branches merge
