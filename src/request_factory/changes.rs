use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::couchdb::types::ViewQuery;

/// Query parameters for the RF changes endpoint.
#[derive(Debug, Deserialize)]
pub struct RfChangesQuery {
    pub since: Option<u64>,
}

/// A single change event returned by the changes endpoint.
#[derive(Debug, Serialize)]
pub struct RfChangeEvent {
    pub id: String,
    pub version: String,
    pub payload: Value,
    pub deleted: bool,
}

/// Response envelope for the RF changes endpoint.
#[derive(Debug, Serialize)]
pub struct RfChangesResponse {
    pub results: Vec<RfChangeEvent>,
    pub last_seq: String,
}

/// GET `/_rf/changes` — returns RF change events from the default database.
///
/// Accepts a `since` query parameter (u64 sequence number). All live documents
/// are returned; deleted tombstones are included when the document's `_deleted`
/// field is `true`.
pub async fn rf_changes_handler(
    State(state): State<crate::couchdb::api::AppState>,
    Query(params): Query<RfChangesQuery>,
) -> impl IntoResponse {
    let since = params.since.unwrap_or(0);

    // Ensure the default database exists; if it doesn't, return an empty result.
    if !state.db_manager.database_exists(&state.rf_default_db) {
        return Json(RfChangesResponse {
            results: vec![],
            last_seq: since.to_string(),
        });
    }

    let db_instance = match state.db_manager.get_database_clone(&state.rf_default_db) {
        Ok(db) => db,
        Err(_) => {
            return Json(RfChangesResponse {
                results: vec![],
                last_seq: since.to_string(),
            });
        }
    };

    // Use a default ViewQuery to fetch all documents.
    let query = ViewQuery {
        conflicts: None,
        descending: None,
        endkey: None,
        endkey_docid: None,
        group: None,
        group_level: None,
        include_docs: Some(true),
        inclusive_end: None,
        key: None,
        keys: None,
        limit: None,
        reduce: None,
        skip: None,
        stale: None,
        startkey: None,
        startkey_docid: None,
        update_seq: None,
        cursor: None,
    };

    let view_result = match db_instance.get_all_documents(&query) {
        Ok(r) => r,
        Err(_) => {
            return Json(RfChangesResponse {
                results: vec![],
                last_seq: since.to_string(),
            });
        }
    };

    let mut results = Vec::new();
    let mut last_seq: u64 = since;

    for row in view_result.rows {
        let doc = match row.doc {
            Some(d) => d,
            None => continue,
        };

        let is_deleted = doc.deleted.unwrap_or(false);

        let event = RfChangeEvent {
            id: doc.id.clone(),
            version: doc.rev.clone(),
            payload: doc.data.clone(),
            deleted: is_deleted,
        };

        results.push(event);
        last_seq += 1;
    }

    Json(RfChangesResponse {
        results,
        last_seq: last_seq.to_string(),
    })
}
