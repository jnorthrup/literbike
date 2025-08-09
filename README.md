# litebike

Low-level, cross-platform networking primitives and building blocks for proxying and tunnel tools. Focused on direct syscalls (via `libc`) for Linux/Android/macOS environments, including constrained/locked-down devices.

## TERMUX NETWORK TOOLS FOR KNOX  
  - hardlinking to litebike will enable the main() of each util to be active

* ip
* route
* ifconfig
* netstat

 these will be crafted from syscalls with rust MM and perfected on note 20 5g available TERMUX

## Highlights

* Universal listener target (port 8888) for multi-protocol frontends (HTTP/SOCKS5/TLS/DoH)
* Auto-detection of TERMUX host from routing table (Android/Termux scenarios)
* SSH localhost forwarding for easy access in remote-dev workflows
* System proxy helpers (macOS), Git proxy wiring, and PAC/WPAD auto-configuration support (targets)
* Bonjour/mDNS service advertisement and UPnP port mapping for NAT traversal (targets)
* Rust + libc POSIX syscalls for portable, minimal dependencies

Note: Some bullets above are project goals and/or test utilities; the crate currently exposes a syscall-oriented library (`syscall_net`) and foundational types/config for higher-level components.

## Repo layout

* `src/syscall_net.rs` — direct-syscall networking (sockets, interfaces, default route)
* `src/types.rs` — enums and helpers for protocols, addresses, flags
* `src/config.rs` — env-configurable runtime options
* `tests/` — integration/unit/bench scaffolding (work in progress)

## Quick start

Build:

```bash
cargo build
```

Run tests:

```bash
cargo test
```

Use as a library (path dependency example):

```toml
[dependencies]
litebike = { path = "../litebike" }
```

### Minimal example: list interfaces and default gateway

```rust
use litebike::syscall_net::{list_interfaces, get_default_gateway};

fn main() -> std::io::Result<()> {
  let ifaces = list_interfaces()?;
  for (name, iface) in ifaces {
    println!("{}: {:?}", name, iface.addrs);
  }

  if let Ok(gw) = get_default_gateway() {
    println!("Default gateway: {}", gw);
  }
  Ok(())
}
```

## Configuration

The `Config` struct reads from environment variables. Set any of the following to override defaults:

* `LITEBIKE_BIND_PORT` (default `8888`)
* `LITEBIKE_INTERFACE` (default `swlan0`)
* `LITEBIKE_LOG` (default `info`)
* `LITEBIKE_FEATURES` (comma-separated list)
* `EGRESS_INTERFACE` (default iface route to 8.8.8.8)

second choices

* `EGRESS_BIND_IP` (default route to 8.8.8.8 IP)
* `LITEBIKE_BIND_ADDR` ( TERMUX_HOST)
Example:

```bash
   LITEBIKE_INTERFACE='SWLAN0:' \
LITEBIKE_BIND_PORT=8888 \
LITEBIKE_LOG=debug \
cargo run --example your_app
```

## Remote development (Android/Termux)

SSH into Termux and optionally forward the universal port 8888 back to your host:

```bash
ssh u0_a471@192.168.225.152 -p 8022 -L 8888:192.168.225.152:8888
```

This enables pushing to a temporary remote, running on the device, and iterating from the host. Adjust username/IP as needed.

## macOS notes

* Default-gateway detection uses `netstat` under the hood today.
* System proxy/PAC helpers are roadmap items; some tests/utilities may reference them.

## License

See `LICENSE`, `COPYING`, `COMMERCIAL.md`, and `COMMERCIAL-LICENSE.md`. Some functionality may be dual-licensed for commercial use.

## Security

This crate exposes raw syscalls and socket operations. Review and test thoroughly before deploying to production or locked-down environments.
