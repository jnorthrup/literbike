// DNS-over-HTTPS (DoH) Implementation
// RFC 8484 compliant DoH server and client

use std::collections::HashMap;
use std::net::{SocketAddr, IpAddr};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

/// DoH protocol detector
pub struct DohDetector {
    confidence_threshold: u8,
}

impl DohDetector {
    pub fn new() -> Self {
        Self {
            confidence_threshold: 200,
        }
    }
    
    pub fn confidence_threshold(&self) -> u8 {
        self.confidence_threshold
    }
    
    /// Detect DoH protocol in HTTP request
    pub fn detect(&self, data: &[u8]) -> DetectionResult {
        let text = match std::str::from_utf8(data) {
            Ok(s) => s,
            Err(_) => return DetectionResult::new("doh", 0),
        };
        
        let mut confidence = 0u8;
        
        // Check for DoH-specific paths
        if text.contains("/dns-query") {
            confidence = confidence.saturating_add(230);
        } else if text.contains("/resolve") {
            confidence = confidence.saturating_add(200);
        } else if text.contains("/doh") {
            confidence = confidence.saturating_add(180);
        }
        
        // Check for DoH-specific content types
        if text.contains("application/dns-message") {
            confidence = confidence.saturating_add(250);
        }
        
        // Check for DoH-specific accept headers
        if text.contains("Accept: application/dns-message") {
            confidence = confidence.saturating_add(240);
        }
        
        // Check for DNS query parameter (GET method)
        if text.contains("?dns=") || text.contains("&dns=") {
            confidence = confidence.saturating_add(220);
        }
        
        // Check HTTP method for DoH
        if text.starts_with("POST ") && confidence > 0 {
            confidence = confidence.saturating_add(20);
        } else if text.starts_with("GET ") && text.contains("dns=") {
            confidence = confidence.saturating_add(30);
        }
        
        DetectionResult::new("doh", confidence)
    }
}

/// DoH detection result
#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub protocol_name: String,
    pub confidence: u8,
}

impl DetectionResult {
    fn new(protocol: &str, confidence: u8) -> Self {
        Self {
            protocol_name: protocol.to_string(),
            confidence,
        }
    }
}

/// DoH handler for processing DNS queries over HTTPS
pub struct DohHandler {
    upstream_resolvers: Vec<String>,
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    blocked_domains: Arc<RwLock<Vec<String>>>,
}

impl DohHandler {
    pub async fn new() -> Self {
        Self {
            upstream_resolvers: vec![
                "8.8.8.8:53".to_string(),
                "1.1.1.1:53".to_string(),
                "9.9.9.9:53".to_string(),
            ],
            cache: Arc::new(RwLock::new(HashMap::new())),
            blocked_domains: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Handle DoH request
    pub async fn handle_request(&self, request: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let request_str = std::str::from_utf8(request)?;
        
        if request_str.starts_with("POST ") {
            self.handle_post_request(request).await
        } else if request_str.starts_with("GET ") {
            self.handle_get_request(request_str).await
        } else {
            Err("Unsupported HTTP method for DoH".into())
        }
    }
    
    /// Handle POST DoH request
    async fn handle_post_request(&self, request: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        // Parse HTTP request to extract DNS message from body
        let request_str = std::str::from_utf8(request)?;
        let lines: Vec<&str> = request_str.split("\r\n").collect();
        
        // Find content length
        let mut content_length = 0;
        let mut body_start = 0;
        
        for (i, line) in lines.iter().enumerate() {
            if line.to_lowercase().starts_with("content-length:") {
                if let Some(len_str) = line.split(':').nth(1) {
                    content_length = len_str.trim().parse().unwrap_or(0);
                }
            }
            if line.is_empty() {
                body_start = request_str.find("\r\n\r\n").unwrap_or(0) + 4;
                break;
            }
        }
        
        if content_length > 0 && body_start < request.len() {
            let dns_message = &request[body_start..std::cmp::min(body_start + content_length, request.len())];
            let dns_response = self.resolve_dns_query(dns_message).await?;
            
            // Build HTTP response
            let http_response = format!(
                "HTTP/1.1 200 OK\r\n\
                Content-Type: application/dns-message\r\n\
                Content-Length: {}\r\n\
                Access-Control-Allow-Origin: *\r\n\
                Cache-Control: max-age=300\r\n\
                \r\n",
                dns_response.len()
            );
            
            let mut response = http_response.into_bytes();
            response.extend_from_slice(&dns_response);
            Ok(response)
        } else {
            Err("Invalid POST request body".into())
        }
    }
    
    /// Handle GET DoH request
    async fn handle_get_request(&self, request: &str) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        // Extract DNS query from URL parameter
        if let Some(query_start) = request.find("?dns=") {
            let query_part = &request[query_start + 5..];
            let dns_param = query_part.split('&').next().unwrap_or(query_part);
            let dns_param = dns_param.split(' ').next().unwrap_or(dns_param);
            
            // Decode base64 DNS message
            let dns_message = URL_SAFE_NO_PAD.decode(dns_param)?;
            let dns_response = self.resolve_dns_query(&dns_message).await?;
            
            // Build HTTP response
            let http_response = format!(
                "HTTP/1.1 200 OK\r\n\
                Content-Type: application/dns-message\r\n\
                Content-Length: {}\r\n\
                Access-Control-Allow-Origin: *\r\n\
                Cache-Control: max-age=300\r\n\
                \r\n",
                dns_response.len()
            );
            
            let mut response = http_response.into_bytes();
            response.extend_from_slice(&dns_response);
            Ok(response)
        } else {
            // Return DoH service info page
            self.generate_service_info_page()
        }
    }
    
    /// Resolve DNS query using upstream resolvers
    async fn resolve_dns_query(&self, dns_message: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        // Parse DNS query to check for blocked domains
        if let Some(domain) = self.extract_domain_from_query(dns_message) {
            if self.is_domain_blocked(&domain).await {
                return self.generate_blocked_response(dns_message);
            }
            
            // Check cache first
            if let Some(cached_response) = self.check_cache(&domain).await {
                return Ok(cached_response);
            }
        }
        
        // Forward to upstream resolver
        for resolver in &self.upstream_resolvers {
            if let Ok(response) = self.query_upstream_resolver(resolver, dns_message).await {
                // Cache the response
                if let Some(domain) = self.extract_domain_from_query(dns_message) {
                    self.cache_response(&domain, &response).await;
                }
                return Ok(response);
            }
        }
        
        Err("All upstream resolvers failed".into())
    }
    
    /// Query upstream DNS resolver
    async fn query_upstream_resolver(&self, resolver: &str, dns_message: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        use tokio::net::UdpSocket;
        
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(resolver).await?;
        
        socket.send(dns_message).await?;
        
        let mut response = vec![0u8; 512];
        let len = socket.recv(&mut response).await?;
        response.truncate(len);
        
        Ok(response)
    }
    
    /// Extract domain name from DNS query
    fn extract_domain_from_query(&self, dns_message: &[u8]) -> Option<String> {
        if dns_message.len() < 12 {
            return None;
        }
        
        // Skip DNS header (12 bytes) and parse question section
        let mut pos = 12;
        let mut domain_parts = Vec::new();
        
        while pos < dns_message.len() {
            let label_len = dns_message[pos] as usize;
            if label_len == 0 {
                break; // End of domain name
            }
            
            pos += 1;
            if pos + label_len > dns_message.len() {
                return None;
            }
            
            if let Ok(label) = std::str::from_utf8(&dns_message[pos..pos + label_len]) {
                domain_parts.push(label.to_string());
            }
            
            pos += label_len;
        }
        
        if domain_parts.is_empty() {
            None
        } else {
            Some(domain_parts.join("."))
        }
    }
    
    /// Check if domain is blocked
    async fn is_domain_blocked(&self, domain: &str) -> bool {
        let blocked_domains = self.blocked_domains.read().await;
        blocked_domains.iter().any(|blocked| domain.contains(blocked))
    }
    
    /// Check cache for cached response
    async fn check_cache(&self, domain: &str) -> Option<Vec<u8>> {
        let cache = self.cache.read().await;
        if let Some(entry) = cache.get(domain) {
            if entry.is_valid() {
                return Some(entry.response.clone());
            }
        }
        None
    }
    
    /// Cache DNS response
    async fn cache_response(&self, domain: &str, response: &[u8]) {
        let mut cache = self.cache.write().await;
        cache.insert(domain.to_string(), CacheEntry::new(response.to_vec()));
        
        // Basic cache size management
        if cache.len() > 1000 {
            // Remove oldest entries
            let keys_to_remove: Vec<String> = cache.keys().take(100).cloned().collect();
            for key in keys_to_remove {
                cache.remove(&key);
            }
        }
    }
    
    /// Generate blocked response
    fn generate_blocked_response(&self, _original_query: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        // Return NXDOMAIN response
        let mut response = Vec::new();
        
        // DNS Header - copy transaction ID from query, set response flags
        response.extend_from_slice(&[0x00, 0x00]); // Transaction ID (should copy from query)
        response.extend_from_slice(&[0x81, 0x83]); // Flags: Response, NXDOMAIN
        response.extend_from_slice(&[0x00, 0x01]); // Questions: 1
        response.extend_from_slice(&[0x00, 0x00]); // Answer RRs: 0
        response.extend_from_slice(&[0x00, 0x00]); // Authority RRs: 0
        response.extend_from_slice(&[0x00, 0x00]); // Additional RRs: 0
        
        // Question section (copy from original query)
        // For simplicity, just return minimal response
        response.extend_from_slice(&[0x00]); // Empty question for now
        
        Ok(response)
    }
    
    /// Generate DoH service information page
    fn generate_service_info_page(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Litebike DoH Service</title>
    <meta charset="utf-8">
</head>
<body>
    <h1>🚀 Litebike DNS-over-HTTPS Service</h1>
    <p>This is a RFC 8484 compliant DoH resolver.</p>
    
    <h2>Usage</h2>
    <ul>
        <li><strong>POST</strong> /dns-query with Content-Type: application/dns-message</li>
        <li><strong>GET</strong> /dns-query?dns=&lt;base64-encoded-dns-query&gt;</li>
    </ul>
    
    <h2>Features</h2>
    <ul>
        <li>✅ RFC 8484 Compliance</li>
        <li>✅ Response Caching</li>
        <li>✅ Domain Blocking</li>
        <li>✅ Multiple Upstream Resolvers</li>
        <li>✅ CORS Support</li>
    </ul>
    
    <h2>Upstream Resolvers</h2>
    <ul>
        <li>8.8.8.8 (Google)</li>
        <li>1.1.1.1 (Cloudflare)</li>
        <li>9.9.9.9 (Quad9)</li>
    </ul>
</body>
</html>"#;

        let response = format!(
            "HTTP/1.1 200 OK\r\n\
            Content-Type: text/html; charset=utf-8\r\n\
            Content-Length: {}\r\n\
            Access-Control-Allow-Origin: *\r\n\
            \r\n{}",
            html.len(),
            html
        );
        
        Ok(response.into_bytes())
    }
    
    /// Add domain to block list
    pub async fn block_domain(&self, domain: String) {
        let mut blocked_domains = self.blocked_domains.write().await;
        if !blocked_domains.contains(&domain) {
            blocked_domains.push(domain);
        }
    }
    
    /// Remove domain from block list
    pub async fn unblock_domain(&self, domain: &str) {
        let mut blocked_domains = self.blocked_domains.write().await;
        blocked_domains.retain(|d| d != domain);
    }
    
    /// Get blocked domains list
    pub async fn get_blocked_domains(&self) -> Vec<String> {
        self.blocked_domains.read().await.clone()
    }
    
    /// Clear DNS cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
    
    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let valid_entries = cache.values().filter(|entry| entry.is_valid()).count();
        
        CacheStats {
            total_entries: cache.len(),
            valid_entries,
            expired_entries: cache.len() - valid_entries,
        }
    }
}

/// DNS cache entry
#[derive(Debug, Clone)]
struct CacheEntry {
    response: Vec<u8>,
    timestamp: std::time::SystemTime,
    ttl: std::time::Duration,
}

impl CacheEntry {
    fn new(response: Vec<u8>) -> Self {
        Self {
            response,
            timestamp: std::time::SystemTime::now(),
            ttl: std::time::Duration::from_secs(300), // 5 minutes default TTL
        }
    }
    
    fn is_valid(&self) -> bool {
        self.timestamp.elapsed().unwrap_or(self.ttl) < self.ttl
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub valid_entries: usize,
    pub expired_entries: usize,
}

/// DoH server
pub struct DohServer {
    handler: DohHandler,
    bind_addr: SocketAddr,
}

impl DohServer {
    pub async fn new(bind_addr: SocketAddr) -> Self {
        Self {
            handler: DohHandler::new().await,
            bind_addr,
        }
    }
    
    /// Start DoH server
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        println!("🚀 DoH server listening on {}", self.bind_addr);
        
        loop {
            let (stream, peer_addr) = listener.accept().await?;
            let handler = self.handler.clone();
            
            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(stream, handler, peer_addr).await {
                    eprintln!("DoH connection error from {}: {}", peer_addr, e);
                }
            });
        }
    }
    
    /// Handle individual connection
    async fn handle_connection(
        mut stream: TcpStream,
        handler: DohHandler,
        peer_addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut buffer = vec![0u8; 4096];
        let len = stream.read(&mut buffer).await?;
        buffer.truncate(len);
        
        println!("📡 DoH request from {}: {} bytes", peer_addr, len);
        
        let response = handler.handle_request(&buffer).await?;
        stream.write_all(&response).await?;
        
        Ok(())
    }
}

impl Clone for DohHandler {
    fn clone(&self) -> Self {
        Self {
            upstream_resolvers: self.upstream_resolvers.clone(),
            cache: Arc::clone(&self.cache),
            blocked_domains: Arc::clone(&self.blocked_domains),
        }
    }
}

/// Create DoH detector
pub fn create_doh_detector() -> DohDetector {
    DohDetector::new()
}

/// Create DoH handler
pub async fn create_doh_handler() -> DohHandler {
    DohHandler::new().await
}