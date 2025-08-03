// Test Macros and Helper Functions
// Provides convenient macros and functions for common testing patterns

use std::time::Duration;
use tokio::time::timeout;

/// Macro to create a simple test server setup
#[macro_export]
macro_rules! setup_test_server {
    ($server_type:expr, $config:expr) => {{
        let server = $server_type($config).await.expect("Failed to create test server");
        let addr = server.addr();
        let handle = tokio::spawn(async move { server.run().await });
        (addr, handle)
    }};
}

/// Macro to test protocol detection with expected results
#[macro_export]
macro_rules! test_protocol_detection {
    ($detector:expr, $input:expr, $expected_protocol:expr, $min_confidence:expr) => {{
        let result = $detector.detect($input);
        assert_eq!(result.protocol_name, $expected_protocol,
                  "Expected protocol '{}', got '{}'", $expected_protocol, result.protocol_name);
        assert!(result.confidence >= $min_confidence,
               "Expected confidence >= {}, got {}", $min_confidence, result.confidence);
        result
    }};
}

/// Macro to test protocol detection failure cases
#[macro_export]
macro_rules! test_protocol_detection_unknown {
    ($detector:expr, $input:expr) => {{
        let result = $detector.detect($input);
        assert_eq!(result.protocol_name, "unknown",
                  "Expected 'unknown', got '{}'", result.protocol_name);
        result
    }};
}

/// Macro to create a proxy test setup with timeout
#[macro_export]
macro_rules! setup_proxy_test {
    ($registry:expr, $timeout:expr) => {{
        use tokio::net::TcpListener;
        use std::sync::Arc;
        
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await
            .expect("Failed to bind proxy listener");
        let proxy_addr = proxy_listener.local_addr()
            .expect("Failed to get proxy address");
        
        let registry = Arc::new($registry);
        let server_handle = tokio::spawn(async move {
            while let Ok((stream, _)) = proxy_listener.accept().await {
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    let _ = registry.handle_connection(stream).await;
                });
            }
        });
        
        // Wait for server to start
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        (proxy_addr, server_handle)
    }};
}

/// Macro to test timeouts with custom error messages
#[macro_export]
macro_rules! test_with_timeout {
    ($duration:expr, $test:expr, $error_msg:expr) => {{
        match timeout($duration, $test).await {
            Ok(result) => result,
            Err(_) => panic!("Test timed out after {:?}: {}", $duration, $error_msg),
        }
    }};
}

/// Macro to run multiple tests concurrently and collect results
#[macro_export]
macro_rules! run_concurrent_tests {
    ($($test_name:ident: $test_expr:expr),* $(,)?) => {{
        let mut handles = Vec::new();
        
        $(
            let handle = tokio::spawn(async move {
                let result = $test_expr.await;
                (stringify!($test_name), result)
            });
            handles.push(handle);
        )*
        
        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok((name, result)) => {
                    match result {
                        Ok(_) => println!("✓ Test {} passed", name),
                        Err(e) => {
                            eprintln!("✗ Test {} failed: {:?}", name, e);
                            results.push((name, Err(e)));
                        }
                    }
                }
                Err(e) => {
                    eprintln!("✗ Test task panicked: {:?}", e);
                    results.push(("unknown", Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)));
                }
            }
        }
        
        if !results.is_empty() {
            panic!("Some concurrent tests failed: {:?}", results);
        }
    }};
}

/// Macro to benchmark protocol detection performance
#[macro_export]
macro_rules! benchmark_detection {
    ($detector:expr, $test_data:expr, $iterations:expr) => {{
        use std::time::Instant;
        
        // Warmup
        for data in $test_data.iter().take(10) {
            let _ = $detector.detect(data);
        }
        
        let start = Instant::now();
        for _ in 0..$iterations {
            for data in &$test_data {
                let _ = $detector.detect(data);
            }
        }
        let duration = start.elapsed();
        
        let total_detections = $iterations * $test_data.len();
        let per_detection = duration / total_detections as u32;
        
        println!("Benchmarked {} detections in {:?} ({:?} per detection)",
                total_detections, duration, per_detection);
        
        (duration, per_detection)
    }};
}

/// Macro to create test cases with expected results
#[macro_export]
macro_rules! protocol_test_cases {
    ($(($input:expr, $expected:expr, $confidence:expr)),* $(,)?) => {{
        vec![
            $(
                ($input.to_vec(), $expected, $confidence),
            )*
        ]
    }};
}

/// Helper function to create a test configuration
pub fn create_test_config() -> crate::utils::TestConfig {
    crate::utils::TestConfig {
        timeout: Duration::from_secs(10),
        max_connections: 100,
        buffer_size: 8192,
        enable_logging: false,
    }
}

/// Helper function to setup test logging conditionally
pub fn maybe_setup_logging() {
    if std::env::var("RUST_LOG").is_ok() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .try_init();
    }
}

/// Helper to assert error contains expected message
pub fn assert_error_contains<E: std::fmt::Display>(error: E, expected_substring: &str) {
    let error_string = error.to_string();
    assert!(error_string.contains(expected_substring),
           "Error '{}' does not contain expected substring '{}'",
           error_string, expected_substring);
}

/// Helper to create temporary data for testing
pub fn create_temp_test_data(size: usize, pattern: u8) -> Vec<u8> {
    vec![pattern; size]
}

/// Helper to verify connection metrics
pub fn verify_metrics(
    state: &crate::utils::TestState,
    expected_connections: u64,
    min_bytes: u64,
    max_errors: usize
) {
    let metrics = state.get_metrics();
    
    assert_eq!(metrics.connections_count, expected_connections,
              "Expected {} connections, got {}", expected_connections, metrics.connections_count);
    
    assert!(metrics.bytes_transferred >= min_bytes,
           "Expected at least {} bytes transferred, got {}", min_bytes, metrics.bytes_transferred);
    
    assert!(metrics.errors.len() <= max_errors,
           "Expected at most {} errors, got {}: {:?}", max_errors, metrics.errors.len(), metrics.errors);
}

/// Helper to create realistic HTTP requests
pub fn create_http_request(method: &str, path: &str, headers: &[(&str, &str)], body: Option<&str>) -> Vec<u8> {
    let mut request = format!("{} {} HTTP/1.1\r\n", method, path);
    
    for (name, value) in headers {
        request.push_str(&format!("{}: {}\r\n", name, value));
    }
    
    if let Some(body) = body {
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    
    request.push_str("\r\n");
    
    if let Some(body) = body {
        request.push_str(body);
    }
    
    request.into_bytes()
}

/// Helper to create SOCKS5 handshake
pub fn create_socks5_handshake(methods: &[u8]) -> Vec<u8> {
    let mut handshake = vec![0x05, methods.len() as u8];
    handshake.extend_from_slice(methods);
    handshake
}

/// Helper to create TLS ClientHello
pub fn create_tls_client_hello(version_major: u8, version_minor: u8) -> Vec<u8> {
    vec![0x16, version_major, version_minor, 0x00, 0x01] // Simplified
}

/// Helper function to wait for condition with timeout
pub async fn wait_for_condition<F, Fut>(
    mut condition: F,
    timeout_duration: Duration,
    check_interval: Duration,
) -> Result<(), &'static str>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = std::time::Instant::now();
    
    while start.elapsed() < timeout_duration {
        if condition().await {
            return Ok(());
        }
        tokio::time::sleep(check_interval).await;
    }
    
    Err("Condition not met within timeout")
}

/// Helper to measure memory usage during test
pub fn measure_memory_usage<F, R>(f: F) -> (R, usize)
where
    F: FnOnce() -> R,
{
    // Simple memory measurement - in real implementation you'd use more sophisticated tools
    let initial_memory = get_memory_usage();
    let result = f();
    let final_memory = get_memory_usage();
    
    (result, final_memory.saturating_sub(initial_memory))
}

fn get_memory_usage() -> usize {
    // Simplified - in real implementation, use system calls or tools like procfs
    // For now, just return 0
    0
}

/// Helper to create a range of test data sizes for performance testing
pub fn create_size_range_tests(min_size: usize, max_size: usize, steps: usize) -> Vec<usize> {
    if steps <= 1 {
        return vec![min_size];
    }
    
    let step_size = (max_size - min_size) / (steps - 1);
    (0..steps)
        .map(|i| min_size + i * step_size)
        .collect()
}

/// Helper to validate test results across multiple iterations
pub fn validate_consistent_results<T, F>(test_fn: F, iterations: usize) -> T
where
    F: Fn() -> T,
    T: PartialEq + std::fmt::Debug + Clone,
{
    let first_result = test_fn();
    
    for i in 1..iterations {
        let result = test_fn();
        assert_eq!(result, first_result,
                  "Inconsistent result on iteration {}: {:?} != {:?}",
                  i, result, first_result);
    }
    
    first_result
}

/// Helper to run flaky tests with retries
pub async fn run_with_retries<F, Fut, T, E>(
    test_fn: F,
    max_retries: usize,
    delay_between_retries: Duration,
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut last_error = None;
    
    for attempt in 0..=max_retries {
        match test_fn().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt < max_retries {
                    println!("Test attempt {} failed: {:?}, retrying...", attempt + 1, e);
                    tokio::time::sleep(delay_between_retries).await;
                }
                last_error = Some(e);
            }
        }
    }
    
    Err(last_error.unwrap())
}