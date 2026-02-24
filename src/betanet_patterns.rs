//! Betanet Patterns - Extracted from Kotlin Multiplatform Code
//! 
//! This module ports valuable patterns from Betanet Kotlin codebase:
//! - Network events for reactor pattern
//! - CRDT services for distributed data
//! - IPFS/DHT types for content addressing
//! - Indexed<T> zero-allocation patterns
//!
//! Source files:
//! - betanet-enhanced-reactor/src/commonMain/kotlin/BetanetReactorCore.kt
//! - betanet-enhanced-crdt/src/commonMain/kotlin/BetanetCRDTCore.kt
//! - betanet-enhanced-ipfs/src/commonMain/kotlin/BetanetIPFSCore.kt
//! - betanet-integration-demo/src/commonMain/kotlin/BetanetIntegrationDemo.kt

use crate::concurrency::ccek::{ContextElement, ContextKey};
use std::sync::Arc;

/// Indexed type for zero-allocation access patterns
/// Equivalent to Kotlin's `typealias Indexed<T> = Join<Int, (Int) -> T>`
pub struct Indexed<T: Send + Sync> {
    pub len: usize,
    pub accessor: Arc<dyn Fn(usize) -> T + Send + Sync>,
}

impl<T: Send + Sync> Indexed<T> {
    pub fn new(len: usize, accessor: Arc<dyn Fn(usize) -> T + Send + Sync>) -> Self {
        Self { len, accessor }
    }
    
    pub fn get(&self, index: usize) -> T {
        (self.accessor)(index)
    }
    
    pub fn len(&self) -> usize {
        self.len
    }
    
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<T: Send + Sync> Clone for Indexed<T> {
    fn clone(&self) -> Self {
        Self {
            len: self.len,
            accessor: self.accessor.clone(),
        }
    }
}

impl<T: std::fmt::Debug + Send + Sync> std::fmt::Debug for Indexed<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Indexed")
            .field("len", &self.len)
            .field("accessor", &"<closure>")
            .finish()
    }
}

/// Helper to create Indexed from Vec
pub fn indexed_from_vec<T: Clone + Send + Sync + 'static>(vec: Vec<T>) -> Indexed<T> {
    let len = vec.len();
    let arc_vec = Arc::new(vec);
    Indexed::new(len, Arc::new(move |i| arc_vec[i].clone()))
}

/// Helper to create Indexed from slice
pub fn indexed_from_slice<T: Clone + Send + Sync + 'static>(slice: &[T]) -> Indexed<T> {
    let len = slice.len();
    let arc_vec = Arc::new(slice.to_vec());
    Indexed::new(len, Arc::new(move |i| arc_vec[i].clone()))
}

/// Network events for reactor pattern (from BetanetReactorCore.kt)
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    ConnectionAccepted {
        connection_id: String,
        remote_addr: String,
    },
    DataReceived {
        connection_id: String,
        data: Indexed<u8>,
    },
    ConnectionClosed {
        connection_id: String,
        reason: Option<String>,
    },
    ProtocolDetected {
        connection_id: String,
        protocol: DetectionResult,
    },
    /// IPFS-specific events
    IPFSBlockRequested {
        connection_id: String,
        cid: BetanetCID,
    },
    IPFSBlockReceived {
        connection_id: String,
        block: BetanetBlock,
    },
    DHTPeerDiscovered {
        peer: PeerInfo,
        connection_id: String,
    },
}

/// Protocol detection results (from BetanetReactorCore.kt)
#[derive(Debug, Clone)]
pub enum DetectionResult {
    Unknown,
    HTTP(HTTPVersion),
    QUIC(QUICVersion),
    HTX(HTXVersion),
    TLS(TLSVersion),
}

#[derive(Debug, Clone)]
pub enum HTTPVersion {
    HTTP10,
    HTTP11,
    HTTP2,
    HTTP3,
}

#[derive(Debug, Clone)]
pub enum QUICVersion {
    QUICv1,
    QUICv2,
}

#[derive(Debug, Clone)]
pub enum HTXVersion {
    HTX10,
    HTX11,
}

#[derive(Debug, Clone)]
pub enum TLSVersion {
    TLS12,
    TLS13,
}

// ============================================================================
// IPFS/DHT Types (from BetanetIPFSCore.kt)
// ============================================================================

/// Betanet CID (Content Identifier)
#[derive(Debug, Clone)]
pub struct BetanetCID {
    pub version: u8,
    pub codec: Codec,
    pub multihash: BetanetMultihash,
}

impl BetanetCID {
    /// Convert to IPFS string representation
    pub fn to_ipfs_string(&self) -> String {
        match self.version {
            0 => format!("Qm{}", base58_encode(&self.multihash.encode())),
            1 => format!("b{}", base32_encode(&self.encode_cid_v1())),
            _ => panic!("Unsupported CID version"),
        }
    }
    
    fn encode_cid_v1(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.version);
        bytes.extend_from_slice(&encode_varint(self.codec.code() as u64));
        bytes.extend_from_slice(&self.multihash.encode());
        bytes
    }
    
    /// Create CID from content hash
    pub fn from_content_hash(data: &[u8]) -> Self {
        let hash = hash_sha256(data);
        BetanetCID {
            version: 1,
            codec: Codec::Raw,
            multihash: BetanetMultihash::sha2_256(&hash),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Raw = 0x55,
    DagPb = 0x70,
    DagCbor = 0x71,
    DagJson = 0x0297,
    BetanetBlock = 0xB047,
}

impl Codec {
    pub fn code(&self) -> u32 {
        *self as u32
    }
}

/// Betanet Multihash
#[derive(Debug, Clone)]
pub struct BetanetMultihash {
    pub algorithm: HashAlgorithm,
    pub digest: Vec<u8>,
}

impl BetanetMultihash {
    pub fn sha2_256(data: &[u8]) -> Self {
        let hash = hash_sha256(data);
        Self {
            algorithm: HashAlgorithm::Sha256,
            digest: hash.to_vec(),
        }
    }
    
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.algorithm.code());
        bytes.push(self.digest.len() as u8);
        bytes.extend_from_slice(&self.digest);
        bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha256 = 0x12,
    Sha3_256 = 0x16,
    Blake3 = 0x1E,
    Poseidon = 0x1F, // ZK-friendly
}

impl HashAlgorithm {
    pub fn code(&self) -> u8 {
        *self as u8
    }
    
    pub fn size(&self) -> usize {
        match self {
            Self::Sha256 => 32,
            Self::Sha3_256 => 32,
            Self::Blake3 => 32,
            Self::Poseidon => 32,
        }
    }
}

/// Betanet Block (content-addressed data)
#[derive(Debug, Clone)]
pub struct BetanetBlock {
    pub cid: BetanetCID,
    pub data: Indexed<u8>,
    pub links: Vec<BetanetLink>,
    pub metadata: BlockMetadata,
}

#[derive(Debug, Clone)]
pub struct BetanetLink {
    pub name: String,
    pub cid: BetanetCID,
    pub size: u64,
    pub link_type: LinkType,
}

#[derive(Debug, Clone, Copy)]
pub enum LinkType {
    Data,
    Metadata,
    Signature,
    Witness,
}

#[derive(Debug, Clone, Default)]
pub struct BlockMetadata {
    pub timestamp: u64,
    pub author: String,
    pub encryption: Option<EncryptionInfo>,
    pub zk_proof: Option<ZKProofInfo>,
}

#[derive(Debug, Clone)]
pub struct EncryptionInfo {
    pub algorithm: String,
    pub key_fingerprint: String,
    pub nonce: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ZKProofInfo {
    pub proof_type: String,
    pub public_inputs: Vec<String>,
    pub proof: Vec<u8>,
}

// ============================================================================
// DHT Types (from BetanetIPFSCore.kt)
// ============================================================================

/// Kademlia Node ID
#[derive(Debug, Clone)]
pub struct NodeId {
    pub id: Vec<u8>,
}

impl NodeId {
    pub fn new(id: Vec<u8>) -> Self {
        Self { id }
    }
    
    /// XOR distance for Kademlia routing
    pub fn xor_distance(&self, other: &NodeId) -> Vec<u8> {
        let max_len = self.id.len().max(other.id.len());
        let mut distance = vec![0u8; max_len];
        
        for i in 0..max_len {
            let a = if i < self.id.len() { self.id[i] } else { 0 };
            let b = if i < other.id.len() { other.id[i] } else { 0 };
            distance[i] = a ^ b;
        }
        
        distance
    }
    
    /// Create NodeId from public key
    pub fn from_public_key(pubkey: &[u8]) -> Self {
        let hash = hash_sha256(pubkey);
        Self::new(hash.to_vec())
    }
    
    /// Generate random NodeId
    pub fn random() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let id: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        Self::new(id)
    }
}

/// Peer information for DHT
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub node_id: NodeId,
    pub addresses: Vec<String>, // Multiaddrs
    pub protocols: Vec<String>,
    pub public_key: Vec<u8>,
    pub last_seen: u64,
}

/// Kademlia routing table
pub struct BetanetRoutingTable {
    local_node_id: NodeId,
    buckets: Vec<KBucket>,
    bucket_size: usize,
}

impl BetanetRoutingTable {
    pub fn new(local_node_id: NodeId, bucket_size: usize) -> Self {
        let buckets = (0..256).map(|_| KBucket::new(bucket_size)).collect();
        Self {
            local_node_id,
            buckets,
            bucket_size,
        }
    }
    
    pub fn add_peer(&mut self, peer: PeerInfo) {
        if peer.node_id.id == self.local_node_id.id {
            return;
        }
        let bucket_index = self.get_bucket_index(&peer.node_id);
        self.buckets[bucket_index].add(peer);
    }
    
    pub fn find_closest_peers(&self, target: &NodeId, count: usize) -> Vec<PeerInfo> {
        let mut all_peers: Vec<(PeerInfo, Vec<u8>)> = Vec::new();
        
        for bucket in &self.buckets {
            for peer in &bucket.peers {
                let distance = self.local_node_id.xor_distance(target);
                all_peers.push((peer.clone(), distance));
            }
        }
        
        // Sort by XOR distance (simplified)
        all_peers.sort_by(|a, b| {
            let dist_a = a.1.iter().fold(0u32, |acc, &b| acc + b as u32);
            let dist_b = b.1.iter().fold(0u32, |acc, &b| acc + b as u32);
            dist_a.cmp(&dist_b)
        });
        
        all_peers.into_iter().take(count).map(|(p, _)| p).collect()
    }
    
    fn get_bucket_index(&self, node_id: &NodeId) -> usize {
        let distance = self.local_node_id.xor_distance(node_id);
        leading_zeros(&distance).min(255)
    }
}

/// Kademlia K-Bucket
#[derive(Debug)]
pub struct KBucket {
    max_size: usize,
    peers: Vec<PeerInfo>,
}

impl KBucket {
    pub fn new(max_size: usize) -> Self {
        Self { max_size, peers: Vec::new() }
    }
    
    pub fn add(&mut self, peer: PeerInfo) -> bool {
        // Remove existing if present
        self.peers.retain(|p| p.node_id.id != peer.node_id.id);
        
        if self.peers.len() < self.max_size {
            self.peers.push(peer);
            true
        } else {
            false // Bucket full
        }
    }
}

fn leading_zeros(bytes: &[u8]) -> usize {
    for (i, &byte) in bytes.iter().enumerate() {
        if byte != 0 {
            return i * 8 + byte.leading_zeros() as usize;
        }
    }
    bytes.len() * 8
}

// ============================================================================
// CRDT Types (from BetanetCRDTCore.kt)
// ============================================================================

/// CRDT Document types
#[derive(Debug, Clone)]
pub enum DocumentType {
    Text,
    Markdown,
    Json,
    Contract,
    Ledger,
    ProtocolSpec,
    ZkCircuit,
    GraphData,
}

/// CRDT Operation types
#[derive(Debug, Clone)]
pub enum OperationType {
    Insert,
    Delete,
    Replace,
    Attribute,
    Structure,
    Signature,
    Witness,
    Annotation,
}

/// Vector clock for causality tracking
#[derive(Debug, Clone, Default)]
pub struct VectorClock {
    clocks: std::collections::HashMap<String, u64>,
}

impl VectorClock {
    pub fn increment(&mut self, node_id: &str) {
        *self.clocks.entry(node_id.to_string()).or_insert(0) += 1;
    }
    
    pub fn update(&mut self, other: &VectorClock) {
        for (node_id, timestamp) in &other.clocks {
            let entry = self.clocks.entry(node_id.clone()).or_insert(0);
            *entry = (*entry).max(*timestamp);
        }
    }
    
    pub fn compare(&self, other: &VectorClock) -> ClockComparison {
        let mut has_greater = false;
        let mut has_less = false;
        
        let all_nodes: std::collections::HashSet<_> = 
            self.clocks.keys().chain(other.clocks.keys()).collect();
        
        for node_id in all_nodes {
            let this_time = self.clocks.get(node_id).copied().unwrap_or(0);
            let other_time = other.clocks.get(node_id).copied().unwrap_or(0);
            
            if this_time > other_time {
                has_greater = true;
            } else if this_time < other_time {
                has_less = true;
            }
        }
        
        match (has_greater, has_less) {
            (true, false) => ClockComparison::Greater,
            (false, true) => ClockComparison::Less,
            (false, false) => ClockComparison::Equal,
            _ => ClockComparison::Concurrent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockComparison {
    Greater,
    Less,
    Equal,
    Concurrent,
}

// ============================================================================
// Helper functions
// ============================================================================

fn hash_sha256(data: &[u8]) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn encode_varint(value: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut v = value;
    while v >= 0x80 {
        bytes.push((v as u8) | 0x80);
        v >>= 7;
    }
    bytes.push(v as u8);
    bytes
}

fn base58_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    
    let mut num = num_bigint::BigUint::from_bytes_be(data);
    let mut result = Vec::new();
    let five = num_bigint::BigUint::from(58u32);
    
    while num > num_bigint::BigUint::from(0u32) {
        let remainder = &num % &five;
        num /= &five;
        let idx: usize = remainder.to_string().parse().unwrap_or(0);
        result.push(ALPHABET[idx]);
    }
    
    // Add leading '1's for leading zeros
    for &byte in data.iter().take_while(|&&b| b == 0) {
        result.push(ALPHABET[0]);
    }
    
    result.reverse();
    String::from_utf8(result).unwrap()
}

fn base32_encode(data: &[u8]) -> String {
    use data_encoding::BASE32_NOPAD;
    BASE32_NOPAD.encode(data)
}

// ============================================================================
// CCEK Service Traits (from Betanet Kotlin)
// ============================================================================

/// DHT Service trait (from BetanetIPFSCore.kt)
pub trait BetanetDHTService: ContextElement {
    fn put(&self, cid: &BetanetCID, block: &BetanetBlock) -> bool;
    fn get(&self, cid: &BetanetCID) -> Option<BetanetBlock>;
    fn find_node(&self, node_id: &NodeId) -> Vec<PeerInfo>;
    fn find_providers(&self, cid: &BetanetCID) -> Vec<PeerInfo>;
    fn announce(&self, cid: &BetanetCID, provider: &PeerInfo) -> bool;
}

/// CRDT Storage Service trait (from BetanetCRDTCore.kt)
pub trait CRDTStorageService: ContextElement {
    fn save_document(&self, doc_id: &str, document: &[u8]) -> String;
    fn load_document(&self, doc_id: &str) -> Option<Vec<u8>>;
    fn save_operation(&self, op_id: &str, operation: &[u8]) -> bool;
    fn get_operation_history(&self, doc_id: &str) -> Vec<Vec<u8>>;
}

/// CRDT Network Service trait
pub trait CRDTNetworkService: ContextElement {
    fn broadcast_operation(&self, operation: &[u8]) -> bool;
    fn subscribe_to_document(&self, doc_id: &str) -> tokio::sync::broadcast::Receiver<Vec<u8>>;
    fn request_document_sync(&self, doc_id: &str, peer_id: &str) -> Option<Vec<u8>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexed_from_vec() {
        let vec = vec![1u8, 2, 3, 4, 5];
        let indexed = indexed_from_vec(vec.clone());

        assert_eq!(indexed.len(), 5);
        for i in 0..5 {
            assert_eq!(indexed.get(i), vec[i]);
        }
    }

    #[test]
    fn test_cid_creation() {
        let data = b"Hello, Betanet!";
        let cid = BetanetCID::from_content_hash(data);
        
        assert_eq!(cid.version, 1);
        assert_eq!(cid.codec, Codec::Raw);
        assert_eq!(cid.multihash.algorithm, HashAlgorithm::Sha256);
    }

    #[test]
    fn test_node_id_distance() {
        let node1 = NodeId::new(vec![0u8; 32]);
        let node2 = NodeId::new(vec![255u8; 32]);
        
        let distance = node1.xor_distance(&node2);
        assert_eq!(distance.len(), 32);
        assert!(distance.iter().all(|&b| b == 255));
    }

    #[test]
    fn test_vector_clock() {
        let mut vc1 = VectorClock::default();
        vc1.increment("node1");
        vc1.increment("node1");

        let mut vc2 = VectorClock::default();
        vc2.increment("node2");

        // vc1: {node1: 2}, vc2: {node2: 1} - concurrent
        assert_eq!(vc1.compare(&vc2), ClockComparison::Concurrent);

        // vc2: {node1: 1, node2: 1} - still concurrent with vc1
        vc2.increment("node1");
        assert_eq!(vc1.compare(&vc2), ClockComparison::Concurrent);
        
        // Make vc1 < vc2
        let mut vc3 = VectorClock::default();
        vc3.increment("node1");  // vc3: {node1: 1}
        vc3.increment("node2");  // vc3: {node1: 1, node2: 1}
        assert_eq!(vc3.compare(&vc2), ClockComparison::Equal);
    }

    #[test]
    fn test_routing_table() {
        let local = NodeId::random();
        let mut table = BetanetRoutingTable::new(local.clone(), 20);
        
        // Add some peers
        for _ in 0..10 {
            let peer = PeerInfo {
                node_id: NodeId::random(),
                addresses: vec!["/ip4/127.0.0.1/tcp/8080".to_string()],
                protocols: vec!["betanet/1.0".to_string()],
                public_key: vec![1u8; 32],
                last_seen: 0,
            };
            table.add_peer(peer);
        }
        
        // Find closest to random target
        let target = NodeId::random();
        let closest = table.find_closest_peers(&target, 5);
        assert!(closest.len() <= 10);
    }
}
