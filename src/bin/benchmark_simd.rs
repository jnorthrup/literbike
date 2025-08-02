use litebike::patricia_detector::{PatriciaDetector, quick_detect};
use litebike::patricia_detector_simd::SimdDetector;
use std::time::Instant;

fn main() {
    let patricia = PatriciaDetector::new();
    let simd = SimdDetector::new();
    
    let test_data = vec![
        b"GET / HTTP/1.1\r\n".to_vec(),
        b"POST /api HTTP/1.1\r\n".to_vec(),
        vec![0x05, 0x01, 0x00],
        vec![0x16, 0x03, 0x03, 0x00, 0x10],
        b"PROXY TCP4 192.168.1.1".to_vec(),
    ];
    
    let iterations = 1_000_000;
    
    // Benchmark Patricia Trie
    let mut patricia_count = 0;
    let start = Instant::now();
    for _ in 0..iterations {
        for data in &test_data {
            match patricia.detect(data) {
                litebike::patricia_detector::Protocol::Http => patricia_count += 1,
                litebike::patricia_detector::Protocol::Socks5 => patricia_count += 2,
                litebike::patricia_detector::Protocol::Tls => patricia_count += 3,
                _ => patricia_count += 4,
            }
        }
    }
    let patricia_time = start.elapsed();
    
    // Benchmark SIMD
    let mut simd_count = 0;
    let start = Instant::now();
    for _ in 0..iterations {
        for data in &test_data {
            match simd.detect_simd(data) {
                litebike::patricia_detector_simd::Protocol::Http => simd_count += 1,
                litebike::patricia_detector_simd::Protocol::Socks5 => simd_count += 2,
                litebike::patricia_detector_simd::Protocol::Tls => simd_count += 3,
                _ => simd_count += 4,
            }
        }
    }
    let simd_time = start.elapsed();
    
    // Benchmark quick_detect
    let mut quick_count = 0;
    let start = Instant::now();
    for _ in 0..iterations {
        for data in &test_data {
            match quick_detect(data) {
                Some(litebike::patricia_detector::Protocol::Http) => quick_count += 1,
                Some(litebike::patricia_detector::Protocol::Socks5) => quick_count += 2,
                Some(litebike::patricia_detector::Protocol::Tls) => quick_count += 3,
                _ => quick_count += 4,
            }
        }
    }
    let quick_time = start.elapsed();
    
    println!("Benchmark Results ({} iterations x {} protocols):", iterations, test_data.len());
    println!("Patricia Trie: {:?} ({} ns/op) [sum: {}]", patricia_time, patricia_time.as_nanos() / (iterations as u128 * test_data.len() as u128), patricia_count);
    println!("SIMD Detector: {:?} ({} ns/op) [sum: {}]", simd_time, simd_time.as_nanos() / (iterations as u128 * test_data.len() as u128), simd_count);
    println!("Quick Detect:  {:?} ({} ns/op) [sum: {}]", quick_time, quick_time.as_nanos() / (iterations as u128 * test_data.len() as u128), quick_count);
    
    if simd_time.as_nanos() > 0 {
        println!("\nSpeedup: {:.2}x", patricia_time.as_nanos() as f64 / simd_time.as_nanos() as f64);
    }
}