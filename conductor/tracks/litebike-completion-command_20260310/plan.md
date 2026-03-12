# Plan: Wire litebike completion Command to the Existing Bash Script

## Scope

`src/bin/litebike.rs` already exposes a `completion` command in the dispatch
table, but the handler still prints a stub message even though the repo ships a
real completion artifact at `completion/litebike-completion.bash`.

## Phase 1: Wire the command

- [x] Replace the stub `run_completion` body with a real completion output path
- [x] Reuse the existing `completion/litebike-completion.bash` artifact rather
  than inventing a new shell-completion format
- [x] Provide a truthful CLI surface for unsupported shell arguments if the
  command accepts a shell selector

## Phase 2: Verify

- [x] `cargo build --bin litebike --features warp,git2`
- [x] `cargo run --bin litebike --features warp,git2 -- completion`
- [x] Confirm the emitted output is the existing bash completion script (or a
  shell-specific wrapper around it)

## Progress Notes

- 2026-03-10: Repo-local evidence:
  - dispatch entry exists in `src/bin/litebike.rs`: `("completion", run_completion)`
  - handler is still a stub: `println!("completion command is not yet implemented.")`
  - docs already instruct operators to install `completion/litebike-completion.bash`
    from the repo into `$PREFIX/share/bash-completion/completions/litebike`
- 2026-03-10: Preferred implementation is to surface the existing bash script
  through the command rather than adding a second completion source of truth.
- 2026-03-10: `claude` completed the slice with a valid rendezvous payload.
  Master verification confirmed:
  - `src/bin/litebike.rs` now emits bash completions via `include_str!`
  - unsupported shell args fail truthfully
  - `completion/litebike-completion.bash` was also updated so the command and
    installable artifact now share the same real script content
  - `cargo build --bin litebike --features warp,git2` passes
  - `cargo run --bin litebike --features warp,git2 -- completion` prints the
    bash completion script
