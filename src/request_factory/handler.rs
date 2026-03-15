use axum::{Json, response::IntoResponse};
use crate::request_factory::wire::{RfRequest, RfResponse};

/// Axum handler for `POST /_rf` — dispatches batched RequestFactory operations.
pub async fn rf_handler(Json(_req): Json<RfRequest>) -> impl IntoResponse {
    // TODO: dispatch each invocation to CouchDB ops
    Json(RfResponse {
        results: vec![],
        side_effects: vec![],
    })
}
