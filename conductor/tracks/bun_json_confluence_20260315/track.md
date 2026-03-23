# Bun JSON Rust Confluence - Daily Driver

## Track: bun_json_confluence_20260315

### Context
Bun's JSON parser has critical thread safety issues in the `HashMapPool` implementation. This track creates a Rust replacement that is memory-safe, performant, and provides a migration path for concurrent JSON parsing workloads.

### Current Status: [~] In Progress

### Implementation Details

#### Race Conditions Fixed
1. **HashMapPool::get()** - Replaced with lock-free SegQueue
2. **HashMapPool::release()** - Atomic operations via crossbeam
3. **Initialization** - Safe using `OnceLock` pattern

#### Architecture
```
literbike/src/json/
├── mod.rs          # Module exports, feature gates
├── parser.rs       # FastJsonParser with serde/simd backends
├── pool.rs         # AtomicPool<T> using crossbeam queues
└── error.rs        # JsonError with position tracking

literbike-ffi/src/json.rs  # C ABI for Bun integration
```

### Dependencies Added
- `serde_json = "1.0"` - Base JSON parsing
- `simd-json = { version = "0.13", optional = true }` - SIMD acceleration
- `crossbeam = "0.8"` - Lock-free concurrent queues
- `ahash = "0.8"` - Fast hashing for duplicate detection
- `pest = { version = "2.7", optional = true }` - JSON5 extensions
- `pest_derive = { version = "2.7", optional = true }`

### Integration with Literbike
- Feature flag `json` in `Cargo.toml`
- Exported via `literbike::json` module
- Optional FFI via `literbike-ffi` crate

### Testing Strategy
1. Unit tests for pool, parser, error handling
2. Concurrency stress tests (100+ concurrent parsers)
3. Fuzzing with `cargo-fuzz`
4. Benchmarking vs Bun's Zig parser
5. Integration tests via Bun FFI

### Performance Targets
- Throughput: Within 2x of Bun's Zig parser
- Latency: P99 < 2x Bun's parser
- Memory: No unbounded growth
- Thread Safety: No locks in hot path

### Next Steps
1. Implement `AtomicPool<T>` (Task 1.2)
2. Implement `FastJsonParser` (Task 1.3)
3. Add Bun-compatible AST (Task 2.1)
4. Create FFI bindings (Task 3.1)

### Blockers
None - all dependencies available in Rust ecosystem

### Risks
- FFI performance overhead
- Bun's AST structure complexity
- Maintaining parity with Zig features

### Mitigation
- Benchmark early and often
- Start with subset of features
- Incremental validation against Bun tests
