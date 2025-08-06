// Protocol Test Runner

use std::time::{Duration, Instant};
use env_logger;
use log::{info, warn, error};

// We'll define the detector locally to avoid import issues
use litebike::protocol_detector::{ProtocolDetector, Protocol, DetectionResult};

#[tokio::main]
async fn main() {
    env_logger::init();
    
    println!("Protocol detection test suite");
    println!("Protocol detection test suite");
    println!("============================================\n");

    let detector = ProtocolDetector::new();
    
    // Run all test phases
    run_basic_tests(&detector);
    run_adversarial_tests(&detector);
    run_performance_tests(&detector);
    run_chaos_fuzzing(&detector);
    
    println!("\n✅ ALL MINIMAL TORTURE TESTS COMPLETED ✅");
    println!("Protocol detection test suite");
}

fn run_basic_tests(detector: &ProtocolDetector) {
    println!("📋 BASIC PROTOCOL TESTS");
    println!("======================");
    
    let mut total = 0;
    let mut passed = 0;
    
    let tests = vec![
        // HTTP tests
        ("HTTP GET", b"GET / HTTP/1.1\r\n\r\n".to_vec(), Protocol::Http),
        ("HTTP POST", b"POST /api HTTP/1.1\r\n\r\n".to_vec(), Protocol::Http),
        ("HTTP CONNECT", b"CONNECT proxy:443 HTTP/1.1\r\n\r\n".to_vec(), Protocol::Http),
        
        // SOCKS5 tests
        ("SOCKS5 simple", vec![0x05, 0x01, 0x00], Protocol::Socks5),
        ("SOCKS5 multi-auth", vec![0x05, 0x03, 0x00, 0x01, 0x02], Protocol::Socks5),
        
        // TLS tests
        ("TLS 1.2", vec![0x16, 0x03, 0x03, 0x00, 0x10], Protocol::Tls),
        ("TLS 1.3", vec![0x16, 0x03, 0x04, 0x00, 0x10], Protocol::Tls),
        
        // Should be unknown
        ("Empty", vec![], Protocol::Unknown),
        ("Random", vec![0xDE, 0xAD, 0xBE, 0xEF], Protocol::Unknown),
        ("Malformed HTTP", b"GETindex".to_vec(), Protocol::Unknown),
    ];
    
    for (name, payload, expected) in tests {
        total += 1;
        let detection_result = detector.detect(&payload);
        let (detected, bytes) = (detection_result.protocol, detection_result.bytes_consumed);
        
        if std::mem::discriminant(&detected) == std::mem::discriminant(&expected) {
            println!("  ✅ {}: {:?} ({} bytes)", name, detected, bytes);
            passed += 1;
        } else {
            println!("  ❌ {}: Expected {:?}, got {:?}", name, expected, detected);
        }
    }
    
    println!("\nBasic Tests: {}/{} passed ({:.1}%)", passed, total, (passed as f64 / total as f64) * 100.0);
}

fn run_adversarial_tests(detector: &ProtocolDetector) {
    println!("\n💀 ADVERSARIAL TESTS");
    println!("===================");
    
    let tests = vec![
        ("1MB HTTP header", {
            let mut payload = b"GET / HTTP/1.1\r\nX-Big: ".to_vec();
            payload.extend(vec![b'A'; 1_000_000]);
            payload.extend(b"\r\n\r\n");
            payload
        }),
        ("SOCKS5 255 methods", {
            let mut payload = vec![0x05, 0xFF];
            payload.extend((0u8..=254u8));
            payload.push(0xFF);
            payload
        }),
        ("TLS max record", {
            let mut payload = vec![0x16, 0x03, 0x03, 0xFF, 0xFF];
            payload.extend(vec![0x00; 65535]);
            payload
        }),
        ("Protocol confusion", {
            let mut payload = b"GET / HTTP/1.1\r\n".to_vec();
            payload.extend(&[0x05, 0x01, 0x00]);
            payload.extend(&[0x16, 0x03, 0x03]);
            payload
        }),
        ("Memory bomb", vec![0x00; 10_000_000]),
    ];
    
    let mut slow_tests = 0;
    let mut very_slow_tests = 0;
    
    for (name, payload) in tests {
        let start = Instant::now();
        let detection_result = detector.detect(&payload);
        let (protocol, bytes) = (detection_result.protocol, detection_result.bytes_consumed);
        let duration = start.elapsed();
        
        if duration > Duration::from_millis(100) {
            very_slow_tests += 1;
            error!("  🐌 {}: VERY SLOW {:?} ({}KB)", name, duration, payload.len() / 1024);
        } else if duration > Duration::from_millis(10) {
            slow_tests += 1;
            warn!("  ⚠️  {}: Slow {:?} ({}KB)", name, duration, payload.len() / 1024);
        } else {
            println!("  ✅ {}: {:?} in {:?} ({}KB, {} bytes consumed)", 
                     name, protocol, duration, payload.len() / 1024, bytes);
        }
    }
    
    println!("\nAdversarial Tests: {} slow, {} very slow", slow_tests, very_slow_tests);
}

fn run_performance_tests(detector: &ProtocolDetector) {
    println!("\n⚡ PERFORMANCE TESTS");
    println!("==================");
    
    let payloads = vec![
        b"GET / HTTP/1.1\r\n\r\n".to_vec(),
        vec![0x05, 0x01, 0x00],
        vec![0x16, 0x03, 0x03, 0x00, 0x10],
        vec![0xDE, 0xAD, 0xBE, 0xEF],
    ];
    
    let iterations = 1_000_000;
    let start = Instant::now();
    
    for i in 0..iterations {
        let payload = &payloads[i % payloads.len()];
        let detection_result = detector.detect(payload);
        let _ = (detection_result.protocol, detection_result.bytes_consumed);
    }
    
    let duration = start.elapsed();
    let throughput = iterations as f64 / duration.as_secs_f64();
    
    println!("  {} iterations in {:?}", iterations, duration);
    println!("  Throughput: {:.0} detections/sec", throughput);
    
    if throughput > 500_000.0 {
        println!("  🚀 Excellent performance!");
    } else if throughput > 100_000.0 {
        println!("  ✅ Good performance");
    } else {
        warn!("  ⚠️  Performance could be better");
    }
}

fn run_chaos_fuzzing(detector: &ProtocolDetector) {
    println!("\n🌪️ CHAOS FUZZING (10 seconds)");
    println!("=============================");
    
    let start_time = Instant::now();
    let test_duration = Duration::from_secs(10);
    let end_time = start_time + test_duration;
    
    let mut total = 0;
    let mut detected = 0;
    let mut rejected = 0;
    let mut slow = 0;
    
    while Instant::now() < end_time {
        // Generate random payload
        let size = 1 + (fast_random() % 1000);
        let payload: Vec<u8> = (0..size).map(|_| (fast_random() % 256) as u8).collect();
        
        let test_start = Instant::now();
        let detection_result = detector.detect(&payload);
        let (protocol, _) = (detection_result.protocol, detection_result.bytes_consumed);
        let test_duration = test_start.elapsed();
        
        total += 1;
        
        match protocol {
            Protocol::Unknown => rejected += 1,
            _ => detected += 1,
        }
        
        if test_duration > Duration::from_millis(1) {
            slow += 1;
        }
    }
    
    let actual_duration = start_time.elapsed();
    let throughput = total as f64 / actual_duration.as_secs_f64();
    
    println!("  {} tests in {:?}", total, actual_duration);
    println!("  Throughput: {:.0} tests/sec", throughput);
    println!("  Detected: {} ({:.1}%)", detected, (detected as f64 / total as f64) * 100.0);
    println!("  Rejected: {} ({:.1}%)", rejected, (rejected as f64 / total as f64) * 100.0);
    println!("  Slow tests: {} ({:.1}%)", slow, (slow as f64 / total as f64) * 100.0);
    
    if rejected > detected * 5 {
        println!("  ✅ Good rejection rate for random data");
    } else {
        warn!("  ⚠️  May be detecting too many random payloads");
    }
}

// Simple PRNG for fuzzing
fn fast_random() -> usize {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEED: AtomicUsize = AtomicUsize::new(0xdeadbeef);
    
    let current = SEED.load(Ordering::Relaxed);
    let next = current.wrapping_mul(1103515245).wrapping_add(12345);
    SEED.store(next, Ordering::Relaxed);
    next
}