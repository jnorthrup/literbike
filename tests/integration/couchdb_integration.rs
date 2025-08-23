use literbike::couchdb::{
    database::DatabaseManager,
    views::{ViewServer, ViewServerConfig},
    m2m::{M2mManager, M2mConfig},
    tensor::{TensorEngine, TensorConfig},
    types::*,
    error::CouchError,
};
use tempfile::tempdir;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn test_database_manager_lifecycle() {
    let temp_dir = tempdir().unwrap();
    let db_manager = DatabaseManager::new(temp_dir.path().to_str().unwrap()).unwrap();
    
    // Test database creation
    let db_info = db_manager.create_database("test_db").unwrap();
    assert_eq!(db_info.db_name, "test_db");
    assert_eq!(db_info.doc_count, 0);
    
    // Test database listing
    let all_dbs = db_manager.list_databases().unwrap();
    assert!(all_dbs.0.contains(&"test_db".to_string()));
    
    // Test database info
    let info = db_manager.get_database_info("test_db").unwrap();
    assert_eq!(info.db_name, "test_db");
    
    // Test database deletion
    let result = db_manager.delete_database("test_db").unwrap();
    assert_eq!(result["ok"], json!(true));
    
    // Verify database is gone
    assert!(db_manager.get_database_info("test_db").is_err());
}

#[tokio::test]
async fn test_document_operations() {
    let temp_dir = tempdir().unwrap();
    let db_manager = DatabaseManager::new(temp_dir.path().to_str().unwrap()).unwrap();
    
    // Create database
    db_manager.create_database("docs_test").unwrap();
    let db_instance = db_manager.get_database_clone("docs_test").unwrap();
    
    // Create test document
    let doc = Document {
        id: "test_doc_1".to_string(),
        rev: "".to_string(),
        deleted: None,
        attachments: None,
        data: json!({
            "type": "test",
            "name": "Test Document",
            "value": 42
        }),
    };
    
    // Test document creation
    let (id, rev) = db_instance.put_document(&doc).unwrap();
    assert_eq!(id, "test_doc_1");
    assert!(rev.starts_with("1-"));
    
    // Test document retrieval
    let retrieved_doc = db_instance.get_document("test_doc_1").unwrap();
    assert_eq!(retrieved_doc.id, "test_doc_1");
    assert_eq!(retrieved_doc.data["name"], json!("Test Document"));
    
    // Test document update
    let mut updated_doc = retrieved_doc;
    updated_doc.data = json!({
        "type": "test",
        "name": "Updated Document",
        "value": 100
    });
    
    let (_, new_rev) = db_instance.put_document(&updated_doc).unwrap();
    assert!(new_rev.starts_with("2-"));
    
    // Test document deletion
    let (_, del_rev) = db_instance.delete_document("test_doc_1", &new_rev).unwrap();
    assert!(del_rev.starts_with("3-"));
    
    // Verify document is deleted
    assert!(db_instance.get_document("test_doc_1").is_err());
}

#[tokio::test]
async fn test_view_server_operations() {
    let view_server = ViewServer::new(ViewServerConfig::default()).unwrap();
    
    // Test view server stats
    let stats = view_server.get_stats();
    assert_eq!(stats["total_views"], json!(0));
    assert_eq!(stats["javascript_enabled"], json!(true));
    
    // Test view caching
    view_server.clear_caches();
    let stats_after_clear = view_server.get_stats();
    assert_eq!(stats_after_clear["total_views"], json!(0));
}

#[tokio::test]
async fn test_m2m_manager() {
    let config = M2mConfig::default();
    let manager = M2mManager::new(Some("test_node".to_string()), config);
    
    // Test node ID
    assert_eq!(manager.get_node_id(), "test_node");
    
    // Test peer management
    let peer = crate::literbike::couchdb::m2m::PeerInfo {
        id: "peer1".to_string(),
        address: "127.0.0.1:8888".to_string(),
        last_seen: chrono::Utc::now(),
        capabilities: vec!["couchdb".to_string()],
        status: crate::literbike::couchdb::m2m::PeerStatus::Connected,
        latency_ms: Some(5),
        message_count: 0,
    };
    
    manager.add_peer(peer).unwrap();
    assert_eq!(manager.list_peers().len(), 1);
    
    let retrieved_peer = manager.get_peer("peer1").unwrap();
    assert_eq!(retrieved_peer.id, "peer1");
    
    assert!(manager.remove_peer("peer1"));
    assert_eq!(manager.list_peers().len(), 0);
    
    // Test metrics
    let metrics = manager.get_metrics();
    assert_eq!(metrics.active_peers, 0);
}

#[tokio::test]
async fn test_tensor_engine() {
    let engine = TensorEngine::new(TensorConfig::default());
    
    // Test engine stats
    let stats = engine.get_stats();
    assert_eq!(stats["total_operations"], json!(0));
    assert_eq!(stats["cached_tensors"], json!(0));
    
    // Test tensor document creation
    let doc = Document {
        id: "tensor_doc".to_string(),
        rev: "1-abc".to_string(),
        deleted: None,
        attachments: None,
        data: json!({
            "type": "tensor",
            "tensor": {
                "shape": [2, 2],
                "data": [1.0, 2.0, 3.0, 4.0],
                "metadata": {}
            }
        }),
    };
    
    let tensor = engine.load_tensor_from_document(&doc).unwrap();
    assert_eq!(tensor.id, "tensor_doc");
    assert_eq!(tensor.shape, vec![2, 2]);
    assert_eq!(tensor.data.len(), 4);
    
    // Test tensor storage
    let stored_doc = engine.store_tensor_as_document(&tensor, Some("new_tensor".to_string())).unwrap();
    assert_eq!(stored_doc.id, "new_tensor");
    assert!(stored_doc.data.get("tensor").is_some());
}

#[tokio::test]
async fn test_error_handling() {
    let temp_dir = tempdir().unwrap();
    let db_manager = DatabaseManager::new(temp_dir.path().to_str().unwrap()).unwrap();
    
    // Test non-existent database
    let result = db_manager.get_database_info("nonexistent");
    assert!(result.is_err());
    match result.unwrap_err() {
        CouchError { error, .. } => assert_eq!(error, "not_found"),
    }
    
    // Test duplicate database creation
    db_manager.create_database("test_dup").unwrap();
    let dup_result = db_manager.create_database("test_dup");
    assert!(dup_result.is_err());
    match dup_result.unwrap_err() {
        CouchError { error, .. } => assert_eq!(error, "conflict"),
    }
    
    // Test invalid database name
    let invalid_result = db_manager.create_database("Invalid_Name");
    assert!(invalid_result.is_err());
    match invalid_result.unwrap_err() {
        CouchError { error, .. } => assert_eq!(error, "bad_request"),
    }
}

#[tokio::test]
async fn test_bulk_operations() {
    let temp_dir = tempdir().unwrap();
    let db_manager = DatabaseManager::new(temp_dir.path().to_str().unwrap()).unwrap();
    
    // Create database
    db_manager.create_database("bulk_test").unwrap();
    let db_instance = db_manager.get_database_clone("bulk_test").unwrap();
    
    // Create multiple documents
    let docs = vec![
        Document {
            id: "bulk_1".to_string(),
            rev: "".to_string(),
            deleted: None,
            attachments: None,
            data: json!({"name": "Document 1", "value": 1}),
        },
        Document {
            id: "bulk_2".to_string(),
            rev: "".to_string(),
            deleted: None,
            attachments: None,
            data: json!({"name": "Document 2", "value": 2}),
        },
        Document {
            id: "bulk_3".to_string(),
            rev: "".to_string(),
            deleted: None,
            attachments: None,
            data: json!({"name": "Document 3", "value": 3}),
        },
    ];
    
    // Store documents individually for testing
    for doc in docs {
        db_instance.put_document(&doc).unwrap();
    }
    
    // Test all docs retrieval
    let query = ViewQuery::default();
    let all_docs = db_instance.get_all_documents(&query).unwrap();
    assert_eq!(all_docs.rows.len(), 3);
    assert_eq!(all_docs.total_rows, 3);
}

#[tokio::test]
async fn test_concurrent_operations() {
    let temp_dir = tempdir().unwrap();
    let db_manager = Arc::new(DatabaseManager::new(temp_dir.path().to_str().unwrap()).unwrap());
    
    // Create database
    db_manager.create_database("concurrent_test").unwrap();
    
    // Spawn multiple tasks to test concurrent access
    let mut handles = vec![];
    
    for i in 0..10 {
        let db_manager_clone = Arc::clone(&db_manager);
        let handle = tokio::spawn(async move {
            let db_instance = db_manager_clone.get_database_clone("concurrent_test").unwrap();
            
            let doc = Document {
                id: format!("concurrent_doc_{}", i),
                rev: "".to_string(),
                deleted: None,
                attachments: None,
                data: json!({"thread": i, "data": format!("Thread {}", i)}),
            };
            
            db_instance.put_document(&doc).unwrap();
            db_instance.get_document(&format!("concurrent_doc_{}", i)).unwrap()
        });
        
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        let doc = handle.await.unwrap();
        assert!(doc.id.starts_with("concurrent_doc_"));
    }
    
    // Verify all documents were created
    let db_instance = db_manager.get_database_clone("concurrent_test").unwrap();
    let all_docs = db_instance.get_all_documents(&ViewQuery::default()).unwrap();
    assert_eq!(all_docs.rows.len(), 10);
}

#[tokio::test]
async fn test_system_database_initialization() {
    let temp_dir = tempdir().unwrap();
    let db_manager = DatabaseManager::new(temp_dir.path().to_str().unwrap()).unwrap();
    
    // Test default initialization
    db_manager.initialize_defaults().unwrap();
    
    let all_dbs = db_manager.list_databases().unwrap();
    assert!(all_dbs.0.contains(&"_users".to_string()));
    assert!(all_dbs.0.contains(&"_replicator".to_string()));
}

#[tokio::test]
async fn test_server_info() {
    let temp_dir = tempdir().unwrap();
    let db_manager = DatabaseManager::new(temp_dir.path().to_str().unwrap()).unwrap();
    
    let server_info = db_manager.get_server_info();
    
    assert_eq!(server_info.couchdb, "Welcome");
    assert_eq!(server_info.version, "1.7.2");
    assert_eq!(server_info.vendor.name, "Literbike CouchDB Emulator");
    assert!(server_info.features.contains(&"attachments".to_string()));
    assert!(server_info.features.contains(&"ipfs".to_string()));
    assert!(server_info.features.contains(&"m2m".to_string()));
    assert!(server_info.features.contains(&"tensor".to_string()));
}

#[cfg(test)]
mod integration_helpers {
    use super::*;
    
    pub fn create_test_document(id: &str, doc_type: &str) -> Document {
        Document {
            id: id.to_string(),
            rev: "".to_string(),
            deleted: None,
            attachments: None,
            data: json!({
                "type": doc_type,
                "created_at": chrono::Utc::now().to_rfc3339(),
                "test_data": "This is test data"
            }),
        }
    }
    
    pub async fn setup_test_database(name: &str) -> (tempfile::TempDir, DatabaseManager) {
        let temp_dir = tempdir().unwrap();
        let db_manager = DatabaseManager::new(temp_dir.path().to_str().unwrap()).unwrap();
        db_manager.create_database(name).unwrap();
        (temp_dir, db_manager)
    }
}