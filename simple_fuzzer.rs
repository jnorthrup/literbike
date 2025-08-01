use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use std::thread;

fn main() {
    println!("🔥 LITEBIKE SIMPLE FUZZER 🔥");
    
    let target = "127.0.0.1:9999";
    
    // Check if server is running
    match TcpStream::connect(target) {
        Ok(_) => println!("✅ Target {} is reachable", target),
        Err(_) => {
            println!("❌ Target {} not reachable - running offline tests", target);
            run_offline_tests();
            return;
        }
    }
    
    println!("Starting basic fuzzing...");
    run_basic_fuzzer(target);
    
    println!("Starting aggressive fuzzing...");
    run_aggressive_fuzzer(target);
    
    println!("🎉 FUZZING COMPLETE 🎉");
}

fn run_offline_tests() {
    println!("Running protocol detection tests...");
    
    let payloads: Vec<(&str, &[u8])> = vec![
        ("HTTP GET", b"GET / HTTP/1.1\r\nHost: test.com\r\n\r\n"),
        ("HTTP CONNECT", b"CONNECT example.com:443 HTTP/1.1\r\n\r\n"),
        ("SOCKS5", &[0x05, 0x01, 0x00]),
        ("TLS", &[0x16, 0x03, 0x03, 0x00, 0x10]),
        ("SSH", b"SSH-2.0-Test\r\n"),
        ("Garbage", &[0xFF, 0xFE, 0xFD, 0xFC]),
    ];
    
    for (name, payload) in payloads {
        println!("Testing {}: {} bytes", name, payload.len());
        let detected = detect_protocol(payload);
        println!("  Detected: {}", detected);
    }
}

fn detect_protocol(payload: &[u8]) -> &'static str {
    if payload.starts_with(b"GET ") || payload.starts_with(b"POST ") || payload.starts_with(b"CONNECT ") {
        "HTTP"
    } else if payload.len() >= 3 && payload[0] == 0x05 {
        "SOCKS5"
    } else if payload.len() >= 3 && payload[0] == 0x16 && payload[1] == 0x03 {
        "TLS"
    } else if payload.starts_with(b"SSH-") {
        "SSH"
    } else {
        "Unknown"
    }
}

fn run_basic_fuzzer(target: &str) {
    println!("🧪 BASIC FUZZER");
    let mut successful = 0;
    let mut failed = 0;
    
    for i in 0..50 {
        let payload = generate_basic_payload(i);
        
        match fuzz_connection(target, &payload) {
            Ok(_) => {
                successful += 1;
                if i % 10 == 0 {
                    println!("✅ Test #{}: {} bytes - OK", i, payload.len());
                }
            },
            Err(e) => {
                failed += 1;
                println!("⚠️  Test #{}: {} bytes - {}", i, payload.len(), e);
            }
        }
        
        thread::sleep(Duration::from_millis(50));
    }
    
    println!("Basic Results: {} successful, {} failed", successful, failed);
}

fn run_aggressive_fuzzer(target: &str) {
    println!("💥 AGGRESSIVE FUZZER");
    
    // Test large payloads
    for size in [1024, 4096, 8192, 16384, 32768] {
        let payload = vec![b'A'; size];
        match fuzz_connection(target, &payload) {
            Ok(_) => println!("✅ Large payload {}: survived", size),
            Err(_) => println!("💀 Large payload {}: failed", size),
        }
    }
    
    // Protocol confusion
    println!("Testing protocol confusion...");
    for i in 0..20 {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"GET / HTTP/1.1\r\n");
        payload.extend_from_slice(&[0x05, 0x01, 0x00]); // SOCKS5
        payload.extend_from_slice(&[0x16, 0x03, 0x03]); // TLS
        payload.extend_from_slice(b"SSH-2.0-Evil\r\n");
        
        // Add pattern data
        for j in 0..100 {
            payload.push((i * j) as u8);
        }
        
        match fuzz_connection(target, &payload) {
            Ok(_) => if i % 5 == 0 { println!("✅ Confusion #{}: OK", i); },
            Err(_) => println!("💀 Confusion #{}: failed", i),
        }
    }
    
    // Malformed packets
    println!("Testing malformed packets...");
    for i in 0..30 {
        let mut payload = Vec::new();
        
        // Add corrupted HTTP
        payload.extend_from_slice(b"GET / HTTP/999.999\r\n");
        payload.extend_from_slice(b"Host: \x00\x01\x02\x03\r\n");
        
        // Add corrupted SOCKS5
        payload.push(0x05); // Version
        payload.push(255);  // Too many methods
        for j in 0..255 {
            payload.push(j);
        }
        
        match fuzz_connection(target, &payload) {
            Ok(_) => if i % 10 == 0 { println!("✅ Malformed #{}: OK", i); },
            Err(_) => println!("💀 Malformed #{}: failed", i),
        }
    }
    
    println!("💥 AGGRESSIVE FUZZER COMPLETE");
}

fn generate_basic_payload(iteration: usize) -> Vec<u8> {
    match iteration % 5 {
        0 => b"GET / HTTP/1.1\r\nHost: test.com\r\n\r\n".to_vec(),
        1 => vec![0x05, 0x01, 0x00],
        2 => vec![0x16, 0x03, 0x03, 0x00, 0x10, 0x01],
        3 => {
            let mut v = Vec::new();
            for i in 0..100 {
                v.push((iteration + i) as u8);
            }
            v
        },
        _ => vec![0xFF; iteration % 1000 + 1],
    }
}

fn fuzz_connection(target: &str, payload: &[u8]) -> Result<(), String> {
    let mut stream = TcpStream::connect(target)
        .map_err(|e| format!("Connect failed: {}", e))?;
    
    stream.set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| format!("Set timeout failed: {}", e))?;
    
    stream.write_all(payload)
        .map_err(|e| format!("Write failed: {}", e))?;
    
    let mut response = [0u8; 1024];
    match stream.read(&mut response) {
        Ok(0) => Err("Connection closed".to_string()),
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Read failed: {}", e)),
    }
}