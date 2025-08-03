// Protocol Detection Performance Benchmarks
// Comprehensive benchmarks for protocol detection latency and throughput

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::time::Duration;

use litebike::protocol_registry::ProtocolDetector;
use litebike::protocol_handlers::{
    HttpDetector, Socks5Detector, TlsDetector, DohDetector
};

#[cfg(feature = "simd")]
use litebike::patricia_detector_simd::PatriciaDetectorSIMD;
use litebike::patricia_detector::PatriciaDetector;

use crate::utils::{
    HttpTestData, Socks5TestData, TlsTestData, DohTestData, FuzzGenerator,
    ProtocolTestData, BenchmarkRunner, ConcurrentTester, PerformanceMetrics
};

/// Benchmark individual protocol detectors
fn bench_individual_detectors(c: &mut Criterion) {
    let mut group = c.benchmark_group("individual_detectors");
    group.measurement_time(Duration::from_secs(10));
    
    // Prepare test data
    let http_data = HttpTestData;
    let socks5_data = Socks5TestData;
    let tls_data = TlsTestData;
    let doh_data = DohTestData;
    
    let http_samples = http_data.valid_requests();
    let socks5_samples = socks5_data.valid_requests();
    let tls_samples = tls_data.valid_requests();
    let doh_samples = doh_data.valid_requests();
    
    // Test different input sizes
    let fuzzer = FuzzGenerator;
    let small_inputs = fuzzer.generate_random_data(16, 64, 100);
    let medium_inputs = fuzzer.generate_random_data(256, 1024, 100);
    let large_inputs = fuzzer.generate_random_data(4096, 8192, 100);
    
    let test_cases = vec![
        ("http_valid", &http_samples),
        ("socks5_valid", &socks5_samples),
        ("tls_valid", &tls_samples),
        ("doh_valid", &doh_samples),
        ("small_random", &small_inputs),
        ("medium_random", &medium_inputs),
        ("large_random", &large_inputs),
    ];
    
    // Benchmark HTTP detector
    let http_detector = HttpDetector::new();
    for (name, data) in &test_cases {
        group.throughput(Throughput::Elements(data.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("http_detector", name),
            data,
            |b, data| {
                b.iter(|| {
                    for sample in data.iter() {
                        let _ = http_detector.detect(sample);
                    }
                });
            },
        );
    }
    
    // Benchmark SOCKS5 detector
    let socks5_detector = Socks5Detector::new();
    for (name, data) in &test_cases {
        group.throughput(Throughput::Elements(data.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("socks5_detector", name),
            data,
            |b, data| {
                b.iter(|| {
                    for sample in data.iter() {
                        let _ = socks5_detector.detect(sample);
                    }
                });
            },
        );
    }
    
    // Benchmark TLS detector
    let tls_detector = TlsDetector::new();
    for (name, data) in &test_cases {
        group.throughput(Throughput::Elements(data.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("tls_detector", name),
            data,
            |b, data| {
                b.iter(|| {
                    for sample in data.iter() {
                        let _ = tls_detector.detect(sample);
                    }
                });
            },
        );
    }
    
    // Benchmark DoH detector
    let doh_detector = DohDetector::new();
    for (name, data) in &test_cases {
        group.throughput(Throughput::Elements(data.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("doh_detector", name),
            data,
            |b, data| {
                b.iter(|| {
                    for sample in data.iter() {
                        let _ = doh_detector.detect(sample);
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark SIMD vs scalar implementations
#[cfg(feature = "simd")]
fn bench_simd_vs_scalar(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_vs_scalar");
    group.measurement_time(Duration::from_secs(15));
    
    let simd_detector = PatriciaDetectorSIMD::new();
    let scalar_detector = PatriciaDetector::new();
    
    // Generate test data with different patterns
    let fuzzer = FuzzGenerator;
    let test_datasets = vec![
        ("random_small", fuzzer.generate_random_data(16, 128, 1000)),
        ("random_medium", fuzzer.generate_random_data(512, 2048, 500)),
        ("random_large", fuzzer.generate_random_data(4096, 8192, 100)),
        ("protocol_mixed", {
            let mut mixed = Vec::new();
            mixed.extend(HttpTestData.valid_requests());
            mixed.extend(Socks5TestData.valid_requests());
            mixed.extend(TlsTestData.valid_requests());
            mixed.extend(DohTestData.valid_requests());
            mixed
        }),
    ];
    
    for (dataset_name, data) in test_datasets {
        group.throughput(Throughput::Elements(data.len() as u64));
        
        // Benchmark SIMD implementation
        group.bench_with_input(
            BenchmarkId::new("simd", dataset_name),
            &data,
            |b, data| {
                b.iter(|| {
                    for sample in data.iter() {
                        let _ = simd_detector.detect(sample);
                    }
                });
            },
        );
        
        // Benchmark scalar implementation
        group.bench_with_input(
            BenchmarkId::new("scalar", dataset_name),
            &data,
            |b, data| {
                b.iter(|| {
                    for sample in data.iter() {
                        let _ = scalar_detector.detect(sample);
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark detection with varying input sizes
fn bench_input_size_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("input_size_scaling");
    group.measurement_time(Duration::from_secs(8));
    
    let http_detector = HttpDetector::new();
    let socks5_detector = Socks5Detector::new();
    
    // Test with different input sizes
    let sizes = vec![16, 64, 256, 1024, 4096, 8192, 16384];
    
    for size in sizes {
        let data = vec![vec![0x41; size]; 100]; // 'A' repeated
        
        group.throughput(Throughput::Bytes(size as u64 * 100));
        
        group.bench_with_input(
            BenchmarkId::new("http", size),
            &data,
            |b, data| {
                b.iter(|| {
                    for sample in data.iter() {
                        let _ = http_detector.detect(sample);
                    }
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("socks5", size),
            &data,
            |b, data| {
                b.iter(|| {
                    for sample in data.iter() {
                        let _ = socks5_detector.detect(sample);
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark detection accuracy vs speed tradeoffs
fn bench_accuracy_vs_speed(c: &mut Criterion) {
    let mut group = c.benchmark_group("accuracy_vs_speed");
    group.measurement_time(Duration::from_secs(10));
    
    let detectors: Vec<(&str, Box<dyn ProtocolDetector>)> = vec![
        ("http", Box::new(HttpDetector::new())),
        ("socks5", Box::new(Socks5Detector::new())),
        ("tls", Box::new(TlsDetector::new())),
        ("doh", Box::new(DohDetector::new())),
    ];
    
    // Create datasets with different false positive/negative characteristics
    let fuzzer = FuzzGenerator;
    let test_cases = vec![
        ("pure_random", fuzzer.generate_random_data(64, 512, 1000)),
        ("malformed_headers", fuzzer.generate_malformed_headers()),
        ("edge_cases", fuzzer.generate_edge_case_data()),
    ];
    
    for (detector_name, detector) in &detectors {
        for (case_name, data) in &test_cases {
            group.throughput(Throughput::Elements(data.len() as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("{}_{}", detector_name, case_name), "detection"),
                data,
                |b, data| {
                    b.iter(|| {
                        let mut detections = 0;
                        for sample in data.iter() {
                            let result = detector.detect(sample);
                            if result.confidence >= detector.confidence_threshold() {
                                detections += 1;
                            }
                        }
                        detections
                    });
                },
            );
        }
    }
    
    group.finish();
}

/// Benchmark worst-case scenarios
fn bench_worst_case_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("worst_case_scenarios");
    group.measurement_time(Duration::from_secs(12));
    
    let http_detector = HttpDetector::new();
    let socks5_detector = Socks5Detector::new();
    
    // Create worst-case inputs that require full scanning
    let worst_case_scenarios = vec![
        ("almost_http", {
            // Data that looks like HTTP but isn't quite
            let mut data = Vec::new();
            data.push(b"GET / HTP/1.1\r\n".to_vec()); // Typo in HTTP
            data.push(b"GETS / HTTP/1.1\r\n".to_vec()); // Invalid method
            data.push(b"GET /very/long/path/that/continues/for/a/while HTTP/1.1\r\n".to_vec());
            data
        }),
        ("almost_socks5", {
            // Data that looks like SOCKS5 but isn't
            let mut data = Vec::new();
            data.push(vec![0x05, 0x00]); // Zero methods
            data.push(vec![0x05, 0xFF, 0x00]); // Too many methods
            data.push(vec![0x04, 0x01, 0x00]); // SOCKS4, not 5
            data
        }),
        ("pathological_patterns", {
            // Patterns designed to stress the detectors
            let mut data = Vec::new();
            data.push(b"GETGETGETGET".to_vec());
            data.push(vec![0x05, 0x05, 0x05, 0x05]);
            data.push(b"GGGGGGGGGGGGGGGGGGGGG".to_vec());
            data
        }),
    ];
    
    for (scenario_name, data) in worst_case_scenarios {
        group.throughput(Throughput::Elements(data.len() as u64));
        
        group.bench_with_input(
            BenchmarkId::new("http", scenario_name),
            &data,
            |b, data| {
                b.iter(|| {
                    for sample in data.iter() {
                        let _ = http_detector.detect(sample);
                    }
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("socks5", scenario_name),
            &data,
            |b, data| {
                b.iter(|| {
                    for sample in data.iter() {
                        let _ = socks5_detector.detect(sample);
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark memory usage patterns
fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");
    group.measurement_time(Duration::from_secs(8));
    
    let http_detector = HttpDetector::new();
    
    // Test with different memory pressure scenarios
    let memory_scenarios = vec![
        ("small_frequent", {
            // Many small allocations
            (0..10000).map(|_| vec![0x41; 64]).collect::<Vec<_>>()
        }),
        ("large_infrequent", {
            // Few large allocations
            (0..10).map(|_| vec![0x41; 65536]).collect::<Vec<_>>()
        }),
        ("mixed_sizes", {
            // Mixed allocation sizes
            let mut data = Vec::new();
            for i in 0..1000 {
                let size = if i % 10 == 0 { 4096 } else { 64 };
                data.push(vec![0x41; size]);
            }
            data
        }),
    ];
    
    for (scenario_name, data) in memory_scenarios {
        group.throughput(Throughput::Elements(data.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("memory_pressure", scenario_name),
            &data,
            |b, data| {
                b.iter(|| {
                    for sample in data.iter() {
                        let _ = http_detector.detect(sample);
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Custom benchmark runner for async testing
#[tokio::main]
async fn run_async_benchmarks() {
    use std::sync::Arc;
    
    println!("Running async concurrent benchmarks...");
    
    // Concurrent detection benchmark
    let http_detector = Arc::new(HttpDetector::new());
    let test_data = Arc::new({
        let mut data = HttpTestData.valid_requests();
        data.extend(FuzzGenerator.generate_random_data(64, 512, 500));
        data
    });
    
    let concurrent_tester = ConcurrentTester::new(8, 1000); // 8 workers, 1000 ops each
    let metrics = concurrent_tester.test_concurrent_detection(
        Arc::clone(&http_detector),
        Arc::clone(&test_data)
    ).await;
    
    println!("Concurrent detection results:");
    println!("  Total operations: {}", metrics.operation_count);
    println!("  Average duration: {:?}", metrics.average_duration);
    println!("  95th percentile: {:?}", metrics.percentile_95);
    println!("  99th percentile: {:?}", metrics.percentile_99);
    println!("  Throughput: {:.2} ops/sec", metrics.throughput_ops_per_sec);
    println!("  Error count: {}", metrics.error_count);
    
    // Memory usage benchmark
    use crate::utils::MemoryTracker;
    let mut memory_tracker = MemoryTracker::new();
    
    println!("\nRunning memory usage benchmark...");
    let large_dataset = FuzzGenerator.generate_random_data(1024, 4096, 10000);
    
    for (i, sample) in large_dataset.iter().enumerate() {
        let _ = http_detector.detect(sample);
        
        if i % 1000 == 0 {
            memory_tracker.record_measurement();
        }
    }
    
    println!("Memory usage results:");
    println!("  Peak usage: {} bytes", memory_tracker.get_peak_usage());
    println!("  Growth: {} bytes", memory_tracker.get_growth());
    
    // Throughput benchmark for different concurrency levels
    println!("\nRunning concurrency scaling benchmark...");
    for workers in [1, 2, 4, 8, 16] {
        let tester = ConcurrentTester::new(workers, 500);
        let start = std::time::Instant::now();
        let metrics = tester.test_concurrent_detection(
            Arc::clone(&http_detector),
            Arc::clone(&test_data)
        ).await;
        let duration = start.elapsed();
        
        println!("  {} workers: {:.2} ops/sec (total time: {:?})", 
                workers, 
                metrics.operation_count as f64 / duration.as_secs_f64(),
                duration);
    }
}

// Standard Criterion benchmarks
criterion_group!(
    benches,
    bench_individual_detectors,
    bench_input_size_scaling,
    bench_accuracy_vs_speed,
    bench_worst_case_scenarios,
    bench_memory_usage,
);

#[cfg(feature = "simd")]
criterion_group!(simd_benches, bench_simd_vs_scalar);

#[cfg(feature = "simd")]
criterion_main!(benches, simd_benches);

#[cfg(not(feature = "simd"))]
criterion_main!(benches);

// Additional benchmark for running async tests
#[cfg(test)]
mod async_benchmark_tests {
    use super::*;

    #[tokio::test]
    async fn run_comprehensive_async_benchmarks() {
        run_async_benchmarks().await;
    }
}

// Regression testing benchmarks
#[cfg(test)]
mod regression_benchmarks {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_performance_regression() {
        let runner = BenchmarkRunner::new()
            .with_warmup_iterations(100)
            .with_measurement_iterations(1000)
            .with_memory_tracking(true);
        
        // Baseline performance expectations (in nanoseconds)
        let performance_baselines: HashMap<&str, u64> = [
            ("http_detector", 1000),     // 1μs max average
            ("socks5_detector", 500),    // 0.5μs max average
            ("tls_detector", 300),       // 0.3μs max average
            ("doh_detector", 2000),      // 2μs max average (text processing)
        ].iter().cloned().collect();
        
        let test_data = FuzzGenerator.generate_random_data(64, 512, 100);
        
        let detectors: Vec<(&str, Box<dyn ProtocolDetector>)> = vec![
            ("http_detector", Box::new(HttpDetector::new())),
            ("socks5_detector", Box::new(Socks5Detector::new())),
            ("tls_detector", Box::new(TlsDetector::new())),
            ("doh_detector", Box::new(DohDetector::new())),
        ];
        
        for (name, detector) in detectors {
            let metrics = runner.benchmark_detection(detector.as_ref(), &test_data);
            let avg_nanos = metrics.average_duration.as_nanos() as u64;
            
            if let Some(&baseline) = performance_baselines.get(name) {
                assert!(avg_nanos <= baseline,
                       "Performance regression detected for {}: {}ns > {}ns baseline",
                       name, avg_nanos, baseline);
                
                println!("✓ {} performance OK: {}ns (baseline: {}ns)", 
                        name, avg_nanos, baseline);
            }
            
            // Memory usage should be reasonable
            assert!(metrics.memory_usage_bytes < 1024 * 1024, // < 1MB
                   "Excessive memory usage for {}: {} bytes", 
                   name, metrics.memory_usage_bytes);
        }
    }
}