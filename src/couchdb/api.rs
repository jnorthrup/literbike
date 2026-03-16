use crate::couchdb::{
    types::*,
    error::{CouchError, CouchResult},
    database::DatabaseManager,
    views::ViewServer,
    m2m::M2mManager,
    tensor::TensorEngine,
    ipfs::{IpfsManager, IpfsKvStore},
};
use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, HeaderMap},
    response::{IntoResponse, Response},
    routing::{get, post, put, delete, head},
    Json, Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use log::{info, warn, error, debug};

/// Main application state
#[derive(Clone)]
pub struct AppState {
    pub db_manager: Arc<DatabaseManager>,
    pub view_server: Arc<ViewServer>,
    pub m2m_manager: Arc<M2mManager>,
    pub tensor_engine: Arc<TensorEngine>,
    pub ipfs_manager: Arc<IpfsManager>,
    pub kv_store: Arc<IpfsKvStore>,
    pub rf_tracker: Arc<crate::request_factory::tracker::OperationsTracker>,
    pub rf_default_db: String,
}

/// OpenAPI documentation
#[derive(OpenApi)]
#[openapi(
    paths(
        get_server_info,
        list_databases,
        create_database,
        delete_database,
        get_database_info,
        get_document,
        put_document,
        delete_document,
        get_all_docs,
        query_view,
        bulk_docs,
        get_changes,
        put_attachment,
        get_attachment,
        delete_attachment,
        ipfs_store,
        ipfs_get,
        ipfs_stats,
        m2m_send_message,
        m2m_list_peers,
        tensor_execute_operation,
        kv_put,
        kv_get,
        kv_delete,
    ),
    components(
        schemas(
            ServerInfo,
            ServerVendor,
            AllDbsResponse,
            DatabaseInfo,
            Document,
            AttachmentInfo,
            ViewQuery,
            ViewResult,
            ViewRow,
            BulkDocs,
            BulkResult,
            ChangesQuery,
            ChangesResponse,
            Change,
            ChangeRevision,
            CouchError,
            TensorOperation,
            TensorOpType,
            M2mMessage,
            M2mMessageType,
            IpfsCid,
            KvEntry,
        )
    ),
    tags(
        (name = "server", description = "Server information and management"),
        (name = "database", description = "Database operations"),
        (name = "document", description = "Document operations"),
        (name = "view", description = "View and query operations"),
        (name = "attachment", description = "Attachment operations"),
        (name = "changes", description = "Changes feed"),
        (name = "ipfs", description = "IPFS integration"),
        (name = "m2m", description = "Machine-to-machine communication"),
        (name = "tensor", description = "Tensor operations"),
        (name = "kv", description = "Key-value store"),
    ),
    info(
        title = "LiterBike CouchDB Emulator API",
        version = "0.1.0",
        description = "A CouchDB 1.7.2 compatible API with IPFS, M2M, and tensor extensions",
        contact(
            name = "LiterBike Team",
            email = "info@literbike.com"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    )
)]
pub struct ApiDoc;

/// Create the main API router
pub fn create_router(state: AppState) -> Router {
    let base = Router::new()
        // Server endpoints
        .route("/", get(get_server_info))
        .route("/_all_dbs", get(list_databases))
        .route("/_stats", get(get_server_stats))

        // Database endpoints
        .route("/:db", put(create_database))
        .route("/:db", delete(delete_database))
        .route("/:db", get(get_database_info))
        .route("/:db/_all_docs", get(get_all_docs))
        .route("/:db/_bulk_docs", post(bulk_docs))
        .route("/:db/_changes", get(get_changes))
        .route("/:db/_compact", post(compact_database))

        // Document endpoints
        .route("/:db/:doc_id", get(get_document))
        .route("/:db/:doc_id", put(put_document))
        .route("/:db/:doc_id", delete(delete_document))
        .route("/:db/:doc_id", head(head_document))

        // View endpoints
        .route("/:db/_design/:ddoc/_view/:view", get(query_view))
        .route("/:db/_design/:ddoc/_view/:view", post(query_view_post))

        // Attachment endpoints
        .route("/:db/:doc_id/:attachment", put(put_attachment))
        .route("/:db/:doc_id/:attachment", get(get_attachment))
        .route("/:db/:doc_id/:attachment", delete(delete_attachment))

        // IPFS endpoints
        .route("/_ipfs/store", post(ipfs_store))
        .route("/_ipfs/get/:cid", get(ipfs_get))
        .route("/_ipfs/stats", get(ipfs_stats))
        .route("/_ipfs/gc", post(ipfs_gc))

        // M2M endpoints
        .route("/_m2m/send", post(m2m_send_message))
        .route("/_m2m/broadcast", post(m2m_broadcast_message))
        .route("/_m2m/peers", get(m2m_list_peers))
        .route("/_m2m/stats", get(m2m_get_stats))

        // Tensor endpoints
        .route("/_tensor/execute", post(tensor_execute_operation))
        .route("/_tensor/stats", get(tensor_get_stats))

        // Key-Value store endpoints
        .route("/_kv/:key", put(kv_put))
        .route("/_kv/:key", get(kv_get))
        .route("/_kv/:key", delete(kv_delete))
        .route("/_kv", get(kv_list_keys))
        .route("/_kv/_stats", get(kv_get_stats))

        // Swagger UI
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));

    #[cfg(feature = "request-factory")]
    let base = base
        .route("/_rf", post(crate::request_factory::handler::rf_handler))
        .route("/_rf/metrics", get(crate::request_factory::handler::rf_metrics_handler))
        .route("/_rf/metrics/reset", post(crate::request_factory::handler::rf_reset_metrics_handler))
        .route("/_rf/changes", get(crate::request_factory::changes::rf_changes_handler));

    base
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// Server endpoints

/// Get server information
#[utoipa::path(
    get,
    path = "/",
    tag = "server",
    responses(
        (status = 200, description = "Server information", body = ServerInfo)
    )
)]
pub async fn get_server_info(State(state): State<AppState>) -> impl IntoResponse {
    let server_info = state.db_manager.get_server_info();
    Json(server_info)
}

/// List all databases
#[utoipa::path(
    get,
    path = "/_all_dbs",
    tag = "database",
    responses(
        (status = 200, description = "List of all databases", body = AllDbsResponse)
    )
)]
pub async fn list_databases(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let dbs = state.db_manager.list_databases()?;
    Ok(Json(dbs))
}

/// Get server statistics
#[utoipa::path(
    get,
    path = "/_stats",
    tag = "server",
    responses(
        (status = 200, description = "Server statistics")
    )
)]
pub async fn get_server_stats(State(state): State<AppState>) -> impl IntoResponse {
    let mut stats = serde_json::Map::new();
    
    // Database stats
    let db_list = state.db_manager.list_databases().unwrap_or(AllDbsResponse(vec![]));
    stats.insert("database_count".to_string(), serde_json::Value::Number(db_list.0.len().into()));
    
    // View stats
    let view_stats = state.view_server.get_stats();
    stats.insert("view_stats".to_string(), serde_json::Value::Object(
        view_stats.into_iter().map(|(k, v)| (k, v)).collect()
    ));
    
    // M2M stats
    let m2m_metrics = state.m2m_manager.get_metrics();
    stats.insert("m2m_stats".to_string(), serde_json::to_value(m2m_metrics).unwrap());
    
    // Tensor stats
    let tensor_stats = state.tensor_engine.get_stats();
    stats.insert("tensor_stats".to_string(), serde_json::Value::Object(
        tensor_stats.into_iter().map(|(k, v)| (k, v)).collect()
    ));
    
    Json(serde_json::Value::Object(stats))
}

// Database endpoints

/// Create a database
#[utoipa::path(
    put,
    path = "/{db}",
    tag = "database",
    params(
        ("db" = String, Path, description = "Database name")
    ),
    responses(
        (status = 201, description = "Database created", body = DatabaseInfo),
        (status = 409, description = "Database already exists", body = CouchError)
    )
)]
pub async fn create_database(
    Path(db_name): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let db_info = state.db_manager.create_database(&db_name)?;
    Ok((StatusCode::CREATED, Json(db_info)))
}

/// Delete a database
#[utoipa::path(
    delete,
    path = "/{db}",
    tag = "database",
    params(
        ("db" = String, Path, description = "Database name")
    ),
    responses(
        (status = 200, description = "Database deleted"),
        (status = 404, description = "Database not found", body = CouchError)
    )
)]
pub async fn delete_database(
    Path(db_name): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let result = state.db_manager.delete_database(&db_name)?;
    Ok(Json(result))
}

/// Get database information
#[utoipa::path(
    get,
    path = "/{db}",
    tag = "database",
    params(
        ("db" = String, Path, description = "Database name")
    ),
    responses(
        (status = 200, description = "Database information", body = DatabaseInfo),
        (status = 404, description = "Database not found", body = CouchError)
    )
)]
pub async fn get_database_info(
    Path(db_name): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let db_info = state.db_manager.get_database_info(&db_name)?;
    Ok(Json(db_info))
}

/// Compact database
#[utoipa::path(
    post,
    path = "/{db}/_compact",
    tag = "database",
    params(
        ("db" = String, Path, description = "Database name")
    ),
    responses(
        (status = 202, description = "Compaction started"),
        (status = 404, description = "Database not found", body = CouchError)
    )
)]
pub async fn compact_database(
    Path(db_name): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let result = state.db_manager.compact_database(&db_name)?;
    Ok((StatusCode::ACCEPTED, Json(result)))
}

// Document endpoints

/// Get a document
#[utoipa::path(
    get,
    path = "/{db}/{doc_id}",
    tag = "document",
    params(
        ("db" = String, Path, description = "Database name"),
        ("doc_id" = String, Path, description = "Document ID")
    ),
    responses(
        (status = 200, description = "Document retrieved", body = Document),
        (status = 404, description = "Document not found", body = CouchError)
    )
)]
pub async fn get_document(
    Path((db_name, doc_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let db_instance = state.db_manager.get_database_clone(&db_name)?;
    let doc = db_instance.get_document(&doc_id)?;
    Ok(Json(doc))
}

/// Put a document
#[utoipa::path(
    put,
    path = "/{db}/{doc_id}",
    tag = "document",
    params(
        ("db" = String, Path, description = "Database name"),
        ("doc_id" = String, Path, description = "Document ID")
    ),
    request_body = Document,
    responses(
        (status = 201, description = "Document created"),
        (status = 409, description = "Document conflict", body = CouchError)
    )
)]
pub async fn put_document(
    Path((db_name, doc_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(mut doc): Json<Document>,
) -> Result<impl IntoResponse, ApiError> {
    doc.id = doc_id;
    let db_instance = state.db_manager.get_database_clone(&db_name)?;
    let (id, rev) = db_instance.put_document(&doc)?;
    
    Ok((StatusCode::CREATED, Json(serde_json::json!({
        "ok": true,
        "id": id,
        "rev": rev
    }))))
}

/// Delete a document
#[utoipa::path(
    delete,
    path = "/{db}/{doc_id}",
    tag = "document",
    params(
        ("db" = String, Path, description = "Database name"),
        ("doc_id" = String, Path, description = "Document ID"),
        ("rev" = String, Query, description = "Document revision")
    ),
    responses(
        (status = 200, description = "Document deleted"),
        (status = 409, description = "Document conflict", body = CouchError)
    )
)]
pub async fn delete_document(
    Path((db_name, doc_id)): Path<(String, String)>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let rev = params.get("rev")
        .ok_or_else(|| CouchError::bad_request("Missing rev parameter"))?;
    
    let db_instance = state.db_manager.get_database_clone(&db_name)?;
    let (id, new_rev) = db_instance.delete_document(&doc_id, rev)?;
    
    Ok(Json(serde_json::json!({
        "ok": true,
        "id": id,
        "rev": new_rev
    })))
}

/// Head request for document (check existence)
#[utoipa::path(
    head,
    path = "/{db}/{doc_id}",
    tag = "document",
    params(
        ("db" = String, Path, description = "Database name"),
        ("doc_id" = String, Path, description = "Document ID")
    ),
    responses(
        (status = 200, description = "Document exists"),
        (status = 404, description = "Document not found")
    )
)]
pub async fn head_document(
    Path((db_name, doc_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let db_instance = state.db_manager.get_database_clone(&db_name)?;
    if db_instance.document_exists(&doc_id) {
        Ok(StatusCode::OK)
    } else {
        Err(ApiError::from(CouchError::not_found("Document not found")))
    }
}

/// Get all documents
#[utoipa::path(
    get,
    path = "/{db}/_all_docs",
    tag = "document",
    params(
        ("db" = String, Path, description = "Database name")
    ),
    responses(
        (status = 200, description = "All documents", body = ViewResult)
    )
)]
pub async fn get_all_docs(
    Path(db_name): Path<String>,
    Query(query): Query<ViewQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let db_instance = state.db_manager.get_database_clone(&db_name)?;
    let result = db_instance.get_all_documents(&query)?;
    Ok(Json(result))
}

/// Bulk document operations
#[utoipa::path(
    post,
    path = "/{db}/_bulk_docs",
    tag = "document",
    params(
        ("db" = String, Path, description = "Database name")
    ),
    request_body = BulkDocs,
    responses(
        (status = 200, description = "Bulk operation results", body = Vec<BulkResult>)
    )
)]
pub async fn bulk_docs(
    Path(db_name): Path<String>,
    State(state): State<AppState>,
    Json(bulk_docs): Json<BulkDocs>,
) -> Result<impl IntoResponse, ApiError> {
    let db_instance = state.db_manager.get_database_clone(&db_name)?;
    let mut results = Vec::new();
    
    for doc in bulk_docs.docs {
        match db_instance.put_document(&doc) {
            Ok((id, rev)) => {
                results.push(BulkResult {
                    ok: Some(true),
                    id,
                    rev: Some(rev),
                    error: None,
                    reason: None,
                });
            }
            Err(e) => {
                results.push(BulkResult {
                    ok: None,
                    id: doc.id,
                    rev: None,
                    error: Some(e.error),
                    reason: Some(e.reason),
                });
            }
        }
    }
    
    Ok(Json(results))
}

/// Get changes feed
#[utoipa::path(
    get,
    path = "/{db}/_changes",
    tag = "changes",
    params(
        ("db" = String, Path, description = "Database name")
    ),
    responses(
        (status = 200, description = "Changes feed", body = ChangesResponse)
    )
)]
pub async fn get_changes(
    Path(db_name): Path<String>,
    Query(_query): Query<ChangesQuery>,
    State(_state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    // Simplified implementation - return empty changes
    let response = ChangesResponse {
        results: vec![],
        last_seq: "0".to_string(),
        pending: Some(0),
    };
    Ok(Json(response))
}

// View endpoints

/// Query a view
#[utoipa::path(
    get,
    path = "/{db}/_design/{ddoc}/_view/{view}",
    tag = "view",
    params(
        ("db" = String, Path, description = "Database name"),
        ("ddoc" = String, Path, description = "Design document name"),
        ("view" = String, Path, description = "View name")
    ),
    responses(
        (status = 200, description = "View query results", body = ViewResult)
    )
)]
pub async fn query_view(
    Path((db_name, ddoc, view_name)): Path<(String, String, String)>,
    Query(query): Query<ViewQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let ddoc_id = format!("_design/{}", ddoc);
    let result = state.view_server.query_view(&ddoc_id, &view_name, &query)?;
    Ok(Json(result))
}

/// Query a view with POST (for complex queries)
#[utoipa::path(
    post,
    path = "/{db}/_design/{ddoc}/_view/{view}",
    tag = "view",
    params(
        ("db" = String, Path, description = "Database name"),
        ("ddoc" = String, Path, description = "Design document name"),
        ("view" = String, Path, description = "View name")
    ),
    request_body = ViewQuery,
    responses(
        (status = 200, description = "View query results", body = ViewResult)
    )
)]
pub async fn query_view_post(
    Path((db_name, ddoc, view_name)): Path<(String, String, String)>,
    State(state): State<AppState>,
    Json(query): Json<ViewQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let ddoc_id = format!("_design/{}", ddoc);
    let result = state.view_server.query_view(&ddoc_id, &view_name, &query)?;
    Ok(Json(result))
}

// Attachment endpoints

/// Put attachment
#[utoipa::path(
    put,
    path = "/{db}/{doc_id}/{attachment}",
    tag = "attachment",
    params(
        ("db" = String, Path, description = "Database name"),
        ("doc_id" = String, Path, description = "Document ID"),
        ("attachment" = String, Path, description = "Attachment name")
    ),
    responses(
        (status = 201, description = "Attachment created")
    )
)]
pub async fn put_attachment(
    Path((db_name, doc_id, attachment_name)): Path<(String, String, String)>,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, ApiError> {
    let content_type = headers.get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("application/octet-stream");

    // Store in IPFS
    let ipfs_cid = state.ipfs_manager.store_data(&body, content_type).await?;

    // Update document with attachment info
    let db_instance = state.db_manager.get_database_clone(&db_name)?;
    let mut doc = db_instance.get_document(&doc_id)?;

    let attachment_info = AttachmentInfo {
        content_type: content_type.to_string(),
        length: body.len() as u64,
        digest: ipfs_cid.cid.clone(),
        stub: Some(true),
        revpos: Some(1),
        data: None,
    };

    if doc.attachments.is_none() {
        doc.attachments = Some(std::collections::HashMap::new());
    }
    doc.attachments.as_mut().unwrap().insert(attachment_name, attachment_info);

    let (id, rev) = db_instance.put_document(&doc)?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({
        "ok": true,
        "id": id,
        "rev": rev
    }))))
}

/// Get attachment
#[utoipa::path(
    get,
    path = "/{db}/{doc_id}/{attachment}",
    tag = "attachment",
    params(
        ("db" = String, Path, description = "Database name"),
        ("doc_id" = String, Path, description = "Document ID"),
        ("attachment" = String, Path, description = "Attachment name")
    ),
    responses(
        (status = 200, description = "Attachment data")
    )
)]
pub async fn get_attachment(
    Path((db_name, doc_id, attachment_name)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let db_instance = state.db_manager.get_database_clone(&db_name)?;
    let doc = db_instance.get_document(&doc_id)?;

    let attachment_info = doc.attachments
        .and_then(|attachments| attachments.get(&attachment_name).cloned())
        .ok_or_else(|| CouchError::not_found("Attachment not found"))?;

    // Get from IPFS using digest as CID
    let (data, _ipfs_cid) = state.ipfs_manager.get_attachment(&attachment_info.digest).await?;

    let mut headers = HeaderMap::new();
    headers.insert("content-type", attachment_info.content_type.parse().unwrap());
    headers.insert("content-length", attachment_info.length.to_string().parse().unwrap());

    Ok((headers, data))
}

/// Delete attachment
#[utoipa::path(
    delete,
    path = "/{db}/{doc_id}/{attachment}",
    tag = "attachment",
    params(
        ("db" = String, Path, description = "Database name"),
        ("doc_id" = String, Path, description = "Document ID"),
        ("attachment" = String, Path, description = "Attachment name")
    ),
    responses(
        (status = 200, description = "Attachment deleted")
    )
)]
pub async fn delete_attachment(
    Path((db_name, doc_id, attachment_name)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let db_instance = state.db_manager.get_database_clone(&db_name)?;
    let mut doc = db_instance.get_document(&doc_id)?;

    if let Some(ref mut attachments) = doc.attachments {
        if let Some(attachment_info) = attachments.remove(&attachment_name) {
            // Unpin from IPFS
            if let Err(e) = state.ipfs_manager.unpin_content(&attachment_info.digest).await {
                warn!("Failed to unpin attachment from IPFS: {}", e);
            }

            let (id, rev) = db_instance.put_document(&doc)?;
            return Ok(Json(serde_json::json!({
                "ok": true,
                "id": id,
                "rev": rev
            })));
        }
    }

    Err(ApiError::from(CouchError::not_found("Attachment not found")))
}

// IPFS endpoints

/// Store data in IPFS
#[utoipa::path(
    post,
    path = "/_ipfs/store",
    tag = "ipfs",
    responses(
        (status = 200, description = "Data stored in IPFS", body = IpfsCid)
    )
)]
pub async fn ipfs_store(
    headers: HeaderMap,
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, ApiError> {
    let content_type = headers.get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("application/octet-stream");

    let ipfs_cid = state.ipfs_manager.store_data(&body, content_type).await?;
    Ok(Json(ipfs_cid))
}

/// Get data from IPFS
#[utoipa::path(
    get,
    path = "/_ipfs/get/{cid}",
    tag = "ipfs",
    params(
        ("cid" = String, Path, description = "IPFS Content ID")
    ),
    responses(
        (status = 200, description = "IPFS data")
    )
)]
pub async fn ipfs_get(
    Path(cid): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let data = state.ipfs_manager.get_data(&cid).await?;

    let mut headers = HeaderMap::new();
    headers.insert("content-type", "application/octet-stream".parse().unwrap());
    headers.insert("content-length", data.len().to_string().parse().unwrap());

    Ok((headers, data))
}

/// Get IPFS statistics
#[utoipa::path(
    get,
    path = "/_ipfs/stats",
    tag = "ipfs",
    responses(
        (status = 200, description = "IPFS statistics")
    )
)]
pub async fn ipfs_stats(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let stats = state.ipfs_manager.get_stats().await?;
    Ok(Json(stats))
}

/// IPFS garbage collection
#[utoipa::path(
    post,
    path = "/_ipfs/gc",
    tag = "ipfs",
    responses(
        (status = 200, description = "Garbage collection completed")
    )
)]
pub async fn ipfs_gc(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let removed = state.ipfs_manager.garbage_collect().await?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "removed_objects": removed.len(),
        "removed_cids": removed
    })))
}

// M2M endpoints

/// Send M2M message
#[utoipa::path(
    post,
    path = "/_m2m/send",
    tag = "m2m",
    request_body = M2mMessage,
    responses(
        (status = 200, description = "Message sent")
    )
)]
pub async fn m2m_send_message(
    State(state): State<AppState>,
    Json(message): Json<M2mMessage>,
) -> Result<impl IntoResponse, ApiError> {
    if let Some(ref recipient) = message.recipient {
        state.m2m_manager.send_message(recipient, message.message_type, message.payload).await?;
    } else {
        state.m2m_manager.broadcast_message(message.message_type, message.payload).await?;
    }
    
    Ok(Json(serde_json::json!({"ok": true})))
}

/// Broadcast M2M message
#[utoipa::path(
    post,
    path = "/_m2m/broadcast",
    tag = "m2m",
    responses(
        (status = 200, description = "Message broadcasted")
    )
)]
pub async fn m2m_broadcast_message(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ApiError> {
    let message_type = payload.get("type")
        .and_then(|t| t.as_str())
        .and_then(|t| serde_json::from_str(&format!("\"{}\"", t)).ok())
        .unwrap_or(M2mMessageType::Custom("broadcast".to_string()));
    
    state.m2m_manager.broadcast_message(message_type, payload).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// List M2M peers
#[utoipa::path(
    get,
    path = "/_m2m/peers",
    tag = "m2m",
    responses(
        (status = 200, description = "List of M2M peers")
    )
)]
pub async fn m2m_list_peers(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let peers = state.m2m_manager.list_peers();
    Json(peers)
}

/// Get M2M statistics
#[utoipa::path(
    get,
    path = "/_m2m/stats",
    tag = "m2m",
    responses(
        (status = 200, description = "M2M statistics")
    )
)]
pub async fn m2m_get_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let metrics = state.m2m_manager.get_metrics();
    Json(metrics)
}

// Tensor endpoints

/// Execute tensor operation
#[utoipa::path(
    post,
    path = "/_tensor/execute",
    tag = "tensor",
    request_body = TensorOperation,
    responses(
        (status = 200, description = "Tensor operation result")
    )
)]
pub async fn tensor_execute_operation(
    State(state): State<AppState>,
    Json(operation): Json<TensorOperation>,
) -> Result<impl IntoResponse, ApiError> {
    // For tensor operations, we need a database to load documents from
    // Use the first database in the input_docs or default to a system database
    let db_name = if let Some(doc_id) = operation.input_docs.first() {
        // Extract database name from document ID if it contains database info
        // For now, assume "main" database
        "main".to_string()
    } else {
        "main".to_string()
    };
    
    let db_instance = state.db_manager.get_database_clone(&db_name)?;
    let result = state.tensor_engine.execute_operation(&operation, &db_instance)?;
    Ok(Json(result))
}

/// Get tensor statistics
#[utoipa::path(
    get,
    path = "/_tensor/stats",
    tag = "tensor",
    responses(
        (status = 200, description = "Tensor engine statistics")
    )
)]
pub async fn tensor_get_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let stats = state.tensor_engine.get_stats();
    Json(stats)
}

// Key-Value store endpoints

/// Put key-value pair
#[utoipa::path(
    put,
    path = "/_kv/{key}",
    tag = "kv",
    params(
        ("key" = String, Path, description = "Key")
    ),
    responses(
        (status = 201, description = "Key-value pair stored", body = KvEntry)
    )
)]
pub async fn kv_put(
    Path(key): Path<String>,
    headers: HeaderMap,
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, ApiError> {
    let content_type = headers.get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("application/octet-stream");

    let metadata = std::collections::HashMap::new();
    let entry = state.kv_store.put(&key, &body, content_type, metadata).await?;

    Ok((StatusCode::CREATED, Json(entry)))
}

/// Get value by key
#[utoipa::path(
    get,
    path = "/_kv/{key}",
    tag = "kv",
    params(
        ("key" = String, Path, description = "Key")
    ),
    responses(
        (status = 200, description = "Key-value pair", body = KvEntry)
    )
)]
pub async fn kv_get(
    Path(key): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let entry = state.kv_store.get(&key).await?;
    Ok(Json(entry))
}

/// Delete key-value pair
#[utoipa::path(
    delete,
    path = "/_kv/{key}",
    tag = "kv",
    params(
        ("key" = String, Path, description = "Key")
    ),
    responses(
        (status = 200, description = "Key-value pair deleted")
    )
)]
pub async fn kv_delete(
    Path(key): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let deleted = state.kv_store.delete(&key).await?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "deleted": deleted
    })))
}

/// List all keys
#[utoipa::path(
    get,
    path = "/_kv",
    tag = "kv",
    responses(
        (status = 200, description = "List of all keys")
    )
)]
pub async fn kv_list_keys(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let keys = state.kv_store.list_keys().await;
    Json(keys)
}

/// Get KV store statistics
#[utoipa::path(
    get,
    path = "/_kv/_stats",
    tag = "kv",
    responses(
        (status = 200, description = "KV store statistics")
    )
)]
pub async fn kv_get_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let stats = state.kv_store.get_stats().await;
    Json(stats)
}

// Error handling

#[derive(Debug)]
pub struct ApiError(CouchError);

impl From<CouchError> for ApiError {
    fn from(err: CouchError) -> Self {
        ApiError(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status_code = StatusCode::from_u16(self.0.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = Json(self.0);
        (status_code, body).into_response()
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T> {
    pub ok: bool,
    #[serde(flatten)]
    pub data: T,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self { ok: true, data }
    }
}