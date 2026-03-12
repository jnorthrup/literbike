# Plan: Repair CouchDB Feature Dependency Wiring

## Scope

`cargo test --lib --features couchdb -- database` is blocked before it reaches
the narrowed `database.rs` surface because the `couchdb` feature does not pull
several dependencies that the `couchdb` modules already import.

## Phase 1: Feature graph repair

- [x] Update `Cargo.toml` so the `couchdb` feature enables the dependencies it
  already uses transitively (`git2`, `notify`, `ipfs-api-backend-hyper`, `cid`,
  `ndarray-linalg`)
- [x] Enable the `axum` integration surface for `utoipa-swagger-ui`
- [x] Re-run the focused `couchdb` compile/test command to expose the next
  source-level blockers after dependency wiring is truthful

## Phase 2: Verify

- [ ] `cargo test --lib --features couchdb -- database`
- [ ] Record the next bounded compile blockers, if any

## Progress Notes

- 2026-03-10: Focused `couchdb` verification currently fails first on:
  - unresolved `utoipa_swagger_ui::SwaggerUi` because the crate is present but
    not built with its `axum` integration feature
  - unresolved crates `notify`, `git2`, `ipfs_api_backend_hyper`, `cid`,
    `ndarray_linalg` while `--features couchdb` is active
- 2026-03-10: `Cargo.toml` currently defines:
  - `couchdb = ["dep:sled", "dep:utoipa", "dep:utoipa-swagger-ui", "dep:axum", "dep:tower", "dep:tower-http", "dep:base64", "dep:ndarray"]`
  - separate `ipfs` and `tensor` feature groups for some crates that the
    `couchdb` API surface imports unconditionally
- 2026-03-10: `claude` completed the manifest repair with a valid rendezvous
  payload. Master verification confirms:
  - `couchdb` now enables `git2`, `notify`, `ipfs-api-backend-hyper`, `cid`,
    and `ndarray-linalg`
  - `utoipa-swagger-ui` now enables `features = ["axum"]`
  - the unresolved-crate and configured-out `SwaggerUi` errors are gone
  - the next blockers are source-level errors in `src/couchdb/git_sync.rs`,
    `src/couchdb/views.rs`, and an unexpected `cfg(feature = "git2")` warning
