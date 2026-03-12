# Plan: Replace CouchDB Views Direct sequence_counter Access

## Scope

Focused `couchdb` verification now fails in `src/couchdb/views.rs` because
`compile_view()` reads `db.sequence_counter` directly even though that field is
private to `DatabaseInstance`.

## Phase 1: Repair the private-field access

- [x] Keep the slice bounded to `src/couchdb/views.rs`
- [x] Replace the direct `sequence_counter` field access in `compile_view()`
      with a truthful same-file alternative
- [x] Preserve the current `CompiledView.last_seq` / `ViewIndexEntry.doc_seq`
      behavior as closely as the available public data allows

## Phase 2: Verify

- [x] `cargo test --lib --features couchdb -- database`
- [x] Record the next remaining `couchdb` blocker after the views sequence fix

## Progress Notes

- 2026-03-11: Master verification confirmed the `AttachmentInfo` derive fix is
  real and the document-equality error is gone.
- 2026-03-11: The next emitted hard error is:
  - `src/couchdb/views.rs:259`: private access to
    `DatabaseInstance.sequence_counter`
  The same file still has later `BTreeMap<serde_json::Value, ...>` ordering
  errors after this access is repaired.
- 2026-03-11: `qwen` completed a bounded edit in `src/couchdb/views.rs` with a
  valid rendezvous payload. Master verification confirmed:
  - `compile_view()` now derives `current_seq` from `all_docs.update_seq`
    instead of direct private-field access
  - the `sequence_counter` privacy error is gone
  - focused verification now fails later in the same file on
    `BTreeMap<serde_json::Value, ...>` requiring `Ord`
