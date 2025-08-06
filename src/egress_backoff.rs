//! Multi-egress back-off logic with error rate monitoring
//! Implements circuit breaker pattern with exponential back-off

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::collections::HashMap;

/// Error tracking window size
const ERROR_WINDOW_SECS: u64 = 60;
const ERROR_THRESHOLD_PERCENT: f64 = 50.0;
const MIN_REQUESTS_FOR_CIRCUIT: u64 = 10;

/// Back-off configuration
const INITIAL_BACKOFF_MS: u64 = 100;
const MAX_BACKOFF_MS: u64 = 30_000;
const BACKOFF_MULTIPLIER: f64 = 2.0;
const JITTER_PERCENT: f64 = 0.1;

/// Health check intervals
const HEALTH_CHECK_INTERVAL_MS: u64 = 5_000;
const RECOVERY_PROBE_INTERVAL_MS: u64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    Closed,     // Normal operation
    Open,       // Failing, reject requests
    HalfOpen,   // Testing recovery
}

/// Tracks errors for a specific time window
#[derive(Debug)]
struct ErrorWindow {
    start_time: Instant,
    success_count: AtomicU64,
    error_count: AtomicU64,
    consecutive_errors: AtomicU64,
}

impl ErrorWindow {
    fn new() -> Self {
        Self {
            start_time: Instant::now(),
            success_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            consecutive_errors: AtomicU64::new(0),
        }
    }

    fn record_success(&self) {
        self.success_count.fetch_add(1, Ordering::Relaxed);
        self.consecutive_errors.store(0, Ordering::Relaxed);
    }

    fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        self.consecutive_errors.fetch_add(1, Ordering::Relaxed);
    }

    fn error_rate(&self) -> f64 {
        let total = self.success_count.load(Ordering::Relaxed) + 
                   self.error_count.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        (self.error_count.load(Ordering::Relaxed) as f64 / total as f64) * 100.0
    }

    fn is_expired(&self) -> bool {
        self.start_time.elapsed() > Duration::from_secs(ERROR_WINDOW_SECS)
    }

    fn total_requests(&self) -> u64 {
        self.success_count.load(Ordering::Relaxed) + 
        self.error_count.load(Ordering::Relaxed)
    }
}

/// Circuit breaker for an egress path
#[derive(Debug)]
pub struct CircuitBreaker {
    state: RwLock<CircuitState>,
    error_window: RwLock<ErrorWindow>,
    last_state_change: RwLock<Instant>,
    current_backoff_ms: AtomicU64,
    last_health_check: RwLock<Instant>,
    consecutive_health_failures: AtomicU64,
}

impl CircuitBreaker {
    fn new() -> Self {
        Self {
            state: RwLock::new(CircuitState::Closed),
            error_window: RwLock::new(ErrorWindow::new()),
            last_state_change: RwLock::new(Instant::now()),
            current_backoff_ms: AtomicU64::new(INITIAL_BACKOFF_MS),
            last_health_check: RwLock::new(Instant::now()),
            consecutive_health_failures: AtomicU64::new(0),
        }
    }

    fn record_success(&self) {
        // Reset back-off on success
        self.current_backoff_ms.store(INITIAL_BACKOFF_MS, Ordering::Relaxed);
        self.consecutive_health_failures.store(0, Ordering::Relaxed);

        // Update error window
        {
            let mut window = self.error_window.write().unwrap();
            if window.is_expired() {
                *window = ErrorWindow::new();
            }
            window.record_success();
        }

        // Transition to closed if half-open
        let mut state = self.state.write().unwrap();
        if *state == CircuitState::HalfOpen {
            *state = CircuitState::Closed;
            *self.last_state_change.write().unwrap() = Instant::now();
            log::info!("Circuit breaker closed after successful recovery");
        }
    }

    fn record_error(&self) {
        // Exponential back-off with jitter
        let current = self.current_backoff_ms.load(Ordering::Relaxed);
        let mut next = (current as f64 * BACKOFF_MULTIPLIER) as u64;
        
        // Add jitter
        let jitter = (next as f64 * JITTER_PERCENT * rand::random::<f64>()) as u64;
        next = next.saturating_add(jitter).min(MAX_BACKOFF_MS);
        
        self.current_backoff_ms.store(next, Ordering::Relaxed);

        // Update error window
        let should_open = {
            let mut window = self.error_window.write().unwrap();
            if window.is_expired() {
                *window = ErrorWindow::new();
            }
            window.record_error();

            // Check if we should open the circuit
            let error_rate = window.error_rate();
            let total_requests = window.total_requests();
            let consecutive = window.consecutive_errors.load(Ordering::Relaxed);

            (error_rate > ERROR_THRESHOLD_PERCENT && total_requests >= MIN_REQUESTS_FOR_CIRCUIT) ||
            consecutive >= 5
        };

        // Open circuit if threshold exceeded
        if should_open {
            let mut state = self.state.write().unwrap();
            if *state == CircuitState::Closed {
                *state = CircuitState::Open;
                *self.last_state_change.write().unwrap() = Instant::now();
                log::warn!("Circuit breaker opened due to high error rate");
            }
        }
    }

    fn should_attempt(&self) -> bool {
        let state = self.state.read().unwrap();
        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if we should transition to half-open
                let backoff_duration = Duration::from_millis(
                    self.current_backoff_ms.load(Ordering::Relaxed)
                );
                let last_change = self.last_state_change.read().unwrap();
                
                if last_change.elapsed() > backoff_duration {
                    drop(state);
                    let mut state_mut = self.state.write().unwrap();
                    *state_mut = CircuitState::HalfOpen;
                    *self.last_state_change.write().unwrap() = Instant::now();
                    log::info!("Circuit breaker half-open, testing recovery");
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true, // Allow limited requests
        }
    }

    fn get_backoff_duration(&self) -> Duration {
        Duration::from_millis(self.current_backoff_ms.load(Ordering::Relaxed))
    }

    fn needs_health_check(&self) -> bool {
        let last_check = self.last_health_check.read().unwrap();
        let interval = match *self.state.read().unwrap() {
            CircuitState::Open => Duration::from_millis(RECOVERY_PROBE_INTERVAL_MS),
            _ => Duration::from_millis(HEALTH_CHECK_INTERVAL_MS),
        };
        last_check.elapsed() > interval
    }

    fn record_health_check(&self, success: bool) {
        *self.last_health_check.write().unwrap() = Instant::now();
        
        if success {
            self.consecutive_health_failures.store(0, Ordering::Relaxed);
            // Don't record as normal success to avoid skewing metrics
        } else {
            self.consecutive_health_failures.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Manages multiple egress paths with circuit breakers
pub struct EgressManager {
    circuits: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
    primary_egress: Option<String>,
    fallback_order: Vec<String>,
}

impl EgressManager {
    pub fn new() -> Self {
        Self {
            circuits: Arc::new(RwLock::new(HashMap::new())),
            primary_egress: None,
            fallback_order: Vec::new(),
        }
    }

    pub fn add_egress(&mut self, name: String, is_primary: bool) {
        let mut circuits = self.circuits.write().unwrap();
        circuits.insert(name.clone(), Arc::new(CircuitBreaker::new()));
        
        if is_primary {
            self.primary_egress = Some(name);
        } else {
            self.fallback_order.push(name);
        }
    }

    pub fn record_success(&self, egress_name: &str) {
        if let Some(circuit) = self.get_circuit(egress_name) {
            circuit.record_success();
        }
    }

    pub fn record_error(&self, egress_name: &str) {
        if let Some(circuit) = self.get_circuit(egress_name) {
            circuit.record_error();
        }
    }

    pub fn get_available_egress(&self) -> Option<(String, Arc<CircuitBreaker>)> {
        // Try primary first
        if let Some(ref primary) = self.primary_egress {
            if let Some(circuit) = self.get_circuit(primary) {
                if circuit.should_attempt() {
                    return Some((primary.clone(), circuit));
                }
            }
        }

        // Try fallbacks in order
        for fallback in &self.fallback_order {
            if let Some(circuit) = self.get_circuit(fallback) {
                if circuit.should_attempt() {
                    return Some((fallback.clone(), circuit));
                }
            }
        }

        // If all circuits are open, return the one with shortest back-off
        self.get_least_backed_off_egress()
    }

    fn get_circuit(&self, name: &str) -> Option<Arc<CircuitBreaker>> {
        self.circuits.read().unwrap().get(name).cloned()
    }

    fn get_least_backed_off_egress(&self) -> Option<(String, Arc<CircuitBreaker>)> {
        let circuits = self.circuits.read().unwrap();
        
        circuits.iter()
            .min_by_key(|(_, circuit)| {
                circuit.get_backoff_duration().as_millis()
            })
            .map(|(name, circuit)| (name.clone(), circuit.clone()))
    }

    pub async fn health_check_all(&self) {
        let circuits = self.circuits.read().unwrap().clone();
        
        for (name, circuit) in circuits {
            if circuit.needs_health_check() {
                // Spawn health check task
                let name_clone = name.clone();
                let circuit_clone = circuit.clone();
                
                tokio::spawn(async move {
                    let success = perform_health_check(&name_clone).await;
                    circuit_clone.record_health_check(success);
                });
            }
        }
    }

    pub fn get_stats(&self) -> HashMap<String, EgressStats> {
        let circuits = self.circuits.read().unwrap();
        let mut stats = HashMap::new();

        for (name, circuit) in circuits.iter() {
            let window = circuit.error_window.read().unwrap();
            let state = circuit.state.read().unwrap();
            
            stats.insert(name.clone(), EgressStats {
                state: *state,
                error_rate: window.error_rate(),
                total_requests: window.total_requests(),
                consecutive_errors: window.consecutive_errors.load(Ordering::Relaxed),
                current_backoff_ms: circuit.current_backoff_ms.load(Ordering::Relaxed),
            });
        }

        stats
    }
}

#[derive(Debug, Clone)]
pub struct EgressStats {
    pub state: CircuitState,
    pub error_rate: f64,
    pub total_requests: u64,
    pub consecutive_errors: u64,
    pub current_backoff_ms: u64,
}

/// Perform health check for an egress path
async fn perform_health_check(egress_name: &str) -> bool {
    // Simple TCP connect test
    match egress_name {
        "primary" => test_tcp_connect("8.8.8.8:53").await,
        "cellular" => test_tcp_connect("1.1.1.1:53").await,
        "wifi" => test_tcp_connect("9.9.9.9:53").await,
        _ => false,
    }
}

async fn test_tcp_connect(addr: &str) -> bool {
    use tokio::net::TcpStream;
    use tokio::time::timeout;

    let result = timeout(
        Duration::from_secs(3),
        TcpStream::connect(addr)
    ).await;

    matches!(result, Ok(Ok(_)))
}

/// Example usage in proxy handler
pub async fn handle_with_backoff<F, Fut, T>(
    egress_manager: &EgressManager,
    request: F,
) -> Result<T, std::io::Error>
where
    F: Fn(&str) -> Fut,
    Fut: std::future::Future<Output = Result<T, std::io::Error>>,
{
    let max_retries = 3;
    let mut last_error = None;

    for attempt in 0..max_retries {
        // Get available egress
        let (egress_name, circuit) = match egress_manager.get_available_egress() {
            Some(egress) => egress,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "All egress paths are unavailable"
                ));
            }
        };

        log::debug!("Attempting request via {} (attempt {})", egress_name, attempt + 1);

        // Execute request
        match request(&egress_name).await {
            Ok(val) => {
                egress_manager.record_success(&egress_name);
                return Ok(val);
            }
            Err(e) => {
                egress_manager.record_error(&egress_name);
                last_error = Some(e);
                
                // Wait before retry (unless last attempt)
                if attempt < max_retries - 1 {
                    let backoff = circuit.get_backoff_duration();
                    log::warn!("Request failed on {}, backing off {:?}", egress_name, backoff);
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::Other, "All retries exhausted")
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_opens_on_errors() {
        let circuit = CircuitBreaker::new();
        
        // Record enough errors to open circuit
        for _ in 0..15 {
            circuit.record_success();
        }
        for _ in 0..20 {
            circuit.record_error();
        }

        assert!(!circuit.should_attempt());
        assert_eq!(*circuit.state.read().unwrap(), CircuitState::Open);
    }

    #[test]
    fn test_exponential_backoff() {
        let circuit = CircuitBreaker::new();
        
        assert_eq!(circuit.current_backoff_ms.load(Ordering::Relaxed), INITIAL_BACKOFF_MS);
        
        circuit.record_error();
        let backoff1 = circuit.current_backoff_ms.load(Ordering::Relaxed);
        assert!(backoff1 > INITIAL_BACKOFF_MS);
        
        circuit.record_error();
        let backoff2 = circuit.current_backoff_ms.load(Ordering::Relaxed);
        assert!(backoff2 > backoff1);
        
        circuit.record_success();
        assert_eq!(circuit.current_backoff_ms.load(Ordering::Relaxed), INITIAL_BACKOFF_MS);
    }

    #[tokio::test]
    async fn test_egress_manager_fallback() {
        let mut manager = EgressManager::new();
        manager.add_egress("primary".to_string(), true);
        manager.add_egress("secondary".to_string(), false);
        
        // Fail primary
        for _ in 0..20 {
            manager.record_error("primary");
        }
        
        // Should fallback to secondary
        let (name, _) = manager.get_available_egress().unwrap();
        assert_eq!(name, "secondary");
    }
}