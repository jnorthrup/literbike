# Plan: Fix Infinite Recursion in Join Trait Impls

## Phase 1: Fix

- [x] `src/rbcursive/mod.rs:112,119` — changed to `self.as_slice().join(separator)`

## Phase 2: Verify

- [x] `cargo check` — 0 unconditional_recursion warnings
- [x] `cargo test --lib` — 278 passed, 0 failed

## Progress Notes

- 2026-03-10: Two Join<T> impls for Vec<String> and Vec<&str> infinitely recurse.
  The `Vec::join` UFCS call resolves to the trait impl not the slice method.
  Fix: self.as_slice().join(separator). Delegation: Worker A=kilo.
