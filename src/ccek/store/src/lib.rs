//! CCEK Store - Storage layer
//!
//! Provides content-addressed storage, CAS gateway, and backend adapters.
//! Based on original CAS implementation from src/cas_storage.rs, src/cas_gateway.rs, src/cas_backends.rs.

pub mod backends;

// Real storage modules
pub mod cas;

#[cfg(feature = "couchdb")]
pub mod couchdb;

#[cfg(feature = "pijul-session")]
pub mod session;

use ccek_core::{Context, Element, Key};
use sha2::{Sha256, Digest};
use std::any::{Any, TypeId};
use thiserror::Error;

// Re-export ccek_core for convenience
pub use ccek_core::{Context as CCEKContext, Element as CCEKElement, Key as CCEKKey};

// ============================================================================
// Errors
// ============================================================================

/// Storage error types
#[derive(Debug, Error, Clone)]
pub enum StoreError {
    #[error("Block not found: {0}")]
    BlockNotFound(String),
    
    #[error("Object not found: {0}")]
    ObjectNotFound(String),
    
    #[error("Backend error: {0}")]
    BackendError(String),
    
    #[error("Invalid block hash: {0}")]
    InvalidHash(String),
    
    #[error("IO error: {0}")]
    IoError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Not implemented")]
    NotImplemented,
}

/// Result type alias for storage operations
pub type StoreResult<T> = Result<T, StoreError>;

// ============================================================================
// Series - Simple type alias for Vec (replaces TrikeShed abstraction)
// ============================================================================

/// Simple series type (alias for Vec)
pub type Series<T> = Vec<T>;

// ============================================================================
// ObjectStore - S3/GCS/Aliyun-compatible object storage
// ============================================================================

/// Object metadata
#[derive(Debug, Clone, Default)]
pub struct ObjectMeta {
    /// Object key/path
    pub key: String,
    /// Content type
    pub content_type: Option<String>,
    /// Content length
    pub size: u64,
    /// Last modified timestamp
    pub last_modified: Option<u64>,
    /// ETag
    pub etag: Option<String>,
    /// Custom metadata
    pub custom: std::collections::HashMap<String, String>,
}

/// Object with data
#[derive(Debug, Clone)]
pub struct Object {
    /// Object metadata
    pub meta: ObjectMeta,
    /// Object data
    pub data: Vec<u8>,
}

/// Object storage trait (S3/GCS/Aliyun-compatible)
#[async_trait::async_trait]
pub trait ObjectStore: Send + Sync {
    /// Get an object by key
    async fn get_object(&self, key: &str) -> StoreResult<Object>;
    
    /// Put an object
    async fn put_object(&self, key: &str, data: Vec<u8>, meta: Option<ObjectMeta>) -> StoreResult<ObjectMeta>;
    
    /// Delete an object
    async fn delete_object(&self, key: &str) -> StoreResult<bool>;
    
    /// List objects with prefix
    async fn list_objects(&self, prefix: Option<&str>) -> StoreResult<Series<ObjectMeta>>;
    
    /// Check if object exists
    async fn head_object(&self, key: &str) -> StoreResult<Option<ObjectMeta>>;
    
    /// Copy object
    async fn copy_object(&self, source: &str, dest: &str) -> StoreResult<ObjectMeta>;
}

// ============================================================================
// BlockId - Content address
// ============================================================================

/// Content address (SHA-256 hash)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockId(pub String);

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl BlockId {
    pub fn new(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }

    pub fn from_bytes(data: &[u8]) -> Self {
        let hash = compute_hash(data);
        Self(hash)
    }

    pub fn hash(&self) -> &str {
        &self.0
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        hex::decode(&self.0).unwrap_or_default()
    }
}

// ============================================================================
// Block - Storage unit
// ============================================================================

/// A storage block with data and metadata
#[derive(Debug, Clone)]
pub struct Block {
    id: BlockId,
    data: Vec<u8>,
    metadata: Option<serde_json::Value>,
}

impl Block {
    pub fn new(data: Vec<u8>) -> Self {
        let id = BlockId::from_bytes(&data);
        Self {
            id,
            data,
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn id(&self) -> &BlockId {
        &self.id
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn metadata(&self) -> Option<&serde_json::Value> {
        self.metadata.as_ref()
    }

    pub fn verify(&self) -> bool {
        let computed = compute_hash(&self.data);
        computed == self.id.0
    }
}

// ============================================================================
// StoreKey / StoreElement - CCEK Key/Element for storage
// ============================================================================

/// Storage Key - passive SDK provider
pub struct StoreKey;

impl StoreKey {
    pub const FACTORY: fn() -> StoreElement = || StoreElement::new(BackendType::Memory);
}

/// Backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    Memory,
    Git,
    Ipfs,
    S3,
    RocksDb,
}

/// Store Element - state container for storage
#[derive(Debug)]
pub struct StoreElement {
    backend: BackendType,
    cache: std::collections::HashMap<BlockId, Block>,
    stats: StoreStats,
}

/// Storage statistics
#[derive(Debug, Clone, Default)]
pub struct StoreStats {
    pub blocks_stored: u64,
    pub blocks_retrieved: u64,
    pub bytes_stored: u64,
    pub bytes_retrieved: u64,
}

impl StoreElement {
    pub fn new(backend: BackendType) -> Self {
        Self {
            backend,
            cache: std::collections::HashMap::new(),
            stats: StoreStats::default(),
        }
    }
    
    pub fn backend(&self) -> BackendType {
        self.backend
    }
    
    pub fn cache_block(&mut self, block: Block) {
        self.stats.blocks_stored += 1;
        self.stats.bytes_stored += block.size() as u64;
        self.cache.insert(block.id().clone(), block);
    }
    
    pub fn get_cached(&self, id: &BlockId) -> Option<&Block> {
        self.cache.get(id)
    }
    
    pub fn stats(&self) -> &StoreStats {
        &self.stats
    }
    
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

impl Element for StoreElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<StoreKey>()
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for StoreKey {
    type Element = StoreElement;
    const FACTORY: fn() -> Self::Element = || StoreElement::new(BackendType::Memory);
}

// ============================================================================
// BlockStore - Fundamental block storage interface
// ============================================================================

/// Fundamental block storage trait
#[async_trait::async_trait]
pub trait BlockStore: Send + Sync {
    async fn put(&self, data: Vec<u8>) -> StoreResult<BlockId>;
    async fn get(&self, id: &BlockId) -> StoreResult<Block>;
    async fn has(&self, id: &BlockId) -> StoreResult<bool>;
    async fn delete(&self, id: &BlockId) -> StoreResult<bool>;
    async fn list(&self) -> StoreResult<Series<BlockId>>;
    fn stats(&self) -> StoreStats;
}

// ============================================================================
// Utility functions
// ============================================================================

/// Compute SHA-256 hash of data
pub fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Verify hash matches data
pub fn verify_hash(data: &[u8], expected_hash: &str) -> bool {
    compute_hash(data) == expected_hash
}

/// Split data into chunks
pub fn chunk_data(data: &[u8], max_size: usize) -> Vec<Vec<u8>> {
    data.chunks(max_size).map(|c| c.to_vec()).collect()
}
