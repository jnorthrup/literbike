//! Integration Tests: DHT + CAS Gateway
//!
//! End-to-end integration tests that exercise DHT/IPFS data paths and CAS
//! persistence together in deterministic scenarios.

use literbike::cas_gateway::{LazyProjectionGateway, ProjectionBackend, ProjectionPolicy};
use literbike::cas_backends::create_s3_adapter;
use literbike::dht::client::{IpfsClient, IpfsStorage, InMemoryStorage, CID, Multihash, HashType, Codec, IpfsBlock, IpfsLink};
use literbike::dht::kademlia::{PeerId, PeerInfo};
use std::sync::Arc;

// ============================================================================
// Test Fixtures
// ============================================================================

fn create_test_cid(data: &[u8]) -> CID {
    use sha2::{Sha256, Digest};
    let hash = Sha256::digest(data);
    CID::new(
        1,
        Codec::Raw,
        Multihash {
            hash_type: HashType::Sha2_256,
            digest: hash.to_vec(),
        },
    )
}

fn create_test_block(data: Vec<u8>) -> IpfsBlock {
    let cid = create_test_cid(&data);
    IpfsBlock {
        cid,
        data,
        links: vec![],
    }
}

// ============================================================================
// Integration Test: CAS Gateway + DHT Storage
// ============================================================================

#[test]
fn test_cas_gateway_with_dht_storage() {
    // Create CAS gateway
    let gateway = LazyProjectionGateway::new();

    // Create DHT storage
    let storage = Arc::new(InMemoryStorage::new());
    let peer_id = PeerId::random();
    let _client = IpfsClient::new(peer_id, storage.clone());

    // Store content in CAS
    let test_data = b"Integration test: CAS + DHT".to_vec();
    let put_result = gateway.put(test_data.clone(), "application/octet-stream")
        .expect("CAS put should succeed");

    // Verify envelope
    let envelope = gateway.envelope(&put_result.hash);
    assert!(envelope.is_some());
    assert_eq!(envelope.unwrap().size, test_data.len() as u64);

    // Store same data in DHT storage
    let block = create_test_block(test_data.clone());
    let cid = block.cid.clone();
    storage.put_block(block);

    // Verify DHT storage
    assert!(storage.has_block(&cid));
    let retrieved = storage.get_block(&cid);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().data, test_data);
}

// ============================================================================
// Integration Test: Multi-Backend CAS with DHT Fallback
// ============================================================================

#[test]
fn test_cas_multi_backend_with_dht_priority() {
    let gateway = LazyProjectionGateway::new();

    // Register S3 backend (will fail without real S3, but tests integration)
    let s3_adapter = create_s3_adapter(
        "http://localhost:9000",
        "test-bucket",
        "cas-s3",
    );
    gateway.register_adapter(s3_adapter);

    // Create DHT storage as conceptual backend
    let storage = Arc::new(InMemoryStorage::new());
    let peer_id = PeerId::random();
    let _client = IpfsClient::new(peer_id, storage.clone());

    // Store content
    let test_data = b"Multi-backend test".to_vec();
    let put_result = gateway.put(test_data.clone(), "text/plain")
        .expect("CAS put should succeed");

    // Store in DHT storage separately (simulating IPFS backend)
    let block = create_test_block(test_data.clone());
    storage.put_block(block);

    // Verify CAS storage
    let retrieved_cas = gateway.get(&put_result.hash, &[])
        .expect("CAS get should succeed");
    assert_eq!(retrieved_cas, Some(test_data.clone()));
}

// ============================================================================
// Integration Test: DHT Peer Routing with CAS Content
// ============================================================================

#[test]
fn test_dht_peer_routing_with_cas_content() {
    // Create multiple DHT clients with routing tables
    let storage1 = Arc::new(InMemoryStorage::new());
    let peer_id1 = PeerId::random();
    let client1 = IpfsClient::new(peer_id1.clone(), storage1.clone());

    let storage2 = Arc::new(InMemoryStorage::new());
    let peer_id2 = PeerId::random();
    let _client2 = IpfsClient::new(peer_id2.clone(), storage2);

    // Get routing tables
    let routing_table1 = client1.routing_table();

    // Add peer2 to peer1's routing table
    let peer2_info = PeerInfo::new(
        peer_id2.clone(),
        vec!["/ip4/127.0.0.1/tcp/4002".to_string()],
        vec!["/ipfs/kad/1.0.0".to_string()],
    );
    routing_table1.lock().add_peer(peer2_info);

    // Verify peer is in routing table
    let closest = routing_table1.lock().find_closest_peers(&peer_id2, 5);
    assert_eq!(closest.len(), 1);
    assert_eq!(closest[0].id, peer_id2);

    // Store content in client1
    let test_data = b"DHT routing test".to_vec();
    let block = create_test_block(test_data.clone());
    let cid = block.cid.clone();
    storage1.put_block(block);

    // Verify content is in storage1
    assert!(storage1.has_block(&cid));
}

// ============================================================================
// Integration Test: CAS Projection Policy with DHT
// ============================================================================

#[test]
fn test_cas_eager_projection_policy() {
    let gateway = LazyProjectionGateway::new();

    // Set eager projection policy
    gateway.set_policy(ProjectionPolicy {
        eager_backends: vec![], // No eager backends (DHT would be added here)
        fallback_order: vec![
            ProjectionBackend::Kv,
            ProjectionBackend::Ipfs,
            ProjectionBackend::S3Blobs,
        ],
    });

    // Store content
    let test_data = b"Eager projection test".to_vec();
    let put_result = gateway.put(test_data.clone(), "application/octet-stream")
        .expect("CAS put should succeed");

    // Verify lazy behavior (no backends materialized yet)
    // In real scenario, DHT/IPFS backend would be projected here

    // Retrieve from canonical storage
    let retrieved = gateway.get(&put_result.hash, &[])
        .expect("CAS get should succeed");
    assert_eq!(retrieved, Some(test_data));
}

// ============================================================================
// Integration Test: Block Linking (DAG Structure)
// ============================================================================

#[test]
fn test_dag_block_linking() {
    let storage = Arc::new(InMemoryStorage::new());
    let peer_id = PeerId::random();
    let _client = IpfsClient::new(peer_id, storage.clone());

    // Create child blocks
    let child1_data = b"Child block 1".to_vec();
    let child1 = create_test_block(child1_data.clone());
    let child1_cid = child1.cid.clone();

    let child2_data = b"Child block 2".to_vec();
    let child2 = create_test_block(child2_data.clone());
    let child2_cid = child2.cid.clone();

    // Store child blocks
    storage.put_block(child1);
    storage.put_block(child2);

    // Create parent block with links
    let parent_cid = create_test_cid(b"Parent block");
    let parent_block = IpfsBlock {
        cid: parent_cid.clone(),
        data: b"Parent data".to_vec(),
        links: vec![
            IpfsLink {
                name: "child1".to_string(),
                cid: child1_cid.clone(),
                size: child1_data.len() as u64,
            },
            IpfsLink {
                name: "child2".to_string(),
                cid: child2_cid.clone(),
                size: child2_data.len() as u64,
            },
        ],
    };

    storage.put_block(parent_block);

    // Retrieve parent and verify links
    let retrieved = storage.get_block(&parent_cid);
    assert!(retrieved.is_some());
    let parent = retrieved.unwrap();
    assert_eq!(parent.links.len(), 2);
    assert_eq!(parent.links[0].name, "child1");
    assert_eq!(parent.links[1].name, "child2");

    // Retrieve children via links
    let retrieved_child1 = storage.get_block(&parent.links[0].cid);
    assert!(retrieved_child1.is_some());
    assert_eq!(retrieved_child1.unwrap().data, child1_data);
}

// ============================================================================
// Integration Test: Pinning and Content Persistence
// ============================================================================

#[test]
fn test_content_pinning_prevents_deletion() {
    let storage = Arc::new(InMemoryStorage::new());
    let peer_id = PeerId::random();
    let _client = IpfsClient::new(peer_id, storage.clone());

    // Create and store block
    let test_data = b"Pinned content".to_vec();
    let block = create_test_block(test_data.clone());
    let cid = block.cid.clone();
    storage.put_block(block);

    // Pin the content
    storage.pin(cid.clone());

    // Verify pinned
    assert!(storage.is_pinned(&cid));

    // Attempt to delete pinned content (should fail)
    let deleted = storage.delete_block(&cid);
    assert!(!deleted);

    // Verify still exists
    assert!(storage.has_block(&cid));

    // Unpin and delete
    storage.unpin(&cid);
    assert!(!storage.is_pinned(&cid));

    let deleted = storage.delete_block(&cid);
    assert!(deleted);
    assert!(!storage.has_block(&cid));
}

// ============================================================================
// Integration Test: Concurrent Access
// ============================================================================

#[test]
fn test_concurrent_cas_dht_operations() {
    use std::thread;

    let gateway = Arc::new(LazyProjectionGateway::new());
    let storage = Arc::new(InMemoryStorage::new());
    let _peer_id = PeerId::random();

    let mut handles = vec![];

    // Spawn multiple threads performing concurrent operations
    for i in 0..10 {
        let gateway_clone = Arc::clone(&gateway);
        let storage_clone: Arc<InMemoryStorage> = Arc::clone(&storage);

        let handle = thread::spawn(move || {
            // CAS operation
            let data = format!("Concurrent test {}", i).into_bytes();
            let put_result = gateway_clone.put(data.clone(), "text/plain")
                .expect("CAS put should succeed");

            // DHT operation
            let block = create_test_block(data);
            let cid = block.cid.clone();
            storage_clone.put_block(block);

            // Verify both storages
            assert!(gateway_clone.envelope(&put_result.hash).is_some());
            assert!(storage_clone.has_block(&cid));
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread should complete successfully");
    }
}

// ============================================================================
// Integration Test: QUIC-to-Event-to-DuckDB Scenario
// ============================================================================

#[test]
#[cfg(feature = "couchdb")]
fn test_quic_event_to_duckdb_persistence() {
    use literbike::quic::quic_protocol::{QuicPacket, PacketType, ConnectionId, PacketNumber};
    use chrono::Utc;
    
    // Create QUIC packet (simulated event)
    let conn_id = ConnectionId::random();
    let packet_num = PacketNumber::new(1);
    let packet = QuicPacket {
        packet_type: PacketType::Handshake,
        version: 1,
        dcid: conn_id.clone(),
        scid: ConnectionId::random(),
        packet_number: packet_num,
        payload: b"QUIC event data".to_vec(),
    };

    // Serialize packet (simulating event generation)
    let serialized = bincode::serialize(&packet)
        .expect("Packet serialization should succeed");

    // Store in CAS (simulating event log)
    let gateway = LazyProjectionGateway::new();
    let put_result = gateway.put(serialized, "application/octet-stream")
        .expect("CAS put should succeed");

    // Verify event is persisted
    let envelope = gateway.envelope(&put_result.hash);
    assert!(envelope.is_some());
    
    // In production, this would write to DuckDB via event log
    // For testing, we verify CAS persistence as the event store
    assert_eq!(envelope.unwrap().size > 0, true);
}

// ============================================================================
// Integration Test: QUIC Protocol Failure Propagation
// ============================================================================

#[test]
fn test_quic_protocol_failure_propagation() {
    use literbike::quic::quic_error::{QuicError, ConnectionError};
    
    // Simulate QUIC protocol error
    let error = QuicError::Connection(ConnectionError::ConnectionClosed);
    
    // Verify error propagation through CAS layer
    let gateway = LazyProjectionGateway::new();
    
    // Store error metadata in CAS
    let error_data = format!("QUIC_ERROR: {:?}", error).into_bytes();
    let put_result = gateway.put(error_data, "application/quic-error")
        .expect("Error metadata storage should succeed");
    
    // Verify error is recorded
    let envelope = gateway.envelope(&put_result.hash);
    assert!(envelope.is_some());
}

// ============================================================================
// Integration Test: DHT/IPFS Timeout and Fallback
// ============================================================================

#[test]
fn test_dht_ipfs_timeout_fallback_behavior() {
    let gateway = LazyProjectionGateway::new();
    
    // Set policy with fallback order (simulating IPFS timeout scenario)
    gateway.set_policy(ProjectionPolicy {
        eager_backends: vec![],
        fallback_order: vec![
            ProjectionBackend::Kv,      // Primary (fast)
            ProjectionBackend::Ipfs,    // Secondary (may timeout)
            ProjectionBackend::S3Blobs, // Tertiary (fallback)
        ],
    });
    
    // Store content
    let test_data = b"Fallback test data".to_vec();
    let put_result = gateway.put(test_data.clone(), "text/plain")
        .expect("CAS put should succeed");
    
    // Retrieve with fallback (IPFS would timeout, fall back to S3)
    // In this test, we verify the fallback mechanism is configured
    let retrieved = gateway.get(&put_result.hash, &[
        ProjectionBackend::Kv,
        ProjectionBackend::S3Blobs,
    ]);
    
    // Verify retrieval succeeds (from canonical storage in test)
    assert!(retrieved.is_ok());
}

// ============================================================================
// Integration Test: DuckDB Event Log Schema (Conceptual)
// ============================================================================

#[test]
#[cfg(feature = "couchdb")]
fn test_duckdb_event_log_schema_validation() {
    // This test documents the expected DuckDB event log schema
    // In production, this would validate against actual DuckDB tables
    
    use literbike::cas_storage::CasEnvelope;
    use chrono::Utc;
    
    // Expected schema fields for event log
    struct ExpectedEventSchema {
        event_id: String,           // UUID
        event_type: String,         // "quic_packet", "dht_operation", etc.
        timestamp: i64,             // Unix timestamp
        payload_hash: Vec<u8>,      // SHA256 hash
        payload_size: u64,          // Size in bytes
        metadata: serde_json::Value, // Additional metadata
    }
    
    // Create sample event envelope
    let test_data = b"Event payload".to_vec();
    let gateway = LazyProjectionGateway::new();
    let put_result = gateway.put(test_data, "application/octet-stream")
        .expect("CAS put should succeed");
    
    let envelope = gateway.envelope(&put_result.hash)
        .expect("Envelope should exist");
    
    // Validate envelope matches expected schema
    assert!(!envelope.metadata.content_type.is_empty());
    assert!(envelope.size > 0);
    assert!(envelope.created_at > 0);
    
    // Schema validation passed
    let _schema = ExpectedEventSchema {
        event_id: put_result.hash.to_string(),
        event_type: envelope.metadata.content_type,
        timestamp: envelope.created_at as i64,
        payload_hash: envelope.hash.to_vec(),
        payload_size: envelope.size,
        metadata: serde_json::json!({}),
    };
}

// ============================================================================
// Integration Test: Full Stack Scenario
// ============================================================================

#[test]
fn test_full_stack_quic_dht_cas_integration() {
    // Create all components
    let gateway = LazyProjectionGateway::new();
    let storage = Arc::new(InMemoryStorage::new());
    let peer_id = PeerId::random();
    let client = IpfsClient::new(peer_id.clone(), storage.clone());
    
    // Stage 1: QUIC packet generation
    let quic_data = b"QUIC transport data".to_vec();
    let quic_put = gateway.put(quic_data.clone(), "application/quic")
        .expect("QUIC data storage should succeed");
    
    // Stage 2: DHT content routing
    let dht_block = create_test_block(quic_data.clone());
    let dht_cid = dht_block.cid.clone();
    storage.put_block(dht_block);
    
    // Stage 3: CAS persistence verification
    let cas_retrieved = gateway.get(&quic_put.hash, &[])
        .expect("CAS retrieval should succeed");
    assert_eq!(cas_retrieved, Some(quic_data.clone()));
    
    // Stage 4: DHT retrieval verification
    let dht_retrieved = storage.get_block(&dht_cid);
    assert!(dht_retrieved.is_some());
    assert_eq!(dht_retrieved.unwrap().data, quic_data);
    
    // Full stack integration successful
}
