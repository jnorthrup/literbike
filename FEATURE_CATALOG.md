# Feature Catalog - Literbike Consolidation

## Bottom-Up Feature Map (Leaf → Root)

### Userspace Crate (lib.rs exports)
```
userspace/src/
├── concurrency/     # CCEK pattern, CancellationToken, Job, SuspendToken, channels
├── nio/              # Non-blocking I/O: reactor, backends (epoll/kqueue/io_uring)
├── kernel/           # io_uring via FFI (libc::syscall), posix_sockets, ebpf
├── network/          # adapters (Http, Quic, Ssh)
├── htx/              # HTX ticket verification (X25519 + HKDF) ✅ EXTRACTED
├── tensor/           # ML/tensor ops
├── database/         # DuckDB integration
└── concurrency/      # Same as above
```

### Literbike Crate (lib.rs exports)
```
literbike/src/
├── concurrency/    # CCEK + tokio bridge (DUPLICATE - should use userspace)
├── reactor/        # Select reactor (DUPLICATE - should use userspace::nio)
├── userspace_nio/  # Re-exports userspace::nio (gated, Linux-only) ✅ FIXED
├── endgame/        # Processing path selection
├── rbcursive/     # Protocol recognition via byte cursors
├── quic/           # QUIC protocol implementation
├── http/           # HTTP server
├── sctp/           # SCTP protocol
├── uring/          # liburing facade
├── simd/           # SIMD optimizations
└── [30+ other modules]
```

## FFI Architecture

The codebase uses **direct libc FFI** for kernel interaction:
- `libc::syscall` for io_uring operations (SYS_io_uring_*)
- Standard POSIX via libc crate (socket, bind, connect, etc.)
- Optional MLIR bindings via bindgen (build.rs)

No smol - async via userspace nio reactor using std::future + custom executor.

## Consolidation Completed ✅

### 1. HTX Extraction ✅
- Created `userspace/src/htx/` with betanet-htx code
- Added `htx` feature to userspace Cargo.toml

### 2. betanet References Removed ✅
- `src/endgame/endgame.rs` - kernel module check changed from "betanet" to "litebike"
- `src/kafka_replacement_smoke.rs` - removed betanet comment
- `src/quic/mod.rs` - updated HTX comment

### 3. userspace nio Fixes ✅
- Added `#[cfg(target_os = "linux")]` guards for io_uring constants
- Added `unsafe impl Send/Sync for MmapBuffer`
- Fixed `SuspendFuture::now_or_never` type ambiguity
- Added `nio` feature gate (Linux-only due to io_uring)

## Pre-existing Structural Issues (NOT caused by consolidation)

These issues existed before consolidation and block compilation:

| Issue | Module | Fix Needed |
|-------|--------|-----------|
| Missing `crate::core_types` | unknown | Create module or stub |
| Missing `crate::indexed` | unknown | Create module or stub |
| Missing `zeroize` dep | Cargo.toml | Add `zeroize = "1.0"` |
| Missing `crypto::SimdCrypto` | unknown | Create module or add dep |
| `rbcursive::RbCursorConfig` not found | rbcursive | Implement or stub |
| `bridge::CcekRuntime` not found | concurrency | Implement or stub |
| `endgame::UringFacade` not found | endgame | Implement or stub |
| `HtxError` not found | unknown | Create or import |
| `HtxTcp` enum variant missing | rbcursive::Protocol | Add variant |

## External Dependency Overlaps

### Literbike externals:
| Crate | Purpose | Can Replace With |
|-------|---------|------------------|
| tokio | async runtime | userspace nio via FFI (241 usages) |
| async-channel | channels | userspace::concurrency |
| futures | futures traits | std::future |
| crossbeam | parallelism | userspace |
| parking_lot | mutex/rwlock | userspace |

### Userspace externals (via FFI):
| Crate | Purpose |
|-------|---------|
| libc | Direct syscall FFI |
| futures | std::future |
| parking_lot | sync primitives |
| dashmap | concurrent map |
| bytemuck | byte casting |

## TODO: Remaining Work

1. **Fix pre-existing structural issues** - Missing modules, imports, dependencies
2. **HTTP/HTX unification** - Wire http module to userspace network adapters
3. **Replace tokio** - 241 usages need migration to userspace::nio (via FFI)
4. **Remove duplicate concurrency** - literbike/src/concurrency vs userspace::concurrency
