//! Daily Driver - Litebike as Daily Driver
//!
//! Provides a streamlined, reduced-footprint integration of Litebike
//! optimized for everyday use as a daily driver with minimal resource usage.

use std::sync::Arc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

pub mod cli;
pub mod switches;
pub mod driver;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriverMode {
    Proxy,
    Gateway,
    Tunnel,
    Monitor,
}

impl Default for DriverMode {
    fn default() -> Self {
        DriverMode::Proxy
    }
}

impl DriverMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "proxy" => DriverMode::Proxy,
            "gateway" | "gw" => DriverMode::Gateway,
            "tunnel" | "tun" => DriverMode::Tunnel,
            "monitor" | "mon" => DriverMode::Monitor,
            _ => DriverMode::Proxy,
        }
    }
}

/// Daily driver configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyDriverConfig {
    pub mode: DriverMode,
    pub bind_address: String,
    pub port: u16,
    pub max_connections: usize,
    pub buffer_size: usize,
    pub idle_timeout_secs: u64,
    pub metrics_enabled: bool,
}

impl Default for DailyDriverConfig {
    fn default() -> Self {
        Self {
            mode: DriverMode::Proxy,
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            max_connections: 25,
            buffer_size: 4096,
            idle_timeout_secs: 300,
            metrics_enabled: false,
        }
    }
}

impl DailyDriverConfig {
    pub fn lite_mode() -> Self {
        Self {
            mode: DriverMode::Proxy,
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            max_connections: 10,
            buffer_size: 2048,
            idle_timeout_secs: 60,
            metrics_enabled: false,
        }
    }

    pub fn edge_mode() -> Self {
        Self {
            mode: DriverMode::Gateway,
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
            max_connections: 50,
            buffer_size: 4096,
            idle_timeout_secs: 300,
            metrics_enabled: true,
        }
    }
}

/// Daily driver state
pub struct DailyDriverState {
    config: Arc<RwLock<DailyDriverConfig>>,
    active: Arc<RwLock<bool>>,
    connections: Arc<RwLock<usize>>,
}

impl DailyDriverState {
    pub fn new(config: DailyDriverConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            active: Arc::new(RwLock::new(false)),
            connections: Arc::new(RwLock::new(0)),
        }
    }

    pub fn is_active(&self) -> bool {
        *self.active.read()
    }

    pub fn set_active(&self, active: bool) {
        *self.active.write() = active;
    }

    pub fn increment_connections(&self) {
        let mut conns = self.connections.write();
        *conns += 1;
    }

    pub fn decrement_connections(&self) {
        let mut conns = self.connections.write();
        *conns = conns.saturating_sub(1);
    }

    pub fn connection_count(&self) -> usize {
        *self.connections.read()
    }

    pub fn config(&self) -> impl Clone {
        self.config.read().clone()
    }

    pub fn update_config(&self, config: DailyDriverConfig) {
        *self.config.write() = config;
    }
}

impl Default for DailyDriverState {
    fn default() -> Self {
        Self::new(DailyDriverConfig::default())
    }
}

/// Memory-efficient connection tracker
pub struct ConnectionTracker {
    max_connections: usize,
    active: Arc<RwLock<usize>>,
}

impl ConnectionTracker {
    pub fn new(max: usize) -> Self {
        Self {
            max_connections: max,
            active: Arc::new(RwLock::new(0)),
        }
    }

    pub fn try_acquire(&self) -> bool {
        let mut active = self.active.write();
        if *active < self.max_connections {
            *active += 1;
            true
        } else {
            false
        }
    }

    pub fn release(&self) {
        let mut active = self.active.write();
        *active = active.saturating_sub(1);
    }

    pub fn active_count(&self) -> usize {
        *self.active.read()
    }

    pub fn available(&self) -> usize {
        self.max_connections.saturating_sub(*self.active.read())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_mode_parsing() {
        assert_eq!(DriverMode::from_str("proxy"), DriverMode::Proxy);
        assert_eq!(DriverMode::from_str("gateway"), DriverMode::Gateway);
        assert_eq!(DriverMode::from_str("tunnel"), DriverMode::Tunnel);
    }

    #[test]
    fn test_lite_mode_config() {
        let config = DailyDriverConfig::lite_mode();
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.buffer_size, 2048);
    }

    #[test]
    fn test_connection_tracker() {
        let tracker = ConnectionTracker::new(2);
        
        assert!(tracker.try_acquire());
        assert!(tracker.try_acquire());
        assert!(!tracker.try_acquire()); // Should fail
        
        tracker.release();
        assert!(tracker.try_acquire()); // Should work again
    }
}
