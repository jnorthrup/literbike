// Crypto Gate for LITEBIKE
// Gates all cryptographic operations

use async_trait::async_trait;
use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;

pub struct CryptoGate {
    enabled: Arc<RwLock<bool>>,
    allowed_methods: Arc<RwLock<HashMap<String, bool>>>,
}

impl CryptoGate {
    pub fn new() -> Self {
        let mut allowed_methods = HashMap::new();
        
        // All crypto methods gated initially
        allowed_methods.insert("aes-128-gcm".to_string(), false);
        allowed_methods.insert("aes-256-gcm".to_string(), false);
        allowed_methods.insert("chacha20-poly1305".to_string(), false);
        allowed_methods.insert("xchacha20-poly1305".to_string(), false);
        allowed_methods.insert("aes-128-cfb".to_string(), false);
        allowed_methods.insert("aes-256-cfb".to_string(), false);
        allowed_methods.insert("aes-128-ctr".to_string(), false);
        allowed_methods.insert("aes-256-ctr".to_string(), false);
        allowed_methods.insert("blake3-chacha20-poly1305".to_string(), false);
        allowed_methods.insert("aes-256-gcm-siv".to_string(), false);
        
        Self {
            enabled: Arc::new(RwLock::new(false)),
            allowed_methods: Arc::new(RwLock::new(allowed_methods)),
        }
    }
    
    pub fn enable_method(&self, method: &str) {
        let mut methods = self.allowed_methods.write();
        if let Some(enabled) = methods.get_mut(method) {
            *enabled = true;
            println!("Crypto gate: Enabled {}", method);
        }
    }
    
    pub fn disable_method(&self, method: &str) {
        let mut methods = self.allowed_methods.write();
        if let Some(enabled) = methods.get_mut(method) {
            *enabled = false;
            println!("Crypto gate: Disabled {}", method);
        }
    }
    
    pub fn enable_all(&self) {
        *self.enabled.write() = true;
        let mut methods = self.allowed_methods.write();
        for (method, enabled) in methods.iter_mut() {
            *enabled = true;
            println!("Crypto gate: Enabled {}", method);
        }
    }
    
    pub fn disable_all(&self) {
        *self.enabled.write() = false;
        let mut methods = self.allowed_methods.write();
        for (method, enabled) in methods.iter_mut() {
            *enabled = false;
        }
        println!("Crypto gate: All methods disabled");
    }
    
    pub fn is_method_allowed(&self, method: &str) -> bool {
        let methods = self.allowed_methods.read();
        methods.get(method).copied().unwrap_or(false)
    }
}

#[async_trait]
impl super::Gate for CryptoGate {
    async fn is_open(&self) -> bool {
        *self.enabled.read()
    }
    
    async fn process(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        if !self.is_open().await {
            return Err("Crypto gate is closed".to_string());
        }
        
        // Check if this looks like encrypted data
        let entropy = calculate_entropy(data);
        if entropy < 6.0 {
            return Err("Data doesn't appear to be encrypted".to_string());
        }
        
        println!("Processing through crypto gate (entropy: {:.2})", entropy);
        
        // Would perform crypto operations here based on allowed methods
        Ok(data.to_vec())
    }
    
    fn name(&self) -> &str {
        "crypto"
    }
    
    fn children(&self) -> Vec<Arc<dyn super::Gate>> {
        vec![]
    }
}

fn calculate_entropy(data: &[u8]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    
    let mut frequencies = [0u32; 256];
    for &byte in data {
        frequencies[byte as usize] += 1;
    }
    
    let len = data.len() as f32;
    let mut entropy = 0.0;
    
    for &count in &frequencies {
        if count > 0 {
            let p = count as f32 / len;
            entropy -= p * p.log2();
        }
    }
    
    entropy
}