# Plan: Conductor CLI Smoke Integration

## Phase 1: Build

- [ ] Compile `conductor-cli` with `cargo build -p conductor-cli`
- [ ] Fix any compilation errors in `conductor-cli/src/main.rs` or `conductor-cli/Cargo.toml`

## Phase 2: Smoke

- [ ] Run `conductor-cli list` against `conductor/tracks/` and verify known tracks appear
- [ ] Run `conductor-cli status` and verify COMPLETE tracks are reported correctly

## Phase 3: Commit

- [ ] Commit `conductor-cli/` as a new workspace member with passing build

## Progress Notes

- 2026-03-09: Track created. conductor-cli/src/main.rs exists (1321 lines) but has never
  been compiled. Listed in root Cargo.toml workspace members. Delegation: Worker A=kilo
  (build/fix), Worker B=opencode (smoke+commit).
