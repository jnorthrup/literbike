# Plan: Update CouchDB IPFS Adapter to Current ipfs-api-backend-hyper API

## Scope

After the dependency, reducer, `git_sync`, and tensor-response fixes, focused
`couchdb` verification now fails first in `src/couchdb/ipfs.rs` due to API
drift against the current `ipfs-api-backend-hyper` crate.

## Phase 1: Repair IPFS adapter API usage

- [x] Update the `Add` request construction to match the current crate fields
- [x] Fix any current stream helper imports needed by the cat/read path
- [x] Keep the slice bounded to `src/couchdb/ipfs.rs`

## Phase 2: Verify

- [x] `cargo test --lib --features couchdb -- database`
- [x] Record the next remaining blocker after the IPFS adapter compiles

## Progress Notes

- 2026-03-10: Current first hard errors in `src/couchdb/ipfs.rs` include:
  - nonexistent `Add` fields like `path` and `progress`
  - `Option<bool>` mismatches for request flags
  - `hash: Some("sha2-256".to_string())` type mismatch
  - missing `TryStreamExt` import for `.map_ok(...)`
- 2026-03-10: First `claude` launch on this corpus failed closed. After the
  monitoring timeout there was still no `src/couchdb/ipfs.rs` diff, no
  rendezvous payload, and focused verification still surfaced the same IPFS API
  errors unchanged. The slice must be rerouted.
- 2026-03-10: `qwen` landed a real bounded diff in `src/couchdb/ipfs.rs` and
  cleared the original `Add` field/option/type mismatches plus the missing
  `TryStreamExt` import. Focused verification now fails later in the same file
  on:
  - borrowed-data escape in `store_data` (`Cursor<&[u8]>` passed to
    `add_with_options` requiring `'static`)
  - missing `repo_stat()` method
  - missing `repo_gc()` method
  - `BitswapStatResponse` field rename (`provide_buf_len`)
  The IPFS track remains open on this same file.
- 2026-03-11: Master reconciliation confirms the bounded `src/couchdb/ipfs.rs`
  diff also cleared the later same-file blockers:
  - `store_data` now uses an owned buffer for `add_with_options`
  - repo stats use `stats_repo()`
  - unsupported GC path is downgraded to a truthful warning/empty result
  - `provide_buf_len` matches the current `BitswapStatResponse`
  Focused verification now fails later in other modules, with the next hard
  blocker at `M2mMessageType` missing `Eq`/`Hash` derives for
  `src/couchdb/m2m.rs`.
