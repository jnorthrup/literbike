# Plan: Fix CouchDB Reduce Pattern Binding Compile Error

## Scope

After the `couchdb` dependency wiring was repaired, one of the remaining
source-level compile blockers is a local pattern-binding error in
`src/couchdb/views.rs`.

## Phase 1: Repair the reducer branch

- [x] Fix the `"_sum" | reduce_fn if reduce_fn.contains("sum")` match arm so it
  compiles and preserves the intended `_sum` / custom-sum behavior
- [x] Add or adjust focused in-file coverage for the repaired branch if needed

## Phase 2: Verify

- [x] `cargo check --lib --features couchdb` (views.rs compiles - other files have separate issues)
- [x] Record the next remaining blocker after the `views.rs` compile fix

## Progress Notes

- 2026-03-10: Current compiler error:
  - `src/couchdb/views.rs:164` — `"_sum" | reduce_fn if reduce_fn.contains("sum")`
    does not bind `reduce_fn` in all alternatives
- 2026-03-10: The file already contains local reduce-path tests near the bottom,
  including `_sum`, so this is a clean one-file slice.
- 2026-03-10: `claude` failed to emit a rendezvous payload before the timeout,
  but master verification confirmed the final `src/couchdb/views.rs` diff is
  bounded to the reducer branch plus a focused test update. The next blocker
  surfaced by `cargo test --lib --features couchdb -- database` is now
  in `src/couchdb/api.rs` and `src/cas_backends.rs`.
- 2026-03-14: views.rs now compiles. The reduce pattern binding issue was already
  fixed in prior work. Remaining errors are Handler trait issues in api.rs
  and Path trait issue in cas_backends.rs.
