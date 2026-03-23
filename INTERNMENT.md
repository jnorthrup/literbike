# Unified Literbike Structure

All codebases consolidated under `/literbike/src/`:

```
literbike/src/
├── [original modules]
├── htx/                     # HTX ticket verification (from betanet)
├── userspace_concurrency/    # CCEK patterns, CancellationToken, channels
├── userspace_nio_module/    # Non-blocking I/O reactor
├── userspace_kernel/         # io_uring FFI, syscall_net
├── userspace_network/        # Network adapters
├── userspace_nio.rs         # (legacy wrapper - to be removed)
└── ...
```

## Consolidation Summary

| Origin | Module | Destination |
|--------|--------|------------|
| betanet-htx | HTX ticket verification | `src/htx/` |
| userspace | concurrency | `src/userspace_concurrency/` |
| userspace | nio | `src/userspace_nio_module/` |
| userspace | kernel | `src/userspace_kernel/` |
| userspace | network | `src/userspace_network/` |

## Features

```toml
# Cargo.toml features
htx = ["dep:sha2", "dep:hkdf", "dep:x25519-dalek", "dep:subtle"]
userspace-nio = ["dep:libc"]
userspace-kernel = ["dep:libc"]
userspace-network = []
```

## Removed External Dependencies

- `userspace = { path = "../userspace" }` - NO LONGER NEEDED

All userspace code is now inline under `src/userspace_*/`.
