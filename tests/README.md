# LiteBike Test Suite

This directory contains a comprehensive test framework for the LiteBike proxy project, covering unit tests, integration tests, performance benchmarks, and feature validation.

## Test Organization
,
### `/unit/` - Unit Tests

- **`protocol_detection.rs`** - Basic protocol detector accuracy tests
- **`protocol_detection_enhanced.rs`** - Advanced testing with property-based testing, SIMD validation, and edge cases
- **`protocol_registry.rs`** - Protocol registry functionality tests
- Coverage: Protocol detection accuracy, edge cases, false positive/negative rates, performance regression

### `/integration/` - Integration Tests

- **`proxy_functionality.rs`** - End-to-end proxy functionality on unified port 8888
- **`comprehensive_scenarios.rs`** - Real-world scenarios with mixed protocols, network conditions, and enterprise workflows
- Coverage: Multi-protocol scenarios, connection handling, error recovery, load testing

### `/benchmarks/` - Performance Tests

- **`protocol_detection_benchmarks.rs`** - Comprehensive performance benchmarks using Criterion
- Coverage: Detection latency, throughput, SIMD vs scalar, memory usage, concurrency scaling

### `/feature_gates/` - Feature Gate Tests

- **`mod.rs`** - Tests for all Cargo feature combinations and binary size validation
- Coverage: Feature compatibility matrix, build time regression, minimal/maximal builds

### `/fixtures/` - Test Data

- Protocol sample data for each supported protocol
- Mock network traffic patterns
- Test certificates and keys for TLS testing
- Configuration files for various scenarios

### `/utils/` - Test Utilities Framework



- **`mod.rs`** - Core testing utilities and configuration

- **`mock_servers.rs`** - Various mock servers (HTTP, echo, SOCKS5, DNS, slow, unreliable)

- **`protocol_generators.rs`** - Protocol data generators for testing and fuzzing
  - Includes HTTP (PAC/WPAD variants), SOCKS5, TLS, DoH, Bonjour/mDNS, and UPnP/SSDP
- **`test_macros.rs`** - Convenient macros for common testing patterns
- **`network_simulation.rs`** - Network condition simulation (latency, bandwidth, packet loss)

- **`performance_helpers.rs`** - Performance measurement and benchmarking utilities




## Running Tests


### Basic Test Execution

```bash

# Run all tests


cargo test




# Run specific test suites





# Run benchmarks


cargo bench



# Run with specific features
cargo test --features full
cargo test --features basic-proxy

cargo test --features doh


cargo test --no-default-features

```




### Advanced Test Scenarios




```bash


# Run with network simulation

RUST_LOG=debug cargo test comprehensive_scenarios


# Run property-based tests
cargo test property_based_tests

# Run SIMD validation tests (requires simd feature)

cargo test --features simd simd_validation

# Run performance regression tests
cargo test regression_benchmarks

# Run feature gate tests (slow)
cargo test --test feature_gates --ignored

# Run load tests
cargo test --test comprehensive_scenarios test_protocol_detection_under_load
```

### Remote smoke test (SSH + :8888)

```bash
# Configure host/user/dir
LB_USER="${LB_USER:-jim}"
LB_HOST="${LB_HOST:-host.example.com}"
LB_DIR="${LB_DIR:-/opt/litebike}"

# Deploy latest, run on :8888, and validate with curl
ssh -o StrictHostKeyChecking=accept-new "${LB_USER}@${LB_HOST}" '
  set -euo pipefail
  LB_DIR="${LB_DIR:-/opt/litebike}"
  [ -d "$LB_DIR/.git" ] && { cd "$LB_DIR"; git fetch --all --prune; BRANCH="$(git rev-parse --abbrev-ref HEAD || echo main)"; git checkout "$BRANCH"; git reset --hard "origin/$BRANCH"; } \
    || { mkdir -p "$LB_DIR"; cd "$LB_DIR"; git clone --depth=1 https://github.com/jnorthrup/litebike .; }
  cargo build --release --bin litebike-proxy
  pkill -f "litebike-proxy.*8888" 2>/dev/null || true
  nohup env BIND_IP=0.0.0.0:8888 ./target/release/litebike-proxy > litebike.log 2>&1 &
  sleep 1
'

curl -I -x "http://${LB_HOST}:8888" https://example.com
```

### macOS proxy smoke test (local PAC + :8888)

```bash
# Assumes litebike-proxy is listening on :8888 locally or reachable at $HOST_IP:8888
HOST_IP="$(ipconfig getifaddr en0 || ipconfig getifaddr en1 || echo 127.0.0.1)"
svc="$(networksetup -listallnetworkservices | sed -n '2p')"

# Check current proxy settings
scutil --proxy
networksetup -getautoproxyurl "$svc"

# Quick verification through HTTP proxy on :8888
curl -I -x "http://${HOST_IP}:8888" https://example.com || echo "Proxy test failed"
```

### Test Configuration

```bash
# Enable verbose logging
RUST_LOG=debug cargo test

# Set test timeout (for slow tests)
RUST_TEST_TIMEOUT=300 cargo test

# Run tests with memory tracking
RUST_TEST_MEMORY_TRACK=1 cargo test

# Run only fast tests (exclude slow integration tests)
cargo test --exclude-ignored
```

## Continuous Integration

### GitHub Actions Workflow

```yaml
name: Comprehensive Test Suite

on: [push, pull_request]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        features: [
          "",                    # No features
          "basic-proxy",         # Default
          "doh",                 # DNS-over-HTTPS
          "auto-discovery",      # Service discovery
          "upnp",               # UPnP support
          "full"                # All features
        ]
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run tests
        run: |
          if [ -z "${{ matrix.features }}" ]; then
            cargo test --no-default-features
          else
            cargo test --features "${{ matrix.features }}"
          fi

  integration-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run integration tests
        run: cargo test --test comprehensive_scenarios

  performance-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run benchmarks
        run: cargo bench

  feature-gate-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Test feature combinations
        run: cargo test --test feature_gates --ignored
```

### Local CI Script

```bash
#!/bin/bash
# scripts/ci-test.sh - Local CI testing script

set -e

echo "=== Running Comprehensive Test Suite ==="

# Unit tests with different feature combinations
echo "Testing feature combinations..."
for features in "" "basic-proxy" "doh" "auto-discovery" "upnp" "full"; do
    echo "Testing features: ${features:-none}"
    if [ -z "$features" ]; then
        cargo test --no-default-features --lib
    else
        cargo test --features "$features" --lib
    fi
done

# Integration tests
echo "Running integration tests..."
cargo test --test comprehensive_scenarios

# Property-based tests
echo "Running enhanced unit tests..."
cargo test --test protocol_detection_enhanced

# Performance regression tests
echo "Running performance tests..."
cargo test regression_benchmarks

# Benchmarks
echo "Running benchmarks..."
cargo bench

echo "=== All tests passed! ==="
```

## Test Categories and Coverage

### Protocol Detection Tests

- **Basic Accuracy**: Valid protocol recognition with confidence scoring
- **Edge Cases**: Malformed data, partial headers, unusual patterns
- **Property-Based**: Deterministic detection, bounds checking, consistency
- **Performance**: Detection latency, memory usage, regression prevention
- **SIMD Validation**: SIMD vs scalar equivalence and performance

### Integration Tests

- **Multi-Protocol**: Simultaneous HTTP, SOCKS5, TLS, DoH handling
- **Real-World Scenarios**: Web browsing simulation, enterprise workflows
- **Network Conditions**: High latency, low bandwidth, packet loss, disconnections
- **Load Testing**: Concurrent connections, protocol detection under stress
- **Error Handling**: Unreachable targets, malformed requests, timeouts

### Performance Benchmarks

- **Protocol Detection**: Individual detector latency and throughput
- **SIMD Optimization**: Performance comparison with scalar implementations
- **Input Size Scaling**: Performance characteristics with varying data sizes
- **Worst-Case Scenarios**: Pathological inputs and stress patterns
- **Memory Usage**: Allocation patterns and memory efficiency

### Feature Gate Tests

- **Build Validation**: All feature combinations compile successfully
- **Binary Size**: Size limits for minimal and full builds
- **Compile Time**: Build time regression prevention
- **Functionality**: Feature-specific functionality verification
- **Compatibility Matrix**: Feature interaction validation

## Coverage Goals and Metrics

### Current Coverage Targets

- **Code Coverage**: 95%+ line coverage across all modules
- **Protocol Accuracy**: 95%+ detection accuracy for valid protocols
- **False Positive Rate**: <5% for random/invalid data
- **Performance**: <1μs average detection latency per protocol
- **Memory**: <1MB peak memory usage for standard workloads
- **Build Time**: <5 minutes for full feature build

### Monitoring and Regression Prevention

- **Performance Baselines**: Automated comparison against historical performance
- **Binary Size Tracking**: Alerts for significant size increases
- **Feature Parity**: Ensuring feature combinations maintain functionality
- **API Stability**: Testing for breaking changes in public interfaces

## Writing New Tests

### Test Structure Guidelines

```rust
// Follow this pattern for new tests
mod new_test_module {
    use super::*;
    use crate::utils::*;

    #[test]
    fn test_specific_functionality() {
        setup_test_logging();
        
        // Test setup
        let test_data = create_test_data();
        
        // Execute test
        let result = function_under_test(test_data);
        
        // Verify results
        assert!(result.is_ok());
        verify_expected_behavior(result);
    }
    
    #[tokio::test]
    async fn test_async_functionality() {
        // For async tests
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

### Performance Test Guidelines

```rust
use crate::utils::{BenchmarkRunner, Timer};

#[test]
fn test_performance_requirement() {
    let runner = BenchmarkRunner::new()
        .with_measurement_iterations(1000);
    
    let metrics = runner.benchmark("operation_name", &test_data, |data| {
        // Operation to benchmark
    });
    
    // Verify performance requirements
    assert!(metrics.average_duration < Duration::from_micros(1000));
    assert!(metrics.memory_usage_bytes < 1024 * 1024);
}
```

### Integration Test Guidelines

```rust
use crate::utils::*;

#[tokio::test]
async fn test_end_to_end_scenario() {
    let registry = setup_full_registry().await;
    let (proxy_addr, _handle) = start_proxy_server(registry).await;
    
    // Set up mock servers
    let target_server = MockHttpServer::new(responses).await.unwrap();
    tokio::spawn(target_server.run());
    
    // Test the complete workflow
    let result = test_proxy_connection(proxy_addr, target_addr).await;
    
    assert!(result.is_ok());
    verify_end_to_end_behavior(result);
}
```

## Troubleshooting Test Issues

### Common Issues

1. **Flaky Tests**: Use `run_with_retries` utility for network-dependent tests
2. **Timeout Issues**: Increase timeouts for slow operations or CI environments
3. **Port Conflicts**: Use `TcpListener::bind("127.0.0.1:0")` for dynamic ports
4. **Feature Gate Failures**: Ensure conditional compilation is correct
5. **Performance Variations**: Use relative performance comparisons, not absolute

### Debugging Tests

```bash
# Run with detailed logging
RUST_LOG=debug cargo test test_name -- --nocapture

# Run specific test with backtrace
RUST_BACKTRACE=1 cargo test test_name

# Run test repeatedly to catch flakiness
for i in {1..10}; do cargo test test_name || break; done

# Profile memory usage
cargo test --features memory-profiling test_name
```

This comprehensive test suite ensures the LiteBike proxy maintains high quality, performance, and reliability across all supported features and use cases.

## Example Protocol Generator Usage

```rust
#[test]
fn mdns_and_ssdp_samples() {
    use crate::utils::protocol_generators::{MdnsTestData, SsdpTestData, HttpTestData};
    let mdns = MdnsTestData;
    let ssdp = SsdpTestData;
    let http = HttpTestData;

    // PAC/WPAD probes
    let pac = http.generate_pac_requests();
    assert!(!pac.is_empty());

    // Bonjour/mDNS queries
    let q = mdns.valid_requests();
    assert!(q.iter().any(|m| m.len() >= 12));

    // SSDP discovery frames
    let s = ssdp.valid_requests();
    assert!(s.iter().any(|r| r.starts_with(b"M-SEARCH")));
}
```
