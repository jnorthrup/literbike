<!-- tokens: T004 -->

# Plan: LiterBike Unified Traffic and Services Launch

## Phase 1: Boundary Lock

- [x] Inventory the transport, traffic, and service subsystems already present
  in `literbike`. (SUBSYSTEMS.md)
- [x] Distill the shortest accurate launch description for the repo as the heavy
  unified runtime companion to `litebike`. (LAUNCH_NARRATIVE.md)
- [x] Remove or supersede repo language that blurs the live split.

## Phase 2: Launch Narrative

- [x] Publish a launch-ready explanation of `literbike` as the deeper transport
  and services plane. (LAUNCH_NARRATIVE.md)
- [x] Call out the modules that justify the split: QUIC, keymux, modelmux, API
  translation, DHT, CAS, reactor/runtime composition, and adapters.
- [x] Describe the handoff relationship from `litebike` edge ingress into
  `literbike`.
- [x] Explicitly name `keymux` and `modelmux` as `literbike` launch ownership
  so the repo is not framed as transport-only.
- [x] Check in a concise split diagram artifact at `split-chart.md`.

## Phase 3: Operational Fit

- [x] Harden icon-menu launcher behavior in `index.html` so the control-path UI
  closes on `Escape` and outside click, with focus returning to the trigger.
- [x] Confirm the launch story matches the current feature flags, binaries, and
  modules in the repo, including the corrected `warp,git2,tls-quic` launch path
  for the visual QUIC/operator surface.
- [x] Identify the minimum examples or docs needed to make the heavy-runtime
  role understandable at launch. (LAUNCH_NARRATIVE.md, SUBSYSTEMS.md)
- [x] Note packaging and feature-gating issues that would confuse operators:
  `quic-vqa` had drifted behind a non-canonical `4433` default and would not
  compile cleanly without the `tls-quic` feature in the active launch command.
- [x] Course-correct the visual/operator launch story so `8888` is treated as
  the canonical unified-port surface and `4433` is explicitly treated as
  temporary drift rather than a stable product port.

## Phase 4: Companion Alignment

- [x] Cross-reference the matching `litebike` launch track.
  - See: `conductor/tracks/literbike_unified_services_launch_20260308/LAUNCH_NARRATIVE.md`
  - Handoff pattern: LiteBike (edge) → LiterBike (heavy runtime)
- [x] Keep future work triage aligned so edge concerns land in `litebike` and
  unified traffic/service concerns land in `literbike`.
  - Edge concerns: protocol classification, lean proxy, fast control path
  - Runtime concerns: QUIC depth, CAS gateway, DHT, KeyMux/ModelMux, orchestration
- [x] Preserve the split as a deliberate product boundary rather than an
  accident of history.
  - Documented in `LAUNCH_NARRATIVE.md` and `SUBSYSTEMS.md`
  - Enforced via workspace structure and feature gating
  - Workspace members clearly separate concerns

## Progress Notes

- 2026-03-08: course correction applied after operator feedback that `4433` was
  a temporary aberration of the `agent8888` story rather than a durable launch
  port.
- 2026-03-08: launch truth now treats `8888` as the canonical operator-facing
  ingress for the visual QUIC/control-plane path, with `4433` removed from the
  current launch narrative.
- 2026-03-08: operational fit note updated to reflect that the visual QUIC
  launch path depends on `warp,git2,tls-quic`, not a bare `warp,git2` build.
- 2026-03-08: ownership course correction applied after operator feedback that
  `literbike` is not just QUIC/services; it is also the `keymux` and
  `modelmux` home for the heavier side of the split.
- 2026-03-09: Phase 1 complete - SUBSYSTEMS.md inventory created
- 2026-03-09: Phase 2 complete - LAUNCH_NARRATIVE.md published
- 2026-03-09: Phase 3 complete - Documentation and examples complete
