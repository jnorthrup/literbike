# Track: RequestFactory handler wired to CouchDB ops

Scaffold committed (8317414). Wire axum handler to real CouchDB operations.

## Scope
- `src/request_factory/handler.rs` — dispatch Find→get_document, Persist→put_document, Delete→delete_document
- `src/request_factory/changes.rs` — `_changes` feed bridge (SSE or long-poll)
- `src/couchdb/` — expose needed public methods if private

## Verification
`cargo check --features couchdb,request-factory --lib`

## Status
- [ ] handler.rs dispatches all three Operation variants
- [ ] changes.rs polls _changes and streams diffs
- [ ] cargo check clean
