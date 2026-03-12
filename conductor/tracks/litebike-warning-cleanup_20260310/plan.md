# Plan: Remove Remaining litebike Binary Warnings

## Scope

Focused cleanup for the remaining warnings reported by:

- `cargo build --bin litebike --features warp,git2`

Current warnings are all local to `src/bin/litebike.rs`.

## Phase 1: Repair warning sources

- [x] Remove unused imports in the `src/bin/litebike.rs` import block
- [x] Resolve the orphaned `run_ssh_automation` warning in a truthful way
- [x] Keep the accepted stub-handler and TLS/CCEK fixes intact

## Phase 2: Verify

- [x] `cargo build --bin litebike --features warp,git2`
- [x] Confirm the warning count for `litebike` is reduced to zero or explain any intentional remainder

## Progress Notes

- 2026-03-10: After the stub-command and `tls-quic` repair tracks closed,
  `litebike` still emits 7 warnings, all in `src/bin/litebike.rs`:
  - unused imports: `Signal`, `TetheringBypass`, `literbike::raw_telnet`,
    `literbike::host_trust`, `literbike::radios`
  - dead function: `run_ssh_automation`
- 2026-03-10: `run_ssh_automation` is currently orphaned (`rg` finds only its
  definition), so the next slice must either wire it into an intended command
  surface or mark/remove it intentionally based on repo-local evidence.
- 2026-03-10: `claude` completed the bounded cleanup with a valid rendezvous
  payload. Master verification confirms the edit is limited to the import block
  and removal of the dormant `run_ssh_automation` placeholder, and
  `cargo build --bin litebike --features warp,git2` now finishes with zero
  warnings for the `litebike` binary.
