// Performance Testing Helpers
// Provides utilities for measuring and benchmarking proxy performance

use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Performance metrics collector
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub operation_count: u64,
    pub total_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub average_duration: Duration,
    pub percentile_95: Duration,
    pub percentile_99: Duration,
    pub throughput_ops_per_sec: f64,
    pub memory_usage_bytes: u64,
    pub error_count: u64,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            operation_count: 0,
            total_duration: Duration::from_secs(0),
            min_duration: Duration::from_secs(u64::MAX),
            max_duration: Duration::from_secs(0),
            average_duration: Duration::from_secs(0),
            percentile_95: Duration::from_secs(0),
            percentile_99: Duration::from_secs(0),
            throughput_ops_per_sec: 0.0,
            memory_usage_bytes: 0,
            error_count: 0,
        }
    }
    
    pub fn from_durations(durations: &[Duration]) -> Self {
        if durations.is_empty() {
            return Self::new();
        }
        
        let mut sorted_durations = durations.to_vec();
        sorted_durations.sort();
        
        let operation_count = durations.len() as u64;
        let total_duration: Duration = durations.iter().sum();
        let min_duration = *sorted_durations.first().unwrap();
        let max_duration = *sorted_durations.last().unwrap();
        let average_duration = total_duration / operation_count as u32;
        
        let percentile_95_idx = (durations.len() as f64 * 0.95) as usize;
        let percentile_99_idx = (durations.len() as f64 * 0.99) as usize;
        let percentile_95 = sorted_durations[percentile_95_idx.min(sorted_durations.len() - 1)];
        let percentile_99 = sorted_durations[percentile_99_idx.min(sorted_durations.len() - 1)];
        
        let throughput_ops_per_sec = operation_count as f64 / total_duration.as_secs_f64();
        
        Self {
            operation_count,
            total_duration,
            min_duration,
            max_duration,
            average_duration,
            percentile_95,
            percentile_99,
            throughput_ops_per_sec,
            memory_usage_bytes: 0, // To be set separately
            error_count: 0,
        }
    }
    
    pub fn with_memory_usage(mut self, bytes: u64) -> Self {
        self.memory_usage_bytes = bytes;
        self
    }
    
    pub fn with_error_count(mut self, errors: u64) -> Self {
        self.error_count = errors;
        self
    }
}

/// Benchmark runner for protocol detection performance
pub struct BenchmarkRunner {
    warmup_iterations: usize,
    measurement_iterations: usize,
    memory_tracking: bool,
}

impl BenchmarkRunner {
    pub fn new() -> Self {
        Self {
            warmup_iterations: 1000,
            measurement_iterations: 10000,
            memory_tracking: false,
        }
    }
    
    pub fn with_warmup_iterations(mut self, iterations: usize) -> Self {
        self.warmup_iterations = iterations;
        self
    }
    
    pub fn with_measurement_iterations(mut self, iterations: usize) -> Self {
        self.measurement_iterations = iterations;
        self
    }
    
    pub fn with_memory_tracking(mut self, enabled: bool) -> Self {
        self.memory_tracking = enabled;
        self
    }
    
    /// Benchmark a function that takes input data
    pub fn benchmark<F, T>(&self, name: &str, test_data: &[T], mut operation: F) -> PerformanceMetrics
    where
        F: FnMut(&T) -> (),
        T: Clone,
    {
        println!("Starting benchmark: {}", name);
        
        // Warmup
        println!("  Warming up with {} iterations...", self.warmup_iterations);
        for _ in 0..self.warmup_iterations {
            for data in test_data.iter().take(10) {
                operation(data);
            }
        }
        
        // Measurement
        println!("  Measuring with {} iterations...", self.measurement_iterations);
        let mut durations = Vec::with_capacity(self.measurement_iterations);
        
        let memory_before = if self.memory_tracking { get_memory_usage() } else { 0 };
        
        for _ in 0..self.measurement_iterations {
            for data in test_data {
                let start = Instant::now();
                operation(data);
                let duration = start.elapsed();
                durations.push(duration);
            }
        }
        
        let memory_after = if self.memory_tracking { get_memory_usage() } else { 0 };
        let memory_usage = memory_after.saturating_sub(memory_before);
        
        let metrics = PerformanceMetrics::from_durations(&durations)
            .with_memory_usage(memory_usage);
        
        println!("  Completed benchmark: {}", name);
        println!("    Operations: {}", metrics.operation_count);
        println!("    Average: {:?}", metrics.average_duration);
        println!("    Min: {:?}", metrics.min_duration);
        println!("    Max: {:?}", metrics.max_duration);
        println!("    95th percentile: {:?}", metrics.percentile_95);
        println!("    99th percentile: {:?}", metrics.percentile_99);
        println!("    Throughput: {:.2} ops/sec", metrics.throughput_ops_per_sec);
        if self.memory_tracking {
            println!("    Memory usage: {} bytes", metrics.memory_usage_bytes);
        }
        
        metrics
    }
    
    /// Benchmark protocol detection specifically
    pub fn benchmark_detection<D>(&self, detector: &D, test_data: &[Vec<u8>]) -> PerformanceMetrics
    where
        D: literbike::protocol_registry::ProtocolDetector,
    {
        self.benchmark(
            &format!("Protocol Detection ({})", detector.protocol_name()),
            test_data,
            |data| {
                let _ = detector.detect(data);
            }
        )
    }
    
    /// Compare multiple detectors on the same data
    pub fn compare_detectors<D>(&self, detectors: &[D], test_data: &[Vec<u8>]) -> HashMap<String, PerformanceMetrics>
    where
        D: literbike::protocol_registry::ProtocolDetector,
    {
        let mut results = HashMap::new();
        
        for detector in detectors {
            let metrics = self.benchmark_detection(detector, test_data);
            results.insert(detector.protocol_name().to_string(), metrics);
        }
        
        // Print comparison
        println!("\nDetector Performance Comparison:");
        println!("{:<15} {:<12} {:<12} {:<12} {:<15}", "Detector", "Avg (μs)", "95th (μs)", "99th (μs)", "Ops/sec");
        println!("{:-<70}", "");
        
        for (name, metrics) in &results {
            println!("{:<15} {:<12.2} {:<12.2} {:<12.2} {:<15.2}",
                    name,
                    metrics.average_duration.as_micros(),
                    metrics.percentile_95.as_micros(),
                    metrics.percentile_99.as_micros(),
                    metrics.throughput_ops_per_sec);
        }
        
        results
    }
}

/// Concurrent performance tester
pub struct ConcurrentTester {
    concurrent_workers: usize,
    operations_per_worker: usize,
    shared_metrics: Arc<SharedMetrics>,
}

#[derive(Debug)]
struct SharedMetrics {
    total_operations: AtomicU64,
    total_errors: AtomicU64,
    start_time: Instant,
}

impl ConcurrentTester {
    pub fn new(concurrent_workers: usize, operations_per_worker: usize) -> Self {
        Self {
            concurrent_workers,
            operations_per_worker,
            shared_metrics: Arc::new(SharedMetrics {
                total_operations: AtomicU64::new(0),
                total_errors: AtomicU64::new(0),
                start_time: Instant::now(),
            }),
        }
    }
    
    /// Test concurrent protocol detection
    pub async fn test_concurrent_detection<D>(
        &self, 
        detector: Arc<D>, 
        test_data: Arc<Vec<Vec<u8>>>
    ) -> PerformanceMetrics
    where
        D: literbike::universal_listener::ProtocolDetector + Send + Sync + 'static + std::panic::RefUnwindSafe,
    {
        let mut tasks = Vec::new();
        let start_time = Instant::now();
        
        for worker_id in 0..self.concurrent_workers {
            let detector = Arc::clone(&detector);
            let test_data = Arc::clone(&test_data);
            let shared_metrics = Arc::clone(&self.shared_metrics);
            let operations_per_worker = self.operations_per_worker;
            
            let task = tokio::spawn(async move {
                let mut local_durations = Vec::new();
                let mut local_errors = 0;
                
                for _ in 0..operations_per_worker {
                    for data in test_data.iter() {
                        let start = Instant::now();
                        
                        match std::panic::catch_unwind(|| detector.detect(data)) {
                            Ok(_) => {
                                let duration = start.elapsed();
                                local_durations.push(duration);
                                shared_metrics.total_operations.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(_) => {
                                local_errors += 1;
                                shared_metrics.total_errors.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                }
                
                (worker_id, local_durations, local_errors)
            });
            
            tasks.push(task);
        }
        
        // Collect results from all workers
        let mut all_durations = Vec::new();
        let mut total_errors = 0;
        
        for task in tasks {
            match task.await {
                Ok((worker_id, durations, errors)) => {
                    println!("Worker {} completed: {} operations, {} errors", 
                            worker_id, durations.len(), errors);
                    all_durations.extend(durations);
                    total_errors += errors;
                }
                Err(e) => {
                    println!("Worker task failed: {:?}", e);
                    total_errors += self.operations_per_worker * test_data.len();
                }
            }
        }
        
        let total_duration = start_time.elapsed();
        let metrics = PerformanceMetrics::from_durations(&all_durations)
            .with_error_count(total_errors as u64);
        
        println!("Concurrent test completed:");
        println!("  Workers: {}", self.concurrent_workers);
        println!("  Total operations: {}", metrics.operation_count);
        println!("  Total duration: {:?}", total_duration);
        println!("  Errors: {}", total_errors);
        println!("  Concurrent throughput: {:.2} ops/sec", 
                metrics.operation_count as f64 / total_duration.as_secs_f64());
        
        metrics
    }
}

/// Memory usage tracker
pub struct MemoryTracker {
    initial_usage: u64,
    peak_usage: u64,
    measurements: Vec<(Instant, u64)>,
}

impl MemoryTracker {
    pub fn new() -> Self {
        let initial_usage = get_memory_usage();
        Self {
            initial_usage,
            peak_usage: initial_usage,
            measurements: vec![(Instant::now(), initial_usage)],
        }
    }
    
    pub fn record_measurement(&mut self) {
        let current_usage = get_memory_usage();
        if current_usage > self.peak_usage {
            self.peak_usage = current_usage;
        }
        self.measurements.push((Instant::now(), current_usage));
    }
    
    pub fn get_current_usage(&self) -> u64 {
        get_memory_usage()
    }
    
    pub fn get_peak_usage(&self) -> u64 {
        self.peak_usage
    }
    
    pub fn get_growth(&self) -> u64 {
        self.get_current_usage().saturating_sub(self.initial_usage)
    }
    
    pub fn get_measurements(&self) -> &[(Instant, u64)] {
        &self.measurements
    }
}

/// Throughput tester for network operations
pub struct ThroughputTester {
    duration: Duration,
    data_size: usize,
}

impl ThroughputTester {
    pub fn new(duration: Duration, data_size: usize) -> Self {
        Self { duration, data_size }
    }
    
    /// Test throughput of proxy connections
    pub async fn test_proxy_throughput(
        &self,
        proxy_addr: std::net::SocketAddr,
        target_addr: std::net::SocketAddr,
    ) -> ThroughputResult {
        use tokio::net::TcpStream;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        let start_time = Instant::now();
        let mut total_bytes_sent = 0;
        let mut total_bytes_received = 0;
        let mut connection_count = 0;
        let mut error_count = 0;
        
        let test_data = vec![0x41u8; self.data_size]; // 'A' repeated
        
        while start_time.elapsed() < self.duration {
            match TcpStream::connect(proxy_addr).await {
                Ok(mut stream) => {
                    connection_count += 1;
                    
                    // Send data through proxy
                    let request = format!(
                        "CONNECT {} HTTP/1.1\r\nHost: {}\r\n\r\n",
                        target_addr, target_addr
                    );
                    
                    if stream.write_all(request.as_bytes()).await.is_ok() {
                        total_bytes_sent += request.len();
                        
                        // Read CONNECT response
                        let mut response = [0u8; 1024];
                        if let Ok(n) = stream.read(&mut response).await {
                            total_bytes_received += n;
                            
                            // If successful, send test data
                            if response[..n].starts_with(b"HTTP/1.1 200") {
                                if stream.write_all(&test_data).await.is_ok() {
                                    total_bytes_sent += test_data.len();
                                    
                                    // Read echo response
                                    let mut echo_response = vec![0u8; test_data.len()];
                                    if let Ok(n) = stream.read(&mut echo_response).await {
                                        total_bytes_received += n;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    error_count += 1;
                }
            }
            
            // Small delay to prevent overwhelming the system
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        
        let actual_duration = start_time.elapsed();
        let bytes_per_second_sent = total_bytes_sent as f64 / actual_duration.as_secs_f64();
        let bytes_per_second_received = total_bytes_received as f64 / actual_duration.as_secs_f64();
        let connections_per_second = connection_count as f64 / actual_duration.as_secs_f64();
        
        ThroughputResult {
            duration: actual_duration,
            total_bytes_sent,
            total_bytes_received,
            connection_count,
            error_count,
            bytes_per_second_sent,
            bytes_per_second_received,
            connections_per_second,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThroughputResult {
    pub duration: Duration,
    pub total_bytes_sent: usize,
    pub total_bytes_received: usize,
    pub connection_count: usize,
    pub error_count: usize,
    pub bytes_per_second_sent: f64,
    pub bytes_per_second_received: f64,
    pub connections_per_second: f64,
}

impl ThroughputResult {
    pub fn print_summary(&self) {
        println!("Throughput Test Results:");
        println!("  Duration: {:?}", self.duration);
        println!("  Connections: {} ({:.2}/sec)", self.connection_count, self.connections_per_second);
        println!("  Errors: {}", self.error_count);
        println!("  Bytes sent: {} ({:.2} bytes/sec)", self.total_bytes_sent, self.bytes_per_second_sent);
        println!("  Bytes received: {} ({:.2} bytes/sec)", self.total_bytes_received, self.bytes_per_second_received);
        println!("  Bandwidth utilization: {:.2} MB/s up, {:.2} MB/s down", 
                self.bytes_per_second_sent / 1_000_000.0,
                self.bytes_per_second_received / 1_000_000.0);
    }
}

/// Latency tester for measuring round-trip times
pub struct LatencyTester {
    samples: usize,
    parallel_connections: usize,
}

impl LatencyTester {
    pub fn new(samples: usize, parallel_connections: usize) -> Self {
        Self { samples, parallel_connections }
    }
    
    /// Test latency through proxy
    pub async fn test_proxy_latency(
        &self,
        proxy_addr: std::net::SocketAddr,
        target_addr: std::net::SocketAddr,
    ) -> Vec<Duration> {
        use tokio::net::TcpStream;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        let mut tasks = Vec::new();
        let samples_per_connection = self.samples / self.parallel_connections;
        
        for _ in 0..self.parallel_connections {
            let task = tokio::spawn(async move {
                let mut latencies = Vec::new();
                
                for _ in 0..samples_per_connection {
                    let start = Instant::now();
                    
                    if let Ok(mut stream) = TcpStream::connect(proxy_addr).await {
                        let request = format!(
                            "GET http://{}/ping HTTP/1.1\r\nHost: {}\r\n\r\n",
                            target_addr, target_addr
                        );
                        
                        if stream.write_all(request.as_bytes()).await.is_ok() {
                            let mut response = [0u8; 1024];
                            if stream.read(&mut response).await.is_ok() {
                                let latency = start.elapsed();
                                latencies.push(latency);
                            }
                        }
                    }
                    
                    // Small delay between requests
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                
                latencies
            });
            
            tasks.push(task);
        }
        
        let mut all_latencies = Vec::new();
        for task in tasks {
            if let Ok(latencies) = task.await {
                all_latencies.extend(latencies);
            }
        }
        
        all_latencies
    }
}

// Platform-specific memory usage functions
#[cfg(target_os = "linux")]
fn get_memory_usage() -> u64 {
    // On Unix systems, read from /proc/self/status
    use std::fs;
    
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                if let Some(kb_str) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = kb_str.parse::<u64>() {
                        return kb * 1024; // Convert KB to bytes
                    }
                }
            }
        }
    }
    
    0 // Fallback if reading fails
}

#[cfg(target_os = "windows")]
fn get_memory_usage() -> u64 {
    // On Windows, use Windows API
    // This is a simplified implementation
    0 // Placeholder
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn get_memory_usage() -> u64 {
    0 // Unsupported platform
}