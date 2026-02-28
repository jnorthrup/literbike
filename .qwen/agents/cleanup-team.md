# Qwen Agent: Code Cleanup Team

## Assignment
Work on cleanup branches to scrub betanet references and prepare codebase for production.

## Branches
- `cleanup/p1-rename-patterns` - Rename betanet_patterns.rs → p2p_patterns.rs
- `cleanup/p1-rename-types` - Rename types (BetanetCID → ContentId, etc.)
- `cleanup/p1-archive-docs` - Move extraction report to archive
- `cleanup/p1-update-agents` - Update .claude/agents/ configs

## Priority
**P1 - High** (Must complete before other work to avoid conflicts)

---

## Task 1: Rename patterns module

**Branch:** `cleanup/p1-rename-patterns`

**Files to modify:**
```bash
git mv src/betanet_patterns.rs src/p2p_patterns.rs
```

**Update references:**
- `src/lib.rs` - Change `pub mod betanet_patterns` → `pub mod p2p_patterns`
- `src/concurrency/ccek.rs` - Update doc comments
- Any other files importing `betanet_patterns`

**Test:**
```bash
cargo test --features quic p2p_patterns
```

---

## Task 2: Rename types

**Branch:** `cleanup/p1-rename-types`

**Type renames:**
```rust
// In src/p2p_patterns.rs
BetanetCID → ContentId
BetanetBlock → ContentBlock
BetanetMultihash → ContentHash
BetanetLink → ContentLink
BetanetDHTService → DHTService
BetanetRoutingTable → RoutingTable
CRDTStorageService → StorageService
CRDTNetworkService → NetworkService
```

**Update all usages:**
- `src/betanet_patterns.rs` (after rename)
- `src/concurrency/ccek.rs`
- Doc comments throughout

**Test:**
```bash
cargo test --features quic
cargo clippy --features quic
```

---

## Task 3: Archive documentation

**Branch:** `cleanup/p1-archive-docs`

**Actions:**
```bash
mkdir -p docs/archive
mv BETANET_EXTRACTION_REPORT.md docs/archive/
```

**Update references:**
- Remove betanet references from README.md
- Update BACKLOG.md if needed

---

## Task 4: Update agent configs

**Branch:** `cleanup/p1-update-agents`

**Files to modify:**
- `.claude/agents/rust-betanet-densifier.md` → `.claude/agents/rust-densifier.md`
- `.claude/agents/pragmatist.md` - Remove betanet reference

**Content changes:**
Replace "Betanet" with "Distributed" or "P2P" in:
- Agent descriptions
- Task descriptions
- Examples

---

## Success Criteria

- [ ] All tests pass: `cargo test --features quic`
- [ ] No clippy warnings: `cargo clippy --features quic`
- [ ] No "betanet" in source files (except archive)
- [ ] Documentation updated
- [ ] All branches merge cleanly to master

---

## Merge Order

1. `cleanup/p1-rename-patterns` → master
2. `cleanup/p1-rename-types` → master (depends on patterns rename)
3. `cleanup/p1-archive-docs` → master
4. `cleanup/p1-update-agents` → master

---

## Commands

```bash
# Start work
git checkout cleanup/p1-rename-patterns

# After changes
git add -A
git commit -m "refactor: Rename betanet_patterns to p2p_patterns"
git push -u origin cleanup/p1-rename-patterns

# Create PR (GitHub)
# Request review
# Merge after approval
```

---

**Created:** 2026-02-24  
**Status:** Ready to start
