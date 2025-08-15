// Test Utilities Framework
// Provides mock servers, protocol generators, test macros, and common testing infrastructure

pub mod mock_servers;
pub mod protocol_generators;
pub mod test_macros;

pub mod performance_helpers;

pub use mock_servers::*;
pub use protocol_generators::*;
pub use test_macros::*;



use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Test configuration for controlling test behavior
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub timeout: Duration,
    pub max_connections: usize,
    pub buffer_size: usize,
    pub enable_logging: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            max_connections: 100,
            buffer_size: 8192,
            enable_logging: false,
        }
    }
}

/// Test result metrics
#[derive(Debug, Clone)]
pub struct TestMetrics {
    pub connections_count: u64,
    pub bytes_transferred: u64,
    pub duration: Duration,
    pub success_rate: f64,
    pub errors: Vec<String>,
}

impl TestMetrics {
    pub fn new() -> Self {
        Self {
            connections_count: 0,
            bytes_transferred: 0,
            duration: Duration::from_secs(0),
            success_rate: 0.0,
            errors: Vec::new(),
        }
    }
    
    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }
    
    pub fn calculate_success_rate(&mut self, total_attempts: u64) {
        if total_attempts > 0 {
            let successful = total_attempts - self.errors.len() as u64;
            self.success_rate = successful as f64 / total_attempts as f64;
        }
    }
}

/// Shared test state for tracking metrics across threads
#[derive(Debug)]
pub struct TestState {
    pub connections: AtomicU64,
    pub bytes_transferred: AtomicU64,
    pub errors: std::sync::Mutex<Vec<String>>,
}

impl TestState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            connections: AtomicU64::new(0),
            bytes_transferred: AtomicU64::new(0),
            errors: std::sync::Mutex::new(Vec::new()),
        })
    }
    
    pub fn record_connection(&self) {
        self.connections.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_bytes(&self, bytes: u64) {
        self.bytes_transferred.fetch_add(bytes, Ordering::Relaxed);
    }
    
    pub fn record_error(&self, error: String) {
        if let Ok(mut errors) = self.errors.lock() {
            errors.push(error);
        }
    }
    
    pub fn get_metrics(&self) -> TestMetrics {
        let mut metrics = TestMetrics::new();
        metrics.connections_count = self.connections.load(Ordering::Relaxed);
        metrics.bytes_transferred = self.bytes_transferred.load(Ordering::Relaxed);
        
        if let Ok(errors) = self.errors.lock() {
            metrics.errors = errors.clone();
        }
        
        metrics
    }
}

/// Utility for creating test socket addresses
pub fn get_test_addr() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

/// Utility for waiting for server startup
pub async fn wait_for_server(addr: SocketAddr, max_attempts: usize) -> bool {
    for _ in 0..max_attempts {
        if TcpStream::connect(addr).await.is_ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

/// Setup logging for tests if enabled
pub fn setup_test_logging() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();
}

/// Helper trait for protocol-specific test data
pub trait ProtocolTestData {
    fn valid_requests(&self) -> Vec<Vec<u8>>;
    fn invalid_requests(&self) -> Vec<Vec<u8>>;
    fn edge_case_requests(&self) -> Vec<Vec<u8>>;
    fn expected_responses(&self) -> Vec<Vec<u8>>;
}

/// Helper for timing operations in tests
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
    
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
    
    pub fn reset(&mut self) {
        self.start = Instant::now();
    }
}