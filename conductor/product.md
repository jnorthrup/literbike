# Product

## What this repo is

`literbike` is a Rust networking/transport codebase that is being hardened into a
QUIC-capable foundation for direct bot-side integrations (including `freqtrade`
ring-agent sidecars) while preserving pragmatic fallback paths.

## Current product direction

- Remove architectural dependence on broker-style transport hops where direct
  transport is sufficient.
- Make QUIC a first-class transport path with explicit interoperability work.
- Keep Python integration practical through stable FFI/ctypes boundaries.

## Primary consumers (current)

- `freqtrade` ring-agent sidecar transport wrapper
- Local QUIC test/server utilities in this repository

## Product constraints

- Brownfield codebase: preserve existing behavior where possible.
- Additive change preferred over large rewrites.
- Interop foundations first, acceleration later (`io_uring` is not critical path).

