//! SSH Gate for LiteBike - Termux-native sshd integration
//!
//! Uses OpenSSH for pubkey authentication, then unlocks keys from keymux.
//! This provides a secure unlock mechanism where keys are only available
//! to authenticated SSH sessions.

use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::{Gate, GateError};

/// SSH session state
#[derive(Debug, Clone)]
pub struct SshSession {
    /// SSH session ID
    pub session_id: String,
    /// Authenticated username
    pub username: String,
    /// Public key fingerprint (SHA256)
    pub pubkey_fingerprint: String,
    /// Unlocked keys from keymux
    pub unlocked_keys: HashMap<String, String>,
    /// Session created timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last activity timestamp
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

/// SSH Gate configuration
#[derive(Debug, Clone)]
pub struct SshGateConfig {
    /// SSH port (default: 2222)
    pub port: u16,
    /// Keymux URL for unlocking keys
    pub keymux_url: String,
    /// Session timeout in seconds
    pub session_timeout: u64,
    /// Max sessions per pubkey
    pub max_sessions_per_key: usize,
}

impl Default for SshGateConfig {
    fn default() -> Self {
        Self {
            port: 2222,
            keymux_url: "http://127.0.0.1:8888".to_string(),
            session_timeout: 3600,
            max_sessions_per_key: 3,
        }
    }
}

/// SSH Gate - handles OpenSSH connections and key unlocking
pub struct SshGate {
    enabled: Arc<RwLock<bool>>,
    config: Arc<RwLock<SshGateConfig>>,
    sessions: Arc<RwLock<HashMap<String, SshSession>>>,
    registered_pubkeys: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl SshGate {
    pub fn new() -> Self {
        Self {
            enabled: Arc::new(RwLock::new(true)),
            config: Arc::new(RwLock::new(SshGateConfig::default())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            registered_pubkeys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_config(config: SshGateConfig) -> Self {
        Self {
            enabled: Arc::new(RwLock::new(true)),
            config: Arc::new(RwLock::new(config)),
            registered_pubkeys: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn enable(&self) {
        *self.enabled.write() = true;
    }

    pub fn disable(&self) {
        *self.enabled.write() = false;
    }

    /// Register a public key for unlocking
    pub fn register_pubkey(&self, pubkey: &str, allowed_providers: Vec<String>) {
        let fingerprint = Self::fingerprint_pubkey(pubkey);
        self.registered_pubkeys.write().insert(fingerprint.clone(), allowed_providers);
        log::info!("Registered pubkey: {}", fingerprint);
    }

    /// Unregister a public key
    pub fn unregister_pubkey(&self, pubkey: &str) {
        let fingerprint = Self::fingerprint_pubkey(pubkey);
        self.registered_pubkeys.write().remove(&fingerprint);
        log::info!("Unregistered pubkey: {}", fingerprint);
    }

    /// Generate SHA256 fingerprint from pubkey
    fn fingerprint_pubkey(pubkey: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(pubkey.as_bytes());
        let result = hasher.finalize();
        format!("SHA256:{}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, result))
    }

    /// Create a new SSH session after successful pubkey auth
    pub fn create_session(&self, username: &str, pubkey_fingerprint: &str) -> Option<String> {
        let config = self.config.read();
        let registered = self.registered_pubkeys.read();
        
        // Check if pubkey is registered
        let allowed_providers = registered.get(pubkey_fingerprint)?;
        
        // Check session limit
        let sessions = self.sessions.read();
        let active_count = sessions.values()
            .filter(|s| s.pubkey_fingerprint == pubkey_fingerprint)
            .count();
        
        if active_count >= config.max_sessions_per_key {
            log::warn!("Max sessions reached for pubkey: {}", pubkey_fingerprint);
            return None;
        }
        drop(sessions);
        
        // Generate session ID
        let session_id = uuid::Uuid::new_v4().to_string();
        
        // Create session
        let session = SshSession {
            session_id: session_id.clone(),
            username: username.to_string(),
            pubkey_fingerprint: pubkey_fingerprint.to_string(),
            unlocked_keys: HashMap::new(),
            created_at: chrono::Utc::now(),
            last_activity: chrono::Utc::now(),
        };
        
        self.sessions.write().insert(session_id.clone(), session);
        log::info!("Created SSH session: {} for user: {}", session_id, username);
        
        Some(session_id)
    }

    /// Unlock keys for a session from keymux
    pub async fn unlock_keys(&self, session_id: &str, providers: &[&str]) -> Result<HashMap<String, String>, GateError> {
        let config = self.config.read();
        let registered = self.registered_pubkeys.read();
        
        let session = self.sessions.read()
            .get(session_id)
            .cloned()
            .ok_or_else(|| GateError::ConnectionFailed("Session not found".to_string()))?;
        
        let allowed_providers = registered.get(&session.pubkey_fingerprint)
            .ok_or_else(|| GateError::ProcessingFailed("Pubkey not registered".to_string()))?;
        
        // Filter requested providers to allowed ones
        let providers_to_unlock: Vec<&str> = providers.iter()
            .filter(|p| allowed_providers.contains(&p.to_string()))
            .copied()
            .collect();
        
        // Request keys from keymux
        let client = reqwest::Client::new();
        let mut unlocked = HashMap::new();
        
        for provider in providers_to_unlock {
            let url = format!("{}/unlock/{}", config.keymux_url, provider);
            
            let response = client
                .post(&url)
                .header("X-Session-ID", session_id)
                .header("X-Pubkey-Fingerprint", &session.pubkey_fingerprint)
                .send()
                .await;
            
            match response {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(key) = resp.text().await {
                        unlocked.insert(provider.to_string(), key);
                        log::info!("Unlocked key for provider: {}", provider);
                    }
                }
                Ok(resp) => {
                    log::warn!("Failed to unlock key for {}: HTTP {}", provider, resp.status());
                }
                Err(e) => {
                    log::error!("Failed to request key for {}: {}", provider, e);
                }
            }
        }
        
        // Update session with unlocked keys
        if let Some(session) = self.sessions.write().get_mut(session_id) {
            session.unlocked_keys.extend(unlocked.clone());
            session.last_activity = chrono::Utc::now();
        }
        
        Ok(unlocked)
    }

    /// End an SSH session
    pub fn end_session(&self, session_id: &str) {
        if self.sessions.write().remove(session_id).is_some() {
            log::info!("Ended SSH session: {}", session_id);
        }
    }

    /// Clean up expired sessions
    pub fn cleanup_expired_sessions(&self) {
        let config = self.config.read();
        let timeout = chrono::Duration::seconds(config.session_timeout as i64);
        let now = chrono::Utc::now();
        
        let mut sessions = self.sessions.write();
        let expired: Vec<String> = sessions.iter()
            .filter(|(_, s)| now - s.last_activity > timeout)
            .map(|(id, _)| id.clone())
            .collect();
        
        for id in expired {
            sessions.remove(&id);
            log::info!("Cleaned up expired session: {}", id);
        }
    }

    /// Get session by ID
    pub fn get_session(&self, session_id: &str) -> Option<SshSession> {
        self.sessions.read().get(session_id).cloned()
    }

    /// List all active sessions
    pub fn list_sessions(&self) -> Vec<SshSession> {
        self.sessions.read().values().cloned().collect()
    }

    /// Detect SSH protocol
    fn detect_ssh(&self, data: &[u8]) -> bool {
        if data.len() < 4 {
            return false;
        }
        
        // SSH-2.0-* identification string
        data.starts_with(b"SSH-2.0-") || data.starts_with(b"SSH-1.99-")
    }

    /// Handle SSH handshake
    async fn handle_ssh_connection(&self, stream: TcpStream) -> Result<(), GateError> {
        let mut stream = stream;
        let mut buf = [0u8; 1024];
        
        // Read SSH identification
        let n = stream.read(&mut buf).await
            .map_err(|e| GateError::ConnectionFailed(e.to_string()))?;
        
        let ident = String::from_utf8_lossy(&buf[..n]);
        log::debug!("SSH identification: {}", ident.trim());
        
        // Send our identification
        stream.write_all(b"SSH-2.0-LiteBike_1.0\r\n").await
            .map_err(|e| GateError::ConnectionFailed(e.to_string()))?;
        
        // In a full implementation, we would:
        // 1. Handle key exchange (curve25519-sha256)
        // 2. Handle user auth (publickey method)
        // 3. On successful pubkey auth, create session and unlock keys
        // 4. Handle channel requests (exec, shell, etc.)
        
        // For now, log and close
        log::info!("SSH connection initiated");
        
        Ok(())
    }
}

#[async_trait]
impl Gate for SshGate {
    async fn is_open(&self, data: &[u8]) -> bool {
        *self.enabled.read() && self.detect_ssh(data)
    }

    async fn process(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        if !self.is_open(data).await {
            return Err("SSH gate is closed".to_string());
        }
        
        // Return SSH identification
        Ok(b"SSH-2.0-LiteBike_1.0\r\n".to_vec())
    }

    async fn process_connection(&self, _data: &[u8], stream: Option<TcpStream>) -> Result<Vec<u8>, GateError> {
        if let Some(stream) = stream {
            self.handle_ssh_connection(stream).await?;
        }
        Ok(vec![])
    }

    fn name(&self) -> &str {
        "ssh"
    }

    fn children(&self) -> Vec<Arc<dyn Gate>> {
        vec![]
    }

    fn priority(&self) -> u8 {
        95 // Higher priority - SSH should be detected early
    }

    fn can_handle_protocol(&self, protocol: &str) -> bool {
        matches!(protocol, "ssh" | "ssh2")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_detection() {
        let gate = SshGate::new();
        
        assert!(gate.detect_ssh(b"SSH-2.0-OpenSSH_9.0\r\n"));
        assert!(gate.detect_ssh(b"SSH-1.99-OpenSSH_8.0\r\n"));
        assert!(!gate.detect_ssh(b"GET / HTTP/1.1\r\n"));
        assert!(!gate.detect_ssh(b"POST /api HTTP/1.1\r\n"));
    }

    #[test]
    fn test_pubkey_fingerprint() {
        let pubkey = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq test@example.com";
        let fingerprint = SshGate::fingerprint_pubkey(pubkey);
        assert!(fingerprint.starts_with("SHA256:"));
    }

    #[test]
    fn test_session_management() {
        let gate = SshGate::new();
        gate.register_pubkey("test-pubkey", vec!["openai".to_string(), "anthropic".to_string()]);
        
        let session_id = gate.create_session("testuser", "SHA256:testfingerprint");
        assert!(session_id.is_some());
        
        let sessions = gate.list_sessions();
        assert_eq!(sessions.len(), 1);
        
        gate.end_session(&session_id.unwrap());
        let sessions = gate.list_sessions();
        assert_eq!(sessions.len(), 0);
    }

    #[tokio::test]
    async fn test_ssh_gate_is_open() {
        let gate = SshGate::new();
        
        let ssh_data = b"SSH-2.0-OpenSSH_9.0\r\n";
        assert!(gate.is_open(ssh_data).await);
        
        let http_data = b"GET / HTTP/1.1\r\n";
        assert!(!gate.is_open(http_data).await);
    }
}
