//! HTTP/2 request structure

use std::collections::HashMap;

/// HTTP/2 request builder
#[derive(Debug, Clone)]
pub struct H2Request {
    /// Request URL
    pub url: String,
    /// HTTP method
    pub method: String,
    /// Request headers
    pub headers: HashMap<String, String>,
    /// Request body
    pub body: Option<Vec<u8>>,
    /// Timeout in seconds
    pub timeout: u64,
    /// Follow redirects
    pub follow_redirects: bool,
    /// Verify SSL certificates
    pub verify_ssl: bool,
}

impl H2Request {
    /// Create a new GET request
    pub fn get(url: &str) -> Self {
        Self {
            url: url.to_string(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout: 30,
            follow_redirects: true,
            verify_ssl: false, // Default to false for self-signed certs in testing
        }
    }

    /// Create a new POST request
    pub fn post(url: &str) -> Self {
        Self {
            url: url.to_string(),
            method: "POST".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout: 30,
            follow_redirects: true,
            verify_ssl: false,
        }
    }

    /// Set request method
    pub fn method(mut self, method: &str) -> Self {
        self.method = method.to_string();
        self
    }

    /// Set request header
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.insert(name.to_lowercase(), value.to_string());
        self
    }

    /// Set request body
    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }

    /// Set request body from string
    pub fn body_text(mut self, body: &str) -> Self {
        self.body = Some(body.as_bytes().to_vec());
        self
    }

    /// Set timeout in seconds
    pub fn timeout(mut self, seconds: u64) -> Self {
        self.timeout = seconds;
        self
    }

    /// Set follow redirects
    pub fn follow_redirects(mut self, follow: bool) -> Self {
        self.follow_redirects = follow;
        self
    }

    /// Set SSL verification
    pub fn verify_ssl(mut self, verify: bool) -> Self {
        self.verify_ssl = verify;
        self
    }

    /// Build the request (consumes self)
    pub fn build(self) -> Self {
        self
    }
}
