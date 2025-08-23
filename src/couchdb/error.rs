use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

/// CouchDB error response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CouchError {
    pub error: String,
    pub reason: String,
}

impl CouchError {
    pub fn new(error: &str, reason: &str) -> Self {
        Self {
            error: error.to_string(),
            reason: reason.to_string(),
        }
    }
    
    pub fn bad_request(reason: &str) -> Self {
        Self::new("bad_request", reason)
    }
    
    pub fn not_found(reason: &str) -> Self {
        Self::new("not_found", reason)
    }
    
    pub fn conflict(reason: &str) -> Self {
        Self::new("conflict", reason)
    }
    
    pub fn unauthorized(reason: &str) -> Self {
        Self::new("unauthorized", reason)
    }
    
    pub fn forbidden(reason: &str) -> Self {
        Self::new("forbidden", reason)
    }
    
    pub fn method_not_allowed(reason: &str) -> Self {
        Self::new("method_not_allowed", reason)
    }
    
    pub fn not_acceptable(reason: &str) -> Self {
        Self::new("not_acceptable", reason)
    }
    
    pub fn precondition_failed(reason: &str) -> Self {
        Self::new("precondition_failed", reason)
    }
    
    pub fn request_entity_too_large(reason: &str) -> Self {
        Self::new("request_entity_too_large", reason)
    }
    
    pub fn unsupported_media_type(reason: &str) -> Self {
        Self::new("unsupported_media_type", reason)
    }
    
    pub fn internal_server_error(reason: &str) -> Self {
        Self::new("internal_server_error", reason)
    }
    
    pub fn service_unavailable(reason: &str) -> Self {
        Self::new("service_unavailable", reason)
    }
}

impl fmt::Display for CouchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.error, self.reason)
    }
}

impl std::error::Error for CouchError {}

impl From<serde_json::Error> for CouchError {
    fn from(err: serde_json::Error) -> Self {
        CouchError::bad_request(&format!("JSON parsing error: {}", err))
    }
}

impl From<sled::Error> for CouchError {
    fn from(err: sled::Error) -> Self {
        CouchError::internal_server_error(&format!("Database error: {}", err))
    }
}

impl From<uuid::Error> for CouchError {
    fn from(err: uuid::Error) -> Self {
        CouchError::bad_request(&format!("UUID error: {}", err))
    }
}

impl From<base64::DecodeError> for CouchError {
    fn from(err: base64::DecodeError) -> Self {
        CouchError::bad_request(&format!("Base64 decode error: {}", err))
    }
}

/// Result type for CouchDB operations
pub type CouchResult<T> = Result<T, CouchError>;

/// HTTP status code mapping for CouchDB errors
impl CouchError {
    pub fn status_code(&self) -> u16 {
        match self.error.as_str() {
            "bad_request" => 400,
            "unauthorized" => 401,
            "forbidden" => 403,
            "not_found" => 404,
            "method_not_allowed" => 405,
            "not_acceptable" => 406,
            "conflict" => 409,
            "precondition_failed" => 412,
            "request_entity_too_large" => 413,
            "unsupported_media_type" => 415,
            "internal_server_error" => 500,
            "service_unavailable" => 503,
            _ => 500,
        }
    }
}