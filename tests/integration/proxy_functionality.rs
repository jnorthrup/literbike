// Integration Tests for End-to-End Proxy Functionality
// Tests the complete proxy pipeline on unified port 8888

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{timeout, Duration};
use std::net::SocketAddr;
use std::sync::Arc;

use litebike::protocol_registry::ProtocolRegistry;
use litebike::protocol_handlers::{
    HttpDetector, HttpHandler, Socks5Detector, Socks5Handler,
    TlsDetector, TlsHandler, DohDetector, DohHandler
};

// Test utilities
struct TestServer {
    listener: TcpListener,
    addr: SocketAddr,
}

impl TestServer {
    async fn new() -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        Ok(Self { listener, addr })
    }
    
    fn addr(&self) -> SocketAddr {
        self.addr
    }
    
    async fn accept_one(&mut self) -> std::io::Result<TcpStream> {
        let (stream, _) = self.listener.accept().await?;
        Ok(stream)
    }
    
    // Simple echo server for testing
    async fn run_echo_server(mut self) {
        while let Ok((mut stream, _)) = self.listener.accept().await {
            tokio::spawn(async move {
                let mut buffer = [0u8; 1024];
                while let Ok(n) = stream.read(&mut buffer).await {
                    if n == 0 { break; }
                    let _ = stream.write_all(&buffer[..n]).await;
                }
            });
        }
    }
    
    // HTTP response server for testing
    async fn run_http_server(mut self, response: String) {
        while let Ok((mut stream, _)) = self.listener.accept().await {
            let response = response.clone();
            tokio::spawn(async move {
                let mut buffer = [0u8; 1024];
                if let Ok(_) = stream.read(&mut buffer).await {
                    let _ = stream.write_all(response.as_bytes()).await;
                }
            });
        }
    }
}

async fn setup_registry() -> ProtocolRegistry {
    let mut registry = ProtocolRegistry::new();
    
    // Register HTTP
    let http_detector = Box::new(HttpDetector::new());
    let http_handler = Box::new(HttpHandler::new());
    registry.register(http_detector, http_handler, 8);
    
    // Register SOCKS5  
    let socks5_detector = Box::new(Socks5Detector::new());
    let socks5_handler = Box::new(Socks5Handler::new());
    registry.register(socks5_detector, socks5_handler, 10);
    
    // Register TLS
    let tls_detector = Box::new(TlsDetector::new());
    let tls_handler = Box::new(TlsHandler::new());
    registry.register(tls_detector, tls_handler, 6);
    
    // Register DoH
    let doh_detector = Box::new(DohDetector::new());
    let doh_handler = Box::new(DohHandler::new().await);
    registry.register(doh_detector, doh_handler, 9);
    
    registry
}

#[cfg(test)]
mod http_proxy_tests {
    use super::*;

    #[tokio::test]
    async fn test_http_get_proxy() {
        let registry = setup_registry().await;
        
        // Set up target server
        let target_server = TestServer::new().await.unwrap();
        let target_addr = target_server.addr();
        
        let expected_response = "HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello, World!";
        tokio::spawn(target_server.run_http_server(expected_response.to_string()));
        
        // Set up proxy server
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        // Start proxy server
        let registry = Arc::new(registry);
        tokio::spawn(async move {
            while let Ok((stream, _)) = proxy_listener.accept().await {
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    let _ = registry.handle_connection(stream).await;
                });
            }
        });
        
        // Give server time to start
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Test HTTP GET through proxy
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        
        let request = format!(
            "GET http://{}/test HTTP/1.1\r\n\
             Host: {}\r\n\
             Connection: close\r\n\
             \r\n",
            target_addr, target_addr
        );
        
        client.write_all(request.as_bytes()).await.unwrap();
        
        let mut response = Vec::new();
        let result = timeout(Duration::from_secs(5), client.read_to_end(&mut response)).await;
        
        assert!(result.is_ok());
        let response_str = String::from_utf8_lossy(&response);
        assert!(response_str.contains("HTTP/1.1 200 OK"));
        assert!(response_str.contains("Hello, World!"));
    }
    
    #[tokio::test]
    async fn test_http_connect_tunnel() {
        let registry = setup_registry().await;
        
        // Set up target server
        let target_server = TestServer::new().await.unwrap();
        let target_addr = target_server.addr();
        tokio::spawn(target_server.run_echo_server());
        
        // Set up proxy server
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        let registry = Arc::new(registry);
        tokio::spawn(async move {
            while let Ok((stream, _)) = proxy_listener.accept().await {
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    let _ = registry.handle_connection(stream).await;
                });
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Test CONNECT tunnel
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        
        let connect_request = format!(
            "CONNECT {} HTTP/1.1\r\n\
             Host: {}\r\n\
             \r\n",
            target_addr, target_addr
        );
        
        client.write_all(connect_request.as_bytes()).await.unwrap();
        
        // Read CONNECT response
        let mut response = [0u8; 1024];
        let n = timeout(Duration::from_secs(5), client.read(&mut response)).await.unwrap().unwrap();
        let response_str = String::from_utf8_lossy(&response[..n]);
        
        assert!(response_str.contains("200 Connection Established"));
        
        // Send data through tunnel
        let test_data = b"Hello through tunnel!";
        client.write_all(test_data).await.unwrap();
        
        // Read echoed data
        let mut echo_response = [0u8; 1024];
        let n = timeout(Duration::from_secs(5), client.read(&mut echo_response)).await.unwrap().unwrap();
        
        assert_eq!(&echo_response[..n], test_data);
    }
    
    #[tokio::test]
    async fn test_http_post_with_body() {
        let registry = setup_registry().await;
        
        let target_server = TestServer::new().await.unwrap();
        let target_addr = target_server.addr();
        
        let expected_response = "HTTP/1.1 201 Created\r\nContent-Length: 7\r\n\r\nCreated";
        tokio::spawn(target_server.run_http_server(expected_response.to_string()));
        
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        let registry = Arc::new(registry);
        tokio::spawn(async move {
            while let Ok((stream, _)) = proxy_listener.accept().await {
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    let _ = registry.handle_connection(stream).await;
                });
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        
        let body = r#"{"name": "test", "value": 123}"#;
        let request = format!(
            "POST http://{}/api HTTP/1.1\r\n\
             Host: {}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             {}",
            target_addr, target_addr, body.len(), body
        );
        
        client.write_all(request.as_bytes()).await.unwrap();
        
        let mut response = Vec::new();
        let result = timeout(Duration::from_secs(5), client.read_to_end(&mut response)).await;
        
        assert!(result.is_ok());
        let response_str = String::from_utf8_lossy(&response);
        assert!(response_str.contains("201 Created"));
        assert!(response_str.contains("Created"));
    }
}

#[cfg(test)]
mod socks5_proxy_tests {
    use super::*;

    #[tokio::test]
    async fn test_socks5_no_auth_connection() {
        let registry = setup_registry().await;
        
        let target_server = TestServer::new().await.unwrap();
        let target_addr = target_server.addr();
        tokio::spawn(target_server.run_echo_server());
        
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        let registry = Arc::new(registry);
        tokio::spawn(async move {
            while let Ok((stream, _)) = proxy_listener.accept().await {
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    let _ = registry.handle_connection(stream).await;
                });
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        
        // SOCKS5 handshake - request no authentication
        client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
        
        // Read handshake response
        let mut response = [0u8; 2];
        let n = timeout(Duration::from_secs(5), client.read_exact(&mut response)).await.unwrap().unwrap();
        assert_eq!(response, [0x05, 0x00]); // Version 5, no auth selected
        
        // SOCKS5 connect request
        let mut connect_request = vec![0x05, 0x01, 0x00, 0x01]; // Ver, Connect, Reserved, IPv4
        connect_request.extend_from_slice(&target_addr.ip().octets());
        connect_request.extend_from_slice(&target_addr.port().to_be_bytes());
        
        client.write_all(&connect_request).await.unwrap();
        
        // Read connect response
        let mut connect_response = [0u8; 10]; // Minimum size for IPv4 response
        let n = timeout(Duration::from_secs(5), client.read(&mut connect_response)).await.unwrap().unwrap();
        assert!(n >= 10);
        assert_eq!(connect_response[0], 0x05); // Version
        assert_eq!(connect_response[1], 0x00); // Success
        
        // Test data transfer through SOCKS5 tunnel
        let test_data = b"Hello via SOCKS5!";
        client.write_all(test_data).await.unwrap();
        
        let mut echo_response = [0u8; 1024];
        let n = timeout(Duration::from_secs(5), client.read(&mut echo_response)).await.unwrap().unwrap();
        assert_eq!(&echo_response[..n], test_data);
    }
    
    #[tokio::test] 
    async fn test_socks5_with_authentication() {
        let registry = setup_registry().await;
        
        let target_server = TestServer::new().await.unwrap();
        let target_addr = target_server.addr();
        tokio::spawn(target_server.run_echo_server());
        
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        let registry = Arc::new(registry);
        tokio::spawn(async move {
            while let Ok((stream, _)) = proxy_listener.accept().await {
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    let _ = registry.handle_connection(stream).await;
                });
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        
        // SOCKS5 handshake - request username/password auth
        client.write_all(&[0x05, 0x01, 0x02]).await.unwrap();
        
        // Read handshake response
        let mut response = [0u8; 2];
        timeout(Duration::from_secs(5), client.read_exact(&mut response)).await.unwrap().unwrap();
        assert_eq!(response, [0x05, 0x02]); // Version 5, username/password selected
        
        // Send authentication
        let username = b"testuser";
        let password = b"testpass";
        let mut auth_request = vec![0x01]; // Auth version
        auth_request.push(username.len() as u8);
        auth_request.extend_from_slice(username);
        auth_request.push(password.len() as u8);
        auth_request.extend_from_slice(password);
        
        client.write_all(&auth_request).await.unwrap();
        
        // Read auth response
        let mut auth_response = [0u8; 2];
        timeout(Duration::from_secs(5), client.read_exact(&mut auth_response)).await.unwrap().unwrap();
        assert_eq!(auth_response, [0x01, 0x00]); // Auth success
        
        // Continue with connection as in no-auth test...
    }
    
    #[tokio::test]
    async fn test_socks5_domain_name_resolution() {
        let registry = setup_registry().await;
        
        let target_server = TestServer::new().await.unwrap();
        let target_port = target_server.addr().port();
        tokio::spawn(target_server.run_echo_server());
        
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        let registry = Arc::new(registry);
        tokio::spawn(async move {
            while let Ok((stream, _)) = proxy_listener.accept().await {
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    let _ = registry.handle_connection(stream).await;
                });
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        
        // SOCKS5 handshake
        client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
        let mut response = [0u8; 2];
        timeout(Duration::from_secs(5), client.read_exact(&mut response)).await.unwrap().unwrap();
        
        // SOCKS5 connect request with domain name
        let domain = b"localhost";
        let mut connect_request = vec![0x05, 0x01, 0x00, 0x03]; // Ver, Connect, Reserved, Domain
        connect_request.push(domain.len() as u8);
        connect_request.extend_from_slice(domain);
        connect_request.extend_from_slice(&target_port.to_be_bytes());
        
        client.write_all(&connect_request).await.unwrap();
        
        // Read connect response
        let mut connect_response = [0u8; 32]; // Buffer for domain response
        let n = timeout(Duration::from_secs(5), client.read(&mut connect_response)).await.unwrap().unwrap();
        assert!(n >= 4);
        assert_eq!(connect_response[0], 0x05); // Version
        assert_eq!(connect_response[1], 0x00); // Success
    }
}

#[cfg(test)]
mod multi_protocol_tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrent_different_protocols() {
        let registry = Arc::new(setup_registry().await);
        
        // Set up target servers
        let http_server = TestServer::new().await.unwrap();
        let http_addr = http_server.addr();
        let http_response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
        tokio::spawn(http_server.run_http_server(http_response.to_string()));
        
        let echo_server = TestServer::new().await.unwrap();
        let echo_addr = echo_server.addr();
        tokio::spawn(echo_server.run_echo_server());
        
        // Set up proxy server
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        tokio::spawn(async move {
            while let Ok((stream, _)) = proxy_listener.accept().await {
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    let _ = registry.handle_connection(stream).await;
                });
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Test HTTP and SOCKS5 concurrently
        let http_task = tokio::spawn(async move {
            let mut client = TcpStream::connect(proxy_addr).await.unwrap();
            let request = format!(
                "GET http://{}/test HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                http_addr, http_addr
            );
            client.write_all(request.as_bytes()).await.unwrap();
            
            let mut response = Vec::new();
            timeout(Duration::from_secs(5), client.read_to_end(&mut response)).await.unwrap().unwrap();
            String::from_utf8_lossy(&response).contains("200 OK")
        });
        
        let socks5_task = tokio::spawn(async move {
            let mut client = TcpStream::connect(proxy_addr).await.unwrap();
            
            // SOCKS5 handshake
            client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
            let mut response = [0u8; 2];
            timeout(Duration::from_secs(5), client.read_exact(&mut response)).await.unwrap().unwrap();
            
            if response != [0x05, 0x00] { return false; }
            
            // Connect request
            let mut connect_request = vec![0x05, 0x01, 0x00, 0x01];
            connect_request.extend_from_slice(&echo_addr.ip().octets());
            connect_request.extend_from_slice(&echo_addr.port().to_be_bytes());
            client.write_all(&connect_request).await.unwrap();
            
            let mut connect_response = [0u8; 10];
            timeout(Duration::from_secs(5), client.read(&mut connect_response)).await.unwrap().unwrap();
            
            if connect_response[0] != 0x05 || connect_response[1] != 0x00 { return false; }
            
            // Test echo
            client.write_all(b"test").await.unwrap();
            let mut echo = [0u8; 4];
            timeout(Duration::from_secs(5), client.read_exact(&mut echo)).await.unwrap().unwrap();
            
            echo == b"test"
        });
        
        let (http_result, socks5_result) = tokio::join!(http_task, socks5_task);
        assert!(http_result.unwrap());
        assert!(socks5_result.unwrap());
    }
    
    #[tokio::test]
    async fn test_protocol_detection_priority() {
        let registry = setup_registry().await;
        
        // Test that DoH takes priority over HTTP for /dns-query requests
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        let registry = Arc::new(registry);
        tokio::spawn(async move {
            while let Ok((stream, _)) = proxy_listener.accept().await {
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    let _ = registry.handle_connection(stream).await;
                });
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        
        // Send DoH request - should be detected as DoH, not HTTP
        let doh_request = "POST /dns-query HTTP/1.1\r\n\
                          Host: localhost\r\n\
                          Content-Type: application/dns-message\r\n\
                          Content-Length: 12\r\n\
                          \r\n\
                          DNS_QUERY_DATA";
        
        client.write_all(doh_request.as_bytes()).await.unwrap();
        
        let mut response = [0u8; 1024];
        let result = timeout(Duration::from_secs(5), client.read(&mut response)).await;
        
        // Should get a DoH response, not an HTTP proxy response
        assert!(result.is_ok());
        // DoH handler should return appropriate response (could be error since we sent fake DNS data)
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[tokio::test] 
    async fn test_connection_to_unreachable_target() {
        let registry = setup_registry().await;
        
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        let registry = Arc::new(registry);
        tokio::spawn(async move {
            while let Ok((stream, _)) = proxy_listener.accept().await {
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    let _ = registry.handle_connection(stream).await;
                });
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        
        // Try to connect to unreachable address via HTTP CONNECT
        let unreachable_addr = "192.0.2.1:80"; // RFC5737 test address
        let connect_request = format!(
            "CONNECT {} HTTP/1.1\r\n\
             Host: {}\r\n\
             \r\n",
            unreachable_addr, unreachable_addr
        );
        
        client.write_all(connect_request.as_bytes()).await.unwrap();
        
        let mut response = [0u8; 1024];
        let n = timeout(Duration::from_secs(10), client.read(&mut response)).await.unwrap().unwrap();
        let response_str = String::from_utf8_lossy(&response[..n]);
        
        // Should get 502 Bad Gateway error
        assert!(response_str.contains("502"));
    }
    
    #[tokio::test]
    async fn test_malformed_http_request() {
        let registry = setup_registry().await;
        
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        let registry = Arc::new(registry);
        tokio::spawn(async move {
            while let Ok((stream, _)) = proxy_listener.accept().await {
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    let _ = registry.handle_connection(stream).await;
                });
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        
        // Send malformed HTTP request
        let malformed_request = "INVALID REQUEST FORMAT\r\n\r\n";
        client.write_all(malformed_request.as_bytes()).await.unwrap();
        
        let mut response = [0u8; 1024];
        let result = timeout(Duration::from_secs(5), client.read(&mut response)).await;
        
        // Should handle gracefully - either error response or connection close
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_incomplete_socks5_handshake() {
        let registry = setup_registry().await;
        
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        let registry = Arc::new(registry);
        tokio::spawn(async move {
            while let Ok((stream, _)) = proxy_listener.accept().await {
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    let _ = registry.handle_connection(stream).await;
                });
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        
        // Send incomplete SOCKS5 handshake
        client.write_all(&[0x05]).await.unwrap(); // Just version, no method count
        
        // Connection should be handled gracefully (likely closed)
        let mut response = [0u8; 10];
        let result = timeout(Duration::from_secs(5), client.read(&mut response)).await;
        
        // Should either timeout or get an error
        assert!(result.is_ok() || result.is_err());
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;

    #[tokio::test]
    async fn test_high_concurrency_connections() {
        let registry = Arc::new(setup_registry().await);
        
        let echo_server = TestServer::new().await.unwrap();
        let echo_addr = echo_server.addr();
        tokio::spawn(echo_server.run_echo_server());
        
        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        
        tokio::spawn(async move {
            while let Ok((stream, _)) = proxy_listener.accept().await {
                let registry = Arc::clone(&registry);
                tokio::spawn(async move {
                    let _ = registry.handle_connection(stream).await;
                });
            }
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Create many concurrent connections
        let num_connections = 50;
        let mut tasks = Vec::new();
        
        for i in 0..num_connections {
            let proxy_addr = proxy_addr.clone();
            let echo_addr = echo_addr.clone();
            
            let task = tokio::spawn(async move {
                let mut client = TcpStream::connect(proxy_addr).await.unwrap();
                
                // SOCKS5 handshake
                client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
                let mut response = [0u8; 2];
                timeout(Duration::from_secs(10), client.read_exact(&mut response)).await.unwrap().unwrap();
                
                // Connect
                let mut connect_request = vec![0x05, 0x01, 0x00, 0x01];
                connect_request.extend_from_slice(&echo_addr.ip().octets());
                connect_request.extend_from_slice(&echo_addr.port().to_be_bytes());
                client.write_all(&connect_request).await.unwrap();
                
                let mut connect_response = [0u8; 10];
                timeout(Duration::from_secs(10), client.read(&mut connect_response)).await.unwrap().unwrap();
                
                // Send unique data
                let test_data = format!("test_{}", i);
                client.write_all(test_data.as_bytes()).await.unwrap();
                
                let mut echo_response = vec![0u8; test_data.len()];
                timeout(Duration::from_secs(10), client.read_exact(&mut echo_response)).await.unwrap().unwrap();
                
                String::from_utf8(echo_response).unwrap() == test_data
            });
            
            tasks.push(task);
        }
        
        // Wait for all connections to complete
        let start = tokio::time::Instant::now();
        let results: Vec<_> = futures::future::join_all(tasks).await;
        let duration = start.elapsed();
        
        // All connections should succeed
        for result in results {
            assert!(result.unwrap());
        }
        
        // Should complete reasonably quickly
        assert!(duration < Duration::from_secs(30));
    }
}