//! DSEL Switches for Runtime Profile Control
//!
//! Domain-specific expression language for dynamic configuration
//! and runtime switching of Litebike profiles.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::cli::RuntimeSwitches;
use super::daily_driver::DriverMode;
use super::edge_profile::EdgeProfileConfig;
use super::exclusive::GateProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DselSwitches {
    pub current_profile: GateProfile,
    pub runtime: RuntimeSwitches,
    pub driver_mode: DriverMode,
}

impl Default for DselSwitches {
    fn default() -> Self {
        Self {
            current_profile: GateProfile::Lite,
            runtime: RuntimeSwitches::default(),
            driver_mode: DriverMode::Proxy,
        }
    }
}

impl DselSwitches {
    pub fn new(profile: GateProfile) -> Self {
        let runtime = match profile {
            GateProfile::Lite => RuntimeSwitches {
                compression_enabled: true,
                crypto_enabled: false,
                max_connections: 10,
                idle_timeout_secs: 60,
                metrics_enabled: false,
            },
            GateProfile::Standard => RuntimeSwitches::default(),
            GateProfile::Edge => RuntimeSwitches {
                compression_enabled: true,
                crypto_enabled: true,
                max_connections: 50,
                idle_timeout_secs: 300,
                metrics_enabled: true,
            },
            GateProfile::Expert => RuntimeSwitches {
                compression_enabled: false,
                crypto_enabled: true,
                max_connections: 100,
                idle_timeout_secs: 600,
                metrics_enabled: true,
            },
        };

        Self {
            current_profile: profile,
            runtime,
            driver_mode: DriverMode::Proxy,
        }
    }

    pub fn switch_to(&mut self, profile: GateProfile) {
        let new_runtime = match profile {
            GateProfile::Lite => RuntimeSwitches {
                compression_enabled: true,
                crypto_enabled: false,
                max_connections: 10,
                idle_timeout_secs: 60,
                metrics_enabled: false,
            },
            GateProfile::Standard => RuntimeSwitches::default(),
            GateProfile::Edge => RuntimeSwitches {
                compression_enabled: true,
                crypto_enabled: true,
                max_connections: 50,
                idle_timeout_secs: 300,
                metrics_enabled: true,
            },
            GateProfile::Expert => RuntimeSwitches {
                compression_enabled: false,
                crypto_enabled: true,
                max_connections: 100,
                idle_timeout_secs: 600,
                metrics_enabled: true,
            },
        };

        self.current_profile = profile;
        self.runtime = new_runtime;
    }

    pub fn apply_switch(&mut self, switch: &Switch) {
        match switch {
            Switch::Compression(enabled) => {
                self.runtime.compression_enabled = *enabled;
            }
            Switch::Crypto(enabled) => {
                self.runtime.crypto_enabled = *enabled;
            }
            Switch::MaxConnections(max) => {
                self.runtime.max_connections = *max;
            }
            Switch::IdleTimeout(secs) => {
                self.runtime.idle_timeout_secs = *secs;
            }
            Switch::Metrics(enabled) => {
                self.runtime.metrics_enabled = *enabled;
            }
            Switch::Profile(profile) => {
                self.switch_to(*profile);
            }
            Switch::DriverMode(mode) => {
                self.driver_mode = *mode;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Switch {
    Compression(bool),
    Crypto(bool),
    MaxConnections(usize),
    IdleTimeout(u64),
    Metrics(bool),
    Profile(GateProfile),
    DriverMode(DriverMode),
}

pub struct DselSwitchBoard {
    switches: Arc<RwLock<DselSwitches>>,
    history: Arc<RwLock<Vec<Switch>>>,
}

impl DselSwitchBoard {
    pub fn new() -> Self {
        Self {
            switches: Arc::new(RwLock::new(DselSwitches::default())),
            history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn new_with_profile(profile: GateProfile) -> Self {
        Self {
            switches: Arc::new(RwLock::new(DselSwitches::new(profile))),
            history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn apply(&self, switch: Switch) {
        let mut switches = self.switches.write();
        switches.apply_switch(&switch);

        let mut history = self.history.write();
        history.push(switch);
    }

    pub fn get_state(&self) -> DselSwitches {
        self.switches.read().clone()
    }

    pub fn get_profile(&self) -> GateProfile {
        self.switches.read().current_profile
    }

    pub fn get_runtime(&self) -> RuntimeSwitches {
        self.switches.read().runtime.clone()
    }

    pub fn history(&self) -> Vec<Switch> {
        self.history.read().clone()
    }

    pub fn clear_history(&self) {
        self.history.write().clear();
    }
}

impl Default for DselSwitchBoard {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse DSEL expression into switches
pub fn parse_dsel(expr: &str) -> Result<Vec<Switch>, String> {
    let mut switches = Vec::new();
    let expr = expr.trim();

    for part in expr.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let parts: Vec<&str> = part.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid switch: {}", part));
        }

        let key = parts[0].trim().to_lowercase();
        let value = parts[1].trim();

        match key.as_str() {
            "profile" => {
                switches.push(Switch::Profile(GateProfile::from_str(value)));
            }
            "compression" | "compress" => {
                switches.push(Switch::Compression(
                    value == "true" || value == "1" || value == "on",
                ));
            }
            "crypto" | "crypt" => {
                switches.push(Switch::Crypto(
                    value == "true" || value == "1" || value == "on",
                ));
            }
            "max_conn" | "max_connections" => {
                let max: usize = value
                    .parse()
                    .map_err(|_| format!("Invalid number: {}", value))?;
                switches.push(Switch::MaxConnections(max));
            }
            "idle" | "idle_timeout" => {
                let secs: u64 = value
                    .parse()
                    .map_err(|_| format!("Invalid number: {}", value))?;
                switches.push(Switch::IdleTimeout(secs));
            }
            "metrics" => {
                switches.push(Switch::Metrics(
                    value == "true" || value == "1" || value == "on",
                ));
            }
            "mode" => {
                switches.push(Switch::DriverMode(DriverMode::from_str(value)));
            }
            _ => {
                return Err(format!("Unknown switch: {}", key));
            }
        }
    }

    Ok(switches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dsel_parsing() {
        let switches = parse_dsel("profile=edge; compression=false; max_connections=50").unwrap();

        assert_eq!(switches.len(), 3);

        if let Switch::Profile(p) = &switches[0] {
            assert_eq!(p, &GateProfile::Edge);
        } else {
            panic!("Expected profile switch");
        }

        if let Switch::Compression(c) = &switches[1] {
            assert!(!c);
        }
    }

    #[test]
    fn test_switch_board() {
        let board = DselSwitchBoard::new();

        board.apply(Switch::Profile(GateProfile::Edge));
        assert_eq!(board.get_profile(), GateProfile::Edge);

        let runtime = board.get_runtime();
        assert!(runtime.crypto_enabled);
    }
}
