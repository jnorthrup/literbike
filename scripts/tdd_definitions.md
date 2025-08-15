# TDD Definitions and Contracts for LiteBike Automation

This file lists small, testable contracts and TDD tasks to validate the sync/control/summary tooling and upstream code assumptions.

Goals (short)
- Verify sync script updates the remote repo and triggers build/test.
- Verify control wrapper starts/stops the loop and writes a PID file.
- Verify summary script correctly reports statuses.

Minimal contracts

1) `git_update(host)` behavior (integration)
- Input: reachable SSH host with a clone at `LITEBIKE_REMOTE_PATH` and branch set
- Output: remote HEAD matches origin/branch after function runs
- Error modes: SSH auth failure -> script returns nonzero

2) Build/test execution
- Input: remote repo at a known good commit
- Output: `build.log` and `test.log` exist and have expected pass markers

3) Control wrapper
- Input: start command with valid config
- Output: PID file created, process alive
- Stop: kills process and removes PID

4) Summary
- Input: results directory with host/timestamp entries
- Output: table shows latest entry per host and statuses parsed

Edge cases
- Partial network failures: ensure retry/backoff tested
- Stale PID files: control wrapper removes stale PIDs
- Missing remote tools (no fswatch/inotify): polling fallback

Suggested tests to add
- Unit tests for `src/ssh_tools.rs` functions using `assert_cmd` and local shells
- Small integration test that uses `localhost` as SSH target (requires local SSH server) to exercise end-to-end flow
- CI job that runs `scripts/ci-test.sh` (existing) and validates artifacts are produced into `target/ci-test`

How to run
- Add tests to `tests/integration/` that call the scripts using `Command::new("bash")` or `assert_cmd` in Rust
- Mock SSH interactions where possible; or mark network tests as `#[ignore]` and run manually in integration environment


