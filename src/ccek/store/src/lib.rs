//! CCEK Store - Storage layer with TrikeShed Series/Cursor abstractions
//!
//! Synthesizes:
//! - CCEK patterns (Key/Element/Context from ccek-core)
//! - TrikeShed Series/Cursor abstractions (lazy sequences, joins, combines)
//! - Storage backends as CCEK Elements
//!
//! # Core Types
//!
//! - `Series<T>` - lazy sequence abstraction
//! - `Join<A, B>` - pair with named fields (a, b)
//! - `Cursor` - columnar data abstraction using Series
//! - `StoreKey` / `StoreElement` - CCEK Key/Element for storage operations
//! - `BlockStore` - fundamental block storage interface
//! - `ObjectStore` - S3/GCS/Aliyun-compatible object storage

pub mod backends;
pub mod json;

use ccek_core::{Context, Element, Key};
use sha2::{Sha256, Digest};
use std::any::{Any, TypeId};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
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
// Series - Lazy Sequence Abstraction (TrikeShed pattern)
// ============================================================================

/// A lazy sequence that yields values on demand
///
/// Inspired by TrikeShed's Series abstraction, this provides
/// zero-allocation iteration over potentially infinite sequences.
pub struct Series<T> {
    /// Current position in the series
    position: usize,
    /// Total length (None for infinite series)
    length: Option<usize>,
    /// Generator function
    generator: Arc<dyn Fn(usize) -> T + Send + Sync>,
}

impl<T: Clone + 'static> Series<T> {
    /// Create a new series from a generator function
    pub fn new<F>(length: Option<usize>, generator: F) -> Self
    where
        F: Fn(usize) -> T + Send + Sync + 'static,
    {
        Self {
            position: 0,
            length,
            generator: Arc::new(generator),
        }
    }
    
    /// Create a series from a Vec
    pub fn from_vec(data: Vec<T>) -> Self
    where
        T: Send + Sync + 'static,
    {
        let data = Arc::new(data);
        let data_clone = data.clone();
        Self::new(Some(data.len()), move |i| data_clone[i].clone())
    }
    
    /// Get length if known
    pub fn len(&self) -> Option<usize> {
        self.length
    }
    
    /// Check if series is empty
    pub fn is_empty(&self) -> bool {
        self.length.map(|l| l == 0).unwrap_or(false)
    }
    
    /// Get value at index
    pub fn get(&self, index: usize) -> Option<T> {
        if let Some(len) = self.length {
            if index >= len {
                return None;
            }
        }
        Some((self.generator)(index))
    }
    
    /// Map over the series (lazy)
    pub fn map<U, F>(self, func: F) -> Series<U>
    where
        F: Fn(T) -> U + Send + Sync + 'static,
        U: Clone,
    {
        let generator = self.generator;
        Series::new(self.length, move |i| func(generator(i)))
    }
    
    /// Filter the series (lazy, becomes finite)
    pub fn filter<P>(self, predicate: P) -> Series<Option<T>>
    where
        P: Fn(&T) -> bool + Send + Sync + 'static,
    {
        let generator = self.generator;
        Series::new(self.length, move |i| {
            let value = generator(i);
            if predicate(&value) {
                Some(value)
            } else {
                None
            }
        })
    }
    
    /// Take first n elements
    pub fn take(self, n: usize) -> Series<T> {
        let new_len = self.length.map(|l| l.min(n)).unwrap_or(n);
        Series {
            position: self.position,
            length: Some(new_len),
            generator: self.generator,
        }
    }
    
    /// Skip first n elements
    pub fn skip(self, n: usize) -> Series<T> {
        let new_len = self.length.map(|l| l.saturating_sub(n));
        Series {
            position: self.position + n,
            length: new_len,
            generator: self.generator,
        }
    }
    
    /// Combine two series into a series of Joins
    pub fn zip<U>(self, other: Series<U>) -> Series<Join<T, U>>
    where
        U: Clone + Send + Sync + 'static,
    {
        let len = match (self.length, other.length) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        
        let gen_a = self.generator;
        let gen_b = other.generator;
        
        Series::new(len, move |i| {
            Join::new(gen_a(i), gen_b(i))
        })
    }
    
    /// Convert to a concrete vector (forces evaluation)
    pub fn collect(self) -> Vec<T> {
        let len = self.length.unwrap_or(0);
        (0..len).filter_map(|i| self.get(i)).collect()
    }
}

impl<T: Clone> Iterator for Series<T> {
    type Item = T;
    
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(len) = self.length {
            if self.position >= len {
                return None;
            }
        }
        let value = self.get(self.position)?;
        self.position += 1;
        Some(value)
    }
    
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.length {
            Some(len) => (len.saturating_sub(self.position), Some(len.saturating_sub(self.position))),
            None => (0, None),
        }
    }
}

impl<T: Clone + fmt::Debug> fmt::Debug for Series<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Series")
            .field("position", &self.position)
            .field("length", &self.length)
            .field("current", &self.get(self.position))
            .finish()
    }
}

impl<T: Clone> Clone for Series<T> {
    fn clone(&self) -> Self {
        Self {
            position: self.position,
            length: self.length,
            generator: self.generator.clone(),
        }
    }
}

// ============================================================================
// Join - Pair with named fields (TrikeShed pattern)
// ============================================================================

/// Categorical composition of two values with named fields
///
/// The Join type is TrikeShed's fundamental abstraction for
/// combining values while preserving their individual identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Join<A, B> {
    /// First component
    pub a: A,
    /// Second component
    pub b: B,
}

impl<A, B> Join<A, B> {
    /// Create a new Join
    pub fn new(a: A, b: B) -> Self {
        Self { a, b }
    }
    
    /// Map over the first component
    pub fn map_a<F, C>(self, func: F) -> Join<C, B>
    where
        F: FnOnce(A) -> C,
    {
        Join::new(func(self.a), self.b)
    }
    
    /// Map over the second component
    pub fn map_b<F, C>(self, func: F) -> Join<A, C>
    where
        F: FnOnce(B) -> C,
    {
        Join::new(self.a, func(self.b))
    }
    
    /// Map over both components
    pub fn map_both<F, G, C, D>(self, f: F, g: G) -> Join<C, D>
    where
        F: FnOnce(A) -> C,
        G: FnOnce(B) -> D,
    {
        Join::new(f(self.a), g(self.b))
    }
    
    /// Swap components
    pub fn swap(self) -> Join<B, A> {
        Join::new(self.b, self.a)
    }
    
    /// Unzip into a tuple
    pub fn unzip(self) -> (A, B) {
        (self.a, self.b)
    }
    
    /// Convert from tuple
    pub fn from_tuple((a, b): (A, B)) -> Self {
        Self::new(a, b)
    }
}

impl<A: Default, B: Default> Default for Join<A, B> {
    fn default() -> Self {
        Self::new(A::default(), B::default())
    }
}

/// Type alias for indexed sequences (Join<usize, Fn>)
pub type Indexed<T> = Join<usize, Arc<dyn Fn(usize) -> T + Send + Sync>>;

/// Create an indexed sequence
pub fn indexed<T, F>(size: usize, accessor: F) -> Indexed<T>
where
    F: Fn(usize) -> T + Send + Sync + 'static,
{
    Join::new(size, Arc::new(accessor))
}

// ============================================================================
// Cursor - Columnar Data Abstraction
// ============================================================================

/// Cursor for navigating columnar data using Series
///
/// A Cursor represents a position within a dataset that can
/// be navigated forward, backward, or to arbitrary positions.
pub struct Cursor<T> {
    /// The underlying series data
    data: Series<T>,
    /// Current position
    position: usize,
    /// Marked positions for navigation
    marks: Vec<usize>,
}

impl<T: Clone + 'static> Cursor<T> {
    /// Create a new cursor from a series
    pub fn new(data: Series<T>) -> Self {
        Self {
            data,
            position: 0,
            marks: Vec::new(),
        }
    }
    
    /// Create a cursor from a vector
    pub fn from_vec(data: Vec<T>) -> Self {
        Self::new(Series::from_vec(data))
    }
    
    /// Get current position
    pub fn position(&self) -> usize {
        self.position
    }
    
    /// Get current value
    pub fn current(&self) -> Option<T> {
        self.data.get(self.position)
    }
    
    /// Move to next position
    pub fn next(&mut self) -> Option<T> {
        self.position += 1;
        self.current()
    }
    
    /// Move to previous position
    pub fn prev(&mut self) -> Option<T> {
        if self.position > 0 {
            self.position -= 1;
            self.current()
        } else {
            None
        }
    }
    
    /// Move to specific position
    pub fn seek(&mut self, position: usize) -> Option<T> {
        self.position = position;
        self.current()
    }
    
    /// Move to first position
    pub fn first(&mut self) -> Option<T> {
        self.position = 0;
        self.current()
    }
    
    /// Move to last position
    pub fn last(&mut self) -> Option<T> {
        if let Some(len) = self.data.len() {
            self.position = len.saturating_sub(1);
            self.current()
        } else {
            None
        }
    }
    
    /// Check if at end
    pub fn is_end(&self) -> bool {
        match self.data.len() {
            Some(len) => self.position >= len,
            None => false,
        }
    }
    
    /// Check if at beginning
    pub fn is_beginning(&self) -> bool {
        self.position == 0
    }
    
    /// Mark current position
    pub fn mark(&mut self) -> usize {
        self.marks.push(self.position);
        self.position
    }
    
    /// Return to last marked position
    pub fn return_to_mark(&mut self) -> Option<T> {
        if let Some(pos) = self.marks.pop() {
            self.position = pos;
            self.current()
        } else {
            None
        }
    }
    
    /// Get remaining elements as series
    pub fn remaining(&self) -> Series<T> {
        let pos = self.position;
        let data = self.data.clone();
        Series::new(
            self.data.len().map(|l| l.saturating_sub(pos)),
            move |i| data.get(pos + i).unwrap(),
        )
    }
    
    /// Map cursor to new type
    pub fn map<U, F>(self, func: F) -> Cursor<U>
    where
        F: Fn(T) -> U + Send + Sync + Clone + 'static,
        U: Clone,
    {
        let func_clone = func.clone();
        Cursor::new(self.data.map(move |t| func_clone(t)))
    }
    
    /// Get underlying series
    pub fn series(&self) -> Series<T> {
        self.data.clone()
    }
    
    /// Collect remaining elements
    pub fn collect_remaining(&self) -> Vec<T> {
        self.remaining().collect()
    }
}

impl<T: Clone + fmt::Debug> fmt::Debug for Cursor<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cursor")
            .field("position", &self.position)
            .field("current", &self.current())
            .field("marks", &self.marks)
            .finish()
    }
}

impl<T: Clone> Clone for Cursor<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            position: self.position,
            marks: self.marks.clone(),
        }
    }
}

// ============================================================================
// BlockId - Content-addressed identifier
// ============================================================================

/// A content-addressed block identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId {
    /// The hash of the block content
    hash: String,
}

impl BlockId {
    /// Create a BlockId from a hash string
    pub fn new(hash: impl Into<String>) -> Self {
        Self { hash: hash.into() }
    }
    
    /// Create a BlockId from raw bytes (computes hash)
    pub fn from_bytes(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hex::encode(hasher.finalize());
        Self::new(hash)
    }
    
    /// Get the hash string
    pub fn hash(&self) -> &str {
        &self.hash
    }
    
    /// Convert to bytes representation
    pub fn to_bytes(&self) -> Vec<u8> {
        hex::decode(&self.hash).unwrap_or_default()
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.hash)
    }
}

impl From<&str> for BlockId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for BlockId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

// ============================================================================
// Block - Storage unit
// ============================================================================

/// A block of data with content-addressed identifier
#[derive(Debug, Clone)]
pub struct Block {
    /// Block identifier (content hash)
    id: BlockId,
    /// Block data
    data: Vec<u8>,
    /// Optional metadata
    metadata: Option<serde_json::Value>,
}

impl Block {
    /// Create a new block from data
    pub fn new(data: Vec<u8>) -> Self {
        let id = BlockId::from_bytes(&data);
        Self {
            id,
            data,
            metadata: None,
        }
    }
    
    /// Create a block with metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
    
    /// Get block ID
    pub fn id(&self) -> &BlockId {
        &self.id
    }
    
    /// Get block data
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    
    /// Get block size
    pub fn size(&self) -> usize {
        self.data.len()
    }
    
    /// Get metadata
    pub fn metadata(&self) -> Option<&serde_json::Value> {
        self.metadata.as_ref()
    }
    
    /// Verify block integrity
    pub fn verify(&self) -> bool {
        let computed = BlockId::from_bytes(&self.data);
        computed == self.id
    }
}

// ============================================================================
// StoreKey - CCEK Key for storage operations
// ============================================================================

/// Passive Key type for storage operations
///
/// StoreKey provides:
/// - FACTORY for creating StoreElement
/// - Constants for storage protocols
/// - Configuration accessors
#[derive(Debug, Clone, Copy, Default)]
pub struct StoreKey {
    _phantom: PhantomData<()>,
}

/// Storage backend types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendType {
    Memory,
    Ipfs,
    S3,
    Couchdb,
}

impl StoreKey {
    /// Get the default backend type
    pub const fn default_backend() -> BackendType {
        BackendType::Memory
    }
    
    /// Get block size limit (4MB)
    pub const fn block_size_limit() -> usize {
        4 * 1024 * 1024
    }
}

impl Key for StoreKey {
    type Element = StoreElement;
    const FACTORY: fn() -> Self::Element = || StoreElement::new(BackendType::Memory);
}

// ============================================================================
// StoreElement - CCEK Element for storage operations
// ============================================================================

/// Operational Element for storage
///
/// StoreElement is stored in Context and provides:
/// - Block storage operations
/// - Backend management
/// - Content-addressed storage
#[derive(Debug)]
pub struct StoreElement {
    /// Backend type
    backend: BackendType,
    /// Block cache (in-memory)
    cache: std::collections::HashMap<BlockId, Block>,
    /// Statistics
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
    /// Create a new store element
    pub fn new(backend: BackendType) -> Self {
        Self {
            backend,
            cache: std::collections::HashMap::new(),
            stats: StoreStats::default(),
        }
    }
    
    /// Get backend type
    pub fn backend(&self) -> BackendType {
        self.backend
    }
    
    /// Store a block in cache
    pub fn cache_block(&mut self, block: Block) {
        self.stats.blocks_stored += 1;
        self.stats.bytes_stored += block.size() as u64;
        self.cache.insert(block.id().clone(), block);
    }
    
    /// Get block from cache
    pub fn get_cached(&self, id: &BlockId) -> Option<&Block> {
        self.cache.get(id)
    }
    
    /// Get statistics
    pub fn stats(&self) -> &StoreStats {
        &self.stats
    }
    
    /// Clear cache
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

// ============================================================================
// BlockStore - Fundamental block storage interface
// ============================================================================

/// Fundamental block storage trait
///
/// BlockStore provides content-addressed storage of blocks.
/// All operations are content-addressed using SHA-256 hashes.
#[async_trait::async_trait]
pub trait BlockStore: Send + Sync {
    /// Store a block, returning its content address
    async fn put(&self, data: Vec<u8>) -> StoreResult<BlockId>;
    
    /// Retrieve a block by its content address
    async fn get(&self, id: &BlockId) -> StoreResult<Block>;
    
    /// Check if a block exists
    async fn has(&self, id: &BlockId) -> StoreResult<bool>;
    
    /// Delete a block
    async fn delete(&self, id: &BlockId) -> StoreResult<bool>;
    
    /// List all blocks (may be expensive)
    async fn list(&self) -> StoreResult<Series<BlockId>>;
    
    /// Get store statistics
    fn stats(&self) -> StoreStats;
}

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
///
/// ObjectStore provides key-based object storage compatible
/// with S3-style APIs.
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
// Storage Context Extensions
// ============================================================================

/// Extension trait for Context to add storage operations
pub trait StorageContextExt {
    /// Add a store element to context
    fn with_store(self, backend: BackendType) -> Self;
    
    /// Get store element from context
    fn get_store(&self) -> Option<&StoreElement>;
    
    /// Check if context has store
    fn has_store(&self) -> bool;
}

impl StorageContextExt for Context {
    fn with_store(self, backend: BackendType) -> Self {
        let element = StoreElement::new(backend);
        self.plus(element)
    }
    
    fn get_store(&self) -> Option<&StoreElement> {
        self.get::<StoreKey>()
            .and_then(|e| e.as_any().downcast_ref::<StoreElement>())
    }
    
    fn has_store(&self) -> bool {
        self.contains::<StoreKey>()
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Compute SHA-256 hash of data
pub fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Verify data against hash
pub fn verify_hash(data: &[u8], expected_hash: &str) -> bool {
    compute_hash(data) == expected_hash
}

/// Chunk data into blocks of maximum size
pub fn chunk_data(data: &[u8], max_size: usize) -> Vec<Vec<u8>> {
    data.chunks(max_size)
        .map(|chunk| chunk.to_vec())
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_series_basic() {
        let series = Series::from_vec(vec![1, 2, 3, 4, 5]);
        assert_eq!(series.len(), Some(5));
        assert_eq!(series.get(0), Some(1));
        assert_eq!(series.get(4), Some(5));
        assert_eq!(series.get(5), None);
    }
    
    #[test]
    fn test_series_map() {
        let series = Series::from_vec(vec![1, 2, 3]);
        let doubled = series.map(|x| x * 2);
        assert_eq!(doubled.get(0), Some(2));
        assert_eq!(doubled.get(1), Some(4));
        assert_eq!(doubled.get(2), Some(6));
    }
    
    #[test]
    fn test_series_zip() {
        let a = Series::from_vec(vec![1, 2, 3]);
        let b = Series::from_vec(vec!["a", "b", "c"]);
        let zipped = a.zip(b);
        
        let first = zipped.get(0).unwrap();
        assert_eq!(first.a, 1);
        assert_eq!(first.b, "a");
    }
    
    #[test]
    fn test_join_operations() {
        let j = Join::new(42, "hello");
        assert_eq!(j.a, 42);
        assert_eq!(j.b, "hello");
        
        let swapped = j.swap();
        assert_eq!(swapped.a, "hello");
        assert_eq!(swapped.b, 42);
        
        let mapped = j.map_a(|x| x * 2);
        assert_eq!(mapped.a, 84);
        assert_eq!(mapped.b, "hello");
    }
    
    #[test]
    fn test_cursor_navigation() {
        let mut cursor = Cursor::from_vec(vec![10, 20, 30, 40, 50]);
        
        assert_eq!(cursor.current(), Some(10));
        assert_eq!(cursor.next(), Some(20));
        assert_eq!(cursor.next(), Some(30));
        assert_eq!(cursor.prev(), Some(20));
        
        cursor.seek(4);
        assert_eq!(cursor.current(), Some(50));
        assert!(cursor.is_end());
    }
    
    #[test]
    fn test_block_id() {
        let data = b"hello world";
        let id = BlockId::from_bytes(data);
        
        assert_eq!(id.hash().len(), 64); // SHA-256 hex = 64 chars
        assert!(verify_hash(data, id.hash()));
    }
    
    #[test]
    fn test_block_creation() {
        let data = b"test data".to_vec();
        let block = Block::new(data.clone());
        
        assert_eq!(block.data(), &data);
        assert!(block.verify());
    }
    
    #[test]
    fn test_indexed() {
        let data = vec![10, 20, 30, 40, 50];
        let idx = indexed(data.len(), |i| data[i]);
        
        assert_eq!(idx.a, 5);
        assert_eq!((idx.b)(0), 10);
        assert_eq!((idx.b)(4), 50);
    }
    
    #[test]
    fn test_storage_context() {
        let ctx = Context::new()
            .with_store(BackendType::Memory);
        
        assert!(ctx.has_store());
        
        let store = ctx.get_store().unwrap();
        assert_eq!(store.backend(), BackendType::Memory);
    }
    
    #[test]
    fn test_series_iterator() {
        let series = Series::from_vec(vec![1, 2, 3]);
        let collected: Vec<i32> = series.collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }
    
    #[test]
    fn test_series_take_skip() {
        let series = Series::from_vec(vec![1, 2, 3, 4, 5]);
        
        let taken = series.clone().take(3);
        assert_eq!(taken.len(), Some(3));
        
        let skipped = series.clone().skip(2);
        assert_eq!(skipped.get(0), Some(3));
        assert_eq!(skipped.len(), Some(3));
    }
    
    #[test]
    fn test_cursor_marks() {
        let mut cursor = Cursor::from_vec(vec![10, 20, 30, 40, 50]);
        
        cursor.seek(2);
        cursor.mark();
        cursor.seek(4);
        
        let returned = cursor.return_to_mark();
        assert_eq!(returned, Some(30));
        assert_eq!(cursor.position(), 2);
    }
}
