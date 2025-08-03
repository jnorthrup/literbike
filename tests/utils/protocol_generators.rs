// Protocol Generators for Testing
// Generates valid and invalid protocol data for comprehensive testing

use std::fmt::Write;
use rand::Rng;

use super::ProtocolTestData;

/// HTTP protocol test data generator
pub struct HttpTestData;

impl ProtocolTestData for HttpTestData {
    fn valid_requests(&self) -> Vec<Vec<u8>> {
        vec![
            b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(),
            b"POST /api HTTP/1.1\r\nHost: example.com\r\nContent-Length: 0\r\n\r\n".to_vec(),
            b"PUT /data HTTP/1.1\r\nHost: example.com\r\nContent-Type: application/json\r\n\r\n".to_vec(),
            b"DELETE /item/123 HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(),
            b"HEAD /info HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(),
            b"OPTIONS * HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(),
            b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(),
            b"PATCH /update HTTP/1.1\r\nHost: example.com\r\nContent-Length: 0\r\n\r\n".to_vec(),
            // HTTP/1.0 requests
            b"GET / HTTP/1.0\r\nHost: example.com\r\n\r\n".to_vec(),
            // HTTP/2 upgrade request
            b"GET / HTTP/1.1\r\nHost: example.com\r\nConnection: Upgrade, HTTP2-Settings\r\nUpgrade: h2c\r\n\r\n".to_vec(),
        ]
    }
    
    fn invalid_requests(&self) -> Vec<Vec<u8>> {
        vec![
            b"INVALID / HTTP/1.1\r\n".to_vec(), // Invalid method
            b"GET\r\n".to_vec(), // Missing path and version
            b"GET / HTTP/2.0\r\n".to_vec(), // Invalid HTTP version
            b"get / http/1.1\r\n".to_vec(), // Lowercase method
            b"GET  / HTTP/1.1\r\n".to_vec(), // Double space
            b"GET / HTTP/1.1".to_vec(), // Missing CRLF
            b"".to_vec(), // Empty request
        ]
    }
    
    fn edge_case_requests(&self) -> Vec<Vec<u8>> {
        vec![
            b"G".to_vec(), // Too short
            b"GE".to_vec(), // Still too short
            b"GET".to_vec(), // Method only
            b"GET /very/long/path/that/goes/on/and/on/and/includes/many/segments/to/test/buffer/limits HTTP/1.1\r\n".to_vec(),
            b"GET / HTTP/1.1\r\nHost: example.com\r\nVery-Long-Header: ".to_vec(), // Incomplete header
            // Request with null bytes
            b"GET /\x00test HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(),
            // Request with non-ASCII characters
            "GET /测试 HTTP/1.1\r\nHost: example.com\r\n\r\n".as_bytes().to_vec(),
        ]
    }
    
    fn expected_responses(&self) -> Vec<Vec<u8>> {
        vec![
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK".to_vec(),
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found".to_vec(),
            b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n".to_vec(),
            b"HTTP/1.1 200 Connection Established\r\n\r\n".to_vec(), // CONNECT response
        ]
    }
}

impl HttpTestData {
    /// Generate HTTP request with specific characteristics
    pub fn generate_request(&self, method: &str, path: &str, headers: &[(&str, &str)], body: Option<&str>) -> Vec<u8> {
        let mut request = format!("{} {} HTTP/1.1\r\n", method, path);
        
        for (name, value) in headers {
            write!(request, "{}: {}\r\n", name, value).unwrap();
        }
        
        if let Some(body) = body {
            write!(request, "Content-Length: {}\r\n", body.len()).unwrap();
        }
        
        request.push_str("\r\n");
        
        if let Some(body) = body {
            request.push_str(body);
        }
        
        request.into_bytes()
    }
    
    /// Generate PAC/WPAD requests
    pub fn generate_pac_requests(&self) -> Vec<Vec<u8>> {
        vec![
            b"GET /wpad.dat HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(),
            b"GET /proxy.pac HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(),
            b"GET /wpad.dat?v=1 HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(),
        ]
    }
    
    /// Generate requests with random data for fuzzing
    pub fn generate_random_requests(&self, count: usize) -> Vec<Vec<u8>> {
        let mut requests = Vec::new();
        let mut rng = rand::thread_rng();
        
        let methods = ["GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "CONNECT", "PATCH"];
        let paths = ["/", "/api", "/test", "/data", "/index.html", "/api/v1/users"];
        
        for _ in 0..count {
            let method = methods[rng.gen_range(0..methods.len())];
            let path = paths[rng.gen_range(0..paths.len())];
            
            let mut request = format!("{} {} HTTP/1.1\r\nHost: example.com\r\n", method, path);
            
            // Add random headers
            if rng.gen_bool(0.5) {
                request.push_str("User-Agent: TestAgent/1.0\r\n");
            }
            if rng.gen_bool(0.3) {
                request.push_str("Content-Type: application/json\r\n");
            }
            if rng.gen_bool(0.2) {
                let content_length: usize = rng.gen_range(0..1000);
                write!(request, "Content-Length: {}\r\n", content_length).unwrap();
            }
            
            request.push_str("\r\n");
            requests.push(request.into_bytes());
        }
        
        requests
    }
}

/// SOCKS5 protocol test data generator
pub struct Socks5TestData;

impl ProtocolTestData for Socks5TestData {
    fn valid_requests(&self) -> Vec<Vec<u8>> {
        vec![
            vec![0x05, 0x01, 0x00], // No auth
            vec![0x05, 0x01, 0x02], // Username/password auth
            vec![0x05, 0x02, 0x00, 0x02], // Both no auth and username/password
            vec![0x05, 0x03, 0x00, 0x01, 0x02], // No auth, GSSAPI, username/password
        ]
    }
    
    fn invalid_requests(&self) -> Vec<Vec<u8>> {
        vec![
            vec![0x04, 0x01, 0x00], // SOCKS4
            vec![0x06, 0x01, 0x00], // Invalid version
            vec![0x05], // Incomplete
            vec![0x05, 0x00], // Zero methods
            vec![0x05, 0xFF, 0x00], // Too many methods
        ]
    }
    
    fn edge_case_requests(&self) -> Vec<Vec<u8>> {
        vec![
            vec![0x05, 0x01], // Missing method
            vec![0x05, 0x02, 0x00], // Incomplete method list
            vec![0x05, 0x01, 0xFF], // Unsupported method
            vec![], // Empty
            vec![0x00], // Just a null byte
        ]
    }
    
    fn expected_responses(&self) -> Vec<Vec<u8>> {
        vec![
            vec![0x05, 0x00], // Method selection success (no auth)
            vec![0x05, 0x02], // Method selection success (username/password)
            vec![0x05, 0xFF], // No acceptable methods
            vec![0x01, 0x00], // Auth success
            vec![0x01, 0x01], // Auth failure
        ]
    }
}

impl Socks5TestData {
    /// Generate SOCKS5 connect request
    pub fn generate_connect_request(&self, target_type: TargetType, target: &str, port: u16) -> Vec<u8> {
        let mut request = vec![0x05, 0x01, 0x00]; // Version, Connect, Reserved
        
        match target_type {
            TargetType::IPv4 => {
                request.push(0x01); // Address type: IPv4
                let ip: std::net::Ipv4Addr = target.parse().unwrap();
                request.extend_from_slice(&ip.octets());
            }
            TargetType::IPv6 => {
                request.push(0x04); // Address type: IPv6
                let ip: std::net::Ipv6Addr = target.parse().unwrap();
                request.extend_from_slice(&ip.octets());
            }
            TargetType::Domain => {
                request.push(0x03); // Address type: Domain
                request.push(target.len() as u8);
                request.extend_from_slice(target.as_bytes());
            }
        }
        
        request.extend_from_slice(&port.to_be_bytes());
        request
    }
    
    /// Generate authentication request
    pub fn generate_auth_request(&self, username: &str, password: &str) -> Vec<u8> {
        let mut request = vec![0x01]; // Auth version
        request.push(username.len() as u8);
        request.extend_from_slice(username.as_bytes());
        request.push(password.len() as u8);
        request.extend_from_slice(password.as_bytes());
        request
    }
    
    /// Generate random SOCKS5 requests for fuzzing
    pub fn generate_random_requests(&self, count: usize) -> Vec<Vec<u8>> {
        let mut requests = Vec::new();
        let mut rng = rand::thread_rng();
        
        for _ in 0..count {
            let version = if rng.gen_bool(0.9) { 0x05 } else { rng.gen_range(0..256) as u8 };
            let nmethods = rng.gen_range(1..5) as u8;
            
            let mut request = vec![version, nmethods];
            
            for _ in 0..nmethods {
                request.push(rng.gen_range(0..256) as u8);
            }
            
            requests.push(request);
        }
        
        requests
    }
}

#[derive(Clone, Copy)]
pub enum TargetType {
    IPv4,
    IPv6,
    Domain,
}

/// TLS protocol test data generator
pub struct TlsTestData;

impl ProtocolTestData for TlsTestData {
    fn valid_requests(&self) -> Vec<Vec<u8>> {
        vec![
            vec![0x16, 0x03, 0x01], // TLS 1.0 handshake
            vec![0x16, 0x03, 0x02], // TLS 1.1 handshake
            vec![0x16, 0x03, 0x03], // TLS 1.2 handshake
            vec![0x16, 0x03, 0x04], // TLS 1.3 handshake
        ]
    }
    
    fn invalid_requests(&self) -> Vec<Vec<u8>> {
        vec![
            vec![0x15, 0x03, 0x03], // Not a handshake (alert)
            vec![0x17, 0x03, 0x03], // Application data
            vec![0x16, 0x02, 0x03], // Invalid version
            vec![0x16, 0x04, 0x03], // Future version
        ]
    }
    
    fn edge_case_requests(&self) -> Vec<Vec<u8>> {
        vec![
            vec![0x16], // Incomplete
            vec![0x16, 0x03], // Missing minor version
            vec![0x16, 0x03, 0x00], // TLS version 1.0-1 (invalid)
            vec![0x16, 0x03, 0xFF], // Unknown TLS version
        ]
    }
    
    fn expected_responses(&self) -> Vec<Vec<u8>> {
        vec![
            // TLS responses would be complex, keeping simple for testing
            vec![0x16, 0x03, 0x03, 0x00, 0x01, 0x02], // Server hello response
        ]
    }
}

impl TlsTestData {
    /// Generate TLS ClientHello with SNI
    pub fn generate_client_hello_with_sni(&self, hostname: &str) -> Vec<u8> {
        // Simplified TLS ClientHello with SNI extension
        let mut hello = vec![
            0x16, 0x03, 0x03, // Content Type: Handshake, Version: TLS 1.2
        ];
        
        // This is a very simplified implementation
        // In reality, ClientHello is much more complex
        let sni_extension = Self::create_sni_extension(hostname);
        
        // Add length and handshake data (simplified)
        let handshake_data = vec![
            0x01, // Handshake type: ClientHello
            0x00, 0x00, 0x20, // Length (32 bytes, simplified)
            0x03, 0x03, // Protocol version
            // Random (32 bytes) - using zeros for simplicity
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, // Session ID length
            0x00, 0x02, // Cipher suites length
            0x00, 0x2F, // Cipher suite: TLS_RSA_WITH_AES_128_CBC_SHA
            0x01, 0x00, // Compression method: null
        ];
        
        let total_length = (handshake_data.len() + sni_extension.len()) as u16;
        hello.extend_from_slice(&total_length.to_be_bytes());
        hello.extend_from_slice(&handshake_data);
        hello.extend_from_slice(&sni_extension);
        
        hello
    }
    
    fn create_sni_extension(hostname: &str) -> Vec<u8> {
        let mut extension = vec![
            0x00, 0x00, // Extension type: SNI
        ];
        
        let hostname_bytes = hostname.as_bytes();
        let extension_length = (5 + hostname_bytes.len()) as u16;
        
        extension.extend_from_slice(&extension_length.to_be_bytes());
        extension.extend_from_slice(&((hostname_bytes.len() + 3) as u16).to_be_bytes()); // Server name list length
        extension.push(0x00); // Name type: hostname
        extension.extend_from_slice(&(hostname_bytes.len() as u16).to_be_bytes());
        extension.extend_from_slice(hostname_bytes);
        
        extension
    }
}

/// DNS-over-HTTPS test data generator
pub struct DohTestData;

impl ProtocolTestData for DohTestData {
    fn valid_requests(&self) -> Vec<Vec<u8>> {
        vec![
            b"POST /dns-query HTTP/1.1\r\nHost: example.com\r\nContent-Type: application/dns-message\r\nContent-Length: 32\r\n\r\n".to_vec(),
            b"GET /dns-query?dns=AAABAAABAAAAAAAAA3d3dwdleGFtcGxlA2NvbQAAAQAB HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(),
            b"POST /resolve HTTP/1.1\r\nHost: example.com\r\nContent-Type: application/dns-message\r\n\r\n".to_vec(),
            b"GET /api HTTP/1.1\r\nHost: example.com\r\nAccept: application/dns-message\r\n\r\n".to_vec(),
        ]
    }
    
    fn invalid_requests(&self) -> Vec<Vec<u8>> {
        vec![
            b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(), // Regular HTTP
            b"POST /api HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(), // No DNS content type
            b"GET /dns HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(), // Close but not exact
        ]
    }
    
    fn edge_case_requests(&self) -> Vec<Vec<u8>> {
        vec![
            b"POST /dns-query HTTP/1.1\r\nContent-Type: application/dns-message\r\n\r\n".to_vec(), // Missing host
            b"GET /dns-query?dns= HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(), // Empty DNS parameter
            b"POST /dns-query HTTP/1.1\r\nContent-Type: application/dns-message\r\nContent-Length: 0\r\n\r\n".to_vec(), // Empty body
        ]
    }
    
    fn expected_responses(&self) -> Vec<Vec<u8>> {
        vec![
            b"HTTP/1.1 200 OK\r\nContent-Type: application/dns-message\r\nContent-Length: 32\r\n\r\n".to_vec(),
            b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n".to_vec(),
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec(),
        ]
    }
}

impl DohTestData {
    /// Generate DNS query in wire format
    pub fn generate_dns_query(&self, domain: &str, record_type: u16) -> Vec<u8> {
        let mut query = Vec::new();
        
        // DNS header
        query.extend_from_slice(&[
            0x12, 0x34, // Transaction ID
            0x01, 0x00, // Flags: Standard query
            0x00, 0x01, // Questions: 1
            0x00, 0x00, // Answer RRs: 0
            0x00, 0x00, // Authority RRs: 0
            0x00, 0x00, // Additional RRs: 0
        ]);
        
        // Question section
        for part in domain.split('.') {
            query.push(part.len() as u8);
            query.extend_from_slice(part.as_bytes());
        }
        query.push(0x00); // Null terminator
        
        query.extend_from_slice(&record_type.to_be_bytes()); // Type
        query.extend_from_slice(&[0x00, 0x01]); // Class: IN
        
        query
    }
    
    /// Generate base64url-encoded DNS query for GET requests
    pub fn generate_base64url_query(&self, domain: &str, record_type: u16) -> String {
        let query = self.generate_dns_query(domain, record_type);
        base64_url::encode(&query)
    }
    
    /// Generate DoH POST request with DNS query
    pub fn generate_doh_post_request(&self, domain: &str, record_type: u16) -> Vec<u8> {
        let dns_query = self.generate_dns_query(domain, record_type);
        
        let request = format!(
            "POST /dns-query HTTP/1.1\r\n\
             Host: dns.example.com\r\n\
             Content-Type: application/dns-message\r\n\
             Content-Length: {}\r\n\
             \r\n",
            dns_query.len()
        );
        
        let mut full_request = request.into_bytes();
        full_request.extend_from_slice(&dns_query);
        full_request
    }
    
    /// Generate DoH GET request with DNS query
    pub fn generate_doh_get_request(&self, domain: &str, record_type: u16) -> Vec<u8> {
        let dns_query_b64 = self.generate_base64url_query(domain, record_type);
        
        let request = format!(
            "GET /dns-query?dns={} HTTP/1.1\r\n\
             Host: dns.example.com\r\n\
             Accept: application/dns-message\r\n\
             \r\n",
            dns_query_b64
        );
        
        request.into_bytes()
    }
}

// Helper module for base64url encoding
mod base64_url {
    use base64::{Engine as _, engine::general_purpose};

    pub fn encode(input: &[u8]) -> String {
        general_purpose::URL_SAFE_NO_PAD.encode(input)
    }
}

/// Protocol fuzzing generator
pub struct FuzzGenerator;

impl FuzzGenerator {
    /// Generate completely random data for protocol fuzzing
    pub fn generate_random_data(&self, min_size: usize, max_size: usize, count: usize) -> Vec<Vec<u8>> {
        let mut data = Vec::new();
        let mut rng = rand::thread_rng();
        
        for _ in 0..count {
            let size = rng.gen_range(min_size..=max_size);
            let mut random_bytes = vec![0u8; size];
            rng.fill(&mut random_bytes[..]);
            data.push(random_bytes);
        }
        
        data
    }
    
    /// Generate data with specific patterns for edge case testing
    pub fn generate_edge_case_data(&self) -> Vec<Vec<u8>> {
        vec![
            vec![], // Empty
            vec![0x00], // Single null byte
            vec![0xFF], // Single 0xFF byte
            vec![0x00; 1024], // All zeros
            vec![0xFF; 1024], // All ones
            (0u8..=255u8).collect(), // All possible byte values
            vec![0x41; 65536], // Large buffer of 'A'
            // Alternating patterns
            (0..1024).map(|i| (i % 2) as u8).collect(),
            // ASCII printable characters
            (32u8..=126u8).cycle().take(1024).collect(),
        ]
    }
    
    /// Generate malformed protocol headers for robustness testing
    pub fn generate_malformed_headers(&self) -> Vec<Vec<u8>> {
        vec![
            b"GET / HTTP/999.999\r\n\r\n".to_vec(), // Invalid HTTP version
            b"METHOD_TOO_LONG_TO_BE_REASONABLE /path HTTP/1.1\r\n\r\n".to_vec(),
            b"GET \r\n".to_vec(), // Missing path
            b"GET /" // Missing HTTP version and CRLF
                .iter()
                .chain(std::iter::repeat(&b' ').take(10000)) // Very long line
                .chain(b" HTTP/1.1\r\n\r\n".iter())
                .cloned()
                .collect(),
            // SOCKS5 malformed
            vec![0x05, 0x01, 0xFF, 0x00, 0x01, 0x02], // Invalid method count vs actual methods
            vec![0x05, 0x00, 0x00], // Zero methods but has method bytes
            // TLS malformed
            vec![0x16, 0x99, 0x99], // Invalid TLS version
            vec![0x16, 0x03, 0x03, 0xFF, 0xFF], // Invalid length
        ]
    }
}