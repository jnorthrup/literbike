# Implementation Plan: Bun JSON Rust Confluence

## Phase 1: Core Rust JSON Module

### Task 1.1: Create Module Structure
- [ ] Create `src/json/mod.rs`
- [ ] Create `src/json/parser.rs`
- [ ] Create `src/json/pool.rs`
- [ ] Create `src/json/error.rs`
- [ ] Add to `src/lib.rs` under `json` feature
- **Verification:** `cargo check --features json` passes

### Task 1.2: Thread-Safe Pool Implementation
- [ ] Implement `AtomicPool<T>` in `src/json/pool.rs`
- [ ] Use `crossbeam::queue::SegQueue` for node pool
- [ ] Add generic node reuse with `Arc<Node>`
- [ ] Implement drop safety with proper cleanup
- **Verification:** `cargo test --features json --lib pool` passes

### Task 1.3: Fast JSON Parser
- [ ] Implement `FastJsonParser` in `src/json/parser.rs`
- [ ] Use `serde_json` for base parsing
- [ ] Add `simd-json` backend as optional feature
- [ ] Support JSON5 extensions via `pest` grammar
- **Verification:** Parse valid JSON and JSON5 test vectors

### Task 1.4: Error Handling
- [ ] Define `JsonError` enum in `src/json/error.rs`
- [ ] Implement `Display` and `Error` traits
- [ ] Add position tracking (line, column)
- [ ] Support Bun-compatible error codes
- **Verification:** Error messages match Bun's format

## Phase 2: Bun Compatibility Layer

### Task 2.1: AST Compatibility
- [ ] Define `Expr` type matching Bun's AST
- [ ] Implement conversion from `serde_json::Value`
- [ ] Support object, array, string, number, boolean, null
- [ ] Add location tracking (`logger.Loc` equivalent)
- **Verification:** AST structure matches `js_ast.Expr`

### Task 2.2: Bun API Emulation
- [ ] Implement `parse()` function matching Bun's signature
- [ ] Add `parseJSON5()` for JSON5 support
- [ ] Support `parseTSConfig()` for TypeScript config
- [ ] Handle allocator emulation with `bumpalo`
- **Verification:** Function signatures compatible with Bun

### Task 2.3: Performance Optimization
- [ ] Benchmark against Bun's Zig parser
- [ ] Profile hotspots with `flamegraph`
- [ ] Add specialization for common patterns
- [ ] Implement zero-copy parsing where possible
- **Verification:** Performance within 2x of Bun's parser

## Phase 3: FFI Integration

### Task 3.1: C ABI Bindings
- [ ] Create `literbike-ffi/src/json.rs`
- [ ] Expose `literbike_json_parse()` function
- [ ] Handle string marshaling (UTF-8 validation)
- [ ] Return opaque pointer to AST
- **Verification:** C header file compiles

### Task 3.2: Memory Management
- [ ] Implement `literbike_json_free()` for AST cleanup
- [ ] Add reference counting for shared substrings
- [ ] Handle panics across FFI boundary
- [ ] Document memory ownership rules
- **Verification:** Valgrind shows no leaks

### Task 3.3: Bun Integration Tests
- [ ] Create test harness in Bun repo
- [ ] Call Rust parser from Zig via FFI
- [ ] Compare output with native parser
- [ ] Add regression test suite
- **Verification:** All existing Bun JSON tests pass

## Phase 4: Testing & Validation

### Task 4.1: Unit Tests
- [ ] Test all JSON value types
- [ ] Test error conditions
- [ ] Test thread safety (concurrent parsing)
- [ ] Test pool reuse under load
- **Verification:** `cargo test --features json` passes all

### Task 4.2: Fuzzing
- [ ] Set up `cargo-fuzz`
- [ ] Create fuzz target for parser
- [ ] Run overnight fuzzing campaigns
- [ ] Fix any found panics/crashes
- **Verification:** No crashes after 24h fuzzing

### Task 4.3: Benchmarks
- [ ] Create `benches/json_parse.rs`
- [ ] Benchmark against Bun's parser
- [ ] Benchmark against `serde_json`
- [ ] Benchmark against `simd-json`
- **Verification:** Document performance characteristics

## Acceptance Criteria

1. **Correctness:** 100% pass rate on Bun's JSON test suite
2. **Thread Safety:** No data races under 100 concurrent parsers
3. **Performance:** Within 2x of Bun's Zig parser
4. **Memory Safety:** Zero memory leaks (verified by Valgrind)
5. **API Compatibility:** Drop-in replacement for Bun's JSON API

## Rollout Plan

1. Feature flag `json` in literbike
2. Optional integration in Bun via build tag
3. A/B testing in production
4. Gradual rollout with metrics
5. Full replacement once validated
