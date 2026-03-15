## [x] Track: GWT RequestFactory RPC over CouchDB — scaffold committed (8317414)

---

## [ ] Track: RequestFactory handler wired to CouchDB ops

Scaffold is in place (`src/request_factory/`). Wire the axum handler to real CouchDB operations.

### Scope
- `src/request_factory/handler.rs` — dispatch Find→get_document, Persist→put_document, Delete→delete_document
- `src/request_factory/changes.rs` — `_changes` feed bridge for differential push (SSE or long-poll)
- `src/couchdb/` — expose any needed public methods

### Status
- [ ] `handler.rs` dispatches all three Operation variants to couchdb module
- [ ] `changes.rs` polls `_changes` and streams diffs back to client
- [ ] `cargo check --features couchdb,request-factory --lib` clean

---

## [ ] Track: Tool loop circuit breaker — unit tests unblocked

`detect_tool_loop` committed (56e03f2) but `cargo test --lib tool_loop` blocked by pre-existing `userspace` crate errors.

### Scope
- `../userspace/src/concurrency/channels/broadcast.rs` — fix unmatched angle brackets (3 sites)
- `../userspace/src/concurrency/channels/mod.rs` — resolve Channel name collision

### Status
- [ ] userspace compiles clean
- [ ] `cargo test --lib tool_loop_tests` passes in literbike

---

## [ ] Track: Snapshot retention policy for opencode leak

Root cause identified: `~/.local/share/opencode/snapshot/` accumulates one bare git repo per session, no TTL or size cap. 22 snapshots = 408MB on one machine, scales to 200GB+ on aarch64 Macs with larger repos or busy sessions.

### Scope
- `packages/opencode/src/` — find snapshot creation code, add retention policy (keep last N, delete oldest)
- Target repo: anomalyco/opencode (advisory fork: anomalyco/opencode-ghsa-xv3r-6x54-766h)
- Branch: advisory-fix-1

### Status
- [ ] Locate snapshot write path in opencode CLI source
- [ ] Add configurable retention (default: keep 5 snapshots)
- [ ] Push to advisory-fix-1 branch

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

## [ ] Track: ModelMux snapshot/session storage via CouchDB

Replace per-request stateless ModelMux with CouchDB-backed session store. Each conversation = a CouchDB document with `_rev` for optimistic concurrency. Enables RequestFactory differential sync for UI clients.

### Status
- [ ] Session document schema in `src/couchdb/`
- [ ] ModelMux proxy writes assistant/user turns to session doc
- [ ] `_changes` feed exposes live session updates

---

## [ ] Track: GWT RequestFactory RPC over CouchDB

Implement a GWT RequestFactory-compatible RPC system in Rust backed by
literbike's existing CouchDB emulator.

### Scope
- `src/request_factory/` — new module: wire protocol, entity proxy, request context, batch handler
- `src/couchdb/` — add `_changes` subscription hook for push/differential sync
- `src/lib.rs` — expose `request_factory` module under new feature flag `request-factory`
- `Cargo.toml` — add feature `request-factory` gated on `couchdb`

### Key concepts to map
| RequestFactory | literbike |
|---|---|
| EntityProxy (id + version) | CouchDB `_id` + `_rev` |
| RequestContext (batch) | `_bulk_docs` |
| Differential sync | `_changes` feed |
| ValueProxy (no identity) | CAS blob |
| ServiceLayer | axum route handlers |

### Wire format
JSON batch envelope over HTTP POST `/_rf`:
- `invocations`: array of `{operation, entity_type, id, version, payload}`
- Response: `{results, side_effects}` with only changed fields

### Status
- [ ] `src/request_factory/mod.rs` — module skeleton + feature gate
- [ ] `src/request_factory/types.rs` — EntityId, Version, EntityProxy, ValueProxy, RequestContext traits
- [ ] `src/request_factory/wire.rs` — batch envelope serde structs
- [ ] `src/request_factory/handler.rs` — axum route `POST /_rf`, dispatches to CouchDB ops
- [ ] `src/request_factory/changes.rs` — `_changes` feed bridge for differential push
- [ ] `cargo check --features couchdb,request-factory --lib` passes

**Verification:** `cargo check --features couchdb,request-factory --lib`
