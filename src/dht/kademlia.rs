//! Kademlia DHT for Literbike - Ported from Trikeshed IpfsCore.kt
//!
//! Source: ../superbikeshed/Trikeshed/src/commonMain/kotlin/borg/trikeshed/ipfs/IpfsCore.kt

use std::collections::HashMap;
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};

// ============================================================================
// PeerId - SHA256-based node identity
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId {
    pub id: Vec<u8>,
}

impl PeerId {
    pub fn new(id: Vec<u8>) -> Self {
        Self { id }
    }

    /// Create PeerId from public key (SHA256 hash)
    pub fn from_public_key(pubkey: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(pubkey);
        let hash = hasher.finalize().to_vec();
        Self::new(hash)
    }

    /// Generate random PeerId
    pub fn random() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let id: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        Self::new(id)
    }

    /// XOR distance to another PeerId
    pub fn xor_distance(&self, other: &PeerId) -> Vec<u8> {
        let max_len = self.id.len().max(other.id.len());
        let mut distance = vec![0u8; max_len];

        for i in 0..max_len {
            let a = if i < self.id.len() { self.id[i] } else { 0 };
            let b = if i < other.id.len() { other.id[i] } else { 0 };
            distance[i] = a ^ b;
        }

        distance
    }

    /// Get bucket index for routing table (leading zeros in XOR distance)
    pub fn bucket_index(&self, other: &PeerId) -> usize {
        let distance = self.xor_distance(other);
        leading_zeros(&distance).min(255)
    }

    /// Base58 encoding for PeerId string representation
    pub fn to_base58(&self) -> String {
        base58_encode(&self.id)
    }

    /// Decode from Base58
    pub fn from_base58(s: &str) -> Option<Self> {
        let bytes = base58_decode(s)?;
        Some(Self::new(bytes))
    }
}

// ============================================================================
// PeerInfo - Peer information with multiaddrs
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: PeerId,
    pub addresses: Vec<String>, // Multiaddrs like "/ip4/127.0.0.1/tcp/4001"
    pub protocols: Vec<String>,
    pub public_key: Vec<u8>,
    pub last_seen: u64,
}

impl PeerInfo {
    pub fn new(id: PeerId, addresses: Vec<String>, protocols: Vec<String>) -> Self {
        Self {
            id,
            addresses,
            protocols,
            public_key: Vec::new(),
            last_seen: 0,
        }
    }
}

// ============================================================================
// KBucket - Kademlia routing bucket (20 peers max)
// ============================================================================

pub struct KBucket {
    pub peers: Vec<PeerInfo>,
    pub max_size: usize,
}

impl KBucket {
    pub fn new(max_size: usize) -> Self {
        Self {
            peers: Vec::with_capacity(max_size),
            max_size,
        }
    }

    /// Add peer to bucket, returns true if successful.
    ///
    /// P0 policy:
    /// - Duplicate peer IDs refresh the entry by removing the old peer and
    ///   appending the new value to the tail (LRU-like "most recently seen").
    /// - No eviction is performed when the bucket is full; new peers are rejected.
    pub fn add(&mut self, peer: PeerInfo) -> bool {
        // Remove existing if present (update)
        if let Some(pos) = self.peers.iter().position(|p| p.id == peer.id) {
            self.peers.remove(pos);
        }

        // Add if space available
        if self.peers.len() < self.max_size {
            self.peers.push(peer);
            true
        } else {
            false // Bucket full
        }
    }

    /// Remove peer by ID
    pub fn remove(&mut self, peer_id: &PeerId) {
        self.peers.retain(|p| p.id != *peer_id);
    }

    /// Check if bucket contains peer
    pub fn contains(&self, peer_id: &PeerId) -> bool {
        self.peers.iter().any(|p| p.id == *peer_id)
    }

    /// Get all peers as indexed vector
    pub fn to_indexed(&self) -> Vec<PeerInfo> {
        self.peers.clone()
    }

    /// Get peer count
    pub fn len(&self) -> usize {
        self.peers.len()
    }

    /// Check if bucket is empty
    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }
}

// ============================================================================
// RoutingTable - Kademlia routing table (256 k-buckets)
// ============================================================================

pub struct RoutingTable {
    pub local_id: PeerId,
    pub buckets: Vec<KBucket>,
    pub bucket_size: usize,
}

impl RoutingTable {
    pub fn new(local_id: PeerId, bucket_size: usize) -> Self {
        let buckets = (0..256).map(|_| KBucket::new(bucket_size)).collect();
        Self {
            local_id,
            buckets,
            bucket_size,
        }
    }

    /// Add peer to appropriate bucket based on XOR distance.
    ///
    /// P0 semantics:
    /// - local peer is never inserted into the routing table
    /// - bucket placement is derived from `PeerId::bucket_index` and the table
    ///   always has 256 buckets
    pub fn add_peer(&mut self, peer: PeerInfo) {
        if peer.id == self.local_id {
            return; // Don't add self
        }
        let bucket_index = self.local_id.bucket_index(&peer.id);
        self.buckets[bucket_index].add(peer);
    }

    /// Find closest peers to target node ID
    pub fn find_closest_peers(&self, target: &PeerId, count: usize) -> Vec<PeerInfo> {
        let mut all_peers: Vec<(PeerInfo, u32)> = Vec::new();

        // Collect all peers with their XOR distance to target
        for bucket in &self.buckets {
            for peer in &bucket.peers {
                let distance = peer.id.xor_distance(target);
                let dist_score: u32 = distance.iter().map(|&b| b as u32).sum();
                all_peers.push((peer.clone(), dist_score));
            }
        }

        // Sort by XOR distance (ascending)
        all_peers.sort_by(|a, b| a.1.cmp(&b.1));

        // Return closest N peers
        all_peers.into_iter().take(count).map(|(p, _)| p).collect()
    }

    /// Get bucket index for a node ID
    fn get_bucket_index(&self, node_id: &PeerId) -> usize {
        self.local_id.bucket_index(node_id)
    }

    /// Get peer by ID
    pub fn get_peer(&self, peer_id: &PeerId) -> Option<&PeerInfo> {
        let bucket_index = self.get_bucket_index(peer_id);
        self.buckets[bucket_index].peers.iter().find(|p| p.id == *peer_id)
    }

    /// Get all peers
    pub fn all_peers(&self) -> Vec<PeerInfo> {
        self.buckets.iter().flat_map(|b| b.to_indexed()).collect()
    }

    /// Get total peer count
    pub fn peer_count(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }
}

// ============================================================================
// Multihash - Content addressing hash
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Multihash {
    pub hash_type: HashType,
    pub digest: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashType {
    Sha256 = 0x12,
    Sha512 = 0x13,
    Sha3_256 = 0x16,
    Sha3_512 = 0x17,
    Blake2b256 = 0xb220,
    Blake2b512 = 0xb240,
}

impl Multihash {
    pub fn new(hash_type: HashType, digest: Vec<u8>) -> Self {
        Self { hash_type, digest }
    }

    /// Create SHA256 multihash from data
    pub fn sha256(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let digest = hasher.finalize().to_vec();
        Self::new(HashType::Sha256, digest)
    }

    /// Encode multihash to bytes
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.hash_type as u8);
        bytes.push(self.digest.len() as u8);
        bytes.extend_from_slice(&self.digest);
        bytes
    }

    /// Decode multihash from bytes
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 2 {
            return None;
        }
        let type_code = bytes[0];
        let size = bytes[1] as usize;
        if bytes.len() < 2 + size {
            return None;
        }
        let hash_type = match type_code {
            0x12 => HashType::Sha256,
            0x13 => HashType::Sha512,
            0x16 => HashType::Sha3_256,
            0x17 => HashType::Sha3_512,
            _ => return None,
        };
        let digest = bytes[2..2 + size].to_vec();
        Some(Self::new(hash_type, digest))
    }
}

// ============================================================================
// CID - Content Identifier
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CID {
    pub version: u8,
    pub codec: Codec,
    pub multihash: Multihash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    DagPb = 0x70,
    DagCbor = 0x71,
    Raw = 0x55,
    Json = 0x0200,
}

impl CID {
    pub fn new(version: u8, codec: Codec, multihash: Multihash) -> Self {
        Self {
            version,
            codec,
            multihash,
        }
    }

    /// Create CIDv0 from data (SHA256 + DagPb)
    pub fn v0(data: &[u8]) -> Self {
        let multihash = Multihash::sha256(data);
        Self::new(0, Codec::DagPb, multihash)
    }

    /// Create CIDv1 from data
    pub fn v1(data: &[u8], codec: Codec) -> Self {
        let multihash = Multihash::sha256(data);
        Self::new(1, codec, multihash)
    }

    /// Encode CID to string (Base58 for v0, Base32 for v1)
    pub fn encode(&self) -> String {
        match self.version {
            0 => {
                // CIDv0 is just the multihash in Base58
                base58_encode(&self.multihash.encode())
            }
            1 => {
                // CIDv1 has version + codec + multihash in Base32
                let mut bytes = Vec::new();
                bytes.push(1); // version
                bytes.extend(encode_varint(self.codec as u64));
                bytes.extend(self.multihash.encode());
                format!("b{}", base32_encode(&bytes))
            }
            _ => String::new(),
        }
    }

    /// Decode CID from string
    pub fn decode(s: &str) -> Option<Self> {
        if s.starts_with('b') {
            // CIDv1 (Base32)
            let bytes = base32_decode(&s[1..])?;
            if bytes.is_empty() || bytes[0] != 1 {
                return None;
            }
            // Parse varint codec (simplified)
            let codec = bytes[1] as u64;
            let multihash = Multihash::decode(&bytes[2..])?;
            Some(Self::new(1, codec_to_enum(codec)?, multihash))
        } else {
            // CIDv0 (Base58)
            let bytes = base58_decode(s)?;
            let multihash = Multihash::decode(&bytes)?;
            Some(Self::new(0, Codec::DagPb, multihash))
        }
    }
}

// ============================================================================
// Helper functions
// ============================================================================

fn leading_zeros(bytes: &[u8]) -> usize {
    for (i, &byte) in bytes.iter().enumerate() {
        if byte != 0 {
            return i * 8 + byte.leading_zeros() as usize;
        }
    }
    bytes.len() * 8
}

fn encode_varint(n: u64) -> Vec<u8> {
    // Simplified varint encoding
    vec![n as u8]
}

fn codec_to_enum(n: u64) -> Option<Codec> {
    match n {
        0x70 => Some(Codec::DagPb),
        0x71 => Some(Codec::DagCbor),
        0x55 => Some(Codec::Raw),
        0x0200 => Some(Codec::Json),
        _ => None,
    }
}

fn base58_encode(data: &[u8]) -> String {
    bs58::encode(data).into_string()
}

fn base58_decode(s: &str) -> Option<Vec<u8>> {
    bs58::decode(s).into_vec().ok()
}

fn base32_encode(data: &[u8]) -> String {
    // Simplified Base32 encoding (placeholder - use data-encoding crate)
    hex::encode(data)
}

fn base32_decode(s: &str) -> Option<Vec<u8>> {
    // Simplified Base32 decoding (placeholder - use data-encoding crate)
    hex::decode(s).ok()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_id_from_public_key_uses_sha256() {
        let pubkey = b"peer-public-key";
        let peer = PeerId::from_public_key(pubkey);

        let mut hasher = Sha256::new();
        hasher.update(pubkey);
        let expected = hasher.finalize().to_vec();

        assert_eq!(peer.id, expected);
        assert_eq!(peer.id.len(), 32);
    }

    #[test]
    fn test_peer_id_base58_roundtrip() {
        let peer = PeerId::new((0u8..32).collect());

        let encoded = peer.to_base58();
        assert_eq!(encoded, bs58::encode(&peer.id).into_string());
        assert_ne!(encoded, hex::encode(&peer.id));

        let decoded = PeerId::from_base58(&encoded).expect("valid base58");
        assert_eq!(decoded, peer);
    }

    #[test]
    fn test_peer_id_base58_rejects_invalid_input() {
        assert!(PeerId::from_base58("0OIl").is_none());
    }

    #[test]
    fn test_peer_id_xor_distance() {
        let peer1 = PeerId::new(vec![0u8; 32]);
        let peer2 = PeerId::new(vec![255u8; 32]);

        let distance = peer1.xor_distance(&peer2);
        assert_eq!(distance.len(), 32);
        assert!(distance.iter().all(|&b| b == 255));

        let reverse = peer2.xor_distance(&peer1);
        assert_eq!(reverse, distance);
        assert!(peer1
            .xor_distance(&peer1)
            .iter()
            .all(|&byte| byte == 0));
    }

    #[test]
    fn test_peer_id_bucket_index() {
        let local = PeerId::new(vec![0u8; 32]);
        let peer1 = PeerId::new(vec![0u8; 32]);
        let peer2 = PeerId::new(vec![255u8; 32]);

        // Same ID = bucket 255
        assert_eq!(local.bucket_index(&peer1), 255);
        // Opposite ID = bucket 0
        assert_eq!(local.bucket_index(&peer2), 0);
    }

    #[test]
    fn test_kbucket_add_remove() {
        let mut bucket = KBucket::new(20);
        let peer = PeerInfo::new(PeerId::random(), vec![], vec![]);

        assert!(bucket.add(peer.clone()));
        assert_eq!(bucket.len(), 1);
        assert!(bucket.contains(&peer.id));

        bucket.remove(&peer.id);
        assert_eq!(bucket.len(), 0);
        assert!(!bucket.contains(&peer.id));
    }

    #[test]
    fn test_kbucket_capacity_rejects_new_peer_without_eviction() {
        let mut bucket = KBucket::new(2);
        let peer1 = PeerInfo::new(PeerId::new(vec![1u8; 32]), vec!["/p1".into()], vec![]);
        let peer2 = PeerInfo::new(PeerId::new(vec![2u8; 32]), vec!["/p2".into()], vec![]);
        let peer3 = PeerInfo::new(PeerId::new(vec![3u8; 32]), vec!["/p3".into()], vec![]);

        assert!(bucket.add(peer1.clone()));
        assert!(bucket.add(peer2.clone()));
        assert!(!bucket.add(peer3.clone()));

        assert_eq!(bucket.len(), 2);
        assert!(bucket.contains(&peer1.id));
        assert!(bucket.contains(&peer2.id));
        assert!(!bucket.contains(&peer3.id));
        assert_eq!(bucket.peers[0].id, peer1.id);
        assert_eq!(bucket.peers[1].id, peer2.id);
    }

    #[test]
    fn test_kbucket_duplicate_add_refreshes_peer_and_updates_value() {
        let mut bucket = KBucket::new(2);
        let peer1 = PeerInfo::new(PeerId::new(vec![1u8; 32]), vec!["/old".into()], vec!["v1".into()]);
        let peer2 = PeerInfo::new(PeerId::new(vec![2u8; 32]), vec!["/peer2".into()], vec!["v1".into()]);

        assert!(bucket.add(peer1.clone()));
        assert!(bucket.add(peer2.clone()));

        let mut peer1_updated = PeerInfo::new(
            peer1.id.clone(),
            vec!["/new".into()],
            vec!["v2".into()],
        );
        peer1_updated.last_seen = 42;
        peer1_updated.public_key = vec![9, 9, 9];

        // Refresh should succeed even when the bucket is full because the
        // duplicate entry is removed before the capacity check.
        assert!(bucket.add(peer1_updated.clone()));
        assert_eq!(bucket.len(), 2);

        // LRU-like refresh semantics: updated peer is moved to the tail.
        assert_eq!(bucket.peers[0].id, peer2.id);
        assert_eq!(bucket.peers[1].id, peer1.id);
        assert_eq!(bucket.peers[1].addresses, peer1_updated.addresses);
        assert_eq!(bucket.peers[1].protocols, peer1_updated.protocols);
        assert_eq!(bucket.peers[1].public_key, peer1_updated.public_key);
        assert_eq!(bucket.peers[1].last_seen, peer1_updated.last_seen);
    }

    #[test]
    fn test_routing_table_add_find() {
        let local = PeerId::random();
        let mut table = RoutingTable::new(local.clone(), 20);

        // Add 10 random peers
        for _ in 0..10 {
            let peer = PeerInfo::new(PeerId::random(), vec![], vec![]);
            table.add_peer(peer);
        }

        assert_eq!(table.peer_count(), 10);

        // Find closest to random target
        let target = PeerId::random();
        let closest = table.find_closest_peers(&target, 5);
        assert_eq!(closest.len(), 5);
    }

    #[test]
    fn test_routing_table_has_256_buckets_and_excludes_local_peer() {
        let local = PeerId::new(vec![0u8; 32]);
        let mut table = RoutingTable::new(local.clone(), 20);

        assert_eq!(table.buckets.len(), 256);

        let local_peer = PeerInfo::new(local.clone(), vec!["/self".into()], vec![]);
        table.add_peer(local_peer);

        assert_eq!(table.peer_count(), 0);
        assert!(table.get_peer(&local).is_none());
    }

    #[test]
    fn test_routing_table_add_peer_places_peer_in_expected_bucket() {
        let local = PeerId::new(vec![0u8; 32]);
        let mut table = RoutingTable::new(local.clone(), 20);

        let peer_id = PeerId::new(vec![255u8; 32]); // maximal XOR distance from local zeros
        let expected_bucket = local.bucket_index(&peer_id);
        let peer = PeerInfo::new(peer_id.clone(), vec!["/ip4/127.0.0.1/tcp/4001".into()], vec![]);

        table.add_peer(peer);

        assert_eq!(expected_bucket, 0);
        assert_eq!(table.peer_count(), 1);
        assert!(table.buckets[expected_bucket].contains(&peer_id));
        assert!(table.get_peer(&peer_id).is_some());
    }

    #[test]
    fn test_routing_table_find_closest_peers_orders_by_simplified_xor_score() {
        let local = PeerId::new(vec![0u8; 32]);
        let mut table = RoutingTable::new(local, 20);
        let target = PeerId::new(vec![0u8; 32]);

        let near = PeerInfo::new(PeerId::new(vec![1u8; 32]), vec!["/near".into()], vec![]);
        let mid = PeerInfo::new(PeerId::new(vec![2u8; 32]), vec!["/mid".into()], vec![]);
        let far = PeerInfo::new(PeerId::new(vec![255u8; 32]), vec!["/far".into()], vec![]);

        table.add_peer(far.clone());
        table.add_peer(mid.clone());
        table.add_peer(near.clone());

        let closest = table.find_closest_peers(&target, 3);
        let ids: Vec<Vec<u8>> = closest.into_iter().map(|p| p.id.id).collect();

        assert_eq!(ids, vec![near.id.id.clone(), mid.id.id.clone(), far.id.id.clone()]);
    }

    #[test]
    fn test_multihash_sha256() {
        let data = b"Hello, Kademlia!";
        let mh = Multihash::sha256(data);

        assert_eq!(mh.hash_type, HashType::Sha256);
        assert_eq!(mh.digest.len(), 32);

        let encoded = mh.encode();
        let decoded = Multihash::decode(&encoded).unwrap();
        assert_eq!(decoded.digest, mh.digest);
    }

    #[test]
    fn test_cid_creation() {
        let data = b"Hello, IPFS!";
        let cid = CID::v0(data);

        assert_eq!(cid.version, 0);
        assert_eq!(cid.codec, Codec::DagPb);
        assert_eq!(cid.multihash.hash_type, HashType::Sha256);
    }
}
