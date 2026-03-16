use literbike::couchdb::{
    api::{create_router, AppState},
    database::DatabaseManager,
    error::CouchError,
    ipfs::{IpfsConfig, IpfsKvStore, IpfsManager, KvStoreConfig},
    m2m::{HeartbeatHandler, M2mConfig, M2mManager, ReplicationHandler},
    tensor::{TensorConfig, TensorEngine},
    views::{ViewServer, ViewServerConfig},
};
use log::{error, info, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    info!("Starting LiterBike CouchDB Emulator v0.1.0");

    // Load configuration
    let config = load_configuration();

    // Initialize components
    let db_manager = Arc::new(DatabaseManager::new(&config.data_dir)?);
    let view_server = Arc::new(ViewServer::new(ViewServerConfig::default())?);
    let m2m_manager = Arc::new(M2mManager::new(None, M2mConfig::default()));
    let tensor_engine = Arc::new(TensorEngine::new(TensorConfig::default()));

    // Initialize IPFS components if enabled
    let (ipfs_manager, kv_store) = if config.ipfs_enabled {
        let ipfs_config = IpfsConfig {
            api_url: config.ipfs_api_url.clone(),
            gateway_url: config.ipfs_gateway_url.clone(),
            ..IpfsConfig::default()
        };

        match IpfsManager::new(ipfs_config) {
            Ok(ipfs_manager) => {
                let ipfs_manager = Arc::new(ipfs_manager);
                let kv_store = Arc::new(IpfsKvStore::new(
                    Arc::clone(&ipfs_manager),
                    KvStoreConfig::default(),
                ));
                info!("IPFS integration enabled");
                (ipfs_manager, kv_store)
            }
            Err(e) => {
                warn!("IPFS integration disabled due to error: {}", e);
                // Create dummy implementations
                let ipfs_manager = Arc::new(IpfsManager::new(IpfsConfig::default())?);
                let kv_store = Arc::new(IpfsKvStore::new(
                    Arc::clone(&ipfs_manager),
                    KvStoreConfig::default(),
                ));
                (ipfs_manager, kv_store)
            }
        }
    } else {
        warn!("IPFS integration disabled in configuration");
        let ipfs_manager = Arc::new(IpfsManager::new(IpfsConfig::default())?);
        let kv_store = Arc::new(IpfsKvStore::new(
            Arc::clone(&ipfs_manager),
            KvStoreConfig::default(),
        ));
        (ipfs_manager, kv_store)
    };

    // Initialize default databases
    db_manager.initialize_defaults()?;

    // Setup M2M handlers
    let node_id = m2m_manager.get_node_id();
    m2m_manager.register_handler(HeartbeatHandler::new(node_id.clone()))?;
    m2m_manager.register_handler(ReplicationHandler::new(node_id))?;

    // Start M2M background services
    let m2m_handles = m2m_manager.start_services().await?;
    info!("Started {} M2M background services", m2m_handles.len());

    // Create application state
    let app_state = AppState {
        db_manager,
        view_server,
        m2m_manager,
        tensor_engine,
        ipfs_manager,
        kv_store,
        rf_tracker: Arc::new(literbike::request_factory::tracker::OperationsTracker::new()),
        rf_default_db: "rf_entities".to_string(),
    };

    // Create router with middleware
    let app =
        create_router(app_state).layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

    // Start server
    let addr: SocketAddr = format!("{}:{}", config.bind_address, config.bind_port)
        .parse()
        .expect("Invalid bind address/port");

    info!("CouchDB Emulator starting on http://{}", addr);
    info!("Swagger UI available at http://{}/swagger-ui", addr);
    info!("API documentation at http://{}/api-docs/openapi.json", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Graceful shutdown
    let server = axum::serve(listener, app);

    tokio::select! {
        result = server => {
            if let Err(e) = result {
                error!("Server error: {}", e);
            }
        }
        _ = signal::ctrl_c() => {
            info!("Received shutdown signal, shutting down gracefully...");
        }
    }

    info!("CouchDB Emulator stopped");
    Ok(())
}

/// Configuration for the CouchDB emulator
#[derive(Debug, Clone)]
struct EmulatorConfig {
    pub bind_address: String,
    pub bind_port: u16,
    pub data_dir: String,
    pub ipfs_enabled: bool,
    pub ipfs_api_url: String,
    pub ipfs_gateway_url: String,
    pub log_level: String,
}

impl Default for EmulatorConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            bind_port: 5984,
            data_dir: "./data".to_string(),
            ipfs_enabled: false,
            ipfs_api_url: "http://127.0.0.1:5001".to_string(),
            ipfs_gateway_url: "http://127.0.0.1:8080".to_string(),
            log_level: "info".to_string(),
        }
    }
}

/// Load configuration from environment variables
fn load_configuration() -> EmulatorConfig {
    let mut config = EmulatorConfig::default();

    if let Ok(addr) = std::env::var("COUCHDB_BIND_ADDRESS") {
        config.bind_address = addr;
    }

    if let Ok(port) = std::env::var("COUCHDB_BIND_PORT") {
        if let Ok(port_num) = port.parse::<u16>() {
            config.bind_port = port_num;
        }
    }

    if let Ok(data_dir) = std::env::var("COUCHDB_DATA_DIR") {
        config.data_dir = data_dir;
    }

    if let Ok(ipfs_enabled) = std::env::var("COUCHDB_IPFS_ENABLED") {
        config.ipfs_enabled = ipfs_enabled.to_lowercase() == "true";
    }

    if let Ok(ipfs_api) = std::env::var("IPFS_API_URL") {
        config.ipfs_api_url = ipfs_api;
    }

    if let Ok(ipfs_gateway) = std::env::var("IPFS_GATEWAY_URL") {
        config.ipfs_gateway_url = ipfs_gateway;
    }

    if let Ok(log_level) = std::env::var("RUST_LOG") {
        config.log_level = log_level;
    }

    info!(
        "Configuration loaded: bind={}:{}, data_dir={}, ipfs_enabled={}",
        config.bind_address, config.bind_port, config.data_dir, config.ipfs_enabled
    );

    config
}

/// Print startup banner
fn print_banner() {
    println!(
        r#"
    ╭─────────────────────────────────────────────────────────╮
    │                                                         │
    │        LiterBike CouchDB Emulator v0.1.0               │
    │                                                         │
    │        CouchDB 1.7.2 Compatible API                    │
    │        + IPFS Integration                               │
    │        + M2M Communication                              │
    │        + Tensor Operations                              │
    │        + Cursor-based Pagination                        │
    │                                                         │
    ╰─────────────────────────────────────────────────────────╯
    "#
    );
}
