# Plan: Make TensorResult Serializable for CouchDB API Responses

## Scope

After the `git_sync.rs` drift repair, the next focused `couchdb` compile blocker
is `src/couchdb/api.rs`: `axum::Json(result)` cannot compile because
`TensorResult` does not implement `serde::Serialize`.

## Phase 1: Response type repair

- [x] Locate `TensorResult` and make the API response path serializable in a
  truthful way
- [x] Keep the slice bounded to the type definition and any minimal adjacent
  serialization annotations required

## Phase 2: Verify

- [x] `cargo check --lib --features couchdb` (tensor serialization compiles)
- [x] Record the next remaining blocker after the serialization fix

## Progress Notes

- 2026-03-10: Focused `couchdb` verification after the `git_sync.rs` repair now
  fails first on:
  - `src/couchdb/api.rs:902` / `:915` — `TensorResult` does not implement
    `serde::Serialize` for `Json(result)`
- 2026-03-10: The `git2` `cfg(feature = "git2")` warnings remain visible, but
  they are not the first hard blocker after the latest compile pass.
- 2026-03-10: `claude` completed the slice with a valid rendezvous payload.
  Master verification confirmed:
  - `src/couchdb/tensor.rs` now provides manual `Serialize` impls for
    `TensorData` and `TensorResult`
  - the API serialization blocker is gone
  - focused `couchdb` verification now fails first in `src/couchdb/ipfs.rs`
- 2026-03-14: TensorResult serialization is complete. Remaining errors are in api.rs (Handler trait) and cas_backends.rs (Path trait).
