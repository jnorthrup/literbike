//! HTTP/2 response structure

use std::collections::HashMap;

/// HTTP/2 response
#[derive(Debug, Clone)]
pub struct H2Response {
    /// HTTP status code
    pub status: u16,
    /// Response headers
    pub headers: HashMap<String, String>,
    /// Response body
    pub body: Vec<u8>,
    /// HTTP/2 stream ID
    pub stream_id: Option<u32>,
    /// Protocol version
    pub version: String,
}

impl H2Response {
    /// Create a new H2Response
    pub fn new() -> Self {
        Self {
            status: 200,
            headers: HashMap::new(),
            body: Vec::new(),
            stream_id: None,
            version: "HTTP/2".to_string(),
        }
    }

    /// Get a header value by name
    pub fn header(&self, name: &str) -> Option<&String> {
        self.headers.get(&name.to_lowercase())
    }

    /// Get body as string (UTF-8)
    pub fn text(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.body.clone())
    }

    /// Check if response is successful (2xx status)
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
}

impl Default for H2Response {
    fn default() -> Self {
        Self::new()
    }
}
