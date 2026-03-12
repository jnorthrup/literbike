# Plan: Repair CouchDB git_sync API Drift

## Scope

After the dependency and reducer fixes, the next hard `couchdb` compile blocker
is localized to `src/couchdb/git_sync.rs`, which still targets an older API
surface.

## Phase 1: Local API drift repair

- [x] Replace the nonexistent `CouchDatabase` type usage with the current
  database surface or otherwise make the drift explicit and compile-clean
- [x] Replace invalid `CouchError::Internal(...)` constructors with the current
  `CouchError::internal_server_error(...)` API
- [x] Keep the slice bounded to `src/couchdb/git_sync.rs`

## Phase 2: Verify

- [ ] `cargo test --lib --features couchdb -- database`
- [ ] Record the next blocker after `git_sync.rs` is compile-clean

## Progress Notes

- 2026-03-10: After the manifest and `views.rs` fixes, focused `couchdb`
  verification now fails first on `src/couchdb/git_sync.rs`:
  - import of nonexistent `crate::couchdb::database::CouchDatabase`
  - repeated use of nonexistent `CouchError::Internal(...)`
- 2026-03-10: `git_sync.rs` appears to depend on async document operations
  (`create_document`, `update_document`, `find_documents`, etc.) that do not
  live on `DatabaseManager`, so the worker must repair or truthfully narrow that
  local integration surface without guessing across the entire `couchdb` stack.
- 2026-03-10: `qwen` failed to emit a rendezvous payload before timeout, but
  master verification confirmed the final `src/couchdb/git_sync.rs` diff is
  bounded to this file and removes the hard compile blockers there by:
  - replacing `CouchDatabase` with a local `DatabaseInstance`-backed surface
  - replacing `CouchError::Internal(...)` with `internal_server_error(...)`
  - making unmappable async document-query drift explicit as runtime errors
  Focused `couchdb` verification now reaches the next blocker in the tensor API
  response type.
