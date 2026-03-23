//! Storage backends for CCEK Store
//!
//! Provides implementations of BlockStore and ObjectStore for various backends:
//! - Memory: In-memory block storage (always available)
//! - IPFS: IPFS-backed storage (requires `ipfs` feature)
//! - S3: S3-compatible object storage (requires `s3` feature)
//! - CouchDB: CouchDB document store (requires `couchdb` feature)

use crate::{Block, BlockId, BlockStore, StoreError, StoreResult, StoreStats, Series};
use async_trait::async_trait;
use std::sync::Arc;

pub mod memory;

#[cfg(feature = "ipfs")]
pub mod ipfs;

#[cfg(feature = "s3")]
pub mod s3;

/// Backend configuration
#[derive(Debug, Clone)]
pub enum BackendConfig {
    /// In-memory configuration
    Memory {
        /// Maximum cache size in bytes
        max_size: usize,
    },
    
    /// IPFS configuration
    #[cfg(feature = "ipfs")]
    Ipfs {
        /// IPFS API URL
        api_url: String,
        /// Gateway URL for retrieval
        gateway_url: String,
        /// Whether to pin content
        pin: bool,
    },
    
    /// S3 configuration
    #[cfg(feature = "s3")]
    S3 {
        /// Endpoint URL
        endpoint: String,
        /// Bucket name
        bucket: String,
        /// Region
        region: String,
        /// Access key
        access_key: String,
        /// Secret key
        secret_key: String,
    },
    
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self::Memory {
            max_size: 100 * 1024 * 1024, // 100MB default
        }
    }
}

/// Factory for creating storage backends
pub struct BackendFactory;

impl BackendFactory {
    /// Create a BlockStore from configuration
    pub fn create_block_store(config: BackendConfig) -> StoreResult<Arc<dyn BlockStore>> {
        match config {
            BackendConfig::Memory { max_size } => {
                Ok(Arc::new(memory::MemoryBlockStore::with_capacity(max_size)))
            }
            
            #[cfg(feature = "ipfs")]
            BackendConfig::Ipfs { api_url, gateway_url, pin } => {
                Ok(Arc::new(ipfs::IpfsBlockStore::new(&api_url, &gateway_url, pin)?))
            }
            
            #[cfg(feature = "s3")]
            BackendConfig::S3 { endpoint, bucket, region, access_key, secret_key } => {
                Ok(Arc::new(s3::S3BlockStore::new(
                    &endpoint, &bucket, &region, &access_key, &secret_key
                )?))
            }
            
        }
    }
    
    /// Create default memory store
    pub fn create_memory_store() -> Arc<dyn BlockStore> {
        Arc::new(memory::MemoryBlockStore::new())
    }
}

/// Composite store that combines multiple backends
///
/// Tries primary first, falls back to secondary on miss
pub struct CompositeBlockStore {
    primary: Arc<dyn BlockStore>,
    secondary: Arc<dyn BlockStore>,
    cache_hits: std::sync::atomic::AtomicU64,
    cache_misses: std::sync::atomic::AtomicU64,
}

impl CompositeBlockStore {
    /// Create a new composite store
    pub fn new(primary: Arc<dyn BlockStore>, secondary: Arc<dyn BlockStore>) -> Self {
        Self {
            primary,
            secondary,
            cache_hits: std::sync::atomic::AtomicU64::new(0),
            cache_misses: std::sync::atomic::AtomicU64::new(0),
        }
    }
    
    /// Get cache statistics
    pub fn cache_stats(&self) -> (u64, u64) {
        (
            self.cache_hits.load(std::sync::atomic::Ordering::Relaxed),
            self.cache_misses.load(std::sync::atomic::Ordering::Relaxed),
        )
    }
}

#[async_trait]
impl BlockStore for CompositeBlockStore {
    async fn put(&self, data: Vec<u8>) -> StoreResult<BlockId> {
        // Write-through: put in both stores
        let id = self.primary.put(data.clone()).await?;
        let _ = self.secondary.put(data).await;
        Ok(id)
    }
    
    async fn get(&self, id: &BlockId) -> StoreResult<Block> {
        // Try primary first
        match self.primary.get(id).await {
            Ok(block) => {
                self.cache_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Ok(block)
            }
            Err(_) => {
                // Fall back to secondary
                let block = self.secondary.get(id).await?;
                self.cache_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Ok(block)
            }
        }
    }
    
    async fn has(&self, id: &BlockId) -> StoreResult<bool> {
        // Check primary first
        if self.primary.has(id).await? {
            return Ok(true);
        }
        // Fall back to secondary
        self.secondary.has(id).await
    }
    
    async fn delete(&self, id: &BlockId) -> StoreResult<bool> {
        // Delete from both
        let primary_deleted = self.primary.delete(id).await?;
        let _ = self.secondary.delete(id).await;
        Ok(primary_deleted)
    }
    
    async fn list(&self) -> StoreResult<Series<BlockId>> {
        // Merge lists from both stores
        self.primary.list().await
    }
    
    fn stats(&self) -> StoreStats {
        let primary_stats = self.primary.stats();
        let secondary_stats = self.secondary.stats();
        
        StoreStats {
            blocks_stored: primary_stats.blocks_stored + secondary_stats.blocks_stored,
            blocks_retrieved: primary_stats.blocks_retrieved + secondary_stats.blocks_retrieved,
            bytes_stored: primary_stats.bytes_stored + secondary_stats.bytes_stored,
            bytes_retrieved: primary_stats.bytes_retrieved + secondary_stats.bytes_retrieved,
        }
    }
}

/// No-op store for testing
pub struct NoopBlockStore;

impl NoopBlockStore {
    /// Create a new noop store
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoopBlockStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BlockStore for NoopBlockStore {
    async fn put(&self, _data: Vec<u8>) -> StoreResult<BlockId> {
        Err(StoreError::NotImplemented)
    }
    
    async fn get(&self, _id: &BlockId) -> StoreResult<Block> {
        Err(StoreError::NotImplemented)
    }
    
    async fn has(&self, _id: &BlockId) -> StoreResult<bool> {
        Ok(false)
    }
    
    async fn delete(&self, _id: &BlockId) -> StoreResult<bool> {
        Ok(false)
    }
    
    async fn list(&self) -> StoreResult<Series<BlockId>> {
        Ok(Vec::new())
    }
    
    fn stats(&self) -> StoreStats {
        StoreStats::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_backend_factory() {
        let store = BackendFactory::create_memory_store();
        // Just verify it creates without panicking
        let _stats = store.stats();
    }
    
    #[test]
    fn test_composite_store_creation() {
        let primary = BackendFactory::create_memory_store();
        let secondary = BackendFactory::create_memory_store();
        
        let composite = CompositeBlockStore::new(primary, secondary);
        let (hits, misses) = composite.cache_stats();
        
        assert_eq!(hits, 0);
        assert_eq!(misses, 0);
    }
}
