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

- [x] `run_proxy_node` in src/bin/litebike.rs — quick_start_knox_proxy wired in working tree
- [ ] `run_scan_ports` — fix compile bug: function takes `_args` but body reads `args[0]`
- [x] `run_bonjour_discover` — gather_radios() report wired in working tree
- [ ] `run_raw_connect` — fix compile bug: function takes `_args` but body reads `args[0]`
- [ ] `run_trust_host` — fix compile bug: function takes `_args` but body reads `args[0]`
- [x] `run_proxy_client` — quick_start_knox_proxy wired in working tree

## Phase 2: Verify

- [ ] Restore or remove missing workspace member `conductor-cli` so Cargo can load the workspace
- [ ] `cargo build --bin litebike` — blocked until workspace loads, then must compile clean
- [ ] `cargo test --lib` — blocked until workspace loads, then 278/0 still passing
- [ ] No new warnings in literbike binary

## Progress Notes

- 2026-03-10: 5 stub fn bodies in src/bin/litebike.rs contain only println! + TODO comment.
  All backing module APIs already exist. Wire them up. Delegation: Worker A=kilo.
- 2026-03-10: Repo reality diverged from the initial track note. The stub bodies are already
  wired in the working tree, but `run_scan_ports`, `run_raw_connect`, and `run_trust_host`
  still reference `args` while their signatures take `_args`, so the file is not compile-clean.
  Verification is additionally blocked because the workspace still references missing member
  `conductor-cli/Cargo.toml`.
