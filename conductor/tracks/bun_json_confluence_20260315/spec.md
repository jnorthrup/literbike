# Bun JSON Rust Confluence Track

## Problem Statement

Bun's JSON parser (`src/interchange/json.zig`) contains thread safety issues in the `HashMapPool` implementation that can cause memory corruption and data races under concurrent parsing workloads.

### Identified Race Conditions

1. **HashMapPool::get()** (lines 21-32)
   - Non-atomic check of `loaded` flag
   - `popFirst()` on linked list without synchronization
   - Multiple threads can retrieve the same node simultaneously

2. **HashMapPool::release()** (lines 34-42)
   - Non-atomic `prepend()` operation
   - Concurrent releases corrupt linked list structure
   - No memory barriers or atomic operations

3. **Initialization Race** (line 9)
   - `loaded` flag set without atomic store
   - First-time initialization not synchronized
   - Undefined behavior when multiple threads race initialization

### Impact

- Memory corruption in concurrent JSON parsing
- Use-after-free vulnerabilities
- Crashes in production workloads
- Data integrity violations

## Solution: Rust-based JSON Confluence

Replace Bun's thread-unsafe Zig JSON parser with a Rust implementation that:
1. Uses proper atomic operations for pool management
2. Leverages Rust's type system for memory safety
3. Provides SIMD-accelerated JSON parsing
4. Maintains API compatibility with Bun's existing interface

## Implementation Phases

### Phase 1: Core JSON Parser in Rust
- [ ] Create `src/json/` module in literbike
- [ ] Implement `FastJsonParser` with `serde_json` backend
- [ ] Add SIMD optimizations using `simd-json` crate
- [ ] Support JSON5 extensions (comments, trailing commas, unquoted keys)

### Phase 2: Thread-Safe Pool Management
- [ ] Implement `AtomicPool<T>` using `crossbeam` crate
- [ ] Use lock-free MPMC queue for node recycling
- [ ] Add benchmark suite comparing to Bun's HashMapPool

### Phase 3: Bun FFI Integration
- [ ] Create C ABI bindings for JSON parser
- [ ] Expose via `literbike-ffi` crate
- [ ] Add benchmark comparing Bun Zig vs Rust implementations

### Phase 4: Validation & Testing
- [ ] Port Bun's JSON test suite
- [ ] Add fuzzing with `cargo-fuzz`
- [ ] Performance benchmarks (parse speed, memory usage)
- [ ] Concurrency stress tests

## Out of Scope

- Complete Bun runtime replacement (only JSON parsing)
- JavaScript AST generation (use existing `js_parser`)
- Source map generation (handled separately)
- CSS/HTML parsing (different subsystems)

## Verification

```bash
# Build with JSON feature
cargo build --features json

# Run tests
cargo test --features json --lib

# Benchmark
cargo bench --features json --bench json_parse

# Fuzzing
cargo fuzz run json_parser fuzz_targets/json.rs
```

## Dependencies

- `serde` (1.0) - Serialization framework
- `serde_json` (1.0) - JSON parsing
- `simd-json` (0.13) - SIMD-accelerated JSON
- `crossbeam` (0.8) - Concurrent data structures
- `ahash` (0.8) - Fast hashing
- `pest` (2.7) - Parser combinator for JSON5
- `pest_derive` (2.7) - Derive macros for pest

## Integration Points

- `literbike/src/lib.rs` - Export `json` module under `json` feature
- `literbike-ffi/` - C bindings for Bun integration
- `literbike/Cargo.toml` - Add feature flags and dependencies
