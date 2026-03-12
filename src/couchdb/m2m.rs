use crate::couchdb::{
    types::{M2mMessage, M2mMessageType},
    error::{CouchError, CouchResult},
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};
use tokio::sync::{mpsc, broadcast, Mutex};
use tokio::time::{interval, timeout, Duration as TokioDuration};
use log::{info, warn, error, debug};
use serde::{Deserialize, Serialize};

/// M2M communication manager for inter-node messaging
pub struct M2mManager {
    node_id: String,
    peers: Arc<RwLock<HashMap<String, PeerInfo>>>,
    message_handlers: Arc<RwLock<HashMap<M2mMessageType, Box<dyn MessageHandler + Send + Sync>>>>,
    message_queue: Arc<Mutex<Vec<M2mMessage>>>,
    broadcast_sender: broadcast::Sender<M2mMessage>,
    config: M2mConfig,
    metrics: Arc<RwLock<M2mMetrics>>,
}

/// Peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: String,
    pub address: String,
    pub last_seen: DateTime<Utc>,
    pub capabilities: Vec<String>,
    pub status: PeerStatus,
    pub latency_ms: Option<u64>,
    pub message_count: u64,
}

/// Peer status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerStatus {
    Connected,
    Disconnected,
    Connecting,
    Error(String),
}

/// M2M configuration
#[derive(Debug, Clone)]
pub struct M2mConfig {
    pub heartbeat_interval_secs: u64,
    pub message_ttl_secs: u64,
    pub max_queue_size: usize,
    pub max_peers: usize,
    pub discovery_enabled: bool,
    pub discovery_port: u16,
    pub encryption_enabled: bool,
}

impl Default for M2mConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_secs: 30,
            message_ttl_secs: 300, // 5 minutes
            max_queue_size: 1000,
            max_peers: 100,
            discovery_enabled: true,
            discovery_port: 8889,
            encryption_enabled: false, // Simplified for demo
        }
    }
}

/// Message handler trait
pub trait MessageHandler {
    fn handle(&self, message: &M2mMessage) -> CouchResult<Option<M2mMessage>>;
    fn message_type(&self) -> M2mMessageType;
}

/// M2M metrics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct M2mMetrics {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_dropped: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub active_peers: usize,
    pub queue_size: usize,
    pub uptime_seconds: u64,
}

impl M2mManager {
    /// Create a new M2M manager
    pub fn new(node_id: Option<String>, config: M2mConfig) -> Self {
        let node_id = node_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let (broadcast_sender, _) = broadcast::channel(1000);
        
        Self {
            node_id,
            peers: Arc::new(RwLock::new(HashMap::new())),
            message_handlers: Arc::new(RwLock::new(HashMap::new())),
            message_queue: Arc::new(Mutex::new(Vec::new())),
            broadcast_sender,
            config,
            metrics: Arc::new(RwLock::new(M2mMetrics::default())),
        }
    }
    
    /// Register a message handler
    pub fn register_handler<H>(&self, handler: H) -> CouchResult<()>
    where
        H: MessageHandler + Send + Sync + 'static,
    {
        let message_type = handler.message_type();
        info!("Registered handler for message type: {:?}", message_type);
        let mut handlers = self.message_handlers.write().unwrap();
        handlers.insert(message_type, Box::new(handler));
        Ok(())
    }
    
    /// Send message to a specific peer
    pub async fn send_message(&self, recipient: &str, message_type: M2mMessageType, payload: serde_json::Value) -> CouchResult<()> {
        let message = M2mMessage {
            id: Uuid::new_v4(),
            sender: self.node_id.clone(),
            recipient: Some(recipient.to_string()),
            message_type,
            payload,
            timestamp: Utc::now(),
            ttl: Some(self.config.message_ttl_secs),
        };
        
        self.queue_message(message).await
    }
    
    /// Broadcast message to all peers
    pub async fn broadcast_message(&self, message_type: M2mMessageType, payload: serde_json::Value) -> CouchResult<()> {
        let message = M2mMessage {
            id: Uuid::new_v4(),
            sender: self.node_id.clone(),
            recipient: None, // None indicates broadcast
            message_type,
            payload,
            timestamp: Utc::now(),
            ttl: Some(self.config.message_ttl_secs),
        };
        
        self.queue_message(message).await
    }
    
    /// Queue message for sending
    async fn queue_message(&self, message: M2mMessage) -> CouchResult<()> {
        let mut queue = self.message_queue.lock().await;
        
        if queue.len() >= self.config.max_queue_size {
            // Remove oldest message
            queue.remove(0);
            
            let mut metrics = self.metrics.write().unwrap();
            metrics.messages_dropped += 1;
            
            warn!("Message queue full, dropped oldest message");
        }
        
        queue.push(message.clone());
        
        // Update metrics
        {
            let mut metrics = self.metrics.write().unwrap();
            metrics.messages_sent += 1;
            metrics.queue_size = queue.len();
        }
        
        // Broadcast to local subscribers
        if let Err(e) = self.broadcast_sender.send(message) {
            debug!("No local subscribers for message: {}", e);
        }
        
        Ok(())
    }
    
    /// Process incoming message
    pub async fn process_message(&self, message: M2mMessage) -> CouchResult<()> {
        debug!("Processing message: {} from {}", message.id, message.sender);
        
        // Check if message has expired
        if let Some(ttl) = message.ttl {
            let age = Utc::now() - message.timestamp;
            if age.num_seconds() > ttl as i64 {
                debug!("Message expired: {}", message.id);
                return Ok(());
            }
        }
        
        // Update metrics
        {
            let mut metrics = self.metrics.write().unwrap();
            metrics.messages_received += 1;
        }
        
        // Find and execute handler
        let handlers = self.message_handlers.read().unwrap();
        if let Some(handler) = handlers.get(&message.message_type) {
            match handler.handle(&message) {
                Ok(Some(response)) => {
                    // Send response back
                    self.queue_message(response).await?;
                }
                Ok(None) => {
                    // No response needed
                }
                Err(e) => {
                    error!("Handler error for message {}: {}", message.id, e);
                }
            }
        } else {
            warn!("No handler for message type: {:?}", message.message_type);
        }
        
        Ok(())
    }
    
    /// Add or update peer
    pub fn add_peer(&self, peer: PeerInfo) -> CouchResult<()> {
        let mut peers = self.peers.write().unwrap();
        
        if peers.len() >= self.config.max_peers {
            return Err(CouchError::bad_request("Maximum peers exceeded"));
        }
        
        peers.insert(peer.id.clone(), peer);
        
        // Update metrics
        let mut metrics = self.metrics.write().unwrap();
        metrics.active_peers = peers.len();
        
        info!("Added peer: {}", peers.len());
        Ok(())
    }
    
    /// Remove peer
    pub fn remove_peer(&self, peer_id: &str) -> bool {
        let mut peers = self.peers.write().unwrap();
        let removed = peers.remove(peer_id).is_some();
        
        if removed {
            let mut metrics = self.metrics.write().unwrap();
            metrics.active_peers = peers.len();
            info!("Removed peer: {}", peer_id);
        }
        
        removed
    }
    
    /// Get peer information
    pub fn get_peer(&self, peer_id: &str) -> Option<PeerInfo> {
        let peers = self.peers.read().unwrap();
        peers.get(peer_id).cloned()
    }
    
    /// List all peers
    pub fn list_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().unwrap();
        peers.values().cloned().collect()
    }
    
    /// Update peer status
    pub fn update_peer_status(&self, peer_id: &str, status: PeerStatus) {
        let mut peers = self.peers.write().unwrap();
        if let Some(peer) = peers.get_mut(peer_id) {
            peer.status = status;
            peer.last_seen = Utc::now();
        }
    }
    
    /// Get M2M statistics
    pub fn get_metrics(&self) -> M2mMetrics {
        let metrics = self.metrics.read().unwrap();
        metrics.clone()
    }
    
    /// Subscribe to messages
    pub fn subscribe(&self) -> broadcast::Receiver<M2mMessage> {
        self.broadcast_sender.subscribe()
    }
    
    /// Get the node ID
    pub fn get_node_id(&self) -> String {
        self.node_id.clone()
    }
    
    /// Start background services
    pub async fn start_services(&self) -> CouchResult<Vec<tokio::task::JoinHandle<()>>> {
        let mut handles = Vec::new();
        
        // Start heartbeat service
        handles.push(self.start_heartbeat_service());
        
        // Start message processing service
        handles.push(self.start_message_processor());
        
        // Start peer discovery if enabled
        if self.config.discovery_enabled {
            handles.push(self.start_peer_discovery());
        }
        
        // Start cleanup service
        handles.push(self.start_cleanup_service());
        
        info!("Started {} M2M background services", handles.len());
        Ok(handles)
    }
    
    /// Start heartbeat service
    fn start_heartbeat_service(&self) -> tokio::task::JoinHandle<()> {
        let node_id = self.node_id.clone();
        let broadcast_sender = self.broadcast_sender.clone();
        let interval_secs = self.config.heartbeat_interval_secs;
        
        tokio::spawn(async move {
            let mut interval = interval(TokioDuration::from_secs(interval_secs));
            
            loop {
                interval.tick().await;
                
                let heartbeat = M2mMessage {
                    id: Uuid::new_v4(),
                    sender: node_id.clone(),
                    recipient: None, // Broadcast
                    message_type: M2mMessageType::HeartBeat,
                    payload: serde_json::json!({
                        "timestamp": Utc::now(),
                        "capabilities": ["couchdb", "ipfs", "tensor"]
                    }),
                    timestamp: Utc::now(),
                    ttl: Some(interval_secs * 3), // 3x heartbeat interval
                };
                
                if let Err(e) = broadcast_sender.send(heartbeat) {
                    debug!("Failed to send heartbeat: {}", e);
                }
            }
        })
    }
    
    /// Start message processor
    fn start_message_processor(&self) -> tokio::task::JoinHandle<()> {
        let message_queue = Arc::clone(&self.message_queue);
        let peers = Arc::clone(&self.peers);
        let metrics = Arc::clone(&self.metrics);
        
        tokio::spawn(async move {
            let mut interval = interval(TokioDuration::from_millis(100)); // Process every 100ms
            
            loop {
                interval.tick().await;
                
                let messages_to_send = {
                    let mut queue = message_queue.lock().await;
                    let messages: Vec<M2mMessage> = queue.drain(..).collect();
                    messages
                };
                
                for message in messages_to_send {
                    // In a real implementation, this would send messages over the network
                    debug!("Would send message: {} to {:?}", message.id, message.recipient);
                    
                    // Update metrics
                    let mut metrics = metrics.write().unwrap();
                    metrics.bytes_sent += serde_json::to_vec(&message).unwrap_or_default().len() as u64;
                }
            }
        })
    }
    
    /// Start peer discovery service
    fn start_peer_discovery(&self) -> tokio::task::JoinHandle<()> {
        let node_id = self.node_id.clone();
        let peers = Arc::clone(&self.peers);
        let discovery_port = self.config.discovery_port;
        
        tokio::spawn(async move {
            let mut interval = interval(TokioDuration::from_secs(60)); // Discover every minute
            
            loop {
                interval.tick().await;
                
                // In a real implementation, this would do UDP multicast discovery
                debug!("Peer discovery scan (port {})", discovery_port);
                
                // Simulate discovering a peer
                let discovered_peer = PeerInfo {
                    id: format!("peer-{}", Uuid::new_v4()),
                    address: format!("127.0.0.1:{}", discovery_port),
                    last_seen: Utc::now(),
                    capabilities: vec!["couchdb".to_string()],
                    status: PeerStatus::Connected,
                    latency_ms: Some(10),
                    message_count: 0,
                };
                
                let mut peers_map = peers.write().unwrap();
                if peers_map.len() < 10 { // Limit for demo
                    peers_map.insert(discovered_peer.id.clone(), discovered_peer);
                }
            }
        })
    }
    
    /// Start cleanup service
    fn start_cleanup_service(&self) -> tokio::task::JoinHandle<()> {
        let peers = Arc::clone(&self.peers);
        let metrics = Arc::clone(&self.metrics);
        
        tokio::spawn(async move {
            let mut interval = interval(TokioDuration::from_secs(300)); // Cleanup every 5 minutes
            
            loop {
                interval.tick().await;
                
                let cutoff = Utc::now() - Duration::minutes(10);
                let mut peers_map = peers.write().unwrap();
                
                let before_count = peers_map.len();
                peers_map.retain(|_, peer| peer.last_seen > cutoff);
                let after_count = peers_map.len();
                
                if before_count != after_count {
                    info!("Cleaned up {} stale peers", before_count - after_count);
                }
                
                // Update metrics
                let mut metrics = metrics.write().unwrap();
                metrics.active_peers = after_count;
            }
        })
    }
}

/// Default handlers for common message types
pub struct HeartbeatHandler {
    node_id: String,
}

impl HeartbeatHandler {
    pub fn new(node_id: String) -> Self {
        Self { node_id }
    }
}

impl MessageHandler for HeartbeatHandler {
    fn handle(&self, message: &M2mMessage) -> CouchResult<Option<M2mMessage>> {
        debug!("Received heartbeat from: {}", message.sender);
        
        // Respond with our own heartbeat if requested
        if message.payload.get("respond").and_then(|v| v.as_bool()).unwrap_or(false) {
            let response = M2mMessage {
                id: Uuid::new_v4(),
                sender: self.node_id.clone(),
                recipient: Some(message.sender.clone()),
                message_type: M2mMessageType::HeartBeat,
                payload: serde_json::json!({
                    "timestamp": Utc::now(),
                    "in_response_to": message.id
                }),
                timestamp: Utc::now(),
                ttl: Some(60),
            };
            
            Ok(Some(response))
        } else {
            Ok(None)
        }
    }
    
    fn message_type(&self) -> M2mMessageType {
        M2mMessageType::HeartBeat
    }
}

/// Replication message handler
pub struct ReplicationHandler {
    node_id: String,
}

impl ReplicationHandler {
    pub fn new(node_id: String) -> Self {
        Self { node_id }
    }
}

impl MessageHandler for ReplicationHandler {
    fn handle(&self, message: &M2mMessage) -> CouchResult<Option<M2mMessage>> {
        info!("Received replication message from: {}", message.sender);
        
        // Extract replication request details
        if let Some(db_name) = message.payload.get("database").and_then(|v| v.as_str()) {
            info!("Replication request for database: {}", db_name);
            
            // In a real implementation, this would trigger database replication
            let response = M2mMessage {
                id: Uuid::new_v4(),
                sender: self.node_id.clone(),
                recipient: Some(message.sender.clone()),
                message_type: M2mMessageType::Replication,
                payload: serde_json::json!({
                    "status": "accepted",
                    "database": db_name,
                    "in_response_to": message.id
                }),
                timestamp: Utc::now(),
                ttl: Some(300),
            };
            
            Ok(Some(response))
        } else {
            Err(CouchError::bad_request("Invalid replication message format"))
        }
    }
    
    fn message_type(&self) -> M2mMessageType {
        M2mMessageType::Replication
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_m2m_manager_creation() {
        let config = M2mConfig::default();
        let manager = M2mManager::new(Some("test-node".to_string()), config);
        
        assert_eq!(manager.node_id, "test-node");
        assert_eq!(manager.list_peers().len(), 0);
    }
    
    #[tokio::test]
    async fn test_peer_management() {
        let config = M2mConfig::default();
        let manager = M2mManager::new(None, config);
        
        let peer = PeerInfo {
            id: "peer1".to_string(),
            address: "127.0.0.1:8888".to_string(),
            last_seen: Utc::now(),
            capabilities: vec!["couchdb".to_string()],
            status: PeerStatus::Connected,
            latency_ms: Some(5),
            message_count: 0,
        };
        
        manager.add_peer(peer.clone()).unwrap();
        assert_eq!(manager.list_peers().len(), 1);
        
        let retrieved = manager.get_peer("peer1").unwrap();
        assert_eq!(retrieved.id, "peer1");
        
        assert!(manager.remove_peer("peer1"));
        assert_eq!(manager.list_peers().len(), 0);
    }
    
    #[tokio::test]
    async fn test_message_handling() {
        let config = M2mConfig::default();
        let manager = M2mManager::new(Some("test-node".to_string()), config);
        
        let handler = HeartbeatHandler::new("test-node".to_string());
        manager.register_handler(handler).unwrap();
        
        let message = M2mMessage {
            id: Uuid::new_v4(),
            sender: "other-node".to_string(),
            recipient: Some("test-node".to_string()),
            message_type: M2mMessageType::HeartBeat,
            payload: serde_json::json!({"respond": true}),
            timestamp: Utc::now(),
            ttl: Some(60),
        };
        
        manager.process_message(message).await.unwrap();
        
        let metrics = manager.get_metrics();
        assert_eq!(metrics.messages_received, 1);
    }
    
    #[test]
    fn test_peer_status() {
        let status = PeerStatus::Connected;
        let serialized = serde_json::to_string(&status).unwrap();
        assert_eq!(serialized, "\"connected\"");
        
        let deserialized: PeerStatus = serde_json::from_str(&serialized).unwrap();
        matches!(deserialized, PeerStatus::Connected);
    }
}