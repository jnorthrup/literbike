# Plan: Shape RBCursive Autovec Scanner Loops for Compiler Vectorization

## Scope

`src/rbcursive/scanner.rs` exposes `AutovecScanner` as the compiler-driven fast
path, but its hot loops still lean on iterator/enumerate traversal and generic
helpers that make the contiguous induction-variable proof weaker than it needs
to be.

The user-supplied acceptance criteria for this track are explicit:

- contiguous access
- simple induction variables
- no hidden aliasing
- no polymorphic iterator overhead
- no accidental allocation in the hot loop
- branch structure simple enough to lower into masks

## Phase 1: Reshape the scanner hot loops

- [x] Keep the slice bounded to `src/rbcursive/scanner.rs`
- [x] Rewrite the autovec single-target scan around explicit indexed slice walks
- [x] Rewrite the autovec multi-target and structural scan loops so the compiler
      sees straightforward lookup-table membership over contiguous bytes
- [x] Preserve existing result semantics and public scanner behavior

## Phase 2: Verify

- [x] `cargo test test_autovec_scanner --lib`
- [x] Record the next remaining autovec hotspot after `AutovecScanner`

## Progress Notes

- 2026-03-11: Repo-local evidence for the new track:
  - `src/rbcursive/scanner.rs` contains the active `AutovecScanner`
  - the hot scan paths still use `.iter().enumerate()` loops
  - file comments already claim "Compiler should auto-vectorize this loop"
- 2026-03-11: User direction for the shaping pass:
  - explicit loop variables
  - indexed contiguous access over abstraction-heavy helpers
  - separate traversal structure from any deferred/projection surfaces
  - avoid helper abstractions that obscure stride or alignment
- 2026-03-11: `qwen` completed a bounded edit in `src/rbcursive/scanner.rs`
  with a valid rendezvous payload. Master verification confirmed:
  - `AutovecScanner::scan_bytes` now uses explicit indexed `while` loops for
    both single-target and lookup-table scan paths
  - `AutovecScanner::scan_structural` now builds its lookup table and traverses
    input with explicit indexed loops
  - the accepted slice stayed inside `src/rbcursive/scanner.rs`
  - `cargo test test_autovec_scanner --lib` passes
- 2026-03-11: The next nearby hotspot is `src/rbcursive/simd/generic.rs`,
  where `scan_single_byte` and `scan_multiple_bytes` still use chunk-local
  `.iter().enumerate()` traversal in loops that are also intended for
  compiler-driven vectorization.
