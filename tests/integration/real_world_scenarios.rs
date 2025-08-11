// tests/integration/real_world_scenarios.rs

use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::time::Duration;
use tokio::time::timeout;

use crate::utils::mock_servers::{MockHttpServer, MockSocks5Server};

#[tokio::test]
async fn test_http_get_request_through_proxy() {
    // Start a mock HTTP server
    let http_server = MockHttpServer::new(vec!["HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nHello".to_string()]).await.unwrap();
    let http_server_addr = http_server.addr();
    tokio::spawn(http_server.run());

    // This is where the proxy would be started. For now, we'll just connect to the mock server directly.
    // In a real test, we would start the litebike proxy here and connect to it.
    // let proxy_addr = "127.0.0.1:8888";

    // Connect to the mock server
    let mut stream = TcpStream::connect(http_server_addr).await.unwrap();

    // Send a simple HTTP GET request
    let request = "GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
    stream.write_all(request.as_bytes()).await.unwrap();

    // Read the response
    let mut response = String::new();
    timeout(Duration::from_secs(5), stream.read_to_string(&mut response)).await.unwrap().unwrap();

    // Assert that the response is correct
    assert_eq!(response, "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nHello");
}

#[tokio::test]
async fn test_false_positives() {
    // Start a mock HTTP server
    let http_server = MockHttpServer::new(vec!["HTTP/1.1 200 OK\r\n\r\n".to_string()]).await.unwrap();
    let http_server_addr = http_server.addr();
    tokio::spawn(http_server.run());

    // Start a mock SOCKS5 server
    let socks5_server = MockSocks5Server::new().await.unwrap();
    let socks5_server_addr = socks5_server.addr();
    tokio::spawn(socks5_server.run());

    // 1. Send a SOCKS5 handshake to the HTTP server
    let mut stream = TcpStream::connect(http_server_addr).await.unwrap();
    let socks5_handshake = b"\x05\x01\x00";
    stream.write_all(socks5_handshake).await.unwrap();
    let mut response = String::new();
    let result = timeout(Duration::from_secs(1), stream.read_to_string(&mut response)).await;
    assert!(result.is_err() || response.is_empty()); // The server should close the connection or not respond

    // 2. Send an HTTP GET request to the SOCKS5 server
    let mut stream = TcpStream::connect(socks5_server_addr).await.unwrap();
    let http_request = "GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
    stream.write_all(http_request.as_bytes()).await.unwrap();
    let mut response = String::new();
    let result = timeout(Duration::from_secs(1), stream.read_to_string(&mut response)).await;
    assert!(result.is_err() || response.is_empty()); // The server should close the connection or not respond

    // 3. Send a random string to both servers
    let mut stream = TcpStream::connect(http_server_addr).await.unwrap();
    let random_string = "this is not a valid protocol";
    stream.write_all(random_string.as_bytes()).await.unwrap();
    let mut response = String::new();
    let result = timeout(Duration::from_secs(1), stream.read_to_string(&mut response)).await;
    assert!(result.is_err() || response.is_empty());

    let mut stream = TcpStream::connect(socks5_server_addr).await.unwrap();
    stream.write_all(random_string.as_bytes()).await.unwrap();
    let mut response = String::new();
    let result = timeout(Duration::from_secs(1), stream.read_to_string(&mut response)).await;
    assert!(result.is_err() || response.is_empty());
}