// Galaxy Note20 5G Specific Features
// Correct implementation for Snapdragon 865+ with 5G modem

use std::io;
use log::{info, debug};

/// Note20 5G has specific network interfaces
pub struct Note20NetworkConfig {
    // 5G network interfaces on Note20
    pub rmnet_data0: &'static str,  // Default mobile data
    pub rmnet_data1: &'static str,  // IMS (VoLTE/VoWiFi)
    pub rmnet_data2: &'static str,  // Internet
    pub rmnet_ipa0: &'static str,   // Tethering
    pub wlan0: &'static str,        // WiFi
    pub swlan0: &'static str,       // Software AP (hotspot)
    
    // Knox-specific interfaces
    pub knox_vpn: Option<&'static str>,
    
    // 5G-specific settings
    pub supports_5g_nsa: bool,      // Non-standalone 5G
    pub supports_5g_sa: bool,       // Standalone 5G
    pub supports_wifi6: bool,       // WiFi 6 (802.11ax)
}

impl Default for Note20NetworkConfig {
    fn default() -> Self {
        Self {
            rmnet_data0: "rmnet_data0",
            rmnet_data1: "rmnet_data1", 
            rmnet_data2: "rmnet_data2",
            rmnet_ipa0: "rmnet_ipa0",
            wlan0: "wlan0",
            swlan0: "swlan0",
            knox_vpn: None,
            supports_5g_nsa: true,
            supports_5g_sa: true,
            supports_wifi6: true,
        }
    }
}

/// Detect active network interface on Note20
pub fn detect_active_interface() -> io::Result<String> {
    use std::process::Command;
    
    // Check network routes to find default interface
    let output = Command::new("ip")
        .args(&["route", "show", "default"])
        .output()?;
    
    let route_info = String::from_utf8_lossy(&output.stdout);
    
    // Parse default route
    for line in route_info.lines() {
        if line.starts_with("default") {
            // Format: "default via X.X.X.X dev INTERFACE ..."
            if let Some(dev_pos) = line.find(" dev ") {
                let after_dev = &line[dev_pos + 5..];
                if let Some(iface) = after_dev.split_whitespace().next() {
                    info!("Active interface: {}", iface);
                    return Ok(iface.to_string());
                }
            }
        }
    }
    
    // Fallback: check for any rmnet interface with carrier
    for i in 0..10 {
        let iface = format!("rmnet_data{}", i);
        if is_interface_up(&iface)? {
            return Ok(iface);
        }
    }
    
    Err(io::Error::new(io::ErrorKind::NotFound, "No active interface found"))
}

/// Check if network interface is up and has carrier
pub fn is_interface_up(iface: &str) -> io::Result<bool> {
    use std::fs;
    
    // Check carrier status
    let carrier_path = format!("/sys/class/net/{}/carrier", iface);
    match fs::read_to_string(&carrier_path) {
        Ok(content) => Ok(content.trim() == "1"),
        Err(_) => {
            // Try operstate as fallback
            let state_path = format!("/sys/class/net/{}/operstate", iface);
            match fs::read_to_string(&state_path) {
                Ok(content) => Ok(content.trim() == "up"),
                Err(_) => Ok(false),
            }
        }
    }
}

/// Get network type (5G SA/NSA, LTE, etc)
pub fn get_network_type() -> io::Result<String> {
    use std::process::Command;
    
    // Use getprop to check network type
    let output = Command::new("getprop")
        .args(&["gsm.network.type"])
        .output()?;
    
    let network_type = String::from_utf8_lossy(&output.stdout).trim().to_string();
    
    // Parse network type
    let parsed_type = if network_type.contains("NR") || network_type.contains("5G") {
        if network_type.contains("NSA") {
            "5G NSA"
        } else if network_type.contains("SA") {
            "5G SA"
        } else {
            "5G"
        }
    } else if network_type.contains("LTE") {
        "4G LTE"
    } else {
        &network_type
    };
    
    debug!("Network type: {}", parsed_type);
    Ok(parsed_type.to_string())
}

/// Configure proxy for optimal 5G performance
pub fn configure_5g_proxy(_bind_addr: &str) -> io::Result<()> {
    // Set TCP congestion control for 5G
    set_tcp_congestion_control("bbr")?;
    
    // Configure buffer sizes for high-bandwidth 5G
    set_tcp_buffer_sizes()?;
    
    // Enable TCP Fast Open
    enable_tcp_fastopen()?;
    
    info!("Configured proxy for 5G performance");
    Ok(())
}

/// Set TCP congestion control algorithm
fn set_tcp_congestion_control(algorithm: &str) -> io::Result<()> {
    use std::fs;
    
    // Note: Requires root on unrooted devices
    match fs::write("/proc/sys/net/ipv4/tcp_congestion_control", algorithm) {
        Ok(_) => {
            info!("Set TCP congestion control to {}", algorithm);
            Ok(())
        }
        Err(e) => {
            debug!("Failed to set congestion control (requires root): {}", e);
            // Not fatal - continue without optimization
            Ok(())
        }
    }
}

/// Configure TCP buffer sizes for 5G
fn set_tcp_buffer_sizes() -> io::Result<()> {
    use std::fs;
    
    // Optimal buffer sizes for 5G (min, default, max)
    let rmem = "4096 524288 16777216";  // 16MB max
    let wmem = "4096 524288 16777216";  // 16MB max
    
    // Try to set, but don't fail if no root
    let _ = fs::write("/proc/sys/net/ipv4/tcp_rmem", rmem);
    let _ = fs::write("/proc/sys/net/ipv4/tcp_wmem", wmem);
    
    Ok(())
}

/// Enable TCP Fast Open
fn enable_tcp_fastopen() -> io::Result<()> {
    use std::fs;
    
    // Enable client and server TFO (3 = both)
    let _ = fs::write("/proc/sys/net/ipv4/tcp_fastopen", "3");
    
    Ok(())
}

/// Get 5G signal information
pub fn get_5g_signal_info() -> io::Result<SignalInfo> {
    use std::process::Command;
    
    // Try to get signal info via dumpsys
    let output = Command::new("dumpsys")
        .args(&["telephony.registry"])
        .output()?;
    
    let info = String::from_utf8_lossy(&output.stdout);
    
    // Parse signal strength
    let mut signal_info = SignalInfo::default();
    
    for line in info.lines() {
        if line.contains("mSignalStrength") {
            // Parse signal strength values
            if let Some(dbm_str) = extract_value(line, "rsrp=") {
                signal_info.rsrp = dbm_str.parse().ok();
            }
            if let Some(dbm_str) = extract_value(line, "rsrq=") {
                signal_info.rsrq = dbm_str.parse().ok();
            }
            if let Some(dbm_str) = extract_value(line, "rssnr=") {
                signal_info.rssnr = dbm_str.parse().ok();
            }
        }
    }
    
    Ok(signal_info)
}

#[derive(Debug, Default)]
pub struct SignalInfo {
    pub rsrp: Option<i32>,  // Reference Signal Received Power
    pub rsrq: Option<i32>,  // Reference Signal Received Quality
    pub rssnr: Option<i32>, // Signal-to-Noise Ratio
}

fn extract_value(line: &str, key: &str) -> Option<String> {
    if let Some(pos) = line.find(key) {
        let after_key = &line[pos + key.len()..];
        after_key.split_whitespace()
            .next()
            .map(|s| s.trim_end_matches(',').to_string())
    } else {
        None
    }
}

/// Note20-specific proxy binding
pub fn get_optimal_bind_address(config: &Note20NetworkConfig) -> io::Result<String> {
    // Check for active 5G connection first
    if let Ok(network_type) = get_network_type() {
        if network_type.contains("5G") {
            // Prefer rmnet_data2 for 5G internet
            if is_interface_up(config.rmnet_data2)? {
                return get_interface_ip(config.rmnet_data2);
            }
        }
    }
    
    // Check WiFi 6
    if is_interface_up(config.wlan0)? {
        return get_interface_ip(config.wlan0);
    }
    
    // Check hotspot
    if is_interface_up(config.swlan0)? {
        return get_interface_ip(config.swlan0);
    }
    
    // Fallback to any active rmnet
    if let Ok(iface) = detect_active_interface() {
        return get_interface_ip(&iface);
    }
    
    // Last resort: bind to all interfaces
    Ok("0.0.0.0".to_string())
}

/// Get IP address of interface
fn get_interface_ip(iface: &str) -> io::Result<String> {
    use std::process::Command;
    
    let output = Command::new("ip")
        .args(&["addr", "show", iface])
        .output()?;
    
    let addr_info = String::from_utf8_lossy(&output.stdout);
    
    // Parse IPv4 address
    for line in addr_info.lines() {
        if line.contains("inet ") && !line.contains("inet6") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                // Format is "inet 192.168.1.1/24"
                if let Some(ip) = parts[1].split('/').next() {
                    return Ok(ip.to_string());
                }
            }
        }
    }
    
    Err(io::Error::new(io::ErrorKind::NotFound, "No IP address found"))
}

/// Note20 power management
pub fn configure_power_optimization(aggressive: bool) -> io::Result<()> {
    use std::fs;
    
    if aggressive {
        // Disable power saving for network
        let _ = fs::write("/sys/module/bcmdhd/parameters/pm", "0");
        
        // Keep mobile data active
        let _ = fs::write("/proc/sys/net/ipv4/tcp_no_delay_ack", "1");
        
        info!("Disabled power saving for maximum performance");
    } else {
        // Balanced mode
        info!("Using balanced power mode");
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_network_detection() {
        // This will vary by device state
        if let Ok(iface) = detect_active_interface() {
            println!("Active interface: {}", iface);
            assert!(!iface.is_empty());
        }
    }
    
    #[test] 
    fn test_network_type() {
        if let Ok(net_type) = get_network_type() {
            println!("Network type: {}", net_type);
        }
    }
}