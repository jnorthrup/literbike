# Plan: Wire 5 Stub CLI Commands in litebike.rs

## Backing APIs (all modules already exist in src/lib.rs)

| Function | Module API |
|----------|-----------|
| `run_proxy_node` | `literbike::knox_proxy::quick_start_knox_proxy()` (async → block_on) |
| `run_scan_ports` | `literbike::raw_telnet::quick_port_scan(target)` — target from args[0] |
| `run_bonjour_discover` | `literbike::radios::gather_radios()` + `literbike::upnp_aggressive` scan |
| `run_raw_connect` | `literbike::raw_telnet::raw_connect(target)` — target from args[0] |
| `run_trust_host` | `literbike::host_trust::is_host_trusted(host)` + `trust_local_network()` |
| `run_proxy_client` | `literbike::knox_proxy::quick_start_knox_proxy()` (same runtime, client mode) |

## Phase 1: Wire implementations

- [x] `run_proxy_node` — remove unsupported `(server_mode, bind_addr)` args; call `quick_start_knox_proxy()` and surface io error
- [x] `run_scan_ports` — remove Tokio runtime/`.await`; call sync `quick_port_scan(target)` and print returned ports
- [x] `run_bonjour_discover` — replace nonexistent `discover_upnp_devices()` with `AggressiveUPnP::new()?.discover_aggressive()`
- [x] `run_raw_connect` — remove Tokio runtime/`.await`; call sync `raw_connect(target)`
- [x] `run_trust_host` — treat `is_host_trusted(host)` as `bool`, not `Result`
- [x] `run_proxy_client` — remove unsupported `(false, server_addr)` args; call `quick_start_knox_proxy()` and surface io error

## Phase 2: Verify

- [x] Workspace loads again with `conductor-cli` present
- [x] `cargo build --bin litebike --features warp,git2` — rerun after stub-command fixes
- [x] Evaluate remaining non-slice litebike build blockers after the stub-command errors are removed
- [x] `cargo test --lib` — rerun once litebike slice is truthful again
- [ ] No new warnings in literbike binary

## Progress Notes

- 2026-03-10: 5 stub fn bodies in src/bin/litebike.rs contain only println! + TODO comment.
  All backing module APIs already exist. Wire them up. Delegation: Worker A=kilo.
- 2026-03-10: Repo reality diverged from the initial track note. The stub bodies are already
  wired in the working tree, but `run_scan_ports`, `run_raw_connect`, and `run_trust_host`
  still reference `args` while their signatures take `_args`, so the file is not compile-clean.
  Verification is additionally blocked because the workspace still references missing member
  `conductor-cli/Cargo.toml`.
- 2026-03-10: Repo reality diverged again. Current `src/bin/litebike.rs` no longer contains
  TODO-only stubs; it contains incorrect API calls instead: `quick_start_knox_proxy` is called
  with unsupported arguments, `quick_port_scan` and `raw_connect` are incorrectly awaited,
  `discover_upnp_devices` does not exist, and `is_host_trusted` is matched as `Result` instead
  of `bool`. Verified with `cargo build --bin litebike --features warp,git2`.
- 2026-03-10: Focused build also shows remaining blockers outside this track after the stub
  slice: `literbike::quic::tls`/`tls_ccek` are referenced without `tls-quic`, and two
  `CoroutineContext` assignments at `src/bin/litebike.rs:2257` and `:3913` are type-mismatched.
- 2026-03-10: Delegated worker runtime = kilo. Multiple bounded launches reached file-reading
  and reasoning states but emitted no product diff or rendezvous payload, so this slice remains
  open and failed closed pending a productive kilo execution.
- 2026-03-10: Runtime reroute: Worker A moved from kilo to opencode for this exact corpus
  after repeated kilo launches failed closed without a diff or required rendezvous payload.
- 2026-03-10: `opencode` also failed closed on the same corpus. It edited
  `src/bin/litebike.rs`, but drifted into unrelated handlers (`run_upnp_gateway`,
  `run_proxy_quick`, wrapper code) and introduced a syntax error
  (`unexpected closing delimiter` at `src/bin/litebike.rs:1776`) before the
  six-target stub slice could be verified. Next reroute must both repair the
  opencode damage and land only the intended handler fixes.
- 2026-03-10: `qwen` failed closed on the follow-up repair slice. It rewrote
  the same bounded corpus but still touched unrelated code paths, replacing
  large `run_upnp_gateway` and `run_proxy_quick` bodies and leaving another
  syntax error (`unexpected closing delimiter` at `src/bin/litebike.rs:3193`).
  The file remains dirty and unverified; next reroute must repair from the
  current working tree and constrain edits to authentic slice needs.
- 2026-03-10: `claude` landed the first authentic bounded diff for this track.
  Master verification confirms the final `git diff -- src/bin/litebike.rs` is
  limited to the six target handlers, the stub-command compile errors are gone,
  `cargo build --bin litebike --features warp,git2` now fails only on the
  separate `tls-quic`/`CoroutineContext` blockers in `run_proxy_server` and
  `run_quic_vqa`, and `cargo test --lib` passes (`278 passed; 0 failed; 1 ignored`).
