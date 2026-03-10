# Litebike TODO

## URGENT: Carrier Tethering Bypass
- [x] Merge FFI Knox bypass branch - BLOCKED: Need to handle uncommitted changes
- [x] Enable tethering bypass immediately
- [x] Configure radio interface detection
- [x] Deploy universal listener with POSIX peek
- [x] Add packet fragmentation for DPI evasion
- [x] Implement protocol obfuscation
- [x] Test carrier bypass effectiveness

**Status:** ✅ COMPLETE - See `docs/CARRIER_BYPASS_IMPLEMENTATION.md`
**Binary:** `cargo run --bin carrier_bypass -- enable`

## Proxy Icon Live DSEL Menu
- [x] Turn the menu icon into a DSEL launcher surface instead of a static review stub
- [x] Check in a curated free-lane DSEL pack for GLM5, Kimi K2.5, and NVIDIA
- [x] Add copy-ready lifecycle commands alongside the pragmatic host-block refs
- [x] Cover the checked-in host-block refs with focused pragmatic-route tests
- [x] Load the menu from the DSEL pack instead of hardcoding the quick picks in `index.html`
- [x] Surface live gateway inventory in the menu, not just a copied `curl` command
- [x] Show env-key presence and selected binding per lane without exposing secret values
- [x] Add a real launch/probe action that hits the unified-port lifecycle path from the menu
- [x] Reflect live readiness, quota, and fallback state in the icon/menu instead of static labels
- [x] Add a browser-visible smoke path for the menu so UI behavior is validated, not just Rust parsing
- [x] Tie menu actions to the proxy DSEL/runtime layer instead of leaving them as clipboard helpers
- [x] Add editing/reload support for operator-maintained DSEL packs under `configs/`

**Status:** ✅ COMPLETE - See `docs/PROXY_ICON_LIVE_DSEL_MENU.md`
**Usage:** Open `index.html` in browser, click DSEL menu icon

## Notes
- Carrier is clipping tethering - need immediate fix
- FFI branch has Knox bypass techniques ready
- Focus on working solution over perfect code

## Implementation Summary

### Carrier Tethering Bypass
Created comprehensive CLI binary with:
- TTL spoofing (Linux iptables, macOS pfctl, Android)
- DNS override (8.8.8.8, 1.1.1.1, 9.9.9.9)
- Traffic shaping (mobile latency emulation)
- Packet fragmentation (DPI evasion)
- Protocol obfuscation (TCP fingerprint randomization)
- Radio interface detection
- POSIX peek functionality
- Knox proxy integration

**Command:** `cargo run --bin carrier_bypass -- enable --fragmentation --obfuscation`

### Proxy Icon Live DSEL Menu
Enhanced `index.html` with:
- Live gateway inventory with health checks
- Environment key presence indicators
- Launch/Probe/Reload actions
- Real-time readiness and quota state
- Browser-visible status updates
- Direct integration with unified-port runtime
- DSEL pack reload support

**Usage:** Serve `index.html` and open in browser
