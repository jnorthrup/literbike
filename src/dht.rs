// Distributed Hash Table — Kademlia-based peer discovery (port of Trikeshed DHT.kt)

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

/// 256-bit node identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(pub [u8; 32]);

impl NodeId {
    pub fn zero() -> Self { Self([0u8; 32]) }
    pub fn from_slice(b: &[u8]) -> Self {
        let mut arr = [0u8; 32];
        let len = b.len().min(32);
        arr[..len].copy_from_slice(&b[..len]);
        Self(arr)
    }
    /// XOR distance metric
    pub fn distance(&self, other: &NodeId) -> [u8; 32] {
        let mut d = [0u8; 32];
        for i in 0..32 { d[i] = self.0[i] ^ other.0[i]; }
        d
    }
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub id: NodeId,
    pub addr: String,
}

/// K-bucket routing table (k=20 per bucket)
pub struct RoutingTable {
    local_id: NodeId,
    buckets: Arc<RwLock<HashMap<u8, Vec<PeerInfo>>>>,
}

impl RoutingTable {
    pub fn new(local_id: NodeId) -> Self {
        Self { local_id, buckets: Arc::new(RwLock::new(HashMap::new())) }
    }
    /// Leading zero bits of XOR distance → bucket index
    fn bucket_index(dist: &[u8; 32]) -> u8 {
        for (i, &b) in dist.iter().enumerate() {
            if b != 0 {
                return (i as u8) * 8 + b.leading_zeros() as u8;
            }
        }
        255
    }
    pub fn insert(&self, peer: PeerInfo) {
        let dist = self.local_id.distance(&peer.id);
        let idx = Self::bucket_index(&dist);
        let mut buckets = self.buckets.write();
        let bucket = buckets.entry(idx).or_default();
        if bucket.iter().any(|p| p.id == peer.id) { return; }
        if bucket.len() < 20 { bucket.push(peer); }
    }
    pub fn find_closest(&self, target: &NodeId, k: usize) -> Vec<PeerInfo> {
        let buckets = self.buckets.read();
        let mut all: Vec<PeerInfo> = buckets.values().flatten().cloned().collect();
        all.sort_by_key(|p| p.id.distance(target));
        all.truncate(k);
        all
    }
}

/// Minimal Kademlia client
pub struct KademliaClient {
    pub local_id: NodeId,
    routing_table: RoutingTable,
}

impl KademliaClient {
    pub fn new(local_id: NodeId) -> Self {
        let routing_table = RoutingTable::new(local_id.clone());
        Self { local_id, routing_table }
    }
    pub fn add_peer(&self, peer: PeerInfo) {
        self.routing_table.insert(peer);
    }
    pub fn lookup(&self, target: &NodeId) -> Vec<PeerInfo> {
        self.routing_table.find_closest(target, 20)
    }
}

/// High-level DHT service (used by CCEK concurrency patterns)
pub struct DHTService {
    client: KademliaClient,
}

impl DHTService {
    pub fn new(local_id: NodeId) -> Self {
        Self { client: KademliaClient::new(local_id) }
    }
    pub fn add_peer(&self, peer: PeerInfo) {
        self.client.add_peer(peer);
    }
    pub fn lookup(&self, target: &NodeId) -> Vec<PeerInfo> {
        self.client.lookup(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn insert_and_find() {
        let local = NodeId::from_slice(b"local");
        let svc = DHTService::new(local.clone());
        let peer = PeerInfo { id: NodeId::from_slice(b"peer1"), addr: "127.0.0.1:4001".into() };
        svc.add_peer(peer);
        let results = svc.lookup(&NodeId::from_slice(b"peer1"));
        assert!(!results.is_empty());
    }
}
