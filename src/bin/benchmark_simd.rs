use litebike::protocol_detector::{Protocol, ProtocolDetector};
use std::time::Instant;

fn main() {
    let patricia = ProtocolDetector::new();
    
    let test_data = vec![
        b"GET / HTTP/1.1\r\n".to_vec(),
        b"POST /api HTTP/1.1\r\n".to_vec(),
        vec![0x05, 0x01, 0x00],
        vec![0x16, 0x03, 0x03, 0x00, 0x10],
        b"PROXY TCP4 192.168.1.1".to_vec(),
    ];
    
    let iterations = 1_000_000;
    
    // Benchmark ProtocolDetector
    let mut detector_count = 0;
    let start = Instant::now();
    for _ in 0..iterations {
        for data in &test_data {
            match patricia.detect(data).protocol {
                Protocol::Http => detector_count += 1,
                Protocol::Socks5 => detector_count += 2,
                Protocol::Tls => detector_count += 3,
                _ => detector_count += 4,
            }
        }
    }
    let detector_time = start.elapsed();
    
    println!("Benchmark Results ({} iterations x {} protocols):", iterations, test_data.len());
    println!("ProtocolDetector: {:?} ({} ns/op) [sum: {}]", detector_time, detector_time.as_nanos() / (iterations as u128 * test_data.len() as u128), detector_count);
}