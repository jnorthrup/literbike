//! IPFS Client implementation for Literbike DHT
//! 
//! Ported from Trikeshed IpfsClient.kt

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use parking_lot::Mutex;
use sha2::{Sha256, Digest};
use super::kademlia::{PeerId, PeerInfo, RoutingTable};

// ============================================================================
// CID and Multihash
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HashType {
    Sha2_256 = 0x12,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Multihash {
    pub hash_type: HashType,
    pub digest: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Codec {
    Raw = 0x55,
    DagPb = 0x70,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CID {
    pub version: u64,
    pub codec: Codec,
    pub multihash: Multihash,
}

impl CID {
    pub fn new(version: u64, codec: Codec, multihash: Multihash) -> Self {
        Self { version, codec, multihash }
    }

    pub fn encode(&self) -> String {
        // Placeholder for real CID encoding (e.g. CIDv1 Base32)
        // For P0 we use a simple hex representation
        format!("cid-v{}-{:x}-{}", self.version, self.codec as u8, hex::encode(&self.multihash.digest))
    }
}

// ============================================================================
// IpfsBlock and Storage
// ============================================================================

#[derive(Debug, Clone)]
pub struct IpfsLink {
    pub name: String,
    pub cid: CID,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct IpfsBlock {
    pub cid: CID,
    pub data: Vec<u8>,
    pub links: Vec<IpfsLink>,
}

pub trait IpfsStorage: Send + Sync {
    fn put_block(&self, block: IpfsBlock);
    fn get_block(&self, cid: &CID) -> Option<IpfsBlock>;
    fn has_block(&self, cid: &CID) -> bool;
    fn delete_block(&self, cid: &CID) -> bool;
    fn pin(&self, cid: CID);
    fn unpin(&self, cid: &CID) -> bool;
    fn is_pinned(&self, cid: &CID) -> bool;
    fn list_pins(&self) -> Vec<CID>;
}

pub struct InMemoryStorage {
    blocks: Mutex<HashMap<String, IpfsBlock>>,
    pins: Mutex<HashSet<CID>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            blocks: Mutex::new(HashMap::new()),
            pins: Mutex::new(HashSet::new()),
        }
    }
}

impl IpfsStorage for InMemoryStorage {
    fn put_block(&self, block: IpfsBlock) {
        self.blocks.lock().insert(block.cid.encode(), block);
    }

    fn get_block(&self, cid: &CID) -> Option<IpfsBlock> {
        self.blocks.lock().get(&cid.encode()).cloned()
    }

    fn has_block(&self, cid: &CID) -> bool {
        self.blocks.lock().contains_key(&cid.encode())
    }

    fn delete_block(&self, cid: &CID) -> bool {
        if self.is_pinned(cid) {
            return false;
        }
        self.blocks.lock().remove(&cid.encode()).is_some()
    }

    fn pin(&self, cid: CID) {
        self.pins.lock().insert(cid);
    }

    fn unpin(&self, cid: &CID) -> bool {
        self.pins.lock().remove(cid)
    }

    fn is_pinned(&self, cid: &CID) -> bool {
        self.pins.lock().contains(cid)
    }

    fn list_pins(&self) -> Vec<CID> {
        self.pins.lock().iter().cloned().collect()
    }
}

// ============================================================================
// IpfsClient
// ============================================================================

pub struct IpfsClient {
    #[allow(dead_code)]
    local_peer_id: PeerId,
    routing_table: Mutex<RoutingTable>,
    storage: Arc<dyn IpfsStorage>,
}

impl IpfsClient {
    pub fn new(local_peer_id: PeerId, storage: Arc<dyn IpfsStorage>) -> Self {
        let routing_table = RoutingTable::new(local_peer_id.clone(), 20); // bucket_size = 20
        Self {
            local_peer_id,
            routing_table: Mutex::new(routing_table),
            storage,
        }
    }

    /// Get routing table for peer management
    pub fn routing_table(&self) -> &Mutex<RoutingTable> {
        &self.routing_table
    }

    /// Add content to IPFS
    pub async fn add(&self, data: Vec<u8>) -> CID {
        let hash = self.compute_hash(&data);
        let multihash = Multihash {
            hash_type: HashType::Sha2_256,
            digest: hash,
        };
        let cid = CID::new(1, Codec::Raw, multihash);

        let block = IpfsBlock {
            cid: cid.clone(),
            data,
            links: Vec::new(),
        };

        self.storage.put_block(block);
        self.announce_block(&cid).await;

        cid
    }

    /// Get content from IPFS
    pub async fn get(&self, cid: &CID) -> Option<Vec<u8>> {
        // Check local storage
        if let Some(block) = self.storage.get_block(cid) {
            return Some(block.data);
        }

        // Find providers via DHT
        let providers = self.find_providers(cid).await;
        if providers.is_empty() {
            return None;
        }

        // Request block from first provider (stubbed)
        let provider = &providers[0];
        if let Some(block) = self.request_block(provider, cid).await {
            if self.verify_block(&block) {
                self.storage.put_block(block.clone());
                return Some(block.data);
            }
        }

        None
    }

    // DHT operations (Internal/Protocols)

    pub async fn announce_block(&self, _cid: &CID) {
        // In real implementation, would announce to DHT network
        // For now, it's a no-op
    }

    pub async fn find_providers(&self, _cid: &CID) -> Vec<PeerInfo> {
        // In real implementation, would query DHT
        // For now, return empty
        Vec::new()
    }

    async fn request_block(&self, _provider: &PeerInfo, _cid: &CID) -> Option<IpfsBlock> {
        // Would use QUIC to request block from peer
        // For now, return None
        None
    }

    fn verify_block(&self, block: &IpfsBlock) -> bool {
        let hash = self.compute_hash(&block.data);
        hash == block.cid.multihash.digest
    }

    fn compute_hash(&self, data: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[tokio::test]
    async fn test_ipfs_client_add_get_roundtrip() {
        let peer_id = PeerId::random();
        let storage = Arc::new(InMemoryStorage::new());
        let client = IpfsClient::new(peer_id, storage);

        let data = b"hello ipfs".to_vec();
        let cid = client.add(data.clone()).await;

        let retrieved = client.get(&cid).await.expect("should find block");
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_ipfs_client_get_not_found() {
        let peer_id = PeerId::random();
        let storage = Arc::new(InMemoryStorage::new());
        let client = IpfsClient::new(peer_id, storage);

        let hash = vec![0u8; 32];
        let multihash = Multihash { hash_type: HashType::Sha2_256, digest: hash };
        let cid = CID::new(1, Codec::Raw, multihash);

        let retrieved = client.get(&cid).await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_storage_pin_unpin() {
        let storage = InMemoryStorage::new();
        let hash = vec![0u8; 32];
        let multihash = Multihash { hash_type: HashType::Sha2_256, digest: hash };
        let cid = CID::new(1, Codec::Raw, multihash);

        assert!(!storage.is_pinned(&cid));
        storage.pin(cid.clone());
        assert!(storage.is_pinned(&cid));
        assert!(storage.unpin(&cid));
        assert!(!storage.is_pinned(&cid));
    }

    #[tokio::test]
    async fn test_cid_encode() {
        let hash = vec![0xaa; 32];
        let multihash = Multihash { hash_type: HashType::Sha2_256, digest: hash };
        let cid = CID::new(1, Codec::Raw, multihash);
        let encoded = cid.encode();
        assert!(encoded.starts_with("cid-v1-55-aaaa"));
    }
}
