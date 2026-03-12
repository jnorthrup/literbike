# Plan: Shape RBCursive Generic Scanner Loops for Compiler Vectorization

## Scope

After the `AutovecScanner` cleanup in `src/rbcursive/scanner.rs`, the next
local auto-vectorization hotspot is `src/rbcursive/simd/generic.rs`. Its
`GenericScanner` hot scan paths still process chunk slices with
`.iter().enumerate()`, which weakens the same contiguous indexed-loop proof
that the compiler needs for predictable vectorization.

The user-supplied acceptance criteria from the autovec track still apply:

- contiguous access
- simple induction variables
- no hidden aliasing
- no polymorphic iterator overhead
- no accidental allocation in the hot loop
- branch structure simple enough to lower into masks

## Phase 1: Reshape the generic scanner hot loops

- [x] Keep the slice bounded to `src/rbcursive/simd/generic.rs`
- [x] Rewrite `scan_single_byte` around explicit indexed slice walks
- [x] Rewrite `scan_multiple_bytes` around explicit indexed lookup-table scans
- [x] Preserve existing generic scanner semantics and tests

## Phase 2: Verify

- [x] `cargo test test_generic_scanner --lib`
- [x] Record the next remaining autovec hotspot after `GenericScanner`

## Progress Notes

- 2026-03-11: Repo-local evidence for the follow-on slice:
  - `src/rbcursive/simd/generic.rs` still uses `.iter().enumerate()` in both
    chunked hot scan loops
  - file comments already describe these paths as auto-vectorization hints
  - the adjacent `AutovecScanner` track just landed cleanly, so this is the
    next bounded scanner-local surface with the same proof-shaping goal
- 2026-03-11: `qwen` completed a bounded edit in
  `src/rbcursive/simd/generic.rs` with a valid rendezvous payload. Master
  verification confirmed:
  - `scan_single_byte` now uses explicit indexed loops for both chunked and
    remainder traversal
  - `scan_multiple_bytes` now uses explicit indexed loops over the lookup table
    for both chunked and remainder traversal
  - the worker added modest upfront `Vec::with_capacity(...)` reservation and
    kept the slice bounded to this file
  - `cargo test test_generic_scanner --lib` passes
- 2026-03-11: The next nearby hotspot is back in
  `src/rbcursive/scanner.rs`, where `gather_bytes` still uses
  `.iter().filter_map(...)` and `popcount` still uses iterator-map-sum helpers
  in the scanner-local implementations.
