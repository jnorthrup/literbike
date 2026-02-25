# Workflow

## Brownfield Foundation-First Workflow

1. Confirm the target seam and current behavior before editing.
2. Make the smallest additive change that creates the required extension point.
3. Feature-gate risky or incomplete functionality (especially crypto/handshake).
4. Keep legacy/fallback behavior working while new path is introduced.
5. Add focused tests for new wire semantics, state transitions, and FFI errors.
6. Validate with crate-level builds/tests before broadening scope.

## Track Planning Conventions

- `spec.md` describes scope, acceptance criteria, and out-of-scope items.
- `plan.md` is phased and checklist-driven.
- Track work should reference real file paths and integration seams.

