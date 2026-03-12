# Plan: Shape RBCursive Scanner Helper Loops for Indexed Traversal

## Scope

The main `AutovecScanner` and `GenericScanner` scan loops are now shaped around
explicit indexed traversal, but `src/rbcursive/scanner.rs` still keeps its
scanner-local `gather_bytes` and `popcount` helpers on iterator adapters. This
is the next bounded scanner-local surface where the compiler proof can be made
more explicit without changing public behavior.

## Phase 1: Reshape helper loops

- [x] Keep the slice bounded to `src/rbcursive/scanner.rs`
- [x] Rewrite the scanner-local `gather_bytes` helper around explicit indexed
      traversal over `positions`
- [x] Rewrite the scanner-local `popcount` helper around explicit indexed
      traversal over `bitmap`
- [x] Preserve existing gather/popcount semantics and tests

## Phase 2: Verify

- [x] `cargo test scanner::tests --lib`
- [x] `cargo test test_gather_operation --lib`
- [x] Record the next remaining scanner-local autovec hotspot after this helper slice

## Progress Notes

- 2026-03-11: Repo-local evidence for the follow-on helper slice:
  - `src/rbcursive/scanner.rs` still uses `.iter().filter_map(...)` in
    `gather_bytes`
  - the same file still uses `bitmap.iter().map(|x| x.count_ones()).sum()` in
    `popcount`
  - the hotter scan loops in both `AutovecScanner` and `GenericScanner` are now
    already shaped, so these helpers are the next scanner-local cleanup
- 2026-03-11: Acceptance surface corrected after focused verification:
  `cargo test test_popcount --lib` only exercises the generic scanner today,
  while `cargo test scanner::tests --lib` covers the scanner-local module where
  this helper slice lives.
- 2026-03-11: `qwen` completed a bounded edit in `src/rbcursive/scanner.rs`
  with a valid rendezvous payload. Master verification confirmed:
  - `ScalarScanner::gather_bytes` and `AutovecScanner::gather_bytes` now use
    explicit indexed loops with upfront `Vec::with_capacity(...)`
  - `ScalarScanner::popcount` and `AutovecScanner::popcount` now use explicit
    indexed accumulation over `bitmap`
  - `cargo test scanner::tests --lib` passes
  - `cargo test test_gather_operation --lib` passes
- 2026-03-11: Next nearby autovec-adjacent hotspots after this helper slice:
  - `src/rbcursive/simd/generic.rs` still keeps `popcount` on iterator-map-sum
  - `src/rbcursive/simd/sse2.rs` still contains iterator-based fallback scans
