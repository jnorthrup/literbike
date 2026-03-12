use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use utoipa::ToSchema;

/// CouchDB document ID type
pub type DocId = String;

/// CouchDB revision identifier
pub type RevId = String;

/// CouchDB attachment digest
pub type AttachmentDigest = String;

/// CouchDB document with metadata
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Document {
    #[serde(rename = "_id")]
    pub id: DocId,
    
    #[serde(rename = "_rev")]
    pub rev: RevId,
    
    #[serde(rename = "_deleted", skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
    
    #[serde(rename = "_attachments", skip_serializing_if = "Option::is_none")]
    pub attachments: Option<HashMap<String, AttachmentInfo>>,
    
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// Attachment information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct AttachmentInfo {
    pub content_type: String,
    pub length: u64,
    pub digest: AttachmentDigest,
    pub stub: Option<bool>,
    pub revpos: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>, // base64 encoded data for inline attachments
}

/// Database information response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DatabaseInfo {
    pub db_name: String,
    pub doc_count: u64,
    pub doc_del_count: u64,
    pub update_seq: u64,
    pub purge_seq: u64,
    pub compact_running: bool,
    pub disk_size: u64,
    pub data_size: u64,
    pub instance_start_time: String,
    pub disk_format_version: u32,
    pub committed_update_seq: u64,
}

/// View query parameters
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ViewQuery {
    pub conflicts: Option<bool>,
    pub descending: Option<bool>,
    pub endkey: Option<serde_json::Value>,
    pub endkey_docid: Option<String>,
    pub group: Option<bool>,
    pub group_level: Option<u32>,
    pub include_docs: Option<bool>,
    pub inclusive_end: Option<bool>,
    pub key: Option<serde_json::Value>,
    pub keys: Option<Vec<serde_json::Value>>,
    pub limit: Option<u32>,
    pub reduce: Option<bool>,
    pub skip: Option<u32>,
    pub stale: Option<String>,
    pub startkey: Option<serde_json::Value>,
    pub startkey_docid: Option<String>,
    pub update_seq: Option<bool>,
    pub cursor: Option<String>, // For cursor-based pagination
}

/// View row result
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ViewRow {
    pub id: Option<String>,
    pub key: serde_json::Value,
    pub value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<Document>,
}

/// View query result
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ViewResult {
    pub total_rows: u64,
    pub offset: u32,
    pub rows: Vec<ViewRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_seq: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>, // For cursor-based pagination
}

/// Design document
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DesignDocument {
    #[serde(rename = "_id")]
    pub id: String,
    
    #[serde(rename = "_rev")]
    pub rev: String,
    
    pub language: Option<String>,
    pub views: Option<HashMap<String, ViewDefinition>>,
    pub shows: Option<HashMap<String, String>>,
    pub lists: Option<HashMap<String, String>>,
    pub updates: Option<HashMap<String, String>>,
    pub filters: Option<HashMap<String, String>>,
    pub validate_doc_update: Option<String>,
}

/// View definition in design document
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ViewDefinition {
    pub map: String,
    pub reduce: Option<String>,
}

/// Bulk document operation
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BulkDocs {
    pub docs: Vec<Document>,
    pub new_edits: Option<bool>,
    pub all_or_nothing: Option<bool>,
}

/// Bulk operation result
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BulkResult {
    pub ok: Option<bool>,
    pub id: String,
    pub rev: Option<String>,
    pub error: Option<String>,
    pub reason: Option<String>,
}

/// Changes feed options
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ChangesQuery {
    pub doc_ids: Option<Vec<String>>,
    pub conflicts: Option<bool>,
    pub descending: Option<bool>,
    pub feed: Option<String>, // normal, longpoll, continuous, eventsource
    pub filter: Option<String>,
    pub heartbeat: Option<u64>,
    pub include_docs: Option<bool>,
    pub attachments: Option<bool>,
    pub att_encoding_info: Option<bool>,
    pub last_event_id: Option<u64>,
    pub limit: Option<u32>,
    pub since: Option<String>,
    pub style: Option<String>, // all_docs, main_only
    pub timeout: Option<u64>,
    pub view: Option<String>,
    pub seq_interval: Option<u32>,
}

/// Change record
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Change {
    pub seq: String,
    pub id: String,
    pub changes: Vec<ChangeRevision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<Document>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
}

/// Change revision info
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ChangeRevision {
    pub rev: String,
}

/// Changes feed response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ChangesResponse {
    pub results: Vec<Change>,
    pub last_seq: String,
    pub pending: Option<u32>,
}

/// Server information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ServerInfo {
    pub couchdb: String,
    pub uuid: String,
    pub version: String,
    pub vendor: ServerVendor,
    pub features: Vec<String>,
    pub git_sha: String,
}

/// Server vendor info
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ServerVendor {
    pub name: String,
    pub version: String,
}

/// All databases response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AllDbsResponse(pub Vec<String>);

/// Replication document
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReplicationDoc {
    #[serde(rename = "_id")]
    pub id: Option<String>,
    
    #[serde(rename = "_rev")]
    pub rev: Option<String>,
    
    pub source: ReplicationEndpoint,
    pub target: ReplicationEndpoint,
    pub continuous: Option<bool>,
    pub create_target: Option<bool>,
    pub doc_ids: Option<Vec<String>>,
    pub filter: Option<String>,
    pub proxy: Option<String>,
    pub since_seq: Option<u64>,
    pub user_ctx: Option<UserContext>,
}

/// Replication endpoint
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum ReplicationEndpoint {
    Database(String),
    Remote {
        url: String,
        headers: Option<HashMap<String, String>>,
        auth: Option<ReplicationAuth>,
    },
}

/// Replication authentication
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReplicationAuth {
    pub username: String,
    pub password: String,
}

/// User context for replication
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserContext {
    pub name: String,
    pub roles: Vec<String>,
}

/// Cursor for pagination
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Cursor {
    pub key: serde_json::Value,
    pub doc_id: Option<String>,
    pub skip: u32,
    pub timestamp: DateTime<Utc>,
}

impl Cursor {
    pub fn new(key: serde_json::Value, doc_id: Option<String>, skip: u32) -> Self {
        Self {
            key,
            doc_id,
            skip,
            timestamp: Utc::now(),
        }
    }
    
    pub fn encode(&self) -> Result<String, serde_json::Error> {
        let cursor_data = serde_json::to_vec(self)?;
        Ok(base64::encode(cursor_data))
    }
    
    pub fn decode(cursor: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let cursor_data = base64::decode(cursor)?;
        Ok(serde_json::from_slice(&cursor_data)?)
    }
}

/// IPFS content identifier for distributed storage
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IpfsCid {
    pub cid: String,
    pub size: u64,
    pub content_type: String,
}

/// M2M communication message
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct M2mMessage {
    pub id: Uuid,
    pub sender: String,
    pub recipient: Option<String>, // None for broadcast
    pub message_type: M2mMessageType,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub ttl: Option<u64>, // Time to live in seconds
}

/// M2M message types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum M2mMessageType {
    Replication,
    ViewUpdate,
    DocumentChange,
    DatabaseCreate,
    DatabaseDelete,
    AttachmentSync,
    HeartBeat,
    Custom(String),
}

/// Tensor operation definition
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TensorOperation {
    pub operation: TensorOpType,
    pub input_docs: Vec<String>, // Document IDs
    pub output_doc: Option<String>,
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Tensor operation types
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TensorOpType {
    MatrixMultiply,
    VectorAdd,
    VectorSubtract,
    DotProduct,
    CrossProduct,
    Transpose,
    Inverse,
    Eigenvalues,
    Svd, // Singular Value Decomposition
    Qr,  // QR Decomposition
    Custom(String),
}

/// Key-value store entry for IPFS-backed attachments
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct KvEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub content_type: String,
    pub ipfs_cid: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub size: u64,
    pub metadata: HashMap<String, String>,
}