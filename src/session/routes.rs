use std::sync::Arc;
use dashmap::DashMap;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
    routing::{get, post, delete},
};
use serde::{Deserialize, Serialize};

use crate::session::{open_channel, record_turn, patch_feed, revert_turn, SessionChannel};

#[derive(Clone)]
pub struct SessionManager {
    pub sessions: Arc<DashMap<String, SessionChannel>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }
}

// Request/response types

#[derive(Deserialize)]
pub struct CreateSessionBody {
    pub session_id: Option<String>,
}

#[derive(Deserialize)]
pub struct RecordTurnBody {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct PatchFeedQuery {
    pub since: Option<String>,
}

// Handlers

pub async fn create_session(
    State(mgr): State<SessionManager>,
    body: Option<Json<CreateSessionBody>>,
) -> impl IntoResponse {
    let session_id = body
        .and_then(|b| b.0.session_id)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    match open_channel(&session_id) {
        Ok(channel) => {
            mgr.sessions.insert(session_id.clone(), channel);
            (
                StatusCode::OK,
                Json(serde_json::json!({ "session_id": session_id, "ok": true })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e, "ok": false })),
        ),
    }
}

pub async fn record_turn_handler(
    Path(id): Path<String>,
    State(mgr): State<SessionManager>,
    Json(body): Json<RecordTurnBody>,
) -> impl IntoResponse {
    let channel = match mgr.sessions.get(&id) {
        Some(c) => c.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "session not found", "ok": false })),
            )
        }
    };

    match record_turn(&channel, &body.role, &body.content) {
        Ok(hash) => (
            StatusCode::OK,
            Json(serde_json::json!({ "hash": hash, "ok": true })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e, "ok": false })),
        ),
    }
}

pub async fn get_patches_handler(
    Path(id): Path<String>,
    Query(query): Query<PatchFeedQuery>,
    State(mgr): State<SessionManager>,
) -> impl IntoResponse {
    let channel = match mgr.sessions.get(&id) {
        Some(c) => c.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "session not found", "ok": false })),
            )
        }
    };

    let since = query.since.as_deref();
    match patch_feed(&channel, since) {
        Ok(patches) => (
            StatusCode::OK,
            Json(serde_json::json!({ "patches": patches, "session_id": id })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e, "ok": false })),
        ),
    }
}

pub async fn revert_turn_handler(
    Path((id, hash)): Path<(String, String)>,
    State(mgr): State<SessionManager>,
) -> impl IntoResponse {
    let channel = match mgr.sessions.get(&id) {
        Some(c) => c.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "session not found", "ok": false })),
            )
        }
    };

    match revert_turn(&channel, &hash) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e, "ok": false })),
        ),
    }
}

pub fn session_router() -> Router<SessionManager> {
    Router::new()
        .route("/session", post(create_session))
        .route("/session/:id/turns", post(record_turn_handler))
        .route("/session/:id/patches", get(get_patches_handler))
        .route("/session/:id/turns/:hash", delete(revert_turn_handler))
}
