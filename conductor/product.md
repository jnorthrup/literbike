# Product

## What this repo is

`literbike` is the heavier heart/backplane repo in the `litebike` /
`literbike` split. It houses the deeper transport, model, DSEL, and service
subsystems that are intended to be gated into `litebike`, which remains the
primary deployable shell and operator front door.

`literbike` can still be run directly for focused backplane validation, FFI
integration, or service-side testing, but those direct launch modes are
secondary. They do not replace `litebike` as the canonical outer shell.

When the two repos are composed, `literbike` rides inside `litebike`
`agent8888` on port `8888`. That `litebike` surface subsumes both repos and is
the only canonical ingress/operator front door.

## Current product direction

- Keep `literbike` as the heavy capability layer that gives `litebike` its
  deeper heart/backplane when the gate is open.
- Remove architectural dependence on broker-style transport hops where direct
  transport is sufficient.
- Make QUIC a first-class transport path with explicit interoperability work.
- Keep keymux/modelmux, model DSEL, CAS, DHT, and broader service composition on
  the `literbike` side of the split.
- Keep Python integration practical through stable FFI/ctypes boundaries.

## Primary consumers (current)

- `litebike` as the shell that mounts `literbike` capabilities when present
- `freqtrade` ring-agent sidecar transport wrapper
- Local backplane validation and transport test utilities in this repository

## Product constraints

- Brownfield codebase: preserve existing behavior where possible.
- Additive change preferred over large rewrites.
- Interop foundations first, acceleration later (`io_uring` is not critical path).
- Do not let direct `literbike` launch paths drift into shell ownership that
  belongs to `litebike`.
