//! Error types for HTTP/2 client operations

use std::fmt;

/// HTTP/2 client error types
#[derive(Debug)]
pub enum H2Error {
    /// Curl error
    Curl(String),
    /// HTTP/2 protocol error
    H2Protocol(String),
    /// IO error
    Io(std::io::Error),
    /// Invalid URL
    InvalidUrl(String),
    /// Connection error
    Connection(String),
    /// Timeout
    Timeout,
    /// Invalid response
    InvalidResponse(String),
}

impl fmt::Display for H2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            H2Error::Curl(msg) => write!(f, "Curl error: {}", msg),
            H2Error::H2Protocol(msg) => write!(f, "HTTP/2 protocol error: {}", msg),
            H2Error::Io(err) => write!(f, "IO error: {}", err),
            H2Error::InvalidUrl(url) => write!(f, "Invalid URL: {}", url),
            H2Error::Connection(msg) => write!(f, "Connection error: {}", msg),
            H2Error::Timeout => write!(f, "Request timeout"),
            H2Error::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
        }
    }
}

impl std::error::Error for H2Error {}

impl From<std::io::Error> for H2Error {
    fn from(err: std::io::Error) -> Self {
        H2Error::Io(err)
    }
}

impl From<curl::Error> for H2Error {
    fn from(err: curl::Error) -> Self {
        H2Error::Curl(err.description().to_string())
    }
}
