# Plan: Compiler Warning Cleanup

## Phase 1: Auto-fix

- [x] `cargo fix --lib -p literbike` — applied auto-suggestions
- [x] Add `.artifacts/` to `.gitignore`

## Phase 2: Manual fixes

- [x] Fixed unused imports in src/concurrency/bridge.rs, src/reactor/context.rs
  (moved test-only imports inside cfg(test) blocks)
- [x] Fixed unused imports/variables across 13 src/ files

## Phase 3: Verify

- [x] `cargo test --lib` — 278 passed, 0 failed (commit includes all fixes)
- [x] Warnings: 54 → 26 (remaining: recursive functions, some impl unused vars)

## Progress Notes

- 2026-03-10: 54 warnings total, 32 unused import/dead_code. cargo fix can handle
  22+. .artifacts/macos/Literbike Control Plane.app should be gitignored.
  Delegation: Worker A=kilo (auto-fix + manual + gitignore).
