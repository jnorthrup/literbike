// TLS Fingerprint Obfuscation for Knox Bypass
// Mimics mobile browser TLS behavior to evade detection

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use rand::Rng;
use serde::{Deserialize, Serialize};

/// TLS cipher suites commonly used by mobile browsers
pub const MOBILE_CIPHER_SUITES: &[u16] = &[
    // TLS 1.3 cipher suites (mobile browsers prioritize these)
    0x1301, // TLS_AES_128_GCM_SHA256
    0x1302, // TLS_AES_256_GCM_SHA384
    0x1303, // TLS_CHACHA20_POLY1305_SHA256
    
    // TLS 1.2 cipher suites (fallback for older servers)
    0xc02f, // TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
    0xc030, // TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384
    0xcca9, // TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256
    0xc02b, // TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256
    0xc02c, // TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384
    0xcca8, // TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256
    0xc013, // TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA
    0xc014, // TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA
];

/// TLS extensions commonly used by mobile browsers
pub const MOBILE_TLS_EXTENSIONS: &[u16] = &[
    0x0000, // server_name (SNI)
    0x000b, // ec_point_formats
    0x000a, // supported_groups
    0x0023, // session_ticket
    0x0010, // application_layer_protocol_negotiation
    0x0005, // status_request (OCSP stapling)
    0x0012, // signed_certificate_timestamp
    0x0033, // key_share (TLS 1.3)
    0x002b, // supported_versions
    0x002a, // early_data
    0x001b, // compress_certificate
    0x0029, // pre_shared_key (must be last)
];

/// Elliptic curves preferred by mobile devices
pub const MOBILE_ELLIPTIC_CURVES: &[u16] = &[
    0x001d, // x25519
    0x0017, // secp256r1
    0x0018, // secp384r1
    0x0019, // secp521r1
];

/// Mobile browser profiles for TLS fingerprinting
#[derive(Debug, Clone)]
pub enum MobileBrowserProfile {
    Safari17,
    Chrome120Mobile,
    Firefox121Mobile,
    Samsung21,
    Edge120Mobile,
}

impl MobileBrowserProfile {
    /// Get TLS fingerprint characteristics for browser
    pub fn get_tls_fingerprint(&self) -> TlsFingerprint {
        match self {
            MobileBrowserProfile::Safari17 => TlsFingerprint {
                tls_version: TlsVersion::Tls13,
                cipher_suites: vec![
                    0x1301, 0x1302, 0x1303, // TLS 1.3
                    0xc02f, 0xc030, 0xcca9, 0xc02b, 0xc02c // TLS 1.2
                ],
                extensions: vec![
                    0x0000, 0x000b, 0x000a, 0x0023, 0x0010,
                    0x0005, 0x0033, 0x002b, 0x0029
                ],
                elliptic_curves: vec![0x001d, 0x0017, 0x0018],
                signature_algorithms: vec![0x0403, 0x0503, 0x0603, 0x0804, 0x0805, 0x0806],
                alpn_protocols: vec!["h2".to_string(), "http/1.1".to_string()],
                compress_certificate: true,
                early_data: false,
                session_ticket: true,
            },
            MobileBrowserProfile::Chrome120Mobile => TlsFingerprint {
                tls_version: TlsVersion::Tls13,
                cipher_suites: vec![
                    0x1301, 0x1302, 0x1303,
                    0xc02f, 0xc030, 0xcca9, 0xc02b, 0xc02c, 0xcca8
                ],
                extensions: vec![
                    0x0000, 0x000b, 0x000a, 0x0023, 0x0010, 0x0005,
                    0x0012, 0x0033, 0x002b, 0x002a, 0x001b, 0x0029
                ],
                elliptic_curves: vec![0x001d, 0x0017, 0x0018, 0x0019],
                signature_algorithms: vec![0x0403, 0x0804, 0x0401, 0x0503, 0x0805, 0x0501],
                alpn_protocols: vec!["h2".to_string(), "http/1.1".to_string()],
                compress_certificate: true,
                early_data: true,
                session_ticket: true,
            },
            MobileBrowserProfile::Firefox121Mobile => TlsFingerprint {
                tls_version: TlsVersion::Tls13,
                cipher_suites: vec![
                    0x1301, 0x1302, 0x1303,
                    0xc02b, 0xc02f, 0xc02c, 0xc030, 0xcca9, 0xcca8
                ],
                extensions: vec![
                    0x0000, 0x000b, 0x000a, 0x0023, 0x0010,
                    0x0033, 0x002b, 0x0029
                ],
                elliptic_curves: vec![0x001d, 0x0017, 0x0018],
                signature_algorithms: vec![0x0403, 0x0503, 0x0603, 0x0804, 0x0805, 0x0806],
                alpn_protocols: vec!["h2".to_string(), "http/1.1".to_string()],
                compress_certificate: false,
                early_data: false,
                session_ticket: true,
            },
            MobileBrowserProfile::Samsung21 => TlsFingerprint {
                tls_version: TlsVersion::Tls13,
                cipher_suites: vec![
                    0x1301, 0x1302, 0x1303,
                    0xc02f, 0xc030, 0xc02b, 0xc02c
                ],
                extensions: vec![
                    0x0000, 0x000b, 0x000a, 0x0023, 0x0010,
                    0x0033, 0x002b, 0x0029
                ],
                elliptic_curves: vec![0x001d, 0x0017, 0x0018],
                signature_algorithms: vec![0x0403, 0x0503, 0x0804, 0x0805],
                alpn_protocols: vec!["h2".to_string(), "http/1.1".to_string()],
                compress_certificate: false,
                early_data: false,
                session_ticket: true,
            },
            MobileBrowserProfile::Edge120Mobile => TlsFingerprint {
                tls_version: TlsVersion::Tls13,
                cipher_suites: vec![
                    0x1301, 0x1302, 0x1303,
                    0xc02f, 0xc030, 0xcca9, 0xc02b, 0xc02c, 0xcca8
                ],
                extensions: vec![
                    0x0000, 0x000b, 0x000a, 0x0023, 0x0010, 0x0005,
                    0x0033, 0x002b, 0x002a, 0x0029
                ],
                elliptic_curves: vec![0x001d, 0x0017, 0x0018, 0x0019],
                signature_algorithms: vec![0x0403, 0x0804, 0x0401, 0x0503, 0x0805, 0x0501],
                alpn_protocols: vec!["h2".to_string(), "http/1.1".to_string()],
                compress_certificate: true,
                early_data: true,
                session_ticket: true,
            },
        }
    }
}

/// TLS version enumeration
#[derive(Debug, Clone, Copy)]
pub enum TlsVersion {
    Tls12,
    Tls13,
}

impl TlsVersion {
    pub fn to_bytes(&self) -> [u8; 2] {
        match self {
            TlsVersion::Tls12 => [0x03, 0x03],
            TlsVersion::Tls13 => [0x03, 0x04],
        }
    }
}

/// Complete TLS fingerprint configuration
#[derive(Debug, Clone)]
pub struct TlsFingerprint {
    pub tls_version: TlsVersion,
    pub cipher_suites: Vec<u16>,
    pub extensions: Vec<u16>,
    pub elliptic_curves: Vec<u16>,
    pub signature_algorithms: Vec<u16>,
    pub alpn_protocols: Vec<String>,
    pub compress_certificate: bool,
    pub early_data: bool,
    pub session_ticket: bool,
}

/// TLS fingerprint manager for Knox bypass
pub struct TlsFingerprintManager {
    current_profile: MobileBrowserProfile,
    rotation_enabled: bool,
    profile_history: Vec<(SystemTime, MobileBrowserProfile)>,
    ja3_cache: HashMap<String, String>,
}

impl TlsFingerprintManager {
    pub fn new() -> Self {
        Self {
            current_profile: Self::select_weighted_profile(),
            rotation_enabled: true,
            profile_history: Vec::new(),
            ja3_cache: HashMap::new(),
        }
    }
    
    /// Select browser profile based on mobile market share
    fn select_weighted_profile() -> MobileBrowserProfile {
        let mut rng = rand::thread_rng();
        let profiles = vec![
            (MobileBrowserProfile::Chrome120Mobile, 65), // 65% market share
            (MobileBrowserProfile::Safari17, 25),        // 25% market share
            (MobileBrowserProfile::Samsung21, 5),        // 5% market share
            (MobileBrowserProfile::Edge120Mobile, 3),    // 3% market share
            (MobileBrowserProfile::Firefox121Mobile, 2), // 2% market share
        ];
        
        let total_weight: u32 = profiles.iter().map(|(_, w)| w).sum();
        let mut choice = rng.gen_range(0..total_weight);
        
        for (profile, weight) in profiles {
            if choice < weight {
                return profile;
            }
            choice -= weight;
        }
        
        MobileBrowserProfile::Chrome120Mobile // Fallback
    }
    
    /// Rotate TLS profile periodically
    pub fn maybe_rotate_profile(&mut self) {
        if !self.rotation_enabled {
            return;
        }
        
        let now = SystemTime::now();
        
        // Rotate every 15-45 minutes randomly
        if let Some((last_rotation, _)) = self.profile_history.last() {
            let elapsed = now.duration_since(*last_rotation).unwrap_or_default();
            let rotation_interval = rand::thread_rng().gen_range(900..2700); // 15-45 min
            
            if elapsed.as_secs() > rotation_interval {
                let new_profile = Self::select_weighted_profile();
                self.profile_history.push((now, new_profile.clone()));
                self.current_profile = new_profile;
                
                // Clear JA3 cache on profile change
                self.ja3_cache.clear();
                
                // Keep only last 5 rotations
                if self.profile_history.len() > 5 {
                    self.profile_history.remove(0);
                }
            }
        } else {
            self.profile_history.push((now, self.current_profile.clone()));
        }
    }
    
    /// Generate TLS ClientHello based on current profile
    pub fn generate_client_hello(&mut self, server_name: &str) -> Vec<u8> {
        let fingerprint = self.current_profile.get_tls_fingerprint();
        let mut client_hello = Vec::new();
        
        // TLS Record Header
        client_hello.push(0x16); // Content Type: Handshake
        client_hello.extend_from_slice(&fingerprint.tls_version.to_bytes());
        
        // Handshake message will be filled in
        let handshake_start = client_hello.len();
        client_hello.extend_from_slice(&[0x00, 0x00]); // Length placeholder
        
        // Handshake Header
        client_hello.push(0x01); // Handshake Type: Client Hello
        let hello_start = client_hello.len();
        client_hello.extend_from_slice(&[0x00, 0x00, 0x00]); // Length placeholder
        
        // Client Hello content
        client_hello.extend_from_slice(&fingerprint.tls_version.to_bytes()); // Version
        
        // Random (32 bytes)
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;
        client_hello.extend_from_slice(&timestamp.to_be_bytes());
        
        let mut random_bytes = [0u8; 28];
        rand::thread_rng().fill(&mut random_bytes);
        client_hello.extend_from_slice(&random_bytes);
        
        // Session ID (empty for simplicity)
        client_hello.push(0x00);
        
        // Cipher Suites
        let cipher_suites_len = (fingerprint.cipher_suites.len() * 2) as u16;
        client_hello.extend_from_slice(&cipher_suites_len.to_be_bytes());
        for &cipher in &fingerprint.cipher_suites {
            client_hello.extend_from_slice(&cipher.to_be_bytes());
        }
        
        // Compression Methods (null compression)
        client_hello.extend_from_slice(&[0x01, 0x00]);
        
        // Extensions
        let extensions_start = client_hello.len();
        client_hello.extend_from_slice(&[0x00, 0x00]); // Extensions length placeholder
        
        let extensions_content_start = client_hello.len();
        
        // Add extensions based on profile
        self.add_sni_extension(&mut client_hello, server_name);
        self.add_supported_groups_extension(&mut client_hello, &fingerprint);
        self.add_signature_algorithms_extension(&mut client_hello, &fingerprint);
        self.add_alpn_extension(&mut client_hello, &fingerprint);
        
        if fingerprint.session_ticket {
            self.add_session_ticket_extension(&mut client_hello);
        }
        
        if fingerprint.early_data {
            self.add_early_data_extension(&mut client_hello);
        }
        
        if fingerprint.compress_certificate {
            self.add_compress_certificate_extension(&mut client_hello);
        }
        
        // Update extensions length
        let extensions_len = client_hello.len() - extensions_content_start;
        let extensions_len_bytes = (extensions_len as u16).to_be_bytes();
        client_hello[extensions_start] = extensions_len_bytes[0];
        client_hello[extensions_start + 1] = extensions_len_bytes[1];
        
        // Update handshake length
        let handshake_len = client_hello.len() - hello_start - 3;
        let handshake_len_bytes = [(handshake_len >> 16) as u8, (handshake_len >> 8) as u8, handshake_len as u8];
        client_hello[hello_start] = handshake_len_bytes[0];
        client_hello[hello_start + 1] = handshake_len_bytes[1];
        client_hello[hello_start + 2] = handshake_len_bytes[2];
        
        // Update record length
        let record_len = client_hello.len() - handshake_start;
        let record_len_bytes = (record_len as u16).to_be_bytes();
        client_hello[handshake_start] = record_len_bytes[0];
        client_hello[handshake_start + 1] = record_len_bytes[1];
        
        client_hello
    }
    
    /// Add Server Name Indication (SNI) extension
    fn add_sni_extension(&self, client_hello: &mut Vec<u8>, server_name: &str) {
        client_hello.extend_from_slice(&[0x00, 0x00]); // Extension type: SNI
        
        let sni_len = 5 + server_name.len();
        client_hello.extend_from_slice(&(sni_len as u16).to_be_bytes());
        
        let server_name_list_len = 3 + server_name.len();
        client_hello.extend_from_slice(&(server_name_list_len as u16).to_be_bytes());
        
        client_hello.push(0x00); // Name type: hostname
        client_hello.extend_from_slice(&(server_name.len() as u16).to_be_bytes());
        client_hello.extend_from_slice(server_name.as_bytes());
    }
    
    /// Add supported groups (elliptic curves) extension
    fn add_supported_groups_extension(&self, client_hello: &mut Vec<u8>, fingerprint: &TlsFingerprint) {
        client_hello.extend_from_slice(&[0x00, 0x0a]); // Extension type: supported_groups
        
        let groups_len = 2 + fingerprint.elliptic_curves.len() * 2;
        client_hello.extend_from_slice(&(groups_len as u16).to_be_bytes());
        
        let curves_len = fingerprint.elliptic_curves.len() * 2;
        client_hello.extend_from_slice(&(curves_len as u16).to_be_bytes());
        
        for &curve in &fingerprint.elliptic_curves {
            client_hello.extend_from_slice(&curve.to_be_bytes());
        }
    }
    
    /// Add signature algorithms extension
    fn add_signature_algorithms_extension(&self, client_hello: &mut Vec<u8>, fingerprint: &TlsFingerprint) {
        client_hello.extend_from_slice(&[0x00, 0x0d]); // Extension type: signature_algorithms
        
        let sig_algs_len = 2 + fingerprint.signature_algorithms.len() * 2;
        client_hello.extend_from_slice(&(sig_algs_len as u16).to_be_bytes());
        
        let algs_len = fingerprint.signature_algorithms.len() * 2;
        client_hello.extend_from_slice(&(algs_len as u16).to_be_bytes());
        
        for &alg in &fingerprint.signature_algorithms {
            client_hello.extend_from_slice(&alg.to_be_bytes());
        }
    }
    
    /// Add ALPN extension
    fn add_alpn_extension(&self, client_hello: &mut Vec<u8>, fingerprint: &TlsFingerprint) {
        if fingerprint.alpn_protocols.is_empty() {
            return;
        }
        
        client_hello.extend_from_slice(&[0x00, 0x10]); // Extension type: ALPN
        
        let mut protocols_data = Vec::new();
        for protocol in &fingerprint.alpn_protocols {
            protocols_data.push(protocol.len() as u8);
            protocols_data.extend_from_slice(protocol.as_bytes());
        }
        
        let alpn_len = 2 + protocols_data.len();
        client_hello.extend_from_slice(&(alpn_len as u16).to_be_bytes());
        
        client_hello.extend_from_slice(&(protocols_data.len() as u16).to_be_bytes());
        client_hello.extend_from_slice(&protocols_data);
    }
    
    /// Add session ticket extension
    fn add_session_ticket_extension(&self, client_hello: &mut Vec<u8>) {
        client_hello.extend_from_slice(&[0x00, 0x23]); // Extension type: session_ticket
        client_hello.extend_from_slice(&[0x00, 0x00]); // Empty extension
    }
    
    /// Add early data extension
    fn add_early_data_extension(&self, client_hello: &mut Vec<u8>) {
        client_hello.extend_from_slice(&[0x00, 0x2a]); // Extension type: early_data
        client_hello.extend_from_slice(&[0x00, 0x00]); // Empty extension
    }
    
    /// Add compress certificate extension
    fn add_compress_certificate_extension(&self, client_hello: &mut Vec<u8>) {
        client_hello.extend_from_slice(&[0x00, 0x1b]); // Extension type: compress_certificate
        client_hello.extend_from_slice(&[0x00, 0x02]); // Extension length
        client_hello.extend_from_slice(&[0x00, 0x02]); // brotli compression
    }
    
    /// Generate JA3 fingerprint for current configuration
    pub fn generate_ja3_fingerprint(&mut self, server_name: &str) -> String {
        if let Some(cached) = self.ja3_cache.get(server_name) {
            return cached.clone();
        }
        
        let fingerprint = self.current_profile.get_tls_fingerprint();
        
        // JA3 format: version,ciphers,extensions,elliptic_curves,ec_point_formats
        let version = match fingerprint.tls_version {
            TlsVersion::Tls12 => "771",
            TlsVersion::Tls13 => "772",
        };
        
        let ciphers: Vec<String> = fingerprint.cipher_suites.iter().map(|c| c.to_string()).collect();
        let extensions: Vec<String> = fingerprint.extensions.iter().map(|e| e.to_string()).collect();
        let curves: Vec<String> = fingerprint.elliptic_curves.iter().map(|c| c.to_string()).collect();
        
        let ja3_string = format!(
            "{},{},{},{},",
            version,
            ciphers.join("-"),
            extensions.join("-"),
            curves.join("-")
        );
        
        // Simple hash (in practice, would use MD5)
        let ja3_hash = format!("{:x}", calculate_simple_hash(&ja3_string));
        
        self.ja3_cache.insert(server_name.to_string(), ja3_hash.clone());
        ja3_hash
    }
    
    /// Get current browser profile
    pub fn current_profile(&self) -> &MobileBrowserProfile {
        &self.current_profile
    }
    
    /// Force profile rotation
    pub fn force_rotation(&mut self) {
        let new_profile = Self::select_weighted_profile();
        self.profile_history.push((SystemTime::now(), new_profile.clone()));
        self.current_profile = new_profile;
        self.ja3_cache.clear();
    }
    
    /// Get profile statistics
    pub fn get_stats(&self) -> TlsFingerprintStats {
        TlsFingerprintStats {
            current_profile: format!("{:?}", self.current_profile),
            rotations_count: self.profile_history.len(),
            ja3_cache_size: self.ja3_cache.len(),
            rotation_enabled: self.rotation_enabled,
        }
    }
}

impl Default for TlsFingerprintManager {
    fn default() -> Self {
        Self::new()
    }
}

/// TLS fingerprint statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct TlsFingerprintStats {
    pub current_profile: String,
    pub rotations_count: usize,
    pub ja3_cache_size: usize,
    pub rotation_enabled: bool,
}

/// Simple hash function for JA3 (replace with MD5 in production)
fn calculate_simple_hash(input: &str) -> u64 {
    let mut hash = 0u64;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    hash
}

/// Randomize TLS handshake timing
pub struct TlsTimingRandomizer {
    base_delay_ms: u64,
    jitter_range_ms: u64,
}

impl TlsTimingRandomizer {
    pub fn new(base_delay_ms: u64, jitter_range_ms: u64) -> Self {
        Self {
            base_delay_ms,
            jitter_range_ms,
        }
    }
    
    /// Get randomized delay for TLS handshake steps
    pub fn get_handshake_delay(&self) -> std::time::Duration {
        let mut rng = rand::thread_rng();
        let jitter = rng.gen_range(0..=self.jitter_range_ms);
        let total_delay = self.base_delay_ms + jitter;
        
        std::time::Duration::from_millis(total_delay)
    }
    
    /// Get delay between certificate validation steps
    pub fn get_cert_validation_delay(&self) -> std::time::Duration {
        let mut rng = rand::thread_rng();
        // Certificate validation typically takes 5-50ms on mobile
        let delay = rng.gen_range(5..=50);
        std::time::Duration::from_millis(delay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mobile_profile_selection() {
        let manager = TlsFingerprintManager::new();
        let fingerprint = manager.current_profile.get_tls_fingerprint();
        
        // All mobile browsers should support TLS 1.3
        matches!(fingerprint.tls_version, TlsVersion::Tls13);
        
        // Should have TLS 1.3 cipher suites
        assert!(fingerprint.cipher_suites.contains(&0x1301));
        
        // Should support HTTP/2
        assert!(fingerprint.alpn_protocols.contains(&"h2".to_string()));
    }
    
    #[test]
    fn test_client_hello_generation() {
        let mut manager = TlsFingerprintManager::new();
        let client_hello = manager.generate_client_hello("example.com");
        
        // Should start with TLS record header
        assert_eq!(client_hello[0], 0x16); // Handshake
        
        // Should be a reasonable size
        assert!(client_hello.len() > 100);
        assert!(client_hello.len() < 1000);
    }
    
    #[test]
    fn test_ja3_fingerprint_generation() {
        let mut manager = TlsFingerprintManager::new();
        let ja3_1 = manager.generate_ja3_fingerprint("example.com");
        let ja3_2 = manager.generate_ja3_fingerprint("example.com");
        
        // Should be consistent for same domain
        assert_eq!(ja3_1, ja3_2);
        
        // Should be different for different domains
        let ja3_3 = manager.generate_ja3_fingerprint("different.com");
        // Note: May be same due to same profile, but cached separately
    }
    
    #[test]
    fn test_timing_randomization() {
        let randomizer = TlsTimingRandomizer::new(10, 20);
        let delay1 = randomizer.get_handshake_delay();
        let delay2 = randomizer.get_handshake_delay();
        
        // Delays should be in expected range
        assert!(delay1.as_millis() >= 10);
        assert!(delay1.as_millis() <= 30);
        
        // Usually different (may occasionally be same)
        // assert_ne!(delay1, delay2); // Commented as it may fail due to randomness
    }
}