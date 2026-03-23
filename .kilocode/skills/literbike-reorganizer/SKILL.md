# Literbike Reorganization Skill

## Vision

```
[Protocol Bytes] → [Broadcast Channel] → [Async Combinators] → [Flow] → [ENDGAME]
     │                    │                    │                   │          │
  Kernel             Fan-out            Speculative         N-way        Densified
  Bypass             to                Longest              Down-        Processing
  (io_uring)         Tributaries       Match                stream       (SIMD)
```

## Interned Structure (AFTER)

```
src/
├── ccek/                          # CCEK Core (was ccek_sdk/)
│   ├── context.rs                # Context enum: Empty | Cons { Element, tail: Arc<Context> }
│   ├── element.rs                # Element trait + implementations (Htx, QUIC, HTTP, NIO)
│   ├── key.rs                    # Key<E> trait (compile-time singleton)
│   ├── job.rs                    # Job interface for structured concurrency
│   ├── scope.rs                  # CoroutineScope implementation
│   └── mod.rs
│
├── channels/                     # Communication Primitives
│   ├── broadcast.rs              # BroadcastChannel: 1-to-many fan-out
│   ├── mpsc.rs                  # Multi-producer single-consumer channels
│   ├── sync.rs                  # Synchronous channels
│   └── mod.rs
│
├── flow/                         # Flow-based Reactive Streams
│   ├── core.rs                  # Flow<T>, FlowCollector<T>, suspend fn operators
│   ├── operators.rs             # map, filter, take, zip, merge, etc.
│   ├── combinators.rs           # AsyncCombinator: speculative parsing
│   └── mod.rs
│
├── litebike/                     # INTERNED from ../litebike
│   ├── agent_8888.rs            # Port 8888 protocol detection
│   ├── agents/                  # Model hierarchy, web tools
│   ├── keymux/                  # Token ledger, DSEL
│   ├── rbcursive/               # Recursive combinators, SIMD, patterns
│   │   ├── protocols/
│   │   └── simd/
│   └── mod.rs
│
├── userspace/                    # INTERNED from ../userspace
│   ├── concurrency/             # Structured concurrency primitives
│   ├── kernel/                  # Kernel interfaces
│   ├── network/                 # Network protocols
│   ├── nio/                     # Non-blocking I/O
│   ├── htx/                     # HTX protocol
│   ├── tensor/                  # Tensor operations
│   ├── database/                # Database interfaces
│   └── mod.rs
│
├── betanet/                      # INTERNED from ../betanet
│   ├── densifier.rs             # Densification logic
│   ├── simd_match.rs            # SIMD pattern matching
│   ├── tensor_core.rs           # Tensor core
│   ├── isam_index.rs            # ISAM index
│   ├── columnar_mmap.rs         # Columnar mmap
│   ├── oroboros_slsa/           # SLSA verification
│   │   ├── bootstrap.rs
│   │   ├── canonicalizer.rs
│   │   ├── kernel_attestation.rs
│   │   └── self_hosting_verifier.rs
│   ├── unified_cursor/          # Unified cursor abstraction
│   │   ├── columnar.rs
│   │   ├── isam_core.rs
│   │   ├── mlir_bridge.rs
│   │   └── simd_ops.rs
│   └── literbike/               # Nested betanet literbike
│       └── lib.rs
│
├── userspace_kernel/             # Kernel Bypass Layer (io_uring)
│   ├── io_uring.rs             # Linux io_uring backend
│   ├── nio.rs                  # NIO reactor abstraction
│   ├── session_island.rs        # Session isolation + CCEK context
│   └── mod.rs
│
├── userspace_network/            # Protocol Tributaries
│   ├── quic/                   # QUIC protocol reactor
│   ├── htx/                    # HTX protocol reactor
│   ├── http/                   # HTTP protocol reactor
│   ├── sctp/                   # SCTP protocol reactor
│   └── mod.rs
│
├── endgame/                     # Densification Layer (SIMD)
│   ├── densification.rs        # Zero-allocation, cache-friendly processing
│   ├── simd/                   # SIMD primitives (scanner, crypto)
│   └── mod.rs
│
└── adapters/                    # External System Adapters
    ├── couchdb/
    ├── ipfs/
    └── kafka/
```

## Agent8888 Composition

| Agent | Subagent | Responsibility | Module |
|-------|----------|----------------|--------|
| **orchestrator** | - | Coordinate, track checklist | - |
| **ccek-agent** | - | Context/Element/Key/Job/Scope | `src/ccek/` |
| **channel-agent** | - | BroadcastChannel, MPSC | `src/channels/` |
| **flow-agent** | - | Flow<T>, AsyncCombinator | `src/flow/` |
| **modelmux-agent** | - | **INTERNED** - agent orchestration | `src/litebike/keymux/` |
| **agent8888-agent** | - | **INTERNED** - port 8888 protocol | `src/litebike/agent_8888.rs` |
| **rbcursive-agent** | - | **INTERNED** - recursive patterns | `src/litebike/rbcursive/` |
| **userspace-agent** | kernel | **INTERNED** - kernel interfaces | `src/userspace/kernel/` |
| | nio | **INTERNED** - NIO reactor | `src/userspace/nio/` |
| | network | **INTERNED** - network | `src/userspace/network/` |
| **betanet-agent** | slsa | **INTERNED** - attestation | `src/betanet/oroboros_slsa/` |
| | cursor | **INTERNED** - unified cursor | `src/betanet/unified_cursor/` |
| | simd | **INTERNED** - SIMD ops | `src/betanet/simd_match.rs` |
| **kernel-agent** | - | io_uring bypass | `src/userspace_kernel/` |
| **endgame-agent** | - | SIMD densification | `src/endgame/` |
| **polisher** | - | Final review | all |

## Workflow

```
Phase 1 (parallel):
  - ccek-agent → implement CCEK Context with COW
  - channel-agent → implement BroadcastChannel
  - flow-agent → implement Flow + AsyncCombinator

Phase 2 (parallel):
  - kernel-agent → wire io_uring
  - userspace-agent → verify internalized modules compile

Phase 3:
  - betanet-agent → verify betanet internals
  - endgame-agent → SIMD integration

Phase 4:
  - polisher → consistency audit
  - orchestrator → cargo check + cargo test
```

## Migration Checklist

- [x] Intern `../litebike/` → `src/litebike/`
- [x] Intern `../userspace/` → `src/userspace/`
- [x] Intern `../betanet/` → `src/betanet/`
- [ ] Remove betanet symlinks from externals
- [ ] Implement CCEK Context enum
- [ ] Create BroadcastChannel
- [ ] Create Flow + AsyncCombinator
- [ ] Wire io_uring to CCEK
- [ ] Update Cargo.toml workspace
- [ ] Verify cargo check --lib
- [ ] Verify cargo test

## Constraints

1. **No tokio**: stdlib + POSIX only
2. **Immutable CCEK Context**: COW semantics
3. **Element scope safety**: Elements only call Key/Element functions
4. **Parent→child widening**: Structured concurrency
