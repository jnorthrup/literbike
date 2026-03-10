# Conductor Workflow

- Work from repo-local evidence first.
- Keep slices bounded to one concrete product improvement at a time.
- Update local `conductor/` track truth before or during implementation.
- Verify with focused tests or smoke commands tied to the changed surface.
- Prefer immediate transport-path work: QUIC, CAS, DHT, or reactor.

## Guiding Principles

1. **The Plan is the Source of Truth:** All work tracked in `plan.md`
2. **Test-Driven Development:** Write tests before implementation
3. **High Code Coverage:** Aim for >80% coverage for all modules
4. **Trikeshed as Gospel:** Port Kotlin patterns from Trikeshed where applicable

## Task Workflow

1. **Select Task:** Choose next available task from `plan.md`
2. **Mark In Progress:** Change `[ ]` to `[~]` in plan.md
3. **Write Failing Tests:** Red phase of TDD
4. **Implement to Pass:** Green phase
5. **Refactor:** Improve clarity without changing behavior
6. **Verify Coverage:** Run `cargo tarpaulin --out Html`
7. **Commit:** Stage changes with proper message
8. **Update Plan:** Mark task `[x]` with commit SHA

## Development Commands

```bash
# Build with QUIC feature
cargo build --features quic

# Run tests
cargo test --features quic --lib

# Integration tests
cargo test --test integration_quic_dht_cas

# Lint
cargo clippy --features quic

# Format
cargo fmt --check
```

## Track Planning Conventions

- `spec.md` describes scope, acceptance criteria, and out-of-scope items
- `plan.md` is phased and checklist-driven
- Track work references real file paths and integration seams
