use axum::{Json, response::IntoResponse, extract::State};
use std::sync::Arc;
use crate::request_factory::tracker::OperationsTracker;
use serde_json::Value;

use crate::couchdb::api::AppState;
use crate::couchdb::Document;
use crate::request_factory::wire::{RfRequest, RfResponse, RfResult, Invocation};

/// Axum handler for `POST /_rf` — dispatches batched RequestFactory operations to CouchDB.
///
/// Maps operations:
/// - `find` → `get_document`
/// - `persist` → `put_document`
/// - `delete` → `delete_document`
pub async fn rf_handler(
    State(state): State<AppState>,
    Json(req): Json<RfRequest>,
) -> impl IntoResponse {
    let tracker = &state.rf_tracker;
    let _timer = tracker.record_batch_start(req.invocations.len());
    let mut results = Vec::with_capacity(req.invocations.len());
    let mut side_effects = Vec::new();

    // Ensure the default database exists
    if !state.db_manager.database_exists(&state.rf_default_db) {
        if let Err(e) = state.db_manager.create_database(&state.rf_default_db) {
            return Json(RfResponse {
                results: vec![],
                side_effects: vec![serde_json::json!({
                    "type": "error",
                    "message": format!("Failed to create database: {}", e)
                })],
            });
        }
    }

    for invocation in req.invocations {
        match dispatch_invocation(&state, &invocation).await {
            Ok(result) => {
                results.push(result);
            }
            Err((error_msg, entity_type)) => {
                tracker.record_error(
                    &invocation.operation,
                    &entity_type,
                    &error_msg,
                );
                results.push(RfResult {
                    id: invocation.id.clone().unwrap_or_default(),
                    version: invocation.version.clone().unwrap_or_default(),
                    payload: None,
                    error: Some(error_msg),
                });
            }
        }
    }

    // Add side effect: metrics snapshot
    let metrics = tracker.get_metrics();
    side_effects.push(serde_json::json!({
        "type": "metrics",
        "data": metrics
    }));

    Json(RfResponse {
        results,
        side_effects,
    })
}

/// Get metrics for RequestFactory operations
pub async fn rf_metrics_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let metrics = state.rf_tracker.get_metrics();
    Json(metrics)
}

/// Reset metrics for RequestFactory operations
pub async fn rf_reset_metrics_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    state.rf_tracker.reset();
    Json(serde_json::json!({ "ok": true }))
}

/// Dispatch a single invocation to the appropriate CouchDB operation
async fn dispatch_invocation(
    state: &AppState,
    invocation: &Invocation,
) -> Result<RfResult, (String, String)> {
    match invocation.operation.as_str() {
        "find" => handle_find(state, invocation).await,
        "persist" => handle_persist(state, invocation).await,
        "delete" => handle_delete(state, invocation).await,
        _ => Err((
            format!("Unknown operation: {}", invocation.operation),
            invocation.entity_type.clone(),
        )),
    }
}

/// Handle a find operation → maps to CouchDB get_document
async fn handle_find(
    state: &AppState,
    invocation: &Invocation,
) -> Result<RfResult, (String, String)> {
    let id = invocation
        .id
        .as_ref()
        .ok_or_else(|| ("Missing id for find operation".to_string(), invocation.entity_type.clone()))?;

    let db_instance = state
        .db_manager
        .get_database_clone(&state.rf_default_db)
        .map_err(|e| (e.to_string(), invocation.entity_type.clone()))?;

    match db_instance.get_document(id) {
        Ok(doc) => {
            state.rf_tracker.record_find_success();
            Ok(RfResult {
                id: doc.id,
                version: doc.rev,
                payload: Some(doc.data),
                error: None,
            })
        }
        Err(e) => Err((e.to_string(), invocation.entity_type.clone())),
    }
}

/// Handle a persist operation → maps to CouchDB put_document
async fn handle_persist(
    state: &AppState,
    invocation: &Invocation,
) -> Result<RfResult, (String, String)> {
    let id = invocation
        .id
        .as_ref()
        .ok_or_else(|| ("Missing id for persist operation".to_string(), invocation.entity_type.clone()))?;

    let db_instance = state
        .db_manager
        .get_database_clone(&state.rf_default_db)
        .map_err(|e| (e.to_string(), invocation.entity_type.clone()))?;

    // Build the document
    let mut doc = Document {
        id: id.clone(),
        rev: invocation.version.clone().unwrap_or_default(),
        deleted: None,
        attachments: None,
        data: invocation.payload.clone().unwrap_or(serde_json::json!({})),
    };

    // Add entity type to the document data
    if let Some(obj) = doc.data.as_object_mut() {
        obj.insert("_entity_type".to_string(), serde_json::Value::String(invocation.entity_type.clone()));
    }

    match db_instance.put_document(&doc) {
        Ok((new_id, new_rev)) => {
            state.rf_tracker.record_persist_success();

            // Return the persisted document data with updated version
            let mut response_data = doc.data;
            if let Some(obj) = response_data.as_object_mut() {
                obj.insert("_id".to_string(), serde_json::Value::String(new_id.clone()));
                obj.insert("_rev".to_string(), serde_json::Value::String(new_rev.clone()));
            }

            Ok(RfResult {
                id: new_id,
                version: new_rev,
                payload: Some(response_data),
                error: None,
            })
        }
        Err(e) => Err((e.to_string(), invocation.entity_type.clone())),
    }
}

/// Handle a delete operation → maps to CouchDB delete_document
async fn handle_delete(
    state: &AppState,
    invocation: &Invocation,
) -> Result<RfResult, (String, String)> {
    let id = invocation
        .id
        .as_ref()
        .ok_or_else(|| ("Missing id for delete operation".to_string(), invocation.entity_type.clone()))?;

    let rev = invocation
        .version
        .as_ref()
        .ok_or_else(|| ("Missing version (rev) for delete operation".to_string(), invocation.entity_type.clone()))?;

    let db_instance = state
        .db_manager
        .get_database_clone(&state.rf_default_db)
        .map_err(|e| (e.to_string(), invocation.entity_type.clone()))?;

    match db_instance.delete_document(id, rev) {
        Ok((deleted_id, deleted_rev)) => {
            state.rf_tracker.record_delete_success();
            Ok(RfResult {
                id: deleted_id,
                version: deleted_rev,
                payload: None,
                error: None,
            })
        }
        Err(e) => Err((e.to_string(), invocation.entity_type.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::couchdb::database::DatabaseManager;
    use crate::couchdb::views::{ViewServer, ViewServerConfig};
    use crate::couchdb::m2m::{M2mManager, M2mConfig};
    use crate::couchdb::tensor::{TensorEngine, TensorConfig};
    use crate::couchdb::ipfs::{IpfsManager, IpfsKvStore, IpfsConfig, KvStoreConfig};
    use tempfile::tempdir;

    fn create_test_state() -> AppState {
        let temp_dir = tempdir().unwrap();
        let db_manager = Arc::new(
            DatabaseManager::new(temp_dir.path().to_str().unwrap()).unwrap()
        );

        let ipfs_manager = Arc::new(IpfsManager::new(IpfsConfig::default()).unwrap());

        AppState {
            db_manager,
            view_server: Arc::new(ViewServer::new(ViewServerConfig::default()).unwrap()),
            m2m_manager: Arc::new(M2mManager::new(Some("test-node".to_string()), M2mConfig::default())),
            tensor_engine: Arc::new(TensorEngine::new(TensorConfig::default())),
            ipfs_manager: Arc::clone(&ipfs_manager),
            kv_store: Arc::new(IpfsKvStore::new(ipfs_manager, KvStoreConfig::default())),
            rf_tracker: Arc::new(OperationsTracker::new()),
            rf_default_db: "rf_entities".to_string(),
        }
    }

    #[tokio::test]
    async fn test_handle_find_not_found() {
        let state = create_test_state();

        let invocation = Invocation {
            operation: "find".to_string(),
            entity_type: "User".to_string(),
            id: Some("nonexistent".to_string()),
            version: None,
            payload: None,
        };

        let result = handle_find(&state, &invocation).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_persist_and_find() {
        let state = create_test_state();

        // First, persist a document
        let persist_invocation = Invocation {
            operation: "persist".to_string(),
            entity_type: "User".to_string(),
            id: Some("user-123".to_string()),
            version: None,
            payload: Some(serde_json::json!({
                "name": "John Doe",
                "email": "john@example.com"
            })),
        };

        let persist_result = handle_persist(&state, &persist_invocation).await;
        assert!(persist_result.is_ok());

        let pr = persist_result.unwrap();
        assert_eq!(pr.id, "user-123");
        assert!(!pr.version.is_empty());
        assert!(pr.error.is_none());

        // Now find it
        let find_invocation = Invocation {
            operation: "find".to_string(),
            entity_type: "User".to_string(),
            id: Some("user-123".to_string()),
            version: None,
            payload: None,
        };

        let find_result = handle_find(&state, &find_invocation).await;
        assert!(find_result.is_ok());

        let fr = find_result.unwrap();
        assert_eq!(fr.id, "user-123");
        assert!(fr.payload.is_some());
    }

    #[tokio::test]
    async fn test_handle_delete() {
        let state = create_test_state();

        // First persist
        let persist_invocation = Invocation {
            operation: "persist".to_string(),
            entity_type: "User".to_string(),
            id: Some("user-to-delete".to_string()),
            version: None,
            payload: Some(serde_json::json!({ "name": "To Delete" })),
        };

        let persist_result = handle_persist(&state, &persist_invocation).await.unwrap();
        let rev = persist_result.version;

        // Then delete
        let delete_invocation = Invocation {
            operation: "delete".to_string(),
            entity_type: "User".to_string(),
            id: Some("user-to-delete".to_string()),
            version: Some(rev),
            payload: None,
        };

        let delete_result = handle_delete(&state, &delete_invocation).await;
        assert!(delete_result.is_ok());

        // Verify it's deleted
        let find_invocation = Invocation {
            operation: "find".to_string(),
            entity_type: "User".to_string(),
            id: Some("user-to-delete".to_string()),
            version: None,
            payload: None,
        };

        let find_result = handle_find(&state, &find_invocation).await;
        assert!(find_result.is_err());
    }

    #[tokio::test]
    async fn test_tracker_metrics() {
        let state = create_test_state();

        // Execute some operations
        let persist_invocation = Invocation {
            operation: "persist".to_string(),
            entity_type: "User".to_string(),
            id: Some("user-1".to_string()),
            version: None,
            payload: Some(serde_json::json!({ "name": "User 1" })),
        };
        let _ = handle_persist(&state, &persist_invocation).await;

        // Check metrics
        let metrics = state.rf_tracker.get_metrics();
        assert_eq!(metrics.persist_count, 1);
        assert_eq!(metrics.success_count, 1);
        assert!(metrics.success_rate > 0.99);
    }
}
