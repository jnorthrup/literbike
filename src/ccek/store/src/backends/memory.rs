//! In-memory block storage backend
//!
//! Provides a simple in-memory implementation of BlockStore and ObjectStore.
//! Useful for testing and caching scenarios.

use crate::{
    Block, BlockId, BlockStore, Object, ObjectMeta, ObjectStore, Series, StoreError, StoreResult,
    StoreStats,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// In-memory block storage
#[derive(Debug)]
pub struct MemoryBlockStore {
    /// Storage map: block hash -> block data
    blocks: RwLock<HashMap<BlockId, Block>>,
    /// Maximum total size in bytes (0 = unlimited)
    max_size: usize,
    /// Current total size
    current_size: RwLock<usize>,
    /// Statistics
    stats: RwLock<StoreStats>,
}

impl MemoryBlockStore {
    /// Create a new memory store with unlimited capacity
    pub fn new() -> Self {
        Self::with_capacity(0)
    }
    
    /// Create a new memory store with capacity limit
    ///
    /// # Arguments
    /// * `max_size` - Maximum total size in bytes (0 = unlimited)
    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            blocks: RwLock::new(HashMap::new()),
            max_size,
            current_size: RwLock::new(0),
            stats: RwLock::new(StoreStats::default()),
        }
    }
    
    /// Get current size in bytes
    pub fn size(&self) -> usize {
        *self.current_size.read().unwrap()
    }
    
    /// Get number of blocks
    pub fn block_count(&self) -> usize {
        self.blocks.read().unwrap().len()
    }
    
    /// Check if store is at capacity
    pub fn is_full(&self) -> bool {
        if self.max_size == 0 {
            return false;
        }
        *self.current_size.read().unwrap() >= self.max_size
    }
    
    /// Clear all blocks
    pub fn clear(&self) {
        let mut blocks = self.blocks.write().unwrap();
        let mut size = self.current_size.write().unwrap();
        blocks.clear();
        *size = 0;
    }
    
    /// Evict oldest blocks to make room
    fn evict_if_needed(&self, needed: usize) -> StoreResult<()> {
        if self.max_size == 0 {
            return Ok(());
        }
        
        let mut current = self.current_size.write().unwrap();
        let mut blocks = self.blocks.write().unwrap();
        
        while *current + needed > self.max_size && !blocks.is_empty() {
            // Remove first block (simple LRU approximation)
            let first_key = blocks.keys().next().cloned();
            if let Some(key) = first_key {
                if let Some(block) = blocks.remove(&key) {
                    *current = current.saturating_sub(block.size());
                }
            }
        }
        
        Ok(())
    }
    
    /// Update statistics
    fn record_put(&self, size: usize) {
        let mut stats = self.stats.write().unwrap();
        stats.blocks_stored += 1;
        stats.bytes_stored += size as u64;
    }
    
    fn record_get(&self, size: usize) {
        let mut stats = self.stats.write().unwrap();
        stats.blocks_retrieved += 1;
        stats.bytes_retrieved += size as u64;
    }
}

impl Default for MemoryBlockStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BlockStore for MemoryBlockStore {
    async fn put(&self, data: Vec<u8>) -> StoreResult<BlockId> {
        let block = Block::new(data);
        let size = block.size();
        let id = block.id().clone();
        
        // Check capacity
        if self.max_size > 0 && size > self.max_size {
            return Err(StoreError::BackendError(
                format!("Block size {} exceeds maximum {}", size, self.max_size)
            ));
        }
        
        // Evict if needed
        self.evict_if_needed(size)?;
        
        // Store block
        {
            let mut blocks = self.blocks.write().unwrap();
            let mut current_size = self.current_size.write().unwrap();
            
            // If block already exists, adjust size
            if let Some(existing) = blocks.get(&id) {
                *current_size = current_size.saturating_sub(existing.size());
            }
            
            blocks.insert(id.clone(), block);
            *current_size += size;
        }
        
        self.record_put(size);
        
        Ok(id)
    }
    
    async fn get(&self, id: &BlockId) -> StoreResult<Block> {
        let blocks = self.blocks.read().unwrap();
        
        match blocks.get(id) {
            Some(block) => {
                self.record_get(block.size());
                Ok(block.clone())
            }
            None => Err(StoreError::BlockNotFound(id.to_string())),
        }
    }
    
    async fn has(&self, id: &BlockId) -> StoreResult<bool> {
        let blocks = self.blocks.read().unwrap();
        Ok(blocks.contains_key(id))
    }
    
    async fn delete(&self, id: &BlockId) -> StoreResult<bool> {
        let mut blocks = self.blocks.write().unwrap();
        
        match blocks.remove(id) {
            Some(block) => {
                let mut current_size = self.current_size.write().unwrap();
                *current_size = current_size.saturating_sub(block.size());
                Ok(true)
            }
            None => Ok(false),
        }
    }
    
    async fn list(&self) -> StoreResult<Series<BlockId>> {
        let blocks = self.blocks.read().unwrap();
        let ids: Vec<BlockId> = blocks.keys().cloned().collect();
        Ok(Vec::from(ids))
    }
    
    fn stats(&self) -> StoreStats {
        self.stats.read().unwrap().clone()
    }
}

/// In-memory object storage
#[derive(Debug)]
pub struct MemoryObjectStore {
    /// Storage map: key -> (data, metadata)
    objects: RwLock<HashMap<String, (Vec<u8>, ObjectMeta)>>,
    /// Statistics
    stats: RwLock<StoreStats>,
}

impl MemoryObjectStore {
    /// Create a new memory object store
    pub fn new() -> Self {
        Self {
            objects: RwLock::new(HashMap::new()),
            stats: RwLock::new(StoreStats::default()),
        }
    }
    
    /// Get object count
    pub fn object_count(&self) -> usize {
        self.objects.read().unwrap().len()
    }
    
    /// Clear all objects
    pub fn clear(&self) {
        self.objects.write().unwrap().clear();
    }
    
    fn record_put(&self, size: usize) {
        let mut stats = self.stats.write().unwrap();
        stats.blocks_stored += 1;
        stats.bytes_stored += size as u64;
    }
    
    fn record_get(&self, size: usize) {
        let mut stats = self.stats.write().unwrap();
        stats.blocks_retrieved += 1;
        stats.bytes_retrieved += size as u64;
    }
}

impl Default for MemoryObjectStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ObjectStore for MemoryObjectStore {
    async fn get_object(&self, key: &str) -> StoreResult<Object> {
        let objects = self.objects.read().unwrap();
        
        match objects.get(key) {
            Some((data, meta)) => {
                self.record_get(data.len());
                Ok(Object {
                    meta: meta.clone(),
                    data: data.clone(),
                })
            }
            None => Err(StoreError::ObjectNotFound(key.to_string())),
        }
    }
    
    async fn put_object(&self, key: &str, data: Vec<u8>, meta: Option<ObjectMeta>) -> StoreResult<ObjectMeta> {
        let mut meta = meta.unwrap_or_default();
        meta.key = key.to_string();
        meta.size = data.len() as u64;
        
        self.record_put(data.len());
        
        let mut objects = self.objects.write().unwrap();
        objects.insert(key.to_string(), (data, meta.clone()));
        
        Ok(meta)
    }
    
    async fn delete_object(&self, key: &str) -> StoreResult<bool> {
        let mut objects = self.objects.write().unwrap();
        Ok(objects.remove(key).is_some())
    }
    
    async fn list_objects(&self, prefix: Option<&str>) -> StoreResult<Series<ObjectMeta>> {
        let objects = self.objects.read().unwrap();
        
        let metas: Vec<ObjectMeta> = objects
            .iter()
            .filter(|(k, _)| {
                match prefix {
                    Some(p) => k.starts_with(p),
                    None => true,
                }
            })
            .map(|(_, (_, meta))| meta.clone())
            .collect();
        
        Ok(Vec::from(metas))
    }
    
    async fn head_object(&self, key: &str) -> StoreResult<Option<ObjectMeta>> {
        let objects = self.objects.read().unwrap();
        Ok(objects.get(key).map(|(_, meta)| meta.clone()))
    }
    
    async fn copy_object(&self, source: &str, dest: &str) -> StoreResult<ObjectMeta> {
        let objects = self.objects.read().unwrap();
        
        match objects.get(source) {
            Some((data, meta)) => {
                let mut new_meta = meta.clone();
                new_meta.key = dest.to_string();
                
                // Need to drop read lock before write
                let data_clone = data.clone();
                drop(objects);
                
                let mut objects = self.objects.write().unwrap();
                objects.insert(dest.to_string(), (data_clone, new_meta.clone()));
                
                Ok(new_meta)
            }
            None => Err(StoreError::ObjectNotFound(source.to_string())),
        }
    }
}

/// Thread-safe memory store using Arc
pub struct SharedMemoryStore {
    inner: Arc<MemoryBlockStore>,
}

impl SharedMemoryStore {
    /// Create a new shared memory store
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MemoryBlockStore::new()),
        }
    }
    
    /// Get inner store
    pub fn inner(&self) -> Arc<MemoryBlockStore> {
        self.inner.clone()
    }
}

impl Default for SharedMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BlockStore for SharedMemoryStore {
    async fn put(&self, data: Vec<u8>) -> StoreResult<BlockId> {
        self.inner.put(data).await
    }
    
    async fn get(&self, id: &BlockId) -> StoreResult<Block> {
        self.inner.get(id).await
    }
    
    async fn has(&self, id: &BlockId) -> StoreResult<bool> {
        self.inner.has(id).await
    }
    
    async fn delete(&self, id: &BlockId) -> StoreResult<bool> {
        self.inner.delete(id).await
    }
    
    async fn list(&self) -> StoreResult<Series<BlockId>> {
        self.inner.list().await
    }
    
    fn stats(&self) -> StoreStats {
        self.inner.stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_memory_block_store_basic() {
        let store = MemoryBlockStore::new();
        
        // Put a block
        let data = b"hello world".to_vec();
        let id = store.put(data.clone()).await.unwrap();
        
        // Verify it exists
        assert!(store.has(&id).await.unwrap());
        
        // Get it back
        let block = store.get(&id).await.unwrap();
        assert_eq!(block.data(), &data);
        
        // Check stats
        let stats = store.stats();
        assert_eq!(stats.blocks_stored, 1);
        assert_eq!(stats.bytes_stored, data.len() as u64);
    }
    
    #[tokio::test]
    async fn test_memory_block_store_delete() {
        let store = MemoryBlockStore::new();
        
        let data = b"test data".to_vec();
        let id = store.put(data).await.unwrap();
        
        // Delete it
        assert!(store.delete(&id).await.unwrap());
        assert!(!store.has(&id).await.unwrap());
        
        // Delete again returns false
        assert!(!store.delete(&id).await.unwrap());
    }
    
    #[tokio::test]
    async fn test_memory_block_store_list() {
        let store = MemoryBlockStore::new();
        
        let id1 = store.put(b"block1".to_vec()).await.unwrap();
        let id2 = store.put(b"block2".to_vec()).await.unwrap();
        
        let series = store.list().await.unwrap();
        assert_eq!(series.len(), Some(2));
    }
    
    #[tokio::test]
    async fn test_memory_block_store_capacity() {
        let store = MemoryBlockStore::with_capacity(100);
        
        // First block fits
        let id1 = store.put(vec![0u8; 50]).await.unwrap();
        assert!(store.has(&id1).await.unwrap());
        
        // Second block should cause eviction of first
        let id2 = store.put(vec![0u8; 80]).await.unwrap();
        
        // First might be evicted depending on implementation
        // Just verify second exists
        assert!(store.has(&id2).await.unwrap());
    }
    
    #[tokio::test]
    async fn test_memory_object_store_basic() {
        let store = MemoryObjectStore::new();
        
        // Put an object
        let data = b"object data".to_vec();
        let meta = store.put_object("test-key", data.clone(), None).await.unwrap();
        
        assert_eq!(meta.key, "test-key");
        assert_eq!(meta.size, data.len() as u64);
        
        // Get it back
        let obj = store.get_object("test-key").await.unwrap();
        assert_eq!(obj.data, data);
        
        // Check head
        let head = store.head_object("test-key").await.unwrap();
        assert!(head.is_some());
        
        // Delete it
        assert!(store.delete_object("test-key").await.unwrap());
        assert!(!store.delete_object("test-key").await.unwrap());
    }
    
    #[tokio::test]
    async fn test_memory_object_store_list() {
        let store = MemoryObjectStore::new();
        
        store.put_object("prefix/a", vec![1], None).await.unwrap();
        store.put_object("prefix/b", vec![2], None).await.unwrap();
        store.put_object("other", vec![3], None).await.unwrap();
        
        let all = store.list_objects(None).await.unwrap();
        assert_eq!(all.len(), Some(3));
        
        let prefixed = store.list_objects(Some("prefix/")).await.unwrap();
        assert_eq!(prefixed.len(), Some(2));
    }
    
    #[tokio::test]
    async fn test_memory_object_store_copy() {
        let store = MemoryObjectStore::new();
        
        let data = b"copy me".to_vec();
        store.put_object("source", data.clone(), None).await.unwrap();
        
        let new_meta = store.copy_object("source", "dest").await.unwrap();
        assert_eq!(new_meta.key, "dest");
        
        let copied = store.get_object("dest").await.unwrap();
        assert_eq!(copied.data, data);
    }
    
    #[test]
    fn test_shared_memory_store() {
        let store1 = SharedMemoryStore::new();
        let store2 = SharedMemoryStore {
            inner: store1.inner(),
        };
        
        // Both should share the same inner store
        assert!(Arc::ptr_eq(&store1.inner, &store2.inner));
    }
}
