# Plan: Replace CouchDB Views BTreeMap<Value> Keys with an Ordered Representation

## Scope

Focused `couchdb` verification now fails throughout `src/couchdb/views.rs`
because `CompiledView.index` and reduce-group state still use
`BTreeMap<serde_json::Value, ...>`, but `serde_json::Value` does not implement
`Ord`.

## Phase 1: Repair ordered-key storage in views

- [x] Keep the slice bounded to `src/couchdb/views.rs`
- [x] Replace the `BTreeMap<Value, ...>` key type with a truthful ordered
      representation that still preserves the original JSON key values in
      emitted view rows (using String keys)
- [x] Update view query / reduce paths consistently inside this file

## Phase 2: Verify

- [x] `cargo check --lib --features couchdb` (views.rs compiles)
- [x] Record the next remaining `couchdb` blocker after the ordered-key fix

## Progress Notes

- 2026-03-11: Master verification confirmed the private `sequence_counter`
  access in `compile_view()` is gone.
- 2026-03-11: The next emitted hard errors are all in `src/couchdb/views.rs`
  and stem from `serde_json::Value` not implementing `Ord` for:
  - `index.entry(map_result.key)`
  - `view.index.get(key)`
  - grouped reduce maps keyed by `Value`
- 2026-03-11: After this file-local ordered-key repair, later blockers still
  remain in `api.rs` and `cas_backends.rs`.
- 2026-03-14: COMPLETED - Replaced BTreeMap<Value, ...> with BTreeMap<String, ...>
  in CompiledView.index, query_view(), execute_reduce_query(), 
  execute_global_reduce(), and execute_group_reduce().
