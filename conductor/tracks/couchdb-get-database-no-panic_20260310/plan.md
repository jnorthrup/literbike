# Plan: Replace CouchDB get_database Panic with Structured Error

## Scope

`src/couchdb/database.rs` still contains the repo's only `unimplemented!()`.
`DatabaseManager::get_database()` returns a `CouchResult`, so it can fail
truthfully without hard-panicking the process.

## Phase 1: Remove the panic

- [x] Replace `unimplemented!("Use get_database_clone instead")` with a
  structured `CouchError`
- [x] Keep the existing `get_database_clone()` API as the preferred safe path
- [x] Add or update focused coverage so this method returns an error instead of
  panicking

## Phase 2: Verify

- [ ] `cargo test --lib couchdb`
- [ ] Confirm there are no remaining `unimplemented!()` sites in `src/` or `tests/`

## Progress Notes

- 2026-03-10: Repo-local evidence:
  - `src/couchdb/database.rs:138` is the only remaining `unimplemented!()` in
    the repository
  - all in-repo call sites already use `get_database_clone()`
  - `CouchError::internal_server_error(...)` already exists and fits this case
- 2026-03-10: First `claude` launch on this bounded corpus failed closed. After
  the monitoring timeout there was still no `src/couchdb/database.rs` diff, no
  rendezvous payload, and the runtime was only holding the Cargo build lock.
  The slice must be rerouted.
- 2026-03-10: `codex exec` is not usable as a delegate on this host right now.
  The runtime exits immediately with a config parse failure:
  `model_reasoning_effort = xhigh` is unsupported. The slice must use a
  different runtime unless the host config is repaired.
- 2026-03-10: Repo truth correction: this track was already satisfied before
  delegation. `src/couchdb/database.rs` now returns
  `CouchError::internal_server_error("get_database is not supported; call get_database_clone instead")`
  and contains `test_get_database_returns_error_not_panic`. Focused verification
  is blocked only by broader `couchdb` feature compile failures outside this
  file, so follow-on work moved to the dependency-wiring track.
