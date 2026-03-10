<!-- tokens: T003 -->

# Spec: LiterBike Unified Traffic and Services Launch

## Overview

This track positions `literbike` as the heavier half of the `litebike` /
`literbike` system: the runtime that goes beyond edge proxying into unified
traffic handling, transport depth, service adapters, and durable orchestration.

`literbike` is not just the place where `litebike` code grew larger. It is the
repo that should launch as the broader transport-and-services backplane that the
lighter `litebike` edge process can feed.

## Problem

- The codebase already spans QUIC, reactor internals, API translation, DHT,
  content-addressed storage, protocol detection, and multi-service adapters.
- Some historical or inherited documentation still blurs the boundary between
  the two repos or over-focuses on the `litebike` binary naming lineage.
- Without a clear launch track, `literbike` can read like an oversized utility
  binary instead of the unified traffic and services runtime it is becoming.

## Goals

- Define `literbike` as the heavy unified runtime for transport and services.
- Make the relationship to `litebike` explicit: `litebike` is the lean edge
  ingress, `literbike` is the deeper traffic and service plane.
- Highlight the subsystems that justify `literbike` existing as its own launch
  unit: QUIC, keymux/modelmux, API translation, DHT, CAS, reactor/runtime
  composition.

## Functional Requirements

### 1. Unified Traffic Runtime Identity

- `literbike` launch materials must describe the repo as the place where mixed
  protocols, transports, and traffic policies are unified beyond the edge.
- The canonical operator-facing ingress for the launch story is the shared
  `8888` agent/unified-port surface; ad hoc alternate launch ports are not part
  of the intended product posture unless explicitly documented as temporary test
  seams.
- The launch story should include:
  - QUIC and transport-heavy handling
  - keymux/modelmux policy, routing, and provider-surface ownership
  - reactor/runtime coordination
  - protocol and API translation
  - traffic adaptation and service bridging

### 2. Unified Services Identity

- `literbike` must be framed as the service-side companion that owns heavier
  features not appropriate for the lean edge repo.
- Launch materials should explicitly call out:
  - keymux and modelmux as first-class `literbike` ownership, not incidental
    library leftovers
  - DHT and distributed service flows
  - content-addressed or durable storage paths
  - provider/service adapters
  - broader orchestration responsibilities

### 3. Companion Boundary with LiteBike

- `literbike` launch materials must define `litebike` as the lightweight edge
  ingress and local proxy/router companion.
- Expected handoff from `litebike` into `literbike` should be described as:
  local classification first, heavy transport/service handling second.
- If `literbike` presents a direct visual/operator surface, that surface must
  still align with the `agent8888` / unified-port contract rather than
  introducing a competing default port identity.
- The repo should avoid implying that all edge utility concerns belong inside
  `literbike`.

### 4. Launch Readiness Artifacts

- Provide a launch-oriented architecture note or track summary that answers:
  - what `literbike` owns
  - how it differs from `litebike`
  - why the split is operationally useful
  - how the two repos compose in a deployment
  - why keymux/modelmux belongs on the `literbike` side of that deployment

## Non-Functional Requirements

- Keep the launch story faithful to the current module graph.
- Prefer boundary clarity over marketing language.
- Avoid inventing subsystem names that are not backed by repo code.
- Preserve room for `literbike` to expand without collapsing the distinction from
  `litebike`.

## Acceptance Criteria

1. `literbike` has a launch track that defines it as the heavier unified traffic
   and services runtime.
2. The track explicitly defines `litebike` as the edge ingress and lightweight
   proxy/router companion.
3. The launch materials identify the concrete subsystems that belong in
   `literbike` today.
4. The deployment relationship between the two repos is described in concise,
   operational terms.
5. Any operator-facing launch path is reconciled to the canonical `8888`
   unified-port story instead of drifting to an alternate default port.

## Out of Scope

- Turning `literbike` into a replacement for small local network utilities
- Pulling basic edge tooling back out of `litebike`
- Broad implementation changes unrelated to launch positioning
