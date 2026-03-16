//! Daily Driver CLI
//!
//! Command-line interface for Litebike daily driver operations.

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

use super::daily_driver::{DailyDriverConfig, DriverMode};
use super::edge_profile::EdgeProfileConfig;
use super::exclusive::GateProfile;

#[derive(Parser)]
#[command(name = "litebike")]
#[command(about = "Litebike daily driver - lightweight proxy gateway", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the daily driver proxy
    Start {
        /// Bind address
        #[arg(short, long, default_value = "127.0.0.1")]
        bind: String,

        /// Port number
        #[arg(short, long, default_value_t = 8080)]
        port: u16,

        /// Driver mode: proxy, gateway, tunnel, monitor
        #[arg(long, default_value = "proxy")]
        mode: String,

        /// Maximum connections
        #[arg(long, default_value_t = 25)]
        max_connections: usize,

        /// Enable metrics
        #[arg(long, default_value_t = false)]
        metrics: bool,
    },

    /// Stop the daily driver
    Stop,

    /// Show status
    Status,

    /// Configure gate profile
    Profile {
        /// Profile: lite, standard, edge, expert
        #[arg()]
        profile: String,
    },

    /// Configure edge profile
    Edge {
        /// Max memory in MB
        #[arg(long, default_value_t = 256)]
        max_memory: usize,

        /// Max connections
        #[arg(long, default_value_t = 50)]
        max_connections: usize,

        /// Enable compression
        #[arg(long, default_value_t = true)]
        compression: bool,

        /// Enable crypto
        #[arg(long, default_value_t = true)]
        crypto: bool,
    },

    /// Switches for runtime control
    Switch {
        /// Toggle compression
        #[arg(long)]
        compression: Option<bool>,

        /// Toggle crypto
        #[arg(long)]
        crypto: Option<bool>,

        /// Set max connections
        #[arg(long)]
        max_connections: Option<usize>,

        /// Set idle timeout (seconds)
        #[arg(long)]
        idle_timeout: Option<u64>,
    },
}

impl Cli {
    pub fn parse_args() -> Self {
        Cli::parse()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverStatus {
    pub active: bool,
    pub mode: String,
    pub bind_address: String,
    pub port: u16,
    pub connections: usize,
    pub max_connections: usize,
    pub profile: String,
}

impl Default for DriverStatus {
    fn default() -> Self {
        Self {
            active: false,
            mode: "proxy".to_string(),
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            connections: 0,
            max_connections: 25,
            profile: "lite".to_string(),
        }
    }
}

/// Runtime switch configuration for dynamic updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSwitches {
    pub compression_enabled: bool,
    pub crypto_enabled: bool,
    pub max_connections: usize,
    pub idle_timeout_secs: u64,
    pub metrics_enabled: bool,
}

impl Default for RuntimeSwitches {
    fn default() -> Self {
        Self {
            compression_enabled: true,
            crypto_enabled: true,
            max_connections: 25,
            idle_timeout_secs: 300,
            metrics_enabled: false,
        }
    }
}

impl RuntimeSwitches {
    pub fn apply_from_cli(&mut self, switches: &Switches) {
        if let Some(compression) = switches.compression {
            self.compression_enabled = compression;
        }
        if let Some(crypto) = switches.crypto {
            self.crypto_enabled = crypto;
        }
        if let Some(max_conn) = switches.max_connections {
            self.max_connections = max_conn;
        }
        if let Some(timeout) = switches.idle_timeout {
            self.idle_timeout_secs = timeout;
        }
    }
}

pub struct Switches {
    pub compression: Option<bool>,
    pub crypto: Option<bool>,
    pub max_connections: Option<usize>,
    pub idle_timeout: Option<u64>,
}

impl Switches {
    pub fn new() -> Self {
        Self {
            compression: None,
            crypto: None,
            max_connections: None,
            idle_timeout: None,
        }
    }

    pub fn with_compression(mut self, enabled: bool) -> Self {
        self.compression = Some(enabled);
        self
    }

    pub fn with_crypto(mut self, enabled: bool) -> Self {
        self.crypto = Some(enabled);
        self
    }

    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = Some(max);
        self
    }

    pub fn with_idle_timeout(mut self, secs: u64) -> Self {
        self.idle_timeout = Some(secs);
        self
    }
}

impl Default for Switches {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_status() {
        let status = DriverStatus::default();
        assert!(!status.active);
        assert_eq!(status.port, 8080);
    }

    #[test]
    fn test_runtime_switches() {
        let mut switches = RuntimeSwitches::default();
        let update = Switches::new()
            .with_compression(false)
            .with_max_connections(100);

        switches.apply_from_cli(&update);

        assert!(!switches.compression_enabled);
        assert_eq!(switches.max_connections, 100);
    }
}
