# Plan: Fix CouchDB M2M Handler Registration Move-After-Insert

## Scope

After `M2mMessageType` in `src/couchdb/types.rs` gained the missing
`PartialEq`/`Eq`/`Hash` derives, the next adjacent M2M compile blocker is in
`src/couchdb/m2m.rs`: `register_handler()` moves `message_type` into the
handler map and then immediately tries to borrow it again for logging.

## Phase 1: Repair the moved-value use

- [x] Keep the slice bounded to `src/couchdb/m2m.rs`
- [x] Repair `register_handler()` so the handler insert and log statement do not
      borrow `message_type` after move
- [x] Preserve the existing registration log message semantics

## Phase 2: Verify

- [x] `cargo test --lib --features couchdb -- database`
- [x] Record the next remaining `couchdb` blocker after the M2M handler fix

## Progress Notes

- 2026-03-11: Master verification confirmed the bounded `src/couchdb/types.rs`
  derive fix is real:
  - `M2mMessageType` now derives `PartialEq`, `Eq`, and `Hash`
  - the original `HashMap` key-bound errors in `src/couchdb/m2m.rs` are gone
- 2026-03-11: Focused `couchdb` verification still fails in multiple modules.
  The adjacent M2M follow-on is:
  - `src/couchdb/m2m.rs:117`: borrow of moved `message_type` after
    `handlers.insert(message_type, ...)`
  Other active errors also remain in `database.rs`, `documents.rs`, `views.rs`,
  and `api.rs`.
- 2026-03-11: `qwen` completed a bounded edit in `src/couchdb/m2m.rs` with a
  valid rendezvous payload. Master verification confirmed:
  - `register_handler()` now logs `message_type` before moving it into the
    handler map
  - the `E0382` moved-value error is cleared
  - focused verification still fails later, with the next emitted hard error in
    `src/couchdb/database.rs` on unsupported `Tree::size_on_disk()`
