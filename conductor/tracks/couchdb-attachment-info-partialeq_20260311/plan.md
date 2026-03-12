# Plan: Derive PartialEq for CouchDB AttachmentInfo

## Scope

Focused `couchdb` verification now fails because `src/couchdb/documents.rs`
compares `Option<HashMap<String, AttachmentInfo>>`, but `AttachmentInfo` in
`src/couchdb/types.rs` does not implement `PartialEq`.

## Phase 1: Repair attachment equality support

- [x] Keep the slice bounded to `src/couchdb/types.rs`
- [x] Add the missing `PartialEq` derive to `AttachmentInfo`
- [x] Preserve serde/schema behavior for the existing attachment fields

## Phase 2: Verify

- [x] `cargo test --lib --features couchdb -- database`
- [x] Record the next remaining `couchdb` blocker after the attachment derive fix

## Progress Notes

- 2026-03-11: Master verification confirmed the previous `database.rs` slice
  cleared the unsupported `Tree::size_on_disk()` call.
- 2026-03-11: The next emitted hard error is:
  - `src/couchdb/documents.rs:181`: `AttachmentInfo` lacks `PartialEq`
  Other active errors still remain in `views.rs`, `api.rs`, and
  `cas_backends.rs`.
- 2026-03-11: `qwen` completed a bounded edit in `src/couchdb/types.rs` with a
  valid rendezvous payload. Master verification confirmed:
  - `AttachmentInfo` now derives `PartialEq`
  - the document-equality compile error is gone
  - focused verification now fails next in `src/couchdb/views.rs` on direct
    private-field access to `DatabaseInstance.sequence_counter`
