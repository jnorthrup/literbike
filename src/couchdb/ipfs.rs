use crate::couchdb::{
    types::{AttachmentInfo, IpfsCid, KvEntry},
    error::{CouchError, CouchResult},
};
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri, request::Add};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::io::Cursor;
use chrono::Utc;
use futures::TryStreamExt;
use log::{info, warn, debug};
use tokio::sync::Mutex;

/// IPFS integration for distributed storage of attachments and documents
pub struct IpfsManager {
    client: IpfsClient,
    cache: Arc<RwLock<HashMap<String, IpfsCid>>>,
    config: IpfsConfig,
}

/// IPFS configuration
#[derive(Debug, Clone)]
pub struct IpfsConfig {
    pub api_url: String,
    pub gateway_url: String,
    pub pin_content: bool,
    pub cache_enabled: bool,
    pub timeout_seconds: u64,
}

impl Default for IpfsConfig {
    fn default() -> Self {
        Self {
            api_url: "http://127.0.0.1:5001".to_string(),
            gateway_url: "http://127.0.0.1:8080".to_string(),
            pin_content: true,
            cache_enabled: true,
            timeout_seconds: 30,
        }
    }
}

impl IpfsManager {
    /// Create a new IPFS manager
    pub fn new(config: IpfsConfig) -> CouchResult<Self> {
        let client = IpfsClient::from_str(&config.api_url)
            .map_err(|e| CouchError::internal_server_error(&format!("Failed to create IPFS client: {}", e)))?;
        
        Ok(Self {
            client,
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        })
    }
    
    /// Store data in IPFS
    pub async fn store_data(&self, data: &[u8], content_type: &str) -> CouchResult<IpfsCid> {
        debug!("Storing {} bytes to IPFS", data.len());

        let add_request = Add {
            trickle: Some(false),
            only_hash: Some(false),
            wrap_with_directory: Some(false),
            chunker: None,
            pin: Some(self.config.pin_content),
            raw_leaves: None,
            cid_version: Some(1),
            hash: Some("sha2-256"),
            inline: None,
            inline_limit: None,
            to_files: None,
        };

        // Use owned Vec<u8> in Cursor to satisfy 'static + Read bounds
        let cursor = Cursor::new(data.to_vec());
        let response = self.client
            .add_with_options(cursor, add_request)
            .await
            .map_err(|e| CouchError::internal_server_error(&format!("IPFS store failed: {}", e)))?;

        let ipfs_cid = IpfsCid {
            cid: response.hash.clone(),
            size: response.size.parse().unwrap_or(0),
            content_type: content_type.to_string(),
        };

        // Cache the result
        if self.config.cache_enabled {
            let mut cache = self.cache.write().unwrap();
            cache.insert(response.hash.clone(), ipfs_cid.clone());
        }

        info!("Stored data to IPFS: {}", response.hash);
        Ok(ipfs_cid)
    }
    
    /// Retrieve data from IPFS
    pub async fn get_data(&self, cid: &str) -> CouchResult<Vec<u8>> {
        debug!("Retrieving data from IPFS: {}", cid);
        
        let response = self.client
            .cat(cid)
            .map_ok(|chunk| chunk.to_vec())
            .try_concat()
            .await
            .map_err(|e| CouchError::not_found(&format!("IPFS get failed: {}", e)))?;
        
        debug!("Retrieved {} bytes from IPFS: {}", response.len(), cid);
        Ok(response)
    }
    
    /// Pin content in IPFS
    pub async fn pin_content(&self, cid: &str) -> CouchResult<()> {
        debug!("Pinning content in IPFS: {}", cid);

        self.client
            .pin_add(cid, true)
            .await
            .map_err(|e| CouchError::internal_server_error(&format!("IPFS pin failed: {}", e)))?;

        info!("Pinned content: {}", cid);
        Ok(())
    }
    
    /// Unpin content from IPFS
    pub async fn unpin_content(&self, cid: &str) -> CouchResult<()> {
        debug!("Unpinning content from IPFS: {}", cid);

        self.client
            .pin_rm(cid, true)
            .await
            .map_err(|e| CouchError::internal_server_error(&format!("IPFS unpin failed: {}", e)))?;

        info!("Unpinned content: {}", cid);
        Ok(())
    }
    
    /// Store attachment in IPFS
    pub async fn store_attachment(&self, data: &[u8], attachment_info: &AttachmentInfo) -> CouchResult<String> {
        let ipfs_cid = self.store_data(data, &attachment_info.content_type).await?;
        
        // Update cache with attachment metadata
        if self.config.cache_enabled {
            let mut cache = self.cache.write().unwrap();
            cache.insert(ipfs_cid.cid.clone(), ipfs_cid.clone());
        }
        
        Ok(ipfs_cid.cid)
    }
    
    /// Retrieve attachment from IPFS
    pub async fn get_attachment(&self, cid: &str) -> CouchResult<(Vec<u8>, IpfsCid)> {
        // Check cache first
        if self.config.cache_enabled {
            let cache = self.cache.read().unwrap();
            if let Some(cached_cid) = cache.get(cid) {
                let data = self.get_data(cid).await?;
                return Ok((data, cached_cid.clone()));
            }
        }
        
        // Retrieve from IPFS
        let data = self.get_data(cid).await?;
        
        // Create basic CID info (content type unknown)
        let ipfs_cid = IpfsCid {
            cid: cid.to_string(),
            size: data.len() as u64,
            content_type: "application/octet-stream".to_string(),
        };
        
        Ok((data, ipfs_cid))
    }
    
    /// Get IPFS node information
    pub async fn get_node_info(&self) -> CouchResult<serde_json::Value> {
        let version = self.client
            .version()
            .await
            .map_err(|e| CouchError::internal_server_error(&format!("Failed to get IPFS version: {}", e)))?;
        
        let id = self.client
            .id(None)
            .await
            .map_err(|e| CouchError::internal_server_error(&format!("Failed to get IPFS ID: {}", e)))?;
        
        Ok(serde_json::json!({
            "version": version.version,
            "commit": version.commit,
            "repo": version.repo,
            "system": version.system,
            "golang": version.golang,
            "id": id.id,
            "public_key": id.public_key,
            "addresses": id.addresses,
            "agent_version": id.agent_version,
            "protocol_version": id.protocol_version
        }))
    }
    
    /// List pinned content
    pub async fn list_pinned(&self) -> CouchResult<Vec<String>> {
        let pins = self.client
            .pin_ls(None, None)
            .await
            .map_err(|e| CouchError::internal_server_error(&format!("Failed to list pins: {}", e)))?;
        
        Ok(pins.keys.into_iter().map(|(cid, _)| cid).collect())
    }
    
    /// Get content statistics
    pub async fn get_stats(&self) -> CouchResult<serde_json::Value> {
        let repo_stats = self.client
            .stats_repo()
            .await
            .map_err(|e| CouchError::internal_server_error(&format!("Failed to get repo stats: {}", e)))?;

        let bitswap_stats = self.client
            .stats_bitswap()
            .await
            .map_err(|e| CouchError::internal_server_error(&format!("Failed to get bitswap stats: {}", e)))?;

        Ok(serde_json::json!({
            "repo": {
                "repo_size": repo_stats.repo_size,
                "num_objects": repo_stats.num_objects,
                "repo_path": repo_stats.repo_path,
                "version": repo_stats.version
            },
            "bitswap": {
                "blocks_received": bitswap_stats.blocks_received,
                "blocks_sent": bitswap_stats.blocks_sent,
                "data_received": bitswap_stats.data_received,
                "data_sent": bitswap_stats.data_sent,
                "dup_blks_received": bitswap_stats.dup_blks_received,
                "dup_data_received": bitswap_stats.dup_data_received,
                "peers": bitswap_stats.peers,
                "provide_buf_len": bitswap_stats.provide_buf_len,
                "wantlist": bitswap_stats.wantlist
            },
            "cache_size": self.cache.read().unwrap().len()
        }))
    }
    
    /// Garbage collect unpinned content
    /// Note: repo_gc is not yet implemented in ipfs-api-backend-hyper 0.6
    pub async fn garbage_collect(&self) -> CouchResult<Vec<String>> {
        warn!("IPFS garbage collection is not supported by the current ipfs-api-backend-hyper version");
        // Return empty list as GC is not available
        Ok(vec![])
    }
    
    /// Clear local cache
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
        debug!("Cleared IPFS cache");
    }
    
    /// Get cache statistics
    pub fn get_cache_stats(&self) -> HashMap<String, serde_json::Value> {
        let cache = self.cache.read().unwrap();
        let mut stats = HashMap::new();
        
        stats.insert("size".to_string(), serde_json::Value::Number(cache.len().into()));
        stats.insert("enabled".to_string(), serde_json::Value::Bool(self.config.cache_enabled));
        
        let total_size: u64 = cache.values().map(|cid| cid.size).sum();
        stats.insert("total_bytes".to_string(), serde_json::Value::Number(total_size.into()));
        
        stats
    }
}

/// Key-Value store with IPFS backing for attachments
pub struct IpfsKvStore {
    ipfs_manager: Arc<IpfsManager>,
    local_cache: Arc<Mutex<HashMap<String, KvEntry>>>,
    config: KvStoreConfig,
}

/// Key-Value store configuration
#[derive(Debug, Clone)]
pub struct KvStoreConfig {
    pub cache_locally: bool,
    pub auto_pin: bool,
    pub compression_enabled: bool,
    pub max_cache_size: usize,
}

impl Default for KvStoreConfig {
    fn default() -> Self {
        Self {
            cache_locally: true,
            auto_pin: true,
            compression_enabled: false, // Disable for simplicity
            max_cache_size: 1000,
        }
    }
}

impl IpfsKvStore {
    /// Create a new IPFS-backed key-value store
    pub fn new(ipfs_manager: Arc<IpfsManager>, config: KvStoreConfig) -> Self {
        Self {
            ipfs_manager,
            local_cache: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }
    
    /// Store a key-value pair
    pub async fn put(&self, key: &str, value: &[u8], content_type: &str, metadata: HashMap<String, String>) -> CouchResult<KvEntry> {
        debug!("Storing KV entry: {} ({} bytes)", key, value.len());
        
        // Store in IPFS
        let ipfs_cid = self.ipfs_manager.store_data(value, content_type).await?;
        
        let entry = KvEntry {
            key: key.to_string(),
            value: value.to_vec(),
            content_type: content_type.to_string(),
            ipfs_cid: Some(ipfs_cid.cid),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            size: value.len() as u64,
            metadata,
        };
        
        // Cache locally if enabled
        if self.config.cache_locally {
            let mut cache = self.local_cache.lock().await;
            
            // Evict oldest entries if cache is full
            if cache.len() >= self.config.max_cache_size {
                let oldest_key = cache
                    .iter()
                    .min_by_key(|(_, entry)| entry.created_at)
                    .map(|(key, _)| key.clone());
                
                if let Some(key_to_remove) = oldest_key {
                    cache.remove(&key_to_remove);
                }
            }
            
            cache.insert(key.to_string(), entry.clone());
        }
        
        info!("Stored KV entry: {} -> {}", key, entry.ipfs_cid.as_ref().unwrap());
        Ok(entry)
    }
    
    /// Retrieve a value by key
    pub async fn get(&self, key: &str) -> CouchResult<KvEntry> {
        debug!("Retrieving KV entry: {}", key);
        
        // Check local cache first
        if self.config.cache_locally {
            let cache = self.local_cache.lock().await;
            if let Some(entry) = cache.get(key) {
                debug!("Found KV entry in local cache: {}", key);
                return Ok(entry.clone());
            }
        }
        
        // If not in cache, this is a limitation of our simple implementation
        // In a real system, we'd store the key->CID mapping separately
        Err(CouchError::not_found(&format!("Key not found: {}", key)))
    }
    
    /// Delete a key-value pair
    pub async fn delete(&self, key: &str) -> CouchResult<bool> {
        debug!("Deleting KV entry: {}", key);
        
        // Remove from local cache
        if self.config.cache_locally {
            let mut cache = self.local_cache.lock().await;
            if let Some(entry) = cache.remove(key) {
                // Optionally unpin from IPFS
                if let Some(cid) = entry.ipfs_cid {
                    if let Err(e) = self.ipfs_manager.unpin_content(&cid).await {
                        warn!("Failed to unpin content {}: {}", cid, e);
                    }
                }
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    /// List all keys
    pub async fn list_keys(&self) -> Vec<String> {
        if self.config.cache_locally {
            let cache = self.local_cache.lock().await;
            cache.keys().cloned().collect()
        } else {
            vec![]
        }
    }
    
    /// Get store statistics
    pub async fn get_stats(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();
        
        if self.config.cache_locally {
            let cache = self.local_cache.lock().await;
            stats.insert("cached_entries".to_string(), serde_json::Value::Number(cache.len().into()));
            
            let total_size: u64 = cache.values().map(|entry| entry.size).sum();
            stats.insert("total_cached_bytes".to_string(), serde_json::Value::Number(total_size.into()));
            
            let avg_size = if cache.len() > 0 { total_size / cache.len() as u64 } else { 0 };
            stats.insert("avg_entry_size".to_string(), serde_json::Value::Number(avg_size.into()));
        }
        
        stats.insert("cache_enabled".to_string(), serde_json::Value::Bool(self.config.cache_locally));
        stats.insert("auto_pin".to_string(), serde_json::Value::Bool(self.config.auto_pin));
        stats.insert("max_cache_size".to_string(), serde_json::Value::Number(self.config.max_cache_size.into()));
        
        stats
    }
    
    /// Clear local cache
    pub async fn clear_cache(&self) {
        if self.config.cache_locally {
            let mut cache = self.local_cache.lock().await;
            cache.clear();
            debug!("Cleared KV store local cache");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    fn create_test_config() -> IpfsConfig {
        IpfsConfig {
            api_url: "http://127.0.0.1:5001".to_string(),
            gateway_url: "http://127.0.0.1:8080".to_string(),
            pin_content: false, // Don't pin in tests
            cache_enabled: true,
            timeout_seconds: 5,
        }
    }
    
    #[tokio::test]
    #[ignore] // Requires running IPFS node
    async fn test_ipfs_store_retrieve() {
        let config = create_test_config();
        let manager = IpfsManager::new(config).unwrap();
        
        let test_data = b"Hello, IPFS!";
        let content_type = "text/plain";
        
        let cid_info = manager.store_data(test_data, content_type).await.unwrap();
        assert!(!cid_info.cid.is_empty());
        assert_eq!(cid_info.size, test_data.len() as u64);
        
        let retrieved = manager.get_data(&cid_info.cid).await.unwrap();
        assert_eq!(retrieved, test_data);
    }
    
    #[tokio::test]
    async fn test_kv_store_operations() {
        let ipfs_config = create_test_config();
        let ipfs_manager = Arc::new(IpfsManager::new(ipfs_config).unwrap());
        let kv_config = KvStoreConfig::default();
        let kv_store = IpfsKvStore::new(ipfs_manager, kv_config);
        
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), "test".to_string());
        
        // This test will work even without IPFS running since we only test the cache
        let test_data = b"test value";
        
        // Note: This test would need a running IPFS node to fully work
        // For now, we can test the cache functionality
        
        let keys = kv_store.list_keys().await;
        assert!(keys.is_empty());
        
        let stats = kv_store.get_stats().await;
        assert_eq!(stats["cached_entries"], serde_json::Value::Number(0.into()));
    }
}