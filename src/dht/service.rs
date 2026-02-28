//! DHT Service implementation for CCEK integration
//! 
//! Provides a ContextElement-compliant service for DHT operations

use std::any::Any;
use std::sync::Arc;
use parking_lot::RwLock;
use super::kademlia::{PeerId, PeerInfo, RoutingTable};
use crate::concurrency::ccek::ContextElement;

pub const DHT_SERVICE_KEY: &str = "dht_service";

/// Interface for DHT persistence (e.g. DuckDB)
pub trait DhtPersistence: Send + Sync {
    /// Save or update a peer in persistent storage
    fn upsert_node(&self, peer: &PeerInfo);
    
    /// Load all known peers from persistent storage
    fn load_nodes(&self) -> Vec<PeerInfo>;

    /// Save or update a DHT value
    fn upsert_value(&self, key: &str, value: &[u8]);
}

/// DHT Service interface for higher-level flows
pub struct DhtService {
    routing_table: Arc<RwLock<RoutingTable>>,
    persistence: Option<Arc<dyn DhtPersistence>>,
}

impl DhtService {
    pub fn new(local_peer_id: PeerId) -> Self {
        Self::new_with_persistence(local_peer_id, None)
    }

    pub fn new_with_persistence(local_peer_id: PeerId, persistence: Option<Arc<dyn DhtPersistence>>) -> Self {
        // bucket_size = 20 for standard Kademlia
        let routing_table = RoutingTable::new(local_peer_id, 20);
        let service = Self {
            routing_table: Arc::new(RwLock::new(routing_table)),
            persistence,
        };
        
        // Rehydrate if persistence is available
        if service.persistence.is_some() {
            service.rehydrate();
        }
        
        service
    }

    /// Add a peer to the routing table and persist if available
    pub fn add_peer(&self, peer: PeerInfo) {
        if let Some(p) = &self.persistence {
            p.upsert_node(&peer);
        }
        self.routing_table.write().add_peer(peer)
    }

    /// Get a peer by ID
    pub fn get_peer(&self, peer_id: &PeerId) -> Option<PeerInfo> {
        self.routing_table.read().get_peer(peer_id).cloned()
    }

    /// Find closest peers to a given ID
    pub fn closest_peers(&self, peer_id: &PeerId, count: usize) -> Vec<PeerInfo> {
        self.routing_table.read().find_closest_peers(peer_id, count)
    }

    /// Reload routing table from persistence
    pub fn rehydrate(&self) {
        if let Some(p) = &self.persistence {
            let nodes = p.load_nodes();
            let mut rt = self.routing_table.write();
            for node in nodes {
                rt.add_peer(node);
            }
        }
    }

    /// Update the persistence adapter and rehydrate
    pub fn set_persistence(&mut self, persistence: Arc<dyn DhtPersistence>) {
        self.persistence = Some(persistence);
        self.rehydrate();
    }
}

impl ContextElement for DhtService {
    fn key(&self) -> &'static str {
        DHT_SERVICE_KEY
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// Support for Clone if needed for ContextElement downcasting
impl Clone for DhtService {
    fn clone(&self) -> Self {
        Self {
            routing_table: Arc::clone(&self.routing_table),
            persistence: self.persistence.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::concurrency::ccek::{EmptyContext, CoroutineContext};
    use parking_lot::Mutex;

    struct MockPersistence {
        nodes: Mutex<Vec<PeerInfo>>,
    }

    impl MockPersistence {
        fn new() -> Self {
            Self { nodes: Mutex::new(Vec::new()) }
        }
    }

    impl DhtPersistence for MockPersistence {
        fn upsert_node(&self, peer: &PeerInfo) {
            self.nodes.lock().push(peer.clone());
        }
        fn load_nodes(&self) -> Vec<PeerInfo> {
            self.nodes.lock().clone()
        }
        fn upsert_value(&self, _key: &str, _value: &[u8]) {}
    }

    #[test]
    fn test_dht_service_context_composition() {
        let peer_id = PeerId::random();
        let service = DhtService::new(peer_id);
        
        // Compose context
        let ctx = CoroutineContext::new() + Arc::new(service);
        
        // Retrieve from context
        let retrieved = ctx.get(DHT_SERVICE_KEY).expect("should find service");
        assert_eq!(retrieved.key(), DHT_SERVICE_KEY);
        
        // Typed retrieval
        let typed = ctx.get_typed::<DhtService>(DHT_SERVICE_KEY).expect("should downcast");
        assert_eq!(typed.key(), DHT_SERVICE_KEY);
    }

    #[test]
    fn test_dht_service_persistence_auto_save() {
        let local_id = PeerId::random();
        let persistence = Arc::new(MockPersistence::new());
        let service = DhtService::new_with_persistence(local_id, Some(persistence.clone()));
        
        let remote_id = PeerId::random();
        let peer = PeerInfo::new(remote_id.clone(), vec!["/ip4/1.1.1.1/tcp/4001".to_string()], vec![]);
        
        service.add_peer(peer);
        
        // Verify mock recorded the save
        let saved = persistence.nodes.lock();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].id, remote_id);
    }

    #[test]
    fn test_dht_service_rehydrate() {
        let local_id = PeerId::random();
        let persistence = Arc::new(MockPersistence::new());
        
        // Pre-populate persistence
        let remote_id = PeerId::random();
        let peer = PeerInfo::new(remote_id.clone(), vec!["/ip4/1.1.1.1/tcp/4001".to_string()], vec![]);
        persistence.upsert_node(&peer);
        
        // New service should rehydrate automatically
        let service = DhtService::new_with_persistence(local_id, Some(persistence));
        let found = service.get_peer(&remote_id).expect("should have rehydrated peer");
        assert_eq!(found.id, remote_id);
    }
}
