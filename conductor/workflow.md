# Project Workflow

## Guiding Principles

1. **The Plan is the Source of Truth:** All work must be tracked in `plan.md`
2. **The Tech Stack is Deliberate:** Changes to the tech stack must be documented in `tech-stack.md` *before* implementation
3. **Test-Driven Development:** Write unit tests before implementing functionality
4. **High Code Coverage:** Aim for >80% code coverage for all modules
5. **Non-Interactive & CI-Aware:** Prefer non-interactive commands. Use `CI=true` for watch-mode tools (tests, linters) to ensure single execution.

## Brownfield Foundation-First Workflow

1. Confirm the target seam and current behavior before editing.
2. Make the smallest additive change that creates the required extension point.
3. Feature-gate risky or incomplete functionality (especially crypto/handshake).
4. Keep legacy/fallback behavior working while new path is introduced.
5. Add focused tests for new wire semantics, state transitions, and FFI errors.
6. Validate with crate-level builds/tests before broadening scope.

## Task Workflow

All tasks follow a strict lifecycle:

### Standard Task Workflow

1. **Select Task:** Choose the next available task from `plan.md` in sequential order

2. **Mark In Progress:** Before beginning work, edit `plan.md` and change the task from `[ ]` to `[~]`

3. **Write Failing Tests (Red Phase):**
   - Create a new test file for the feature or bug fix.
   - Write one or more unit tests that clearly define the expected behavior and acceptance criteria for the task.
   - **CRITICAL:** Run the tests and confirm that they fail as expected. This is the "Red" phase of TDD. Do not proceed until you have failing tests.

4. **Implement to Pass Tests (Green Phase):**
   - Write the minimum amount of application code necessary to make the failing tests pass.
   - Run the test suite again and confirm that all tests now pass. This is the "Green" phase.

5. **Refactor (Optional but Recommended):**
   - With the safety of passing tests, refactor the implementation code and the test code to improve clarity, remove duplication, and enhance performance without changing the external behavior.
   - Rerun tests to ensure they still pass after refactoring.

6. **Verify Coverage:** Run coverage reports using the project's chosen tools:
   ```bash
   cargo tarpaulin --out Html
   ```
   Target: >80% coverage for new code.

7. **Document Deviations:** If implementation differs from tech stack:
   - **STOP** implementation
   - Update `tech-stack.md` with new design
   - Add dated note explaining the change
   - Resume implementation

8. **Commit Code Changes:**
   - Stage all code changes related to the task.
   - Propose a clear, concise commit message e.g, `feat(quic): Add packet encoding for handshake`.

9. **Attach Task Summary with Git Notes:**
   - **Step 9.1: Get Commit Hash:** Obtain the hash of the *just-completed commit* (`git log -1 --format="%H"`).
   - **Step 9.2: Draft Note Content:** Create a detailed summary for the completed task. This should include the task name, a summary of changes, a list of all created/modified files, and the core "why" for the change.
   - **Step 9.3: Attach Note:** Use the `git notes` command to attach the summary to the commit.

10. **Get and Record Task Commit SHA:**
    - **Step 10.1: Update Plan:** Read `plan.md`, find the line for the completed task, update its status from `[~]` to `[x]`, and append the first 7 characters of the *just-completed commit's* commit hash.
    - **Step 10.2: Write Plan:** Write the updated content back to `plan.md`.

11. **Commit Plan Update:**
    - **Action:** Stage the modified `plan.md` file.
    - **Action:** Commit this change with a descriptive message (e.g., `conductor(plan): Mark task 'Add QUIC codec' as complete`).

## Track Planning Conventions

- `spec.md` describes scope, acceptance criteria, and out-of-scope items.
- `plan.md` is phased and checklist-driven.
- Track work should reference real file paths and integration seams.

## Development Commands

### Build & Test
```bash
# Build with QUIC feature
cargo build --features quic

# Run tests
cargo test --features quic --lib

# Run specific module tests
cargo test --features quic --lib quic::quic_protocol::tests

# Integration tests
cargo test --test integration
```

### Code Quality
```bash
# Lint
cargo clippy --features quic

# Format
cargo fmt --check

# Doc check
cargo doc --no-deps
```

### FFI Build
```bash
# Build Python extension
cd literbike-py && cargo build --release

# Build C ABI library
cd literbike-quic-capi && cargo build --release
```

## Quality Gates

Before marking any task complete, verify:

- [ ] All tests pass
- [ ] Code coverage meets requirements (>80%)
- [ ] No clippy warnings
- [ ] Code formatting is correct
- [ ] Documentation builds without errors
- [ ] No linting or static analysis errors
- [ ] FFI boundaries work correctly (Python and C)

## Commit Guidelines

### Message Format
```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Types
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Formatting, missing semicolons, etc.
- `refactor`: Code change that neither fixes a bug nor adds a feature
- `test`: Adding missing tests
- `chore`: Maintenance tasks

## Definition of Done

A task is complete when:

1. All code implemented to specification
2. Unit tests written and passing
3. Code coverage meets project requirements
4. Documentation complete (if applicable)
5. Code passes all configured linting and static analysis checks
6. FFI integration verified (if applicable)
7. Implementation notes added to `plan.md`
8. Changes committed with proper message
9. Git note with task summary attached to the commit
