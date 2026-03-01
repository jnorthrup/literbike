# Spec: CAS Lazy N-Way Gateway Projections

## Overview

Create a canonical content-addressed storage (CAS) abstraction and project it
through lazy adapters into five target systems:

- `git`
- `torrent`
- `ipfs`
- `s3-blobs`
- `kv`

The gateway must avoid eager replication. Objects are materialized per backend
only when requested, while preserving deterministic content addressing.

## Problem

- Current storage pathways are backend-specific and duplicate object semantics.
- Cross-backend portability is manual and error-prone.
- Full fan-out writes are expensive and not always needed.

## Goals

- Define one canonical CAS object schema and identity contract.
- Add lazy projection adapters for all five target backends.
- Preserve integrity and deterministic addressing across projections.
- Support read-through and write-through behavior with bounded side effects.

## Functional Requirements

1. A canonical CAS model must define:
   - object identity (digest + algorithm),
   - metadata envelope (size, media type, timestamps),
   - chunking/manifest strategy for large objects.

2. A gateway interface must support:
   - `put` into canonical CAS,
   - lazy `project` into selected backend(s),
   - `get` by canonical identity with backend fallback order.

3. Backend adapters must exist for:
   - `git` (blob/tree projection),
   - `torrent` (content projection with metadata artifact),
   - `ipfs` (CID mapping),
   - `s3-blobs` (object key strategy),
   - `kv` (small object and manifest index support).

4. Lazy projection rules must be explicit:
   - no backend materialization until requested or policy-triggered,
   - idempotent projection calls,
   - deterministic mapping from canonical ID to backend handle.

5. Integrity checks must validate:
   - digest parity between canonical CAS and backend materialization,
   - round-trip retrieval from each backend into canonical bytes.

## Acceptance Criteria

1. Canonical CAS schema and projection API are documented and implemented.
2. All five adapters compile and pass parity tests for fixed fixtures.
3. Lazy projection is verified (no eager writes) under test instrumentation.
4. Failure behavior is defined for partial projection outages and retries.
5. Track plan references concrete integration seams and validation steps.

## Out of Scope

- Distributed consensus or global replication policy.
- Backend-specific performance tuning beyond baseline correctness.
- Production credential management and secret distribution.
