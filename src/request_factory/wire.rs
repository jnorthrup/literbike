use serde::{Deserialize, Serialize};
use serde_json::Value;

/// HTTP batch request envelope — POST `/_rf`
#[derive(Debug, Serialize, Deserialize)]
pub struct RfRequest {
    pub invocations: Vec<Invocation>,
}

/// One operation within a batch.
#[derive(Debug, Serialize, Deserialize)]
pub struct Invocation {
    pub operation: String,
    pub entity_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Maps to CouchDB `_rev`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}

/// HTTP batch response envelope.
#[derive(Debug, Serialize, Deserialize)]
pub struct RfResponse {
    pub results: Vec<RfResult>,
    pub side_effects: Vec<Value>,
}

/// Result for one invocation.
#[derive(Debug, Serialize, Deserialize)]
pub struct RfResult {
    pub id: String,
    /// Updated CouchDB `_rev` after write.
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
