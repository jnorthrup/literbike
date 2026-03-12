# Plan: Derive Hash/Eq for CouchDB M2M Message Types

## Scope

After the accepted `src/couchdb/ipfs.rs` API update, focused `couchdb`
verification now fails first because `src/couchdb/m2m.rs` stores handlers in a
`HashMap<M2mMessageType, ...>`, but `M2mMessageType` in
`src/couchdb/types.rs` does not currently derive `Eq` and `Hash`.

## Phase 1: Repair message-type trait bounds

- [x] Keep the slice bounded to `src/couchdb/types.rs`
- [x] Add the required derives to `M2mMessageType` so it can be used as a
      `HashMap` key in the M2M handler registry
- [x] Preserve serde/schema behavior for the existing enum variants

## Phase 2: Verify

- [x] `cargo test --lib --features couchdb -- database`
- [x] Record the next remaining `couchdb` blocker after the M2M trait fix

## Progress Notes

- 2026-03-11: Focused verification after the accepted IPFS adapter repair no
  longer fails in `src/couchdb/ipfs.rs`. The first hard errors now include:
  - `src/couchdb/m2m.rs`: `HashMap` key operations on `M2mMessageType` require
    `Eq` and `Hash`
  - `src/couchdb/database.rs`: missing `sled::Tree::size_on_disk()`
  - `src/couchdb/documents.rs`: `AttachmentInfo` lacks `PartialEq`
  - `src/couchdb/api.rs`: axum handler / `SwaggerUi` version mismatch
- 2026-03-11: The M2M trait-bound repair is the narrowest next source slice,
  and the compiler help already points at adding derives on the enum in
  `src/couchdb/types.rs`.
- 2026-03-11: `qwen` completed a bounded edit in `src/couchdb/types.rs` with a
  valid rendezvous payload. Master verification confirmed:
  - `M2mMessageType` now derives `PartialEq`, `Eq`, and `Hash`
  - the original `HashMap` trait-bound errors in `src/couchdb/m2m.rs` are
    cleared
  - full `cargo test --lib --features couchdb -- database` remains blocked by
    later errors, including an adjacent M2M moved-value use in
    `src/couchdb/m2m.rs:117`
