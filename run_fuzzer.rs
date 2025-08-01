use std::io;
use std::time::{Duration, Instant};
use rand::prelude::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};

#[tokio::main]
async fn main() -> io::Result<()> {
    println!("🔥 STARTING LITEBIKE FUZZER 🔥");
    
    let target = "127.0.0.1";
    let port = 8080;
    
    // Check if server is running
    match TcpStream::connect((target, port)).await {
        Ok(_) => println!("✅ Target {}:{} is reachable", target, port),
        Err(_) => {
            println!("❌ Target {}:{} is not reachable - starting internal test mode", target, port);
            return run_internal_tests().await;
        }
    }
    
    println!("Starting protocol fuzzing...");
    run_protocol_fuzzer(target, port).await?;
    
    println!("Starting violent fuzzing...");
    run_violent_fuzzer(target, port).await?;
    
    println!("🎉 FUZZING COMPLETE 🎉");
    Ok(())
}

async fn run_internal_tests() -> io::Result<()> {
    println!("Running internal protocol detection tests...");
    
    let test_payloads = vec![
        ("HTTP GET", b"GET / HTTP/1.1\r\nHost: test.com\r\n\r\n".to_vec()),
        ("HTTP CONNECT", b"CONNECT example.com:443 HTTP/1.1\r\n\r\n".to_vec()),
        ("SOCKS5 Handshake", vec![0x05, 0x01, 0x00]),
        ("TLS ClientHello", vec![0x16, 0x03, 0x03, 0x00, 0x10, 0x01, 0x00, 0x00, 0x0C]),
        ("SSH Banner", b"SSH-2.0-Test\r\n".to_vec()),
        ("Malformed", vec![0xFF, 0xFE, 0xFD, 0xFC]),
    ];
    
    for (name, payload) in test_payloads {
        println!("Testing {}: {} bytes", name, payload.len());
        // Simulate protocol detection logic
        let detected = detect_protocol(&payload);
        println!("  Detected: {}", detected);
    }
    
    Ok(())
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

async fn run_protocol_fuzzer(target: &str, port: u16) -> io::Result<()> {
    println!("🧪 PROTOCOL FUZZER ENGAGED");
    let mut successful = 0;
    let mut failed = 0;
    
    for i in 0..100 {
        let payload = generate_fuzz_payload(i);
        
        match fuzz_connection(target, port, &payload).await {
            Ok(_) => {
                successful += 1;
                if i % 25 == 0 {
                    println!("✅ Fuzz #{}: {} bytes - OK", i, payload.len());
                }
            },
            Err(e) => {
                failed += 1;
                if matches!(e.kind(), io::ErrorKind::ConnectionRefused | io::ErrorKind::ConnectionReset) {
                    println!("💀 Fuzz #{}: {} bytes - SERVER CRASH?", i, payload.len());
                }
            }
        }
        
        sleep(Duration::from_millis(10)).await;
    }
    
    println!("Protocol Fuzzer Results: {} successful, {} failed", successful, failed);
    Ok(())
}

async fn run_violent_fuzzer(target: &str, port: u16) -> io::Result<()> {
    println!("💥 VIOLENT FUZZER UNLEASHED");
    let mut rng = thread_rng();
    
    // Stack overflow patterns
    for size in [1024, 4096, 8192, 16384] {
        let payload = vec![0x41; size];
        match fuzz_connection(target, port, &payload).await {
            Ok(_) => println!("✅ Stack test {}: survived", size),
            Err(_) => println!("💀 Stack test {}: potential crash", size),
        }
    }
    
    // Protocol confusion
    for _ in 0..50 {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"GET / HTTP/1.1\r\n");
        payload.extend_from_slice(&[0x05, 0x01, 0x00]); // SOCKS5
        payload.extend_from_slice(&[0x16, 0x03, 0x03]); // TLS
        payload.extend_from_slice(b"SSH-2.0-Evil\r\n");
        
        // Add random garbage
        let mut garbage = vec![0u8; rng.gen_range(50..500)];
        rng.fill_bytes(&mut garbage);
        payload.extend_from_slice(&garbage);
        
        let _ = fuzz_connection(target, port, &payload).await;
    }
    
    println!("💀 VIOLENT FUZZER COMPLETE");
    Ok(())
}

fn generate_fuzz_payload(iteration: usize) -> Vec<u8> {
    let mut rng = thread_rng();
    
    match iteration % 6 {
        0 => {
            // HTTP variants
            let methods = ["GET", "POST", "PUT", "DELETE", "CONNECT", "TRACE"];
            let method = methods[rng.gen_range(0..methods.len())];
            format!("{} / HTTP/1.1\r\nHost: fuzz.test\r\n\r\n", method).into_bytes()
        },
        1 => {
            // SOCKS5 variants
            vec![0x05, rng.gen_range(0..5), rng.gen(), rng.gen()]
        },
        2 => {
            // TLS variants
            let mut payload = vec![0x16, 0x03, 0x03];
            payload.extend_from_slice(&(rng.gen::<u16>()).to_be_bytes());
            for _ in 0..rng.gen_range(10..100) {
                payload.push(rng.gen());
            }
            payload
        },
        3 => {
            // Random binary
            let size = rng.gen_range(1..1000);
            let mut payload = vec![0u8; size];
            rng.fill_bytes(&mut payload);
            payload
        },
        4 => {
            // Buffer overflow attempt
            vec![0x41; rng.gen_range(1000..5000)]
        },
        _ => {
            // Format string
            "%x%x%x%x%x%x%x%x%x%x".repeat(rng.gen_range(1..10)).into_bytes()
        }
    }
}

async fn fuzz_connection(target: &str, port: u16, payload: &[u8]) -> io::Result<()> {
    let mut stream = timeout(
        Duration::from_secs(2),
        TcpStream::connect((target, port))
    ).await??;
    
    stream.write_all(payload).await?;
    
    let mut response = [0u8; 1024];
    match timeout(Duration::from_secs(1), stream.read(&mut response)).await {
        Ok(Ok(n)) => {
            if n == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Connection closed"));
            }
        },
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err(io::Error::new(io::ErrorKind::TimedOut, "Response timeout")),
    }
    
    Ok(())
}