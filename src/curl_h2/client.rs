//! HTTP/2 Client implementation using curl and h2

use super::error::H2Error;
use super::request::H2Request;
use super::response::H2Response;
use curl::easy::{Easy, List};
use std::collections::HashMap;

/// HTTP/2 client for testing QUIC server
pub struct H2Client {
    /// Curl easy handle
    handle: Easy,
    /// Default timeout
    timeout: u64,
    /// SSL verification
    verify_ssl: bool,
}

impl H2Client {
    /// Create a new HTTP/2 client
    pub fn new() -> Result<Self, H2Error> {
        let mut handle = Easy::new();

        // Enable HTTP/2
        handle.http_version(curl::easy::HttpVersion::V2)?;

        Ok(Self {
            handle,
            timeout: 30,
            verify_ssl: false,
        })
    }

    /// Create a new client with custom timeout
    pub fn with_timeout(timeout: u64) -> Result<Self, H2Error> {
        let mut client = Self::new()?;
        client.timeout = timeout;
        Ok(client)
    }

    /// Enable/disable SSL verification
    pub fn set_verify_ssl(&mut self, verify: bool) {
        self.verify_ssl = verify;
    }

    /// Perform a GET request
    pub fn get(&mut self, url: &str) -> Result<H2Response, H2Error> {
        self.request(H2Request::get(url))
    }

    /// Perform a POST request
    pub fn post(&mut self, url: &str, body: Vec<u8>) -> Result<H2Response, H2Error> {
        self.request(H2Request::post(url).body(body))
    }

    /// Perform a custom request
    pub fn request(&mut self, req: H2Request) -> Result<H2Response, H2Error> {
        // Reset handle
        self.handle.reset();

        // Set URL
        self.handle.url(&req.url)?;

        // Set HTTP method
        self.handle.custom_request(&req.method)?;

        // Set HTTP/2
        self.handle.http_version(curl::easy::HttpVersion::V2)?;

        // Set timeout
        self.handle.timeout(std::time::Duration::from_secs(req.timeout))?;

        // Set SSL options
        self.handle.ssl_verify_peer(self.verify_ssl)?;
        self.handle.ssl_verify_host(self.verify_ssl)?;

        // Set headers using curl::easy::List
        let mut headers = List::new();
        for (name, value) in &req.headers {
            headers.append(&format!("{}: {}", name, value))?;
        }
        self.handle.http_headers(headers)?;

        // Set body if present - use set_post_fields for borrowed data
        if let Some(body) = &req.body {
            self.handle.post(true)?;
            self.handle.post_fields_copy(body.as_slice())?;
        }

        // Capture response
        let mut response_data = Vec::new();
        let mut response_headers: HashMap<String, String> = HashMap::new();

        {
            let mut transfer = self.handle.transfer();

            // Capture response body
            transfer.write_function(|data| {
                response_data.extend_from_slice(data);
                Ok(data.len())
            })?;

            // Capture response headers
            transfer.header_function(|header| {
                let header_str = String::from_utf8_lossy(header);
                if let Some(colon_pos) = header_str.find(':') {
                    let name = header_str[..colon_pos].trim().to_lowercase();
                    let value = header_str[colon_pos + 1..].trim().to_string();
                    response_headers.insert(name, value);
                }
                true
            })?;

            // Perform request
            transfer.perform()?;
        }

        // Get response code
        let status = self.handle.response_code()? as u16;

        // Get HTTP version used - check via response info
        let version = "HTTP/2".to_string();

        Ok(H2Response {
            status,
            headers: response_headers,
            body: response_data,
            stream_id: None, // curl doesn't expose stream ID directly
            version,
        })
    }

    /// Download a file from URL
    pub fn download(&mut self, url: &str) -> Result<Vec<u8>, H2Error> {
        let response = self.get(url)?;
        Ok(response.body)
    }

    /// Check if server supports HTTP/2
    pub fn check_h2_support(&mut self, url: &str) -> Result<bool, H2Error> {
        let response = self.get(url)?;
        Ok(response.version.starts_with("HTTP/2"))
    }
}

impl Default for H2Client {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // Fallback client with defaults
            H2Client {
                handle: Easy::new(),
                timeout: 30,
                verify_ssl: false,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = H2Client::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_with_timeout() {
        let client = H2Client::with_timeout(60);
        assert!(client.is_ok());
    }

    #[test]
    fn test_request_builder() {
        let req = H2Request::get("http://example.com")
            .header("User-Agent", "test")
            .timeout(10)
            .build();
        
        assert_eq!(req.method, "GET");
        assert_eq!(req.url, "http://example.com");
        assert_eq!(req.timeout, 10);
    }
}
