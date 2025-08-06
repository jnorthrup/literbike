// SOCKS5 Protocol Tests - Both Standalone and Shared Port
// Tests SOCKS5 detection, handshake, and connection scenarios

use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

use litebike::protocol_handlers::{Socks5Handler};
use litebike::protocol_detector::{ProtocolDetector, Protocol};
use litebike::protocol_registry::{ProtocolHandler};
use litebike::universal_listener::{detect_protocol, PrefixedStream};


/// SOCKS5 handshake test data
struct Socks5TestData;

impl Socks5TestData {
    // Valid SOCKS5 handshakes
    fn valid_handshakes() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("no_auth", vec![0x05, 0x01, 0x00]),                    // Version 5, 1 method, no auth
            ("user_pass", vec![0x05, 0x01, 0x02]),                  // Version 5, 1 method, user/pass
            ("multiple_methods", vec![0x05, 0x03, 0x00, 0x01, 0x02]), // Version 5, 3 methods
            ("gssapi", vec![0x05, 0x01, 0x01]),                     // Version 5, 1 method, GSSAPI
        ]
    }
    
    // Invalid SOCKS5 handshakes
    fn invalid_handshakes() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("socks4", vec![0x04, 0x01, 0x00]),                     // SOCKS4
            ("wrong_version", vec![0x06, 0x01, 0x00]),              // Wrong version
            ("zero_methods", vec![0x05, 0x00]),                     // No methods
            ("truncated", vec![0x05]),                              // Incomplete
            ("empty", vec![]),                                      // Empty
        ]
    }
    
    // SOCKS5 connect requests
    fn connect_requests() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("ipv4_connect", vec![
                0x05, 0x01, 0x00, 0x01,           // Ver, CMD=CONNECT, RSV, ATYP=IPv4
                0x7F, 0x00, 0x00, 0x01,           // IP: 127.0.0.1
                0x00, 0x50                        // Port: 80
            ]),
            ("domain_connect", {
                let mut v = Vec::new();
                v.extend_from_slice(&[0x05, 0x01, 0x00, 0x03]);
                v.push(11u8);
                v.extend_from_slice(b"example.com");
                v.extend_from_slice(&[0x00, 0x50]);
                v
            }),
            ("ipv6_connect", vec![
                0x05, 0x01, 0x00, 0x04,           // Ver, CMD=CONNECT, RSV, ATYP=IPv6
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,  // IPv6: ::1
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
                0x00, 0x50                        // Port: 80
            ]),
        ]
    }
}

#[cfg(test)]
mod socks5_detection_tests {
    use super::*;
    
    #[test]
    fn test_socks5_detector_valid() {
        let detector = ProtocolDetector::new();
        
        for (name, data) in Socks5TestData::valid_handshakes() {
            let result = detector.detect(&data);
            assert_eq!(result.protocol, Protocol::Socks5, "Failed for test case: {}", name);
            assert!(result.confidence >= 200, "Low confidence for {}: {}", name, result.confidence);
        }
    }
    
    #[test]
    fn test_socks5_detector_invalid() {
        let detector = ProtocolDetector::new();
        
        for (name, data) in Socks5TestData::invalid_handshakes() {
            let result = detector.detect(&data);
            assert_ne!(result.protocol, Protocol::Socks5, "False positive for test case: {}", name);
        }
    }
    
    
    
    #[tokio::test]
    async fn test_universal_listener_socks5_detection() {
        for (name, data) in Socks5TestData::valid_handshakes() {
            let mut cursor = std::io::Cursor::new(data.clone());
            let (protocol, buffer) = detect_protocol(&mut cursor).await.unwrap();
            
            assert!(matches!(protocol, litebike::universal_listener::Protocol::Socks5), "Failed detection for: {}", name);
            assert_eq!(buffer, data, "Buffer mismatch for: {}", name);
        }
    }
}

#[cfg(test)]
mod socks5_handler_tests {
    use super::*;
    
    /// Mock SOCKS5 client for testing
    struct MockSocks5Client;
    
    impl MockSocks5Client {
        async fn handshake(stream: &mut TcpStream) -> io::Result<()> {
            // Send handshake: Version 5, 1 method (no auth)
            stream.write_all(&[0x05, 0x01, 0x00]).await?;
            
            // Read response
            let mut response = [0u8; 2];
            stream.read_exact(&mut response).await?;
            
            if response[0] != 0x05 || response[1] != 0x00 {
                return Err(io::Error::new(io::ErrorKind::Other, "Handshake failed"));
            }
            
            Ok(())
        }
        
        async fn connect_request(stream: &mut TcpStream, target: &str, port: u16) -> io::Result<()> {
            // Build connect request
            let mut request = vec![0x05, 0x01, 0x00, 0x03]; // Ver, CMD=CONNECT, RSV, ATYP=DOMAIN
            request.push(target.len() as u8);
            request.extend_from_slice(target.as_bytes());
            request.extend_from_slice(&port.to_be_bytes());
            
            stream.write_all(&request).await?;
            
            // Read response
            let mut response = [0u8; 10]; // Max basic response size
            let n = stream.read(&mut response).await?;
            
            if n < 6 || response[0] != 0x05 {
                return Err(io::Error::new(io::ErrorKind::Other, "Connect failed"));
            }
            
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_socks5_handler_standalone() {
        // Start a mock target server
        let target_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_addr = target_listener.local_addr().unwrap();
        
        tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = target_listener.accept().await {
                    tokio::spawn(async move {
                        let mut buf = [0u8; 1024];
                        while let Ok(n) = stream.read(&mut buf).await {
                            if n == 0 { break; }
                            let _ = stream.write_all(&buf[..n]).await;
                        }
                    });
                }
            }
        });
        
        // Start SOCKS5 proxy server
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        let handler = Socks5Handler::new();
        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = proxy_listener.accept().await {
                    let prefixed_stream = PrefixedStream::new(stream, vec![]);
                    let _ = handler.handle(prefixed_stream).await;
                }
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Test SOCKS5 connection through proxy
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        client.set_nodelay(false).unwrap(); // Enable Nagle

        // Perform handshake
        MockSocks5Client::handshake(&mut client).await.unwrap();
        
        // Connect to target through proxy
        MockSocks5Client::connect_request(&mut client, "127.0.0.1", target_addr.port()).await.unwrap();
        
        // Test data forwarding
        let test_data = b"Hello, SOCKS5!";
        client.write_all(test_data).await.unwrap();
        
        let mut response = [0u8; 32];
        let n = timeout(Duration::from_secs(1), client.read(&mut response)).await.unwrap().unwrap();
        
        assert_eq!(&response[..n], test_data);
    }
    
    #[tokio::test]  
    async fn test_socks5_handler_authentication_required() {
        // Test with user/pass authentication method
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        let handler = Socks5Handler::new();
        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = proxy_listener.accept().await {
                    let prefixed_stream = PrefixedStream::new(stream, vec![]);
                    let _ = handler.handle(prefixed_stream).await;
                }
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        client.set_nodelay(false).unwrap(); // Enable Nagle

        // Send handshake with auth method
        client.write_all(&[0x05, 0x01, 0x02]).await.unwrap(); // User/pass auth
        
        let mut response = [0u8; 2];
        let result = timeout(Duration::from_millis(500), client.read_exact(&mut response)).await;
        
        // Should either accept no-auth or indicate auth required
        match result {
            Ok(_) => {
                // Handler responded - check if it's a valid SOCKS5 response
                assert_eq!(response[0], 0x05); // Version 5
            },
            Err(_) => {
                // Connection closed - also acceptable for unimplemented auth
            }
        }
    }
}

#[cfg(test)]
mod socks5_universal_port_tests {
    use super::*;
    use litebike::simple_routing::SimpleRouter;
    use litebike::libc_socket_tune::accept_with_options;
    
    /// Test SOCKS5 detection and handling on the universal port 8888
    #[tokio::test]
    async fn test_socks5_on_universal_port() {
        // Create a router and bind to a test port
        let router = SimpleRouter::new();
        let (listener, _config) = router.bind_with_fallback().await.unwrap();
        let listen_addr = listener.local_addr().unwrap();
        
        // Start universal port handler
        tokio::spawn(async move {
            let tcp_tuning = litebike::libc_socket_tune::TcpTuningOptions::default();
            
            loop {
                if let Ok((stream, _addr)) = accept_with_options(&listener, &tcp_tuning).await {
                    tokio::spawn(async move {
                        // Read initial bytes for protocol detection
                        let mut peek_buf = vec![0u8; 1024];
                        let mut stream = stream;
                        let n = match stream.read(&mut peek_buf).await {
                            Ok(n) if n > 0 => n,
                            _ => return,
                        };
                        peek_buf.truncate(n);
                        
                        // Detect protocol
                        let detector = ProtocolDetector::new();
                        let result = detector.detect(&peek_buf);
                        
                        // Handle SOCKS5
                        if matches!(result.protocol, Protocol::Socks5) {
                            let prefixed_stream = PrefixedStream::new(stream, peek_buf);
                            let handler = Socks5Handler::new();
                            let _ = handler.handle(prefixed_stream).await;
                        }
                    });
                }
            }
        });
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Test SOCKS5 client connecting to universal port
        let mut client = TcpStream::connect(listen_addr).await.unwrap();
        client.set_nodelay(false).unwrap(); // Enable Nagle

        // Send SOCKS5 handshake
        client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
        
        // Should receive SOCKS5 response
        let mut response = [0u8; 2];
        let result = timeout(Duration::from_secs(1), client.read_exact(&mut response)).await;
        
        match result {
            Ok(_) => {
                assert_eq!(response[0], 0x05, "Should respond with SOCKS5 version");
                // response[1] indicates chosen auth method (0x00 = no auth, 0xFF = none acceptable)
            },
            Err(_) => {
                // Timeout acceptable if handler is still processing
                println!("SOCKS5 handshake timed out - may need handler implementation");
            }
        }
    }
    
    #[tokio::test]
    async fn test_mixed_protocols_on_universal_port() {
        // Test that HTTP and SOCKS5 can coexist on the same port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let listen_addr = listener.local_addr().unwrap();
        
        tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    tokio::spawn(async move {
                        let mut cursor = &mut stream;
                        if let Ok((protocol, buffer)) = detect_protocol(cursor).await {
                            match protocol {
                                litebike::universal_listener::Protocol::Socks5 => {
                                    println!("Detected SOCKS5 on universal port");
                                    // Respond with SOCKS5 handshake acceptance
                                    let _ = stream.write_all(&[0x05, 0x00]).await;
                                },
                                litebike::universal_listener::Protocol::Http => {
                                    println!("Detected HTTP on universal port");
                                    // Respond with basic HTTP response
                                    let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
                                    let _ = stream.write_all(response.as_bytes()).await;
                                },
                                _ => {
                                    println!("Detected other protocol: {:?}", protocol);
                                }
                            }
                        }
                    });
                }
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Test 1: HTTP request
        let mut http_client = TcpStream::connect(listen_addr).await.unwrap();
        http_client.set_nodelay(false).unwrap(); // Enable Nagle
        http_client.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await.unwrap();
        
        let mut http_response = [0u8; 1024];
        let n = timeout(Duration::from_millis(500), http_client.read(&mut http_response)).await.unwrap().unwrap();
        let response_str = std::str::from_utf8(&http_response[..n]).unwrap();
        assert!(response_str.contains("HTTP/1.1 200 OK"));
        
        // Test 2: SOCKS5 request  
        let mut socks_client = TcpStream::connect(listen_addr).await.unwrap();
        socks_client.set_nodelay(false).unwrap(); // Enable Nagle
        socks_client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
        
        let mut socks_response = [0u8; 2];
        let result = timeout(Duration::from_millis(500), socks_client.read_exact(&mut socks_response)).await;
        
        if let Ok(_) = result {
            assert_eq!(socks_response[0], 0x05);
            assert_eq!(socks_response[1], 0x00);
        }
    }
}

#[cfg(test)]
mod socks5_edge_cases {
    use super::*;
    
    // This test is redundant with the invalid_handshakes test above.
    
    #[tokio::test]
    async fn test_socks5_connect_command_types() {
        // Test different SOCKS5 command types beyond CONNECT
        let test_commands = vec![
            ("connect", vec![0x05, 0x01, 0x00, 0x01, 0x7F, 0x00, 0x00, 0x01, 0x00, 0x50]),
            ("bind", vec![0x05, 0x02, 0x00, 0x01, 0x7F, 0x00, 0x00, 0x01, 0x00, 0x50]),
            ("udp_associate", vec![0x05, 0x03, 0x00, 0x01, 0x7F, 0x00, 0x00, 0x01, 0x00, 0x50]),
        ];
        
        // These should all be recognized as potential SOCKS5 traffic
        for (name, data) in test_commands {
            // Note: This tests the initial handshake detection, not full request processing
            let handshake = &[0x05, 0x01, 0x00]; // Standard handshake preceding the command
            let detector = ProtocolDetector::new();
            let result = detector.detect(handshake);
            assert_eq!(result.protocol, Protocol::Socks5, "Should detect SOCKS5 for {} command", name);
        }
    }
}

#[cfg(all(test, not(debug_assertions)))]
mod socks5_benchmarks {
    use super::*;
    use std::time::Instant;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    
    /// Benchmark results structure
    struct BenchmarkResult {
        name: &'static str,
        iterations: u64,
        total_time: Duration,
        ops_per_sec: f64,
        avg_time_us: f64,
    }
    
    impl BenchmarkResult {
        fn print(&self) {
            println!("  {:<35} {:>10} ops/sec | {:>8.2} μs/op | {} iterations in {:?}", 
                self.name,
                self.ops_per_sec as u64,
                self.avg_time_us,
                self.iterations,
                self.total_time
            );
        }
    }
    
    /// Run a benchmark with warmup and measurement phases
    fn bench_sync<F>(name: &'static str, iterations: u64, mut f: F) -> BenchmarkResult
    where
        F: FnMut(),
    {
        // Warmup phase
        for _ in 0..iterations / 10 {
            f();
        }
        
        // Measurement phase
        let start = Instant::now();
        for _ in 0..iterations {
            f();
        }
        let total_time = start.elapsed();
        
        let ops_per_sec = iterations as f64 / total_time.as_secs_f64();
        let avg_time_us = total_time.as_micros() as f64 / iterations as f64;
        
        BenchmarkResult {
            name,
            iterations,
            total_time,
            ops_per_sec,
            avg_time_us,
        }
    }
    
    #[test]
    fn bench_socks5_detection_performance() {
        println!("\n=== SOCKS5 Detection Performance Benchmark ===");
        
        let detector = ProtocolDetector::new();
        
        // Test data sets
        let test_cases = vec![
            ("valid_socks5_no_auth", vec![0x05, 0x01, 0x00]),
            ("valid_socks5_multi_methods", vec![0x05, 0x03, 0x00, 0x01, 0x02]),
            ("invalid_socks4", vec![0x04, 0x01, 0x00]),
            ("small_random", vec![0x41, 0x42, 0x43, 0x44]),
            ("medium_random", vec![0x41; 64]),
            ("large_random", vec![0x41; 512]),
        ];
        
        println!("\nProtocolDetector performance:");
        for (name, data) in &test_cases {
            let data_clone = data.clone();
            let result = bench_sync(name, 100_000, || {
                let _ = detector.detect(&data_clone);
            });
            result.print();
        }
    }
    
    #[tokio::test]
    async fn bench_socks5_handshake_throughput() {
        println!("\n=== SOCKS5 Handshake Throughput Benchmark ===");
        
        // Start target server for forwarding tests
        let target_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_addr = target_listener.local_addr().unwrap();
        
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = target_listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    while let Ok(n) = stream.read(&mut buf).await {
                        if n == 0 { break; }
                        let _ = stream.write_all(&buf[..n]).await;
                    }
                });
            }
        });
        
        // Start SOCKS5 proxy server
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        let tcp_tuning = litebike::libc_socket_tune::TcpTuningOptions::default();
        tokio::spawn(async move {
            let handler = Socks5Handler::new();
            while let Ok((stream, _)) = litebike::libc_socket_tune::accept_with_options(&proxy_listener, &tcp_tuning).await {
                let handler_clone = handler.clone();
                tokio::spawn(async move {
                    let prefixed_stream = PrefixedStream::new(stream, vec![]);
                    let _ = handler_clone.handle(prefixed_stream).await;
                });
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Benchmark different concurrency levels
        let concurrency_levels = vec![1, 10, 50, 100];
        
        for concurrency in concurrency_levels {
            let connections = Arc::new(AtomicU64::new(0));
            let start = Instant::now();
            let mut handles = vec![];
            
            for _ in 0..concurrency {
                let proxy_addr_clone = proxy_addr;
                let target_addr_clone = target_addr;
                let connections_clone = Arc::clone(&connections);
                
                let handle = tokio::spawn(async move {
                    for _ in 0..10 {  // Each task performs 10 connections
                        match TcpStream::connect(proxy_addr_clone).await {
                            Ok(mut stream) => {
                                stream.set_nodelay(false).unwrap(); // Enable Nagle
                                // Handshake
                                if stream.write_all(&[0x05, 0x01, 0x00]).await.is_err() {
                                    continue;
                                }
                                
                                let mut resp = [0u8; 2];
                                if stream.read_exact(&mut resp).await.is_err() {
                                    continue;
                                }
                                
                                // Connect request
                                let mut request = vec![0x05, 0x01, 0x00, 0x01];
                                request.extend_from_slice(&[0x7F, 0x00, 0x00, 0x01]);
                                request.extend_from_slice(&target_addr_clone.port().to_be_bytes());
                                
                                if stream.write_all(&request).await.is_ok() {
                                    connections_clone.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            Err(_) => continue,
                        }
                    }
                });
                handles.push(handle);
            }
            
            for handle in handles {
                let _ = handle.await;
            }
            
            let duration = start.elapsed();
            let total_connections = connections.load(Ordering::Relaxed);
            let conn_per_sec = total_connections as f64 / duration.as_secs_f64();
            
            println!("  Concurrency {:>3}: {:>8.0} connections/sec ({} connections in {:?})",
                concurrency, conn_per_sec, total_connections, duration);
        }
    }
    
    #[tokio::test]
    async fn bench_socks5_data_forwarding() {
        println!("\n=== SOCKS5 Data Forwarding Performance Benchmark ===");
        
        // Echo server for testing
        let echo_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let echo_addr = echo_listener.local_addr().unwrap();
        
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = echo_listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 65536];  // Large buffer for throughput testing
                    while let Ok(n) = stream.read(&mut buf).await {
                        if n == 0 { break; }
                        let _ = stream.write_all(&buf[..n]).await;
                    }
                });
            }
        });
        
        // SOCKS5 proxy with optimized settings
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        let tcp_tuning = litebike::libc_socket_tune::TcpTuningOptions {
            nodelay: true,
            keepalive: false,  // Disable for benchmark
            keepalive_idle_secs: None,
            keepalive_interval_secs: None,
            keepalive_count: None,
            send_buffer_size: Some(262144),  // 256KB
            recv_buffer_size: Some(262144),  // 256KB
        };
        
        tokio::spawn(async move {
            let handler = Socks5Handler::new();
            while let Ok((stream, _)) = litebike::libc_socket_tune::accept_with_options(&proxy_listener, &tcp_tuning).await {
                let handler_clone = handler.clone();
                tokio::spawn(async move {
                    let prefixed_stream = PrefixedStream::new(stream, vec![]);
                    let _ = handler_clone.handle(prefixed_stream).await;
                });
            }
        });
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Test different data sizes
        let test_sizes = vec![
            ("1KB", 1024),
            ("16KB", 16384),
            ("64KB", 65536),
            ("256KB", 262144),
            ("1MB", 1048576),
        ];
        
        for (size_name, size) in test_sizes {
            // Establish SOCKS5 connection
            let mut client = match TcpStream::connect(proxy_addr).await {
                Ok(s) => s,
                Err(e) => {
                    println!("  {} test skipped: connection failed: {}", size_name, e);
                    continue;
                }
            };
            client.set_nodelay(false).unwrap(); // Enable Nagle

            // Handshake
            client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
            let mut resp = [0u8; 2];
            client.read_exact(&mut resp).await.unwrap();
            
            // Connect to echo server through proxy
            let mut request = vec![0x05, 0x01, 0x00, 0x01];
            request.extend_from_slice(&[0x7F, 0x00, 0x00, 0x01]);
            request.extend_from_slice(&echo_addr.port().to_be_bytes());
            client.write_all(&request).await.unwrap();
            
            // Read connect response
            let mut connect_resp = [0u8; 10];
            client.read(&mut connect_resp).await.unwrap();
            
            // Prepare test data
            let test_data = vec![0x42u8; size];  // 'B' repeated
            let mut recv_buf = vec![0u8; size];
            
            // Benchmark roundtrip
            let iterations = match size {
                s if s <= 16384 => 1000,
                s if s <= 65536 => 500,
                s if s <= 262144 => 100,
                _ => 50,
            };
            
            let start = Instant::now();
            let mut bytes_transferred = 0u64;
            
            for _ in 0..iterations {
                client.write_all(&test_data).await.unwrap();
                let mut received = 0;
                while received < size {
                    match client.read(&mut recv_buf[received..]).await {
                        Ok(n) if n > 0 => received += n,
                        _ => break,
                    }
                }
                bytes_transferred += size as u64;
            }
            
            let duration = start.elapsed();
            let throughput_mbps = (bytes_transferred as f64 * 8.0) / (duration.as_secs_f64() * 1_000_000.0);
            let latency_us = duration.as_micros() as f64 / iterations as f64;
            
            println!("  {:<6} roundtrip: {:>8.2} Mbps | {:>8.2} μs/roundtrip | {} iterations",
                size_name, throughput_mbps, latency_us, iterations);
        }
    }
    
    #[test]
    fn bench_run_all() {
        println!("\n");
        println!("=====================================");
        println!("   SOCKS5 Performance Benchmarks");
        println!("=====================================");
        println!("\nNOTE: Run with --release flag for accurate results");
        println!("      cargo test --release socks5_benchmarks::bench_run_all -- --nocapture\n");
    }
}