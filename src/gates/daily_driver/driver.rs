//! CLI Driver Implementation
//!
//! Entry point for daily driver CLI commands, integrating with gate controller and state.

use std::sync::Arc;
use tokio::sync::RwLock as TokioRwLock;

use super::cli::{Cli, Commands};
use super::daily_driver::{DailyDriverConfig, DailyDriverState, DriverMode};
use super::switches::{DselSwitchBoard, Switch};
use super::exclusive::{ExclusiveGateController, GateProfile};

/// CLI driver for daily operations
pub struct CliDriver {
    controller: Arc<ExclusiveGateController>,
    state: Arc<DailyDriverState>,
    switchboard: Arc<DselSwitchBoard>,
}

impl CliDriver {
    pub fn new() -> Self {
        let controller = Arc::new(ExclusiveGateController::with_edge_gates());
        let state = Arc::new(DailyDriverState::new(DailyDriverConfig::default()));
        let switchboard = Arc::new(DselSwitchBoard::new());
        
        Self {
            controller,
            state,
            switchboard,
        }
    }

    pub fn with_profile(profile: GateProfile) -> Self {
        let controller = Arc::new(ExclusiveGateController::with_edge_gates());
        controller.set_profile(profile);
        
        let config = match profile {
            GateProfile::Lite => DailyDriverConfig::lite_mode(),
            GateProfile::Standard => DailyDriverConfig::default(),
            GateProfile::Edge => DailyDriverConfig::edge_mode(),
            GateProfile::Expert => DailyDriverConfig::default(),
        };
        
        let state = Arc::new(DailyDriverState::new(config));
        let switchboard = Arc::new(DselSwitchBoard::new_with_profile(profile));
        
        Self {
            controller,
            state,
            switchboard,
        }
    }

    pub async fn execute(&self, cli: Cli) -> Result<String, String> {
        match cli.command {
            Commands::Start { bind, port, mode, max_connections, metrics } => {
                self.start_command(bind, port, mode, max_connections, metrics).await
            }
            Commands::Stop => {
                self.stop_command().await
            }
            Commands::Status => {
                self.status_command().await
            }
            Commands::Profile { profile } => {
                self.profile_command(profile).await
            }
            Commands::Edge { max_memory, max_connections, compression, crypto } => {
                self.edge_command(max_memory, max_connections, compression, crypto).await
            }
            Commands::Switch { compression, crypto, max_connections, idle_timeout } => {
                self.switch_command(compression, crypto, max_connections, idle_timeout).await
            }
        }
    }

    async fn start_command(
        &self,
        bind: String,
        port: u16,
        mode: String,
        max_connections: usize,
        metrics: bool,
    ) -> Result<String, String> {
        let driver_mode = DriverMode::from_str(&mode);
        
        let config = DailyDriverConfig {
            mode: driver_mode,
            bind_address: bind,
            port,
            max_connections,
            metrics_enabled: metrics,
            ..DailyDriverConfig::default()
        };
        
        self.state.update_config(config);
        self.state.set_active(true);
        
        let profile = self.controller.get_profile();
        Ok(format!("✅ Daily driver started in {} mode (profile: {})", 
                   mode, profile.as_str()))
    }

    async fn stop_command(&self) -> Result<String, String> {
        self.state.set_active(false);
        Ok("✅ Daily driver stopped".to_string())
    }

    async fn status_command(&self) -> Result<String, String> {
        let active = self.state.is_active();
        let config = self.state.config();
        let profile = self.controller.get_profile();
        let connections = self.state.connection_count();
        
        let status = format!(
            "📊 Daily Driver Status:\n\
             Active: {}\n\
             Mode: {:?}\n\
             Profile: {}\n\
             Bind: {}:{}\n\
             Connections: {}/{}",
            active,
            config.mode,
            profile.as_str(),
            config.bind_address,
            config.port,
            connections,
            config.max_connections
        );
        
        Ok(status)
    }

    async fn profile_command(&self, profile: String) -> Result<String, String> {
        let gate_profile = GateProfile::from_str(&profile);
        self.controller.set_profile(gate_profile);
        
        let switches = vec![Switch::Profile(gate_profile)];
        for switch in switches {
            self.switchboard.apply(switch);
        }
        
        Ok(format!("✅ Profile set to {} ({})", profile, gate_profile.as_str()))
    }

    async fn edge_command(
        &self,
        max_memory: usize,
        max_connections: usize,
        compression: bool,
        crypto: bool,
    ) -> Result<String, String> {
        let mut runtime = self.switchboard.get_runtime();
        runtime.max_connections = max_connections;
        runtime.compression_enabled = compression;
        runtime.crypto_enabled = crypto;
        
        let config = DailyDriverConfig {
            max_connections,
            ..DailyDriverConfig::default()
        };
        
        self.state.update_config(config);
        
        let profile = self.controller.get_profile();
        let result = format!(
            "✅ Edge configuration applied:\n\
             Memory: {} MB\n\
             Connections: {}\n\
             Compression: {}\n\
             Crypto: {}",
            max_memory / 256,
            max_connections,
            compression,
            crypto
        );
        
        Ok(result)
    }

    async fn switch_command(
        &self,
        compression: Option<bool>,
        crypto: Option<bool>,
        max_connections: Option<usize>,
        idle_timeout: Option<u64>,
    ) -> Result<String, String> {
        let mut changes = Vec::new();
        
        if let Some(comp) = compression {
            changes.push(Switch::Compression(comp));
        }
        if let Some(crypt) = crypto {
            changes.push(Switch::Crypto(crypt));
        }
        if let Some(max_conn) = max_connections {
            changes.push(Switch::MaxConnections(max_conn));
        }
        if let Some(timeout) = idle_timeout {
            changes.push(Switch::IdleTimeout(timeout));
        }
        
        for switch in changes {
            self.switchboard.apply(switch);
        }
        
        let runtime = self.switchboard.get_runtime();
        let result = format!(
            "✅ Runtime switches applied:\n\
             Compression: {}\n\
             Crypto: {}\n\
             Max Connections: {}\n\
             Idle Timeout: {}s",
            runtime.compression_enabled,
            runtime.crypto_enabled,
            runtime.max_connections,
            runtime.idle_timeout_secs
        );
        
        Ok(result)
    }
}

impl Default for CliDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cli_driver_creation() {
        let driver = CliDriver::new();
        assert!(!driver.state.is_active());
    }

    #[tokio::test]
    async fn test_start_command() {
        let driver = CliDriver::new();
        let result = driver.start_command("127.0.0.1".to_string(), 8080, "proxy".to_string(), 25, false).await;
        
        assert!(result.is_ok());
        assert!(driver.state.is_active());
    }

    #[tokio::test]
    async fn test_status_command() {
        let driver = CliDriver::new();
        let result = driver.status_command().await;
        
        assert!(result.is_ok());
        let status = result.unwrap();
        assert!(status.contains("Active: false"));
    }

    #[tokio::test]
    async fn test_profile_command() {
        let driver = CliDriver::new();
        let result = driver.profile_command("edge".to_string()).await;
        
        assert!(result.is_ok());
        assert_eq!(driver.controller.get_profile(), GateProfile::Edge);
    }
}
