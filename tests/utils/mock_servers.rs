// Mock Servers for Testing
// Provides various types of test servers for protocol testing

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use log::{debug, info};

use super::{TestConfig, TestState};

/// HTTP mock server with configurable responses
pub struct MockHttpServer {
    listener: TcpListener,
    config: TestConfig,
    responses: Vec<String>,
    state: Arc<TestState>,
}

impl MockHttpServer {
    pub async fn new(responses: Vec<String>) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        Ok(Self {
            listener,
            config: TestConfig::default(),
            responses,
            state: TestState::new(),
        })
    }
    
    pub fn addr(&self) -> SocketAddr {
        self.listener.local_addr().unwrap()
    }
    
    pub fn state(&self) -> Arc<TestState> {
        Arc::clone(&self.state)
    }
    
    /// Run the server with rotating responses
    pub async fn run(mut self) {
        let mut response_index = 0;
        
        while let Ok((mut stream, addr)) = self.listener.accept().await {
            let response = if !self.responses.is_empty() {
                let resp = self.responses[response_index % self.responses.len()].clone();
                response_index += 1;
                resp
            } else {
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK".to_string()
            };
            
            let state = Arc::clone(&self.state);
            
            tokio::spawn(async move {
                state.record_connection();
                
                let mut buffer = vec![0u8; 4096];
                match stream.read(&mut buffer).await {
                    Ok(n) => {
                        state.record_bytes(n as u64);
                        debug!("HTTP server received {} bytes from {}", n, addr);
                        
                        if let Err(e) = stream.write_all(response.as_bytes()).await {
                            state.record_error(format!("Failed to write response: {}", e));
                        } else {
                            state.record_bytes(response.len() as u64);
                        }
                    }
                    Err(e) => {
                        state.record_error(format!("Failed to read request: {}", e));
                    }
                }
            });
        }
    }
    
    /// Run server that responds based on request content
    pub async fn run_smart_responses(mut self, handler: impl Fn(&str) -> String + Send + Sync + 'static) {
        let handler = Arc::new(handler);
        
        while let Ok((mut stream, addr)) = self.listener.accept().await {
            let state = Arc::clone(&self.state);
            let handler = Arc::clone(&handler);
            
            tokio::spawn(async move {
                state.record_connection();
                
                let mut buffer = vec![0u8; 4096];
                match stream.read(&mut buffer).await {
                    Ok(n) => {
                        state.record_bytes(n as u64);
                        let request = String::from_utf8_lossy(&buffer[..n]);
                        debug!("HTTP server received request from {}: {}", addr, 
                               request.lines().next().unwrap_or(""));
                        
                        let response = handler(&request);
                        
                        if let Err(e) = stream.write_all(response.as_bytes()).await {
                            state.record_error(format!("Failed to write response: {}", e));
                        } else {
                            state.record_bytes(response.len() as u64);
                        }
                    }
                    Err(e) => {
                        state.record_error(format!("Failed to read request: {}", e));
                    }
                }
            });
        }
    }
}

/// Echo server for testing bi-directional communication
pub struct MockEchoServer {
    listener: TcpListener,
    state: Arc<TestState>,
    delay: Option<Duration>,
}

impl MockEchoServer {
    pub async fn new() -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        Ok(Self {
            listener,
            state: TestState::new(),
            delay: None,
        })
    }
    
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }
    
    pub fn addr(&self) -> SocketAddr {
        self.listener.local_addr().unwrap()
    }
    
    pub fn state(&self) -> Arc<TestState> {
        Arc::clone(&self.state)
    }
    
    pub async fn run(mut self) {
        while let Ok((mut stream, addr)) = self.listener.accept().await {
            let state = Arc::clone(&self.state);
            let delay = self.delay;
            
            tokio::spawn(async move {
                state.record_connection();
                debug!("Echo server connection from {}", addr);
                
                if let Some(delay) = delay {
                    tokio::time::sleep(delay).await;
                }
                
                let mut buffer = vec![0u8; 4096];
                while let Ok(n) = stream.read(&mut buffer).await {
                    if n == 0 { break; }
                    
                    state.record_bytes(n as u64);
                    
                    if let Err(e) = stream.write_all(&buffer[..n]).await {
                        state.record_error(format!("Echo failed: {}", e));
                        break;
                    } else {
                        state.record_bytes(n as u64);
                    }
                }
            });
        }
    }
}

/// SOCKS5 mock server for testing SOCKS5 proxy functionality
pub struct MockSocks5Server {
    listener: TcpListener,
    state: Arc<TestState>,
    auth_required: bool,
    valid_credentials: Option<(String, String)>,
}

impl MockSocks5Server {
    pub async fn new() -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        Ok(Self {
            listener,
            state: TestState::new(),
            auth_required: false,
            valid_credentials: None,
        })
    }
    
    pub fn with_auth(mut self, username: String, password: String) -> Self {
        self.auth_required = true;
        self.valid_credentials = Some((username, password));
        self
    }
    
    pub fn addr(&self) -> SocketAddr {
        self.listener.local_addr().unwrap()
    }
    
    pub fn state(&self) -> Arc<TestState> {
        Arc::clone(&self.state)
    }
    
    pub async fn run(mut self) {
        while let Ok((mut stream, addr)) = self.listener.accept().await {
            let state = Arc::clone(&self.state);
            let auth_required = self.auth_required;
            let valid_creds = self.valid_credentials.clone();
            
            tokio::spawn(async move {
                state.record_connection();
                debug!("SOCKS5 server connection from {}", addr);
                
                // Handle SOCKS5 handshake
                let mut buf = [0u8; 2];
                if let Err(e) = stream.read_exact(&mut buf).await {
                    state.record_error(format!("Failed to read handshake: {}", e));
                    return;
                }
                
                if buf[0] != 5 {
                    state.record_error(format!("Invalid SOCKS version: {}", buf[0]));
                    return;
                }
                
                let nmethods = buf[1] as usize;
                let mut methods = vec![0u8; nmethods];
                if let Err(e) = stream.read_exact(&mut methods).await {
                    state.record_error(format!("Failed to read methods: {}", e));
                    return;
                }
                
                let selected_method = if auth_required {
                    if methods.contains(&2) { 2 } else { 0xFF }
                } else {
                    if methods.contains(&0) { 0 } else { 0xFF }
                };
                
                if let Err(e) = stream.write_all(&[5, selected_method]).await {
                    state.record_error(format!("Failed to send method selection: {}", e));
                    return;
                }
                
                if selected_method == 0xFF {
                    state.record_error("No acceptable authentication methods".to_string());
                    return;
                }
                
                // Handle authentication if required
                if selected_method == 2 {
                    if let Err(e) = Self::handle_auth(&mut stream, &valid_creds, &state).await {
                        state.record_error(format!("Authentication failed: {}", e));
                        return;
                    }
                }
                
                // Handle SOCKS5 request (simplified - just respond with success)
                let mut req_buf = [0u8; 4];
                if let Err(e) = stream.read_exact(&mut req_buf).await {
                    state.record_error(format!("Failed to read request: {}", e));
                    return;
                }
                
                // Skip reading address for simplicity, just send success response
                let response = [5, 0, 0, 1, 127, 0, 0, 1, 0, 80]; // Success, bound to 127.0.0.1:80
                if let Err(e) = stream.write_all(&response).await {
                    state.record_error(format!("Failed to send response: {}", e));
                    return;
                }
                
                debug!("SOCKS5 handshake completed for {}", addr);
            });
        }
    }
    
    async fn handle_auth(stream: &mut TcpStream, valid_creds: &Option<(String, String)>, state: &Arc<TestState>) -> std::io::Result<()> {
        let mut auth_buf = [0u8; 1];
        stream.read_exact(&mut auth_buf).await?;
        
        if auth_buf[0] != 1 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid auth version"));
        }
        
        // Read username
        stream.read_exact(&mut auth_buf).await?;
        let ulen = auth_buf[0] as usize;
        let mut username = vec![0u8; ulen];
        if ulen > 0 {
            stream.read_exact(&mut username).await?;
        }
        
        // Read password
        stream.read_exact(&mut auth_buf).await?;
        let plen = auth_buf[0] as usize;
        let mut password = vec![0u8; plen];
        if plen > 0 {
            stream.read_exact(&mut password).await?;
        }
        
        let username_str = String::from_utf8_lossy(&username);
        let password_str = String::from_utf8_lossy(&password);
        
        let auth_success = if let Some((valid_user, valid_pass)) = valid_creds {
            username_str == valid_user && password_str == valid_pass
        } else {
            true // Accept any credentials if none specified
        };
        
        let response = if auth_success { [1, 0] } else { [1, 1] };
        stream.write_all(&response).await?;
        
        if !auth_success {
            return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Invalid credentials"));
        }
        
        debug!("SOCKS5 authentication successful for user: {}", username_str);
        Ok(())
    }
}

/// Slow server for testing timeouts and performance
pub struct MockSlowServer {
    listener: TcpListener,
    state: Arc<TestState>,
    response_delay: Duration,
}

impl MockSlowServer {
    pub async fn new(response_delay: Duration) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        Ok(Self {
            listener,
            state: TestState::new(),
            response_delay,
        })
    }
    
    pub fn addr(&self) -> SocketAddr {
        self.listener.local_addr().unwrap()
    }
    
    pub fn state(&self) -> Arc<TestState> {
        Arc::clone(&self.state)
    }
    
    pub async fn run(mut self) {
        while let Ok((mut stream, addr)) = self.listener.accept().await {
            let state = Arc::clone(&self.state);
            let delay = self.response_delay;
            
            tokio::spawn(async move {
                state.record_connection();
                debug!("Slow server connection from {}", addr);
                
                let mut buffer = vec![0u8; 1024];
                if let Ok(n) = stream.read(&mut buffer).await {
                    state.record_bytes(n as u64);
                    
                    // Introduce delay
                    tokio::time::sleep(delay).await;
                    
                    let response = "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nSlow";
                    if let Err(e) = stream.write_all(response.as_bytes()).await {
                        state.record_error(format!("Failed to write response: {}", e));
                    } else {
                        state.record_bytes(response.len() as u64);
                    }
                }
            });
        }
    }
}

/// Server that drops connections after specific patterns
pub struct MockUnreliableServer {
    listener: TcpListener,
    state: Arc<TestState>,
    drop_pattern: DropPattern,
}

#[derive(Clone)]
pub enum DropPattern {
    EveryNth(usize),
    AfterBytes(usize),
    Random(f64), // Probability of dropping (0.0-1.0)
}

impl MockUnreliableServer {
    pub async fn new(drop_pattern: DropPattern) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        Ok(Self {
            listener,
            state: TestState::new(),
            drop_pattern,
        })
    }
    
    pub fn addr(&self) -> SocketAddr {
        self.listener.local_addr().unwrap()
    }
    
    pub fn state(&self) -> Arc<TestState> {
        Arc::clone(&self.state)
    }
    
    pub async fn run(mut self) {
        let mut connection_count = 0;
        
        while let Ok((mut stream, addr)) = self.listener.accept().await {
            connection_count += 1;
            let state = Arc::clone(&self.state);
            let drop_pattern = self.drop_pattern.clone();
            
            tokio::spawn(async move {
                state.record_connection();
                debug!("Unreliable server connection {} from {}", connection_count, addr);
                
                let should_drop = match drop_pattern {
                    DropPattern::EveryNth(n) => connection_count % n == 0,
                    DropPattern::AfterBytes(_) => false, // Will be handled during read
                    DropPattern::Random(prob) => rand::random::<f64>() < prob,
                };
                
                if should_drop && !matches!(drop_pattern, DropPattern::AfterBytes(_)) {
                    debug!("Dropping connection {} immediately", connection_count);
                    state.record_error(format!("Connection {} dropped by pattern", connection_count));
                    return;
                }
                
                let mut buffer = vec![0u8; 1024];
                let mut bytes_read = 0;
                
                while let Ok(n) = stream.read(&mut buffer).await {
                    if n == 0 { break; }
                    
                    bytes_read += n;
                    state.record_bytes(n as u64);
                    
                    if let DropPattern::AfterBytes(limit) = drop_pattern {
                        if bytes_read >= limit {
                            debug!("Dropping connection {} after {} bytes", connection_count, bytes_read);
                            state.record_error(format!("Connection {} dropped after {} bytes", connection_count, bytes_read));
                            return;
                        }
                    }
                    
                    // Echo the data back
                    if let Err(e) = stream.write_all(&buffer[..n]).await {
                        state.record_error(format!("Failed to echo data: {}", e));
                        break;
                    } else {
                        state.record_bytes(n as u64);
                    }
                }
            });
        }
    }
}

// Helper to add randomization for testing
use rand::Rng;

/// DNS mock server for DoH testing
pub struct MockDnsServer {
    listener: TcpListener,
    state: Arc<TestState>,
}

impl MockDnsServer {
    pub async fn new() -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        Ok(Self {
            listener,
            state: TestState::new(),
        })
    }
    
    pub fn addr(&self) -> SocketAddr {
        self.listener.local_addr().unwrap()
    }
    
    pub fn state(&self) -> Arc<TestState> {
        Arc::clone(&self.state)
    }
    
    pub async fn run(mut self) {
        while let Ok((mut stream, addr)) = self.listener.accept().await {
            let state = Arc::clone(&self.state);
            
            tokio::spawn(async move {
                state.record_connection();
                debug!("DNS server connection from {}", addr);
                
                let mut buffer = vec![0u8; 4096];
                if let Ok(n) = stream.read(&mut buffer).await {
                    state.record_bytes(n as u64);
                    
                    let request = String::from_utf8_lossy(&buffer[..n]);
                    
                    let response = if request.contains("/dns-query") {
                        // DoH response
                        "HTTP/1.1 200 OK\r\n\
                         Content-Type: application/dns-message\r\n\
                         Content-Length: 32\r\n\
                         \r\n\
                         \x00\x00\x81\x80\x00\x01\x00\x01\x00\x00\x00\x00\
                         \x03www\x07example\x03com\x00\x00\x01\x00\x01\
                         \xc0\x0c\x00\x01\x00\x01\x00\x00\x00\x3c\x00\x04\
                         \x5d\xb8\xd8\x22"
                    } else {
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n"
                    };
                    
                    if let Err(e) = stream.write_all(response.as_bytes()).await {
                        state.record_error(format!("Failed to write DNS response: {}", e));
                    } else {
                        state.record_bytes(response.len() as u64);
                    }
                }
            });
        }
    }
}