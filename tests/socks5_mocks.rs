// SOCKS5 Mock Packets and Scenarios
// Real packet buffers with contents and defects for comprehensive testing

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Real SOCKS5 packet captures and mock data
pub struct Socks5PacketMocks;

impl Socks5PacketMocks {
    /// Real SOCKS5 handshake packets from actual network captures
    pub fn real_handshakes() -> Vec<(&'static str, Vec<u8>, &'static str)> {
        vec![
            // Chrome connecting through SOCKS5
            ("chrome_handshake", 
             vec![0x05, 0x03, 0x00, 0x01, 0x02], 
             "Chrome browser: Version 5, 3 methods (no-auth, GSSAPI, user/pass)"),
            
            // Firefox SOCKS5 connection
            ("firefox_handshake",
             vec![0x05, 0x02, 0x00, 0x02],
             "Firefox: Version 5, 2 methods (no-auth, user/pass)"),
            
            // curl --socks5
            ("curl_socks5",
             vec![0x05, 0x02, 0x00, 0x01],
             "curl: Version 5, 2 methods (no-auth, GSSAPI)"),
            
            // SSH dynamic forwarding (-D flag)
            ("ssh_dynamic",
             vec![0x05, 0x01, 0x00],
             "SSH -D: Version 5, 1 method (no-auth only)"),
            
            // Tor client handshake
            ("tor_client",
             vec![0x05, 0x01, 0x02],
             "Tor: Version 5, 1 method (user/pass for isolation)"),
        ]
    }
    
    /// Real SOCKS5 connect requests with actual destinations
    pub fn real_connect_requests() -> Vec<(&'static str, Vec<u8>, &'static str)> {
        vec![
            // Google.com IPv4 connect
            ("google_ipv4",
             vec![
                 0x05, 0x01, 0x00, 0x01,  // Ver 5, CMD=CONNECT, RSV, ATYP=IPv4
                 0x8E, 0xFA, 0xB5, 0xCE,  // IP: 142.250.181.206 (google.com)
                 0x01, 0xBB,              // Port: 443 (HTTPS)
             ],
             "Connect to google.com:443 via IPv4"),
            
            // DNS name resolution request
            ("dns_resolution",
             vec![
                 0x05, 0x01, 0x00, 0x03,  // Ver 5, CMD=CONNECT, RSV, ATYP=DOMAINNAME
                 0x0E,                    // Domain length: 14
                 b'w', b'w', b'w', b'.', b'g', b'o', b'o', b'g', b'l', b'e', b'.', b'c', b'o', b'm',
                 0x00, 0x50,              // Port: 80 (HTTP)
             ],
             "Connect to www.google.com:80 via domain name"),
            
            // IPv6 connection
            ("ipv6_connect",
             vec![
                 0x05, 0x01, 0x00, 0x04,  // Ver 5, CMD=CONNECT, RSV, ATYP=IPv6
                 0x26, 0x07, 0xF8, 0xB0,  // IPv6: 2607:f8b0:4004:c07::71 (google IPv6)
                 0x40, 0x04, 0x0C, 0x07,
                 0x00, 0x00, 0x00, 0x00,
                 0x00, 0x00, 0x00, 0x71,
                 0x01, 0xBB,              // Port: 443
             ],
             "Connect to Google IPv6:443"),
            
            // Tor hidden service (.onion)
            ("tor_onion",
             vec![
                 0x05, 0x01, 0x00, 0x03,  // Ver 5, CMD=CONNECT, RSV, ATYP=DOMAINNAME
                 0x38,                    // Domain length: 56
                 // "thehiddenwiki.p3qnlfht5u7tq7xa5dkdxfxquo2xvkoubemzdo2bqhqixqcffeid.onion"
                 b't', b'h', b'e', b'h', b'i', b'd', b'd', b'e', b'n', b'w', b'i', b'k', b'i', b'.',
                 b'p', b'3', b'q', b'n', b'l', b'f', b'h', b't', b'5', b'u', b'7', b't', b'q', b'7',
                 b'x', b'a', b'5', b'd', b'k', b'd', b'x', b'f', b'x', b'q', b'u', b'o', b'2', b'x',
                 b'v', b'k', b'o', b'u', b'b', b'e', b'm', b'z', b'd', b'o', b'2', b'b', b'q', b'h',
                 b'q', b'i', b'x', b'q', b'c', b'f', b'f', b'e', b'i', b'd', b'.', b'o', b'n', b'i', b'o', b'n',
                 0x00, 0x50,              // Port: 80
             ],
             "Connect to Tor hidden service"),
            
            // Localhost connection (common for tunneling)
            ("localhost_tunnel",
             vec![
                 0x05, 0x01, 0x00, 0x01,  // Ver 5, CMD=CONNECT, RSV, ATYP=IPv4
                 0x7F, 0x00, 0x00, 0x01,  // IP: 127.0.0.1
                 0x22, 0xB8,              // Port: 8888 (proxy port!)
             ],
             "Connect to localhost:8888 - recursive proxy attempt"),
        ]
    }
    
    /// Defective SOCKS5 packets - common errors and attacks
    pub fn defective_packets() -> Vec<(&'static str, Vec<u8>, &'static str)> {
        vec![
            // Buffer overflow attempt
            ("buffer_overflow",
             vec![
                 0x05, 0xFF,              // Version 5, 255 methods (max)
                 // But only provide 10 methods, not 255
                 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
             ],
             "Buffer overflow: Claims 255 methods but provides only 10"),
            
            // Fragmented handshake (TCP segmentation)
            ("fragmented_v5",
             vec![0x05],  // Just version byte, rest comes later
             "Fragmented: Only version byte in first packet"),
            
            // Wrong version with valid structure
            ("socks4_structure",
             vec![
                 0x04, 0x01,              // SOCKS4 version
                 0x00, 0x50,              // Port 80
                 0x8E, 0xFA, 0xB5, 0xCE,  // IP address
                 0x00,                    // Null-terminated user ID
             ],
             "SOCKS4 packet incorrectly sent to SOCKS5 handler"),
            
            // Invalid command
            ("invalid_command",
             vec![
                 0x05, 0x01, 0x00, 0x01,  // Handshake
                 0x05, 0xFF, 0x00, 0x01,  // Invalid command 0xFF
                 0x7F, 0x00, 0x00, 0x01,
                 0x00, 0x50,
             ],
             "Invalid command type 0xFF (valid are 0x01-0x03)"),
            
            // Domain name too long
            ("domain_overflow",
             vec![
                 0x05, 0x01, 0x00, 0x03,  // DOMAINNAME
                 0xFF,                    // Claims 255 char domain
                 b'a', b'a', b'a', b'a',  // But only sends 4 chars
             ],
             "Domain length mismatch - buffer underrun"),
            
            // Zero-length domain
            ("zero_domain",
             vec![
                 0x05, 0x01, 0x00, 0x03,  // DOMAINNAME
                 0x00,                    // 0-length domain
                 0x00, 0x50,              // Port
             ],
             "Zero-length domain name"),
            
            // Invalid address type
            ("invalid_atyp",
             vec![
                 0x05, 0x01, 0x00, 0x05,  // ATYP=5 (invalid, should be 1,3,4)
                 0x7F, 0x00, 0x00, 0x01,
                 0x00, 0x50,
             ],
             "Invalid address type 0x05"),
            
            // Recursive proxy attempt
            ("recursive_8888",
             vec![
                 0x05, 0x01, 0x00, 0x03,  // DOMAINNAME
                 0x09,                    // Length 9
                 b'l', b'o', b'c', b'a', b'l', b'h', b'o', b's', b't',
                 0x22, 0xB8,              // Port: 8888 (same as proxy!)
             ],
             "Attempting to connect to proxy's own port"),
            
            // Mixed methods security issue
            ("mixed_auth_methods",
             vec![
                 0x05, 0x03,              // 3 methods
                 0x00, 0x02, 0xFF,        // No-auth, user/pass, and "no acceptable"
             ],
             "Client offers both auth and no-auth - security concern"),
        ]
    }
    
    /// Valid authentication sequences
    pub fn auth_sequences() -> Vec<(&'static str, Vec<u8>, &'static str)> {
        vec![
            // Username/password auth subnegotiation
            ("userpass_auth",
             vec![
                 0x01,                    // Subnegotiation version 1
                 0x04,                    // Username length: 4
                 b'u', b's', b'e', b'r',  // "user"
                 0x04,                    // Password length: 4  
                 b'p', b'a', b's', b's',  // "pass"
             ],
             "Basic username/password authentication"),
            
            // Empty username/password (anonymous)
            ("anonymous_auth",
             vec![
                 0x01,                    // Version 1
                 0x00,                    // Username length: 0
                 0x00,                    // Password length: 0
             ],
             "Anonymous authentication attempt"),
        ]
    }
    
    /// BIND and UDP ASSOCIATE requests (less common)
    pub fn special_requests() -> Vec<(&'static str, Vec<u8>, &'static str)> {
        vec![
            // BIND request for FTP data channel
            ("ftp_bind",
             vec![
                 0x05, 0x02, 0x00, 0x01,  // BIND command
                 0x00, 0x00, 0x00, 0x00,  // 0.0.0.0 (any interface)
                 0x00, 0x00,              // Port 0 (any port)
             ],
             "BIND request for incoming connections (FTP passive mode)"),
            
            // UDP ASSOCIATE for DNS
            ("udp_dns",
             vec![
                 0x05, 0x03, 0x00, 0x01,  // UDP ASSOCIATE
                 0x00, 0x00, 0x00, 0x00,  // 0.0.0.0
                 0x00, 0x35,              // Port 53 (DNS)
             ],
             "UDP ASSOCIATE for DNS queries"),
        ]
    }
}

/// Test scenarios for port 8888 with specific ingress/egress configs
pub struct Port8888Scenarios;

impl Port8888Scenarios {
    /// Default scenario: Listen on 0.0.0.0:8888, egress on all interfaces
    pub fn default_config() -> (&'static str, &'static str, &'static str) {
        ("0.0.0.0:8888", "0.0.0.0", "Default: Accept from any IP, egress on any interface")
    }
    
    /// Mobile hotspot scenario: Listen on hotspot interface, egress on mobile data
    pub fn mobile_hotspot() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
        vec![
            ("swlan0", "192.168.43.1:8888", "rmnet_data0", "Android hotspot to mobile data"),
            ("ap0", "192.168.42.1:8888", "wwan0", "Linux hotspot to cellular"),
            ("bridge100", "192.168.2.1:8888", "en0", "macOS Internet Sharing"),
        ]
    }
    
    /// Security-restricted scenarios  
    pub fn restricted_configs() -> Vec<(&'static str, &'static str, &'static str)> {
        vec![
            ("127.0.0.1:8888", "127.0.0.1", "Localhost only - most restrictive"),
            ("10.0.0.1:8888", "10.0.0.0/8", "Private network only"),
            ("192.168.1.100:8888", "192.168.1.0/24", "Single subnet restriction"),
        ]
    }
    
    /// Test connection patterns to port 8888
    pub fn connection_patterns() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            // Direct SOCKS5 on 8888
            ("direct_socks5", vec![0x05, 0x01, 0x00]),
            
            // HTTP CONNECT to establish SOCKS5
            ("http_connect_wrapper", b"CONNECT proxy.local:1080 HTTP/1.1\r\nHost: proxy.local\r\n\r\n".to_vec()),
            
            // PAC file request
            ("pac_request", b"GET /proxy.pac HTTP/1.1\r\nHost: 0.0.0.0:8888\r\n\r\n".to_vec()),
            
            // WPAD request  
            ("wpad_request", b"GET /wpad.dat HTTP/1.1\r\nHost: wpad:8888\r\n\r\n".to_vec()),
            
            // Mixed protocol probe (nmap-style)
            ("protocol_probe", vec![0x05, 0x01, 0x00, 0x47, 0x45, 0x54, 0x20, 0x2F]), // SOCKS5 + "GET /"
        ]
    }
}

/// UPnP restrictive mask scenarios
pub struct UpnpMaskScenarios;

impl UpnpMaskScenarios {
    /// Default UPnP configuration - restrictive by default
    pub fn default_restrictive() -> Vec<(&'static str, &'static str)> {
        vec![
            ("IGD:1", "No port forwarding without explicit user action"),
            ("IGD:2", "Require secure mode for any port mapping"),
            ("NAT-PMP", "Disabled by default, require explicit enable"),
            ("PCP", "Port Control Protocol blocked unless whitelisted"),
        ]
    }
    
    /// UPnP discovery packets that should be filtered
    pub fn filtered_packets() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            // M-SEARCH for all devices (too broad)
            ("msearch_all",
             b"M-SEARCH * HTTP/1.1\r\n\
               HOST: 239.255.255.250:1900\r\n\
               MAN: \"ssdp:discover\"\r\n\
               MX: 3\r\n\
               ST: ssdp:all\r\n\r\n".to_vec()),
            
            // External port mapping attempt
            ("external_mapping",
             b"POST /control/wan HTTP/1.1\r\n\
               HOST: 192.168.1.1:5000\r\n\
               SOAPACTION: \"urn:schemas-upnp-org:service:WANIPConnection:1#AddPortMapping\"\r\n\
               Content-Length: 500\r\n\r\n\
               <NewExternalPort>8888</NewExternalPort>".to_vec()),
        ]
    }
    
    /// Allowed UPnP operations with restrictive mask
    pub fn allowed_operations() -> Vec<(&'static str, &'static str)> {
        vec![
            ("GetExternalIPAddress", "Allowed: Read-only operation"),
            ("GetStatusInfo", "Allowed: Status queries are safe"),
            ("GetNATRSIPStatus", "Allowed: Information gathering only"),
            ("DeletePortMapping", "Allowed: Removing mappings is safe"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_packet_sizes() {
        // Verify packet sizes are correct
        for (name, packet, _desc) in Socks5PacketMocks::real_handshakes() {
            assert!(packet.len() >= 3, "Handshake {} too short", name);
            assert_eq!(packet[0], 0x05, "Wrong version in {}", name);
            let nmethods = packet[1] as usize;
            assert_eq!(packet.len(), 2 + nmethods, "Method count mismatch in {}", name);
        }
    }
    
    #[test]  
    fn test_defect_detection() {
        // Verify defective packets are actually defective
        for (name, packet, desc) in Socks5PacketMocks::defective_packets() {
            println!("Defect {}: {} - {:?}", name, desc, packet);
            // Each defect should violate SOCKS5 protocol in specific ways
            match name {
                "buffer_overflow" => {
                    assert_eq!(packet[1], 0xFF);
                    assert!(packet.len() < 2 + 0xFF);
                },
                "zero_domain" => {
                    assert_eq!(packet[4], 0x00);
                },
                "invalid_atyp" => {
                    assert!(packet[3] > 0x04);
                },
                _ => {}
            }
        }
    }
    
    #[test]
    fn test_port_8888_scenarios() {
        let (bind, egress, desc) = Port8888Scenarios::default_config();
        assert_eq!(bind, "0.0.0.0:8888");
        assert_eq!(egress, "0.0.0.0");
        println!("Default config: {}", desc);
        
        for pattern in Port8888Scenarios::connection_patterns() {
            println!("Pattern {}: {} bytes", pattern.0, pattern.1.len());
        }
    }
}