//! Content-Addressed Storage for CAS-Free Synchronization
//!
//! This module provides content-addressed storage primitives for durability,
//! deduplication, and crash recovery. Used as WARM PATH complement to
//! HOT PATH atomic operations.

use sha2::{Sha256, Digest};
use anyhow::Result;
use std::collections::HashMap;
use parking_lot::RwLock;

// ============================================================================
// Content Hash Types
// ============================================================================

/// SHA256 content hash (32 bytes)
pub type ContentHash = [u8; 32];

/// Merkle tree root hash
pub type MerkleRoot = [u8; 32];

/// Content-addressed blob with hash
#[derive(Debug, Clone)]
pub struct ContentBlob {
    pub hash: ContentHash,
    pub data: Vec<u8>,
    pub size: u32,
}

impl ContentBlob {
    /// Create content blob from data (computes hash automatically)
    pub fn new(data: Vec<u8>) -> Self {
        let hash = Sha256::digest(&data);
        let size = data.len() as u32;
        Self {
            hash: hash.into(),
            data,
            size,
        }
    }

    /// Create content blob from pre-computed hash (for verification)
    pub fn with_hash(data: Vec<u8>, hash: ContentHash) -> Self {
        let size = data.len() as u32;
        Self { hash, data, size }
    }

    /// Verify content matches hash
    pub fn verify(&self) -> bool {
        let computed = Sha256::digest(&self.data);
        computed.as_slice() == &self.hash
    }

    /// Get idempotent key (content hash)
    pub fn idempotent_key(&self) -> ContentHash {
        self.hash
    }
}

// ============================================================================
// Merkle Tree for Batch Consistency
// ============================================================================

/// Merkle tree node
#[derive(Debug, Clone)]
pub enum MerkleNode {
    Leaf(ContentHash),
    Branch {
        left: MerkleRoot,
        right: MerkleRoot,
        hash: MerkleRoot,
    },
}

impl MerkleNode {
    pub fn hash(&self) -> MerkleRoot {
        match self {
            MerkleNode::Leaf(hash) => *hash,
            MerkleNode::Branch { hash, .. } => *hash,
        }
    }

    /// Build Merkle tree from list of hashes
    pub fn build_tree(hashes: &[ContentHash]) -> Option<MerkleNode> {
        if hashes.is_empty() {
            return None;
        }

        let mut nodes: Vec<MerkleNode> = hashes.iter()
            .map(|&h| MerkleNode::Leaf(h))
            .collect();

        while nodes.len() > 1 {
            let mut next_level = Vec::new();
            let mut i = 0;

            while i < nodes.len() {
                let left = nodes[i].hash();
                let right = if i + 1 < nodes.len() {
                    nodes[i + 1].hash()
                } else {
                    left // Duplicate last node if odd
                };

                let combined = [left.as_slice(), right.as_slice()].concat();
                let hash = Sha256::digest(&combined);
                let hash_array: [u8; 32] = hash.into();

                next_level.push(MerkleNode::Branch {
                    left,
                    right,
                    hash: hash_array,
                });

                i += 2;
            }

            nodes = next_level;
        }

        nodes.into_iter().next()
    }

    /// Get Merkle root from tree
    pub fn root(&self) -> MerkleRoot {
        self.hash()
    }
}

// ============================================================================
// In-Memory Content-Addressed Store (Thread-Safe)
// ============================================================================

/// Content store statistics
#[derive(Debug, Clone, Default)]
pub struct ContentStats {
    pub total_blobs: u64,
    pub total_bytes: u64,
    pub total_refs: u64,
    pub dedup_ratio: f64,
}

/// In-memory content-addressed store with thread-safe access
pub struct ContentAddressedStore {
    blobs: RwLock<HashMap<ContentHash, ContentBlob>>,
    refs: RwLock<HashMap<String, ContentHash>>,
    merkle_roots: RwLock<HashMap<MerkleRoot, usize>>, // root -> leaf count
}

impl ContentAddressedStore {
    /// Create new in-memory store
    pub fn new() -> Self {
        Self {
            blobs: RwLock::new(HashMap::new()),
            refs: RwLock::new(HashMap::new()),
            merkle_roots: RwLock::new(HashMap::new()),
        }
    }

    /// Store content blob (idempotent - same content = same hash = no duplicate)
    pub fn store(&self, blob: &ContentBlob) -> Result<ContentHash> {
        let mut blobs = self.blobs.write();
        
        // Check if already exists (deduplication)
        if blobs.contains_key(&blob.hash) {
            return Ok(blob.hash);
        }
        
        blobs.insert(blob.hash, blob.clone());
        Ok(blob.hash)
    }

    /// Retrieve content by hash
    pub fn retrieve(&self, hash: &ContentHash) -> Result<Option<ContentBlob>> {
        let blobs = self.blobs.read();
        Ok(blobs.get(hash).cloned())
    }

    /// Store content with reference key (for stream state, etc.)
    pub fn store_ref(&self, ref_key: &str, _ref_type: &str, blob: &ContentBlob) -> Result<()> {
        // Store content first
        self.store(blob)?;
        
        // Store reference
        let mut refs = self.refs.write();
        refs.insert(ref_key.to_string(), blob.hash);
        
        Ok(())
    }

    /// Retrieve content by reference key
    pub fn retrieve_ref(&self, ref_key: &str) -> Result<Option<ContentBlob>> {
        let refs = self.refs.read();
        
        if let Some(hash) = refs.get(ref_key) {
            let blobs = self.blobs.read();
            Ok(blobs.get(hash).cloned())
        } else {
            Ok(None)
        }
    }

    /// Store Merkle root
    pub fn store_merkle_root(&self, root: &MerkleRoot, leaf_count: usize) -> Result<()> {
        let mut merkle_roots = self.merkle_roots.write();
        merkle_roots.insert(*root, leaf_count);
        Ok(())
    }

    /// Get content statistics
    pub fn stats(&self) -> Result<ContentStats> {
        let blobs = self.blobs.read();
        let refs = self.refs.read();
        
        let total_blobs = blobs.len() as u64;
        let total_bytes = blobs.values().map(|b| b.size as u64).sum();
        let total_refs = refs.len() as u64;
        
        Ok(ContentStats {
            total_blobs,
            total_bytes,
            total_refs,
            dedup_ratio: 0.0, // Would need more tracking
        })
    }
}

impl Default for ContentAddressedStore {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_blob_creation() {
        let data = b"Hello, CAS-Free World!";
        let blob = ContentBlob::new(data.to_vec());

        assert_eq!(blob.size, data.len() as u32);
        assert!(blob.verify());
    }

    #[test]
    fn test_content_blob_idempotency() {
        let data = b"Test data";
        let blob1 = ContentBlob::new(data.to_vec());
        let blob2 = ContentBlob::new(data.to_vec());

        // Same content = same hash (natural idempotency)
        assert_eq!(blob1.hash, blob2.hash);
        assert_eq!(blob1.idempotent_key(), blob2.idempotent_key());
    }

    #[test]
    fn test_merkle_tree() {
        let hashes: Vec<ContentHash> = (0..4u32)
            .map(|i| {
                let data = i.to_be_bytes();
                Sha256::digest(&data).into()
            })
            .collect();

        let tree = MerkleNode::build_tree(&hashes);
        assert!(tree.is_some());

        let root = tree.unwrap().root();
        assert_eq!(root.len(), 32);
    }

    #[test]
    fn test_content_store() -> Result<()> {
        let store = ContentAddressedStore::new();

        let blob = ContentBlob::new(b"Test content".to_vec());
        let hash = store.store(&blob)?;

        // Retrieve
        let retrieved = store.retrieve(&hash)?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().data, b"Test content");

        // Store same content again (idempotent)
        let blob2 = ContentBlob::new(b"Test content".to_vec());
        let hash2 = store.store(&blob2)?;

        // Same hash = deduplicated
        assert_eq!(hash, hash2);

        Ok(())
    }

    #[test]
    fn test_content_store_refs() -> Result<()> {
        let store = ContentAddressedStore::new();

        let blob = ContentBlob::new(b"Test content".to_vec());
        store.store_ref("test_key", "test_type", &blob)?;

        // Retrieve by ref
        let retrieved = store.retrieve_ref("test_key")?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().data, b"Test content");

        Ok(())
    }

    #[test]
    fn test_store_stats() -> Result<()> {
        let store = ContentAddressedStore::new();

        for i in 0..10 {
            let data = format!("Test {}", i);
            let blob = ContentBlob::new(data.into_bytes());
            store.store(&blob)?;
        }

        let stats = store.stats()?;
        assert_eq!(stats.total_blobs, 10);
        assert!(stats.total_bytes > 0);

        Ok(())
    }
}
