// Mock the shit out of protocols - comprehensive stress testing

use litebike::protocol_detector::{Protocol, ProtocolDetector};
use std::time::Instant;

fn main() {
    println!("=== PROTOCOL MOCKING STRESS TEST ===\n");
    
    let detector = ProtocolDetector::new();
    let mut total_tests = 0;
    let mut passed = 0;
    let mut failed = 0;
    
    // Test 1: Legit protocols should work
    println!("TEST 1: Legitimate Protocols");
    println!("{}", "-".repeat(50));
    
    let legit_tests: Vec<(&str, &[u8], Protocol)> = vec![
        ("HTTP GET", b"GET / HTTP/1.1\r\n", Protocol::Http),
        ("HTTP POST", b"POST /api HTTP/1.1\r\n", Protocol::Http),
        ("HTTP CONNECT", b"CONNECT proxy:443 HTTP/1.1\r\n", Protocol::Http),
        ("SOCKS5", &[0x05, 0x01, 0x00], Protocol::Socks5),
        ("TLS 1.2", &[0x16, 0x03, 0x03, 0x00, 0x10], Protocol::Tls),
        ("TLS 1.3", &[0x16, 0x03, 0x04, 0x00, 0x10], Protocol::Tls),
    ];
    
    for (name, payload, expected) in legit_tests {
        total_tests += 1;
        let result = detector.detect(payload);
        if result.protocol == expected {
            println!("✓ {}: {:?} in {} bytes", name, result.protocol, result.bytes_consumed);
            passed += 1;
        } else {
            println!("✗ {}: Expected {:?}, got {:?}", name, expected, result.protocol);
            failed += 1;
        }
    }
    
    // Test 2: Malformed shit that should NOT be detected
    println!("\n\nTEST 2: Malformed Protocols (should fail)");
    println!("{}", "-".repeat(50));
    
    let malformed: Vec<(&str, &[u8])> = vec![
        ("Empty", &[]),
        ("Single byte", &[0x47]),
        ("GET no space", b"GET/index"),
        ("get lowercase", b"get / http/1.1"),
        ("SOCKS4", &[0x04, 0x01]),
        ("TLS wrong version", &[0x16, 0x02, 0x00]),
        ("Random garbage", &[0xDE, 0xAD, 0xBE, 0xEF]),
        ("Almost HTTP", b"HTT"),
        ("Null bytes", b"GET\0/\0"),
        ("High ASCII", &[0x80, 0xFF, 0xFE]),
    ];
    
    for (name, payload) in malformed {
        total_tests += 1;
        let result = detector.detect(payload);
        if result.protocol == Protocol::Unknown {
            println!("✓ {}: Correctly rejected", name);
            passed += 1;
        } else {
            println!("✗ {}: Incorrectly detected as {:?}", name, result.protocol);
            failed += 1;
        }
    }
    
    // Test 3: Edge cases and buffer boundaries
    println!("\n\nTEST 3: Edge Cases");
    println!("{}", "-".repeat(50));
    
    let edge_cases = vec![
        ("4KB HTTP", {
            let mut v = b"GET /".to_vec();
            v.extend(vec![b'a'; 4090]);
            v.extend(b" HTTP/1.1");
            v
        }),
        ("Truncated TLS", vec![0x16, 0x03]),
        ("SOCKS5 + garbage", vec![0x05, 0x01, 0x00, 0xFF, 0xFE, 0xFD]),
        ("Mixed protocols", b"GET / HTTP/1.1\r\n\x05\x01\x00".to_vec()),
    ];
    
    for (name, payload) in edge_cases {
        total_tests += 1;
        let result = detector.detect(&payload);
        println!("{}: {:?} in {} bytes", name, result.protocol, result.bytes_consumed);
        if result.protocol != Protocol::Unknown {
            passed += 1;
        } else {
            failed += 1;
        }
    }
    
    // Test 4: Performance with noise
    println!("\n\nTEST 4: Performance Under Noise");
    println!("{}", "-".repeat(50));
    
    let iterations = 100_000;
    let start = Instant::now();
    
    for i in 0..iterations {
        let noise = match i % 5 {
            0 => vec![0x05, 0x01, 0x00],  // SOCKS5
            1 => b"GET / HTTP/1.1".to_vec(),  // HTTP
            2 => vec![0x16, 0x03, 0x03],  // TLS
            3 => vec![0xFF, 0xFE, 0xFD],  // Garbage
            _ => vec![],  // Empty
        };
        let _ = detector.detect(&noise);
    }
    
    let elapsed = start.elapsed();
    let per_detect = elapsed.as_nanos() / iterations as u128;
    println!("Processed {} detections in {:?}", iterations, elapsed);
    println!("Average: {}ns per detection", per_detect);
    
    // Test 5: Adversarial inputs
    println!("\n\nTEST 5: Adversarial Inputs");
    println!("{}", "-".repeat(50));
    
    let adversarial = vec![
        ("SOCKS5 overflow", {
            let mut v = vec![0x05];
            v.extend(vec![0xFF; 256]);
            v
        }),
        ("HTTP injection", b"GET /../../../etc/passwd HTTP/1.1".to_vec()),
        ("TLS heartbleed style", {
            let mut v = vec![0x16, 0x03, 0x03, 0xFF, 0xFF];
            v.extend(vec![0x00; 65535]);
            v
        }),
        ("Polyglot", vec![0x05, 0x47, 0x45, 0x54, 0x20]),  // SOCKS5 + "GET "
    ];
    
    for (name, payload) in adversarial {
        total_tests += 1;
        let result = detector.detect(&payload.as_slice());
        println!("{}: {:?} (consumed {} of {} bytes)",
                 name, result.protocol, result.bytes_consumed, payload.len());
        // These are adversarial, so any non-panic is success
        passed += 1;
    }
    
    // Final summary
    println!("\n\n=== FINAL RESULTS ===");
    println!("Total tests: {}", total_tests);
    println!("Passed: {} ({:.1}%)", passed, (passed as f64 / total_tests as f64) * 100.0);
    println!("Failed: {} ({:.1}%)", failed, (failed as f64 / total_tests as f64) * 100.0);
    
    if failed > 0 {
        println!("\n⚠️  Some tests failed - detector may have issues");
        std::process::exit(1);
    } else {
        println!("\n✅ All tests passed!");
    }
}