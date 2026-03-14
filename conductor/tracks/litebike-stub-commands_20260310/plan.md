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
- [x] `cargo build --bin litebike --features warp,git2` — PASSES
- [x] Evaluate remaining non-slice litebike build blockers after the stub-command errors are removed
- [x] `cargo test --lib` — 273 passed

## Progress Notes

- 2026-03-14: VERIFIED - All stub commands are wired correctly:
  - `quick_start_knox_proxy()` takes no args
  - `quick_port_scan(target)` and `raw_connect(target)` are synchronous
  - `host_trust::is_host_trusted(host)` returns bool
  - `discover_upnp_devices` replaced with `literbike::upnp` module
- 2026-03-14: Build passes with minor warnings only
