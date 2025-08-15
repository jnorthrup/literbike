// TCP Fingerprint Mitigation for Knox Bypass
// Mimics mobile device TCP characteristics to evade carrier detection

use std::collections::HashMap;
use std::net::TcpStream;
use std::os::fd::AsRawFd;
use std::time::{SystemTime, UNIX_EPOCH};
use rand::Rng;
use libc::{setsockopt, SOL_TCP, SO_SNDBUF, SO_RCVBUF, TCP_NODELAY, TCP_KEEPINTVL, TCP_KEEPIDLE};

/// Mobile device TCP characteristics
pub struct TcpFingerprint {
    pub window_size: u32,
    pub mss: u16,
    pub ttl: u8,
    pub window_scale: u8,
    pub timestamp_enabled: bool,
    pub sack_enabled: bool,
    pub nodelay: bool,
    pub keepalive_idle: u32,
    pub keepalive_interval: u32,
}

/// Mobile device profiles for different carriers/manufacturers
#[derive(Debug, Clone)]
pub enum MobileProfile {
    IPhone14,
    IPhone15,
    SamsungS24,
    PixelPro7,
    OnePlus11,
}

impl MobileProfile {
    /// Get realistic TCP parameters for mobile device
    pub fn get_tcp_fingerprint(&self) -> TcpFingerprint {
        match self {
            MobileProfile::IPhone14 => TcpFingerprint {
                window_size: 65535,
                mss: 1460,
                ttl: 64,
                window_scale: 6,
                timestamp_enabled: true,
                sack_enabled: true,
                nodelay: false,
                keepalive_idle: 7200,
                keepalive_interval: 75,
            },
            MobileProfile::IPhone15 => TcpFingerprint {
                window_size: 131072,
                mss: 1460,
                ttl: 64,
                window_scale: 7,
                timestamp_enabled: true,
                sack_enabled: true,
                nodelay: false,
                keepalive_idle: 7200,
                keepalive_interval: 75,
            },
            MobileProfile::SamsungS24 => TcpFingerprint {
                window_size: 87380,
                mss: 1440,
                ttl: 64,
                window_scale: 6,
                timestamp_enabled: true,
                sack_enabled: true,
                nodelay: true,
                keepalive_idle: 7200,
                keepalive_interval: 60,
            },
            MobileProfile::PixelPro7 => TcpFingerprint {
                window_size: 65536,
                mss: 1460,
                ttl: 64,
                window_scale: 6,
                timestamp_enabled: true,
                sack_enabled: true,
                nodelay: false,
                keepalive_idle: 7200,
                keepalive_interval: 60,
            },
            MobileProfile::OnePlus11 => TcpFingerprint {
                window_size: 87380,
                mss: 1460,
                ttl: 64,
                window_scale: 6,
                timestamp_enabled: true,
                sack_enabled: true,
                nodelay: true,
                keepalive_idle: 7200,
                keepalive_interval: 60,
            },
        }
    }
}

/// TCP fingerprint manager for Knox bypass
pub struct TcpFingerprintManager {
    current_profile: MobileProfile,
    rotation_enabled: bool,
    profile_history: Vec<(SystemTime, MobileProfile)>,
}

impl TcpFingerprintManager {
    pub fn new() -> Self {
        Self {
            current_profile: Self::select_random_profile(),
            rotation_enabled: true,
            profile_history: Vec::new(),
        }
    }
    
    /// Select a random mobile profile weighted by popularity
    fn select_random_profile() -> MobileProfile {
        let mut rng = rand::thread_rng();
        let profiles = vec![
            (MobileProfile::IPhone14, 25),      // 25% weight
            (MobileProfile::IPhone15, 30),      // 30% weight  
            (MobileProfile::SamsungS24, 20),    // 20% weight
            (MobileProfile::PixelPro7, 15),     // 15% weight
            (MobileProfile::OnePlus11, 10),     // 10% weight
        ];
        
        let total_weight: u32 = profiles.iter().map(|(_, w)| w).sum();
        let mut choice = rng.gen_range(0..total_weight);
        
        for (profile, weight) in profiles {
            if choice < weight {
                return profile;
            }
            choice -= weight;
        }
        
        MobileProfile::IPhone15 // Fallback
    }
    
    /// Rotate to a new profile periodically
    pub fn maybe_rotate_profile(&mut self) {
        if !self.rotation_enabled {
            return;
        }
        
        let now = SystemTime::now();
        
        // Rotate every 10-30 minutes randomly
        if let Some((last_rotation, _)) = self.profile_history.last() {
            let elapsed = now.duration_since(*last_rotation).unwrap_or_default();
            let rotation_interval = rand::thread_rng().gen_range(600..1800); // 10-30 min
            
            if elapsed.as_secs() > rotation_interval {
                let new_profile = Self::select_random_profile();
                self.profile_history.push((now, new_profile.clone()));
                self.current_profile = new_profile;
                
                // Keep only last 10 rotations
                if self.profile_history.len() > 10 {
                    self.profile_history.remove(0);
                }
            }
        } else {
            // First rotation
            self.profile_history.push((now, self.current_profile.clone()));
        }
    }
    
    /// Apply mobile TCP fingerprint to socket
    pub fn apply_fingerprint(&self, stream: &TcpStream) -> std::io::Result<()> {
        let fingerprint = self.current_profile.get_tcp_fingerprint();
        let fd = stream.as_raw_fd();
        
        unsafe {
            // Set send buffer size (mobile devices typically have smaller buffers)
            let send_buf = (fingerprint.window_size / 2) as i32;
            if setsockopt(
                fd,
                libc::SOL_SOCKET,
                SO_SNDBUF,
                &send_buf as *const _ as *const libc::c_void,
                std::mem::size_of::<i32>() as u32,
            ) != 0 {
                log::warn!("Failed to set SO_SNDBUF");
            }
            
            // Set receive buffer size
            let recv_buf = fingerprint.window_size as i32;
            if setsockopt(
                fd,
                libc::SOL_SOCKET,
                SO_RCVBUF,
                &recv_buf as *const _ as *const libc::c_void,
                std::mem::size_of::<i32>() as u32,
            ) != 0 {
                log::warn!("Failed to set SO_RCVBUF");
            }
            
            // Set TCP_NODELAY based on profile
            let nodelay = if fingerprint.nodelay { 1i32 } else { 0i32 };
            if setsockopt(
                fd,
                SOL_TCP,
                TCP_NODELAY,
                &nodelay as *const _ as *const libc::c_void,
                std::mem::size_of::<i32>() as u32,
            ) != 0 {
                log::warn!("Failed to set TCP_NODELAY");
            }
            
            // Set keepalive parameters
            let keepalive_idle = fingerprint.keepalive_idle as i32;
            if setsockopt(
                fd,
                SOL_TCP,
                TCP_KEEPIDLE,
                &keepalive_idle as *const _ as *const libc::c_void,
                std::mem::size_of::<i32>() as u32,
            ) != 0 {
                log::warn!("Failed to set TCP_KEEPIDLE");
            }
            
            let keepalive_interval = fingerprint.keepalive_interval as i32;
            if setsockopt(
                fd,
                SOL_TCP,
                TCP_KEEPINTVL,
                &keepalive_interval as *const _ as *const libc::c_void,
                std::mem::size_of::<i32>() as u32,
            ) != 0 {
                log::warn!("Failed to set TCP_KEEPINTVL");
            }
        }
        
        log::debug!("Applied TCP fingerprint: {:?}", self.current_profile);
        Ok(())
    }
    
    /// Get current profile information
    pub fn current_profile(&self) -> &MobileProfile {
        &self.current_profile
    }
    
    /// Generate realistic TCP Initial Sequence Number
    pub fn generate_mobile_isn(&self) -> u32 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;
            
        // Mobile devices often use time-based ISN with some randomization
        let time_component = now.wrapping_mul(4096);
        let random_component = rand::thread_rng().gen::<u16>() as u32;
        
        time_component.wrapping_add(random_component)
    }
    
    /// Calculate appropriate MSS for current profile and path MTU
    pub fn calculate_mss(&self, path_mtu: u16) -> u16 {
        let base_mss = self.current_profile.get_tcp_fingerprint().mss;
        
        // Account for headers (20 TCP + 20 IP = 40 bytes minimum)
        let max_mss = path_mtu.saturating_sub(40);
        
        std::cmp::min(base_mss, max_mss)
    }
    
    /// Get window scaling factor for current profile
    pub fn get_window_scale(&self) -> u8 {
        self.current_profile.get_tcp_fingerprint().window_scale
    }
    
    /// Check if timestamps should be enabled
    pub fn timestamp_enabled(&self) -> bool {
        self.current_profile.get_tcp_fingerprint().timestamp_enabled
    }
    
    /// Check if SACK should be enabled
    pub fn sack_enabled(&self) -> bool {
        self.current_profile.get_tcp_fingerprint().sack_enabled
    }
}

impl Default for TcpFingerprintManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Mobile device TCP congestion control algorithms
#[derive(Debug, Clone)]
pub enum MobileCongestionControl {
    Cubic,      // Default on Android
    Bbr,        // Modern high-performance
    Reno,       // Conservative fallback
}

impl MobileCongestionControl {
    /// Get typical congestion control for mobile profile
    pub fn for_profile(profile: &MobileProfile) -> Self {
        match profile {
            MobileProfile::IPhone14 | MobileProfile::IPhone15 => Self::Cubic,
            MobileProfile::SamsungS24 => Self::Bbr,
            MobileProfile::PixelPro7 => Self::Bbr,
            MobileProfile::OnePlus11 => Self::Cubic,
        }
    }
    
    /// Apply congestion control to socket (Linux-specific)
    #[cfg(target_os = "linux")]
    pub fn apply_to_socket(&self, stream: &TcpStream) -> std::io::Result<()> {
        use std::ffi::CString;
        
        let algorithm = match self {
            Self::Cubic => "cubic",
            Self::Bbr => "bbr",
            Self::Reno => "reno",
        };
        
        let fd = stream.as_raw_fd();
        let algorithm_cstr = CString::new(algorithm).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid algorithm name")
        })?;
        
        unsafe {
            if setsockopt(
                fd,
                libc::IPPROTO_TCP,
                libc::TCP_CONGESTION,
                algorithm_cstr.as_ptr() as *const libc::c_void,
                algorithm_cstr.as_bytes().len() as u32,
            ) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        
        log::debug!("Applied congestion control: {}", algorithm);
        Ok(())
    }
    
    #[cfg(not(target_os = "linux"))]
    pub fn apply_to_socket(&self, _stream: &TcpStream) -> std::io::Result<()> {
        // Congestion control setting is Linux-specific
        log::debug!("Congestion control not supported on this platform");
        Ok(())
    }
}

/// Mobile-specific TCP option handling
pub struct MobileTcpOptions {
    pub mss: Option<u16>,
    pub window_scale: Option<u8>,
    pub timestamp: bool,
    pub sack_permitted: bool,
    pub no_operation_padding: bool,
}

impl MobileTcpOptions {
    /// Generate mobile-typical TCP options
    pub fn for_profile(profile: &MobileProfile) -> Self {
        let fingerprint = profile.get_tcp_fingerprint();
        
        Self {
            mss: Some(fingerprint.mss),
            window_scale: Some(fingerprint.window_scale),
            timestamp: fingerprint.timestamp_enabled,
            sack_permitted: fingerprint.sack_enabled,
            no_operation_padding: true, // Mobile devices often include NOPs
        }
    }
    
    /// Encode TCP options for raw socket usage
    pub fn encode(&self) -> Vec<u8> {
        let mut options = Vec::new();
        
        // MSS option (kind=2, length=4)
        if let Some(mss) = self.mss {
            options.extend_from_slice(&[2, 4]);
            options.extend_from_slice(&mss.to_be_bytes());
        }
        
        // SACK permitted (kind=4, length=2)
        if self.sack_permitted {
            options.extend_from_slice(&[4, 2]);
        }
        
        // Timestamp (kind=8, length=10)
        if self.timestamp {
            options.extend_from_slice(&[8, 10]);
            // Timestamp value and echo reply (8 bytes)
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u32;
            options.extend_from_slice(&timestamp.to_be_bytes());
            options.extend_from_slice(&[0, 0, 0, 0]); // Echo reply
        }
        
        // NOP padding to align to 4-byte boundary
        if self.no_operation_padding {
            while options.len() % 4 != 0 {
                options.push(1); // NOP
            }
        }
        
        // Window scale (kind=3, length=3)
        if let Some(scale) = self.window_scale {
            if options.len() + 3 <= 40 { // Max 40 bytes of options
                options.extend_from_slice(&[3, 3, scale]);
            }
        }
        
        // Final NOP padding
        while options.len() % 4 != 0 && options.len() < 40 {
            options.push(1); // NOP
        }
        
        options
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mobile_profile_selection() {
        let manager = TcpFingerprintManager::new();
        let fingerprint = manager.current_profile.get_tcp_fingerprint();
        
        // All mobile devices should have TTL 64
        assert_eq!(fingerprint.ttl, 64);
        
        // Window size should be realistic for mobile
        assert!(fingerprint.window_size >= 32768);
        assert!(fingerprint.window_size <= 262144);
        
        // MSS should be in mobile range
        assert!(fingerprint.mss >= 1360);
        assert!(fingerprint.mss <= 1500);
    }
    
    #[test]
    fn test_tcp_options_encoding() {
        let profile = MobileProfile::IPhone15;
        let options = MobileTcpOptions::for_profile(&profile);
        let encoded = options.encode();
        
        // Should contain MSS option
        assert!(encoded.contains(&2)); // MSS kind
        
        // Should not be empty
        assert!(!encoded.is_empty());
        
        // Should be properly padded
        assert_eq!(encoded.len() % 4, 0);
    }
    
    #[test]
    fn test_isn_generation() {
        let manager = TcpFingerprintManager::new();
        let isn1 = manager.generate_mobile_isn();
        let isn2 = manager.generate_mobile_isn();
        
        // ISNs should be different
        assert_ne!(isn1, isn2);
        
        // Should be non-zero (extremely unlikely to be zero)
        assert_ne!(isn1, 0);
    }
}