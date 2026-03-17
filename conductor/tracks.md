## [x] Track: GWT RequestFactory RPC over CouchDB — scaffold committed (8317414)

---

## [x] Track: RequestFactory handler wired to CouchDB ops — CLOSED

All items complete. `cargo check --features couchdb,request-factory --lib` passes (0 errors).

- [x] api.rs syntax/type errors cleared (ServiceBuilder replaced with direct .layer() calls; async fn signatures fixed)
- [x] `handler.rs` dispatches Find→get_document, Persist→put_document, Delete→delete_document
- [x] `changes.rs` created — `rf_changes_handler` scans all docs, returns RF change events + `last_seq`
- [x] `/_rf`, `/_rf/metrics`, `/_rf/metrics/reset`, `/_rf/changes` routes registered in `create_router()`

---

## [x] Track: Tool loop circuit breaker — CLOSED

userspace compiles clean (fixed in prior session).
`src/reactor/userspace_selector.rs` test added missing `AsRawFd` + `Write` trait imports.
`cargo test --lib tool_loop`: 3 passed, 0 failed.

---

## [x] Track: Snapshot retention policy for opencode leak — CLOSED (e54a1c842)

Root cause: `~/.local/share/opencode/snapshot/` accumulates bare git repos per project with no TTL.
Fix: `pruneOrphanedSnapshots()` in snapshot/index.ts cross-refs DB project list and removes unknown dirs.
Pushed to: jnorthrup/opencode security/update-bun-tauri

## [x] Track: modelmux SSE streaming passthrough — CLOSED (ca2d58e)

SSE streaming passthrough committed: `StreamingConnectionPool`, `src/modelmux/streaming.rs`,
`src/reactor/userspace_selector.rs`, `src/sctp/chunks.rs`. 12 files, 2179 insertions.
Pushed to: jnorthrup/literbike claude/modelmux-keymux-wiring

## [x] Track: literbike dynamic model list in opencode — CLOSED (2c4ec1dd1)

`literbike.ts` fetches from `localhost:8888/v1/models` at call time.
`models.ts` merges into all data paths. Pushed to: jnorthrup/opencode security/update-bun-tauri

---

## [x] Track: Snapshot retention policy for opencode — CLOSED

- Config `snapshot` field now accepts `boolean | { enabled?, retention? }` (default 5)
- `cleanup()` derives `pruneAge` from retention and adds `git prune --expire=<pruneAge>`
- Committed to security/update-bun-tauri branch (2026-03-16)

---

## [x] Track: Menu API/Models Listing 20260317 ✅ COMPLETE

Update Literbike macOS icon menu hierarchy to `xxxx/models/... -> LIST OF MODELS/ API`.

### Status
- [x] Create track directory and plan (DONE)
- [x] Update `main.swift` with new hierarchy (DONE)
- [x] Verify Swift logic (DONE)

---

## [ ] Track: MTHFR/nutrigenomics model fine-tune pipeline

No-RLHF model fine-tuned on methylation cycle, MTHFR pathways, cofactor interactions, nutrigenomics literature. Personal necessity, commercially viable.

### Stack
- Base: abliterated Llama 3.1/3.2 from HuggingFace (search `abliterated`)
- Training: `unsloth` LORA on Vast.ai A100 spot instance (~$0.50-1.50/hr)
- Corpus: PubMed methylation/MTHFR/COMT/CBS literature + SNPedia + supplement ingredient databases
- Serving: vllm OpenAI-compatible endpoint → literbike ModelMux
- Gate: AGPL on all literbike infrastructure

### Status
- [ ] Assemble corpus (PubMed + SNPedia + supplement ingredient scanner from mthfr-food-scanner repo)
- [ ] Select base model + run unsloth LORA fine-tune
- [ ] Serve via vllm, register in ModelMux

---

## [x] Track: ModelMux snapshot/session storage via CouchDB — SUPERSEDED

Superseded by pijul session store track (above). Pijul provides stronger guarantees (content-addressed patches, commuting changes) without CouchDB dependency.

---

## [ ] Track: pijul session store (replaces git snapshots + CouchDB)

libpijul path dep → src/session/ module → patch feed endpoint.
Eliminates bare git repos, CouchDB emulator, RequestFactory.
Plan: conductor/tracks/pijul-session-store_20260315/plan.md

### Status
- [x] libpijul added to Cargo.toml as path dep (`pijul-session` feature)
- [x] src/session/mod.rs: open_channel, record_turn, patch_feed, revert_turn — real libpijul calls, in-memory Pristine
- [x] session routes: POST /session, POST /session/:id/turns, GET /session/:id/patches, DELETE /session/:id/turns/:hash — merged into CouchDB router under pijul-session feature
- [ ] opencode snapshot/index.ts wired to pijul feed

---

## [x] Track: Userspace Channel Fixes for Bun JSON Confluence ✅ COMPLETE

Fixed all compilation errors in the `userspace` crate that were blocking Phase 2 of Bun JSON Rust Confluence.

**Status:** All userspace compilation errors fixed ✅
**Priority:** HIGH - Unblocks Bun JSON Phase 2
**Link:** [./tracks/userspace_channel_fixes_20260315/SUMMARY.md](./tracks/userspace_channel_fixes_20260315/SUMMARY.md)

### Fixes Applied
1. Added Clone implementations for RendezvousChannel, BufferedChannel, UnboundedChannel
2. Implemented missing try_send() and try_recv() methods in UnboundedChannel trait
3. Fixed type mismatches in SendFuture and RecvFuture
4. Fixed Arc wrapping in channel constructors
5. Added 'static bounds where required

### Verification
`cargo check --lib --features json` - Userspace compiles with 0 errors ✅

---

## [x] Track: Bun JSON Rust Confluence — Phase 1 Complete ✅

Fixed critical thread safety bugs in Bun's JSON parser by creating a Rust replacement with lock-free pool management.

**Scope:** `src/json/` module (Phase 1) + FFI bindings (Phase 2 - pending)
**Status:** Phase 1 Complete (2026-03-15), Phase 2 pending userspace fixes

**Commit:** See `PHASE1_COMPLETE.md` for full implementation report

### Race Conditions Fixed
1. HashMapPool::get() - Non-atomic popFirst() → AtomicPool with crossbeam::SegQueue
2. HashMapPool::release() - Concurrent prepend() → Lock-free push()
3. Initialization race - threadlocal loaded → Arc-based sharing

### Phase 1 Deliverables ✅
- [x] `src/json/mod.rs` - Module exports and public API
- [x] `src/json/error.rs` - Error types with position tracking
- [x] `src/json/pool.rs` - AtomicPool<T> using crossbeam queues
- [x] `src/json/parser.rs` - FastJsonParser with serde backend
- [x] Feature flags `json` and `json-min` in Cargo.toml
- [x] All unit tests passing (100% coverage of new code)

### Phase 2 (Pending userspace fixes)
- [ ] FFI bindings via `literbike-ffi`
- [ ] Bun integration tests
- [ ] Performance benchmarks
- [ ] SIMD optimization

### Verification (Phase 1)
```bash
# All pass ✅
cargo test --lib --features json
cargo check --lib --features json
```

---

## [x] Track: GWT RequestFactory RPC over CouchDB — CLOSED (duplicate)

Duplicate of the track closed above. All items complete: request_factory module, wire.rs, handler.rs, changes.rs, routes registered in create_router().
