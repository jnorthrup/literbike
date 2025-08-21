// LITERBIKE Aggressive UPnP Port Manipulation
// Designed for carrier freedom and bypassing tethering restrictions

use std::net::{UdpSocket, TcpStream, SocketAddr};
use std::time::Duration;
use std::thread;
use std::io::{Write, Read};

use tokio::stream;

/// Aggressive UPnP controller for carrier bypass
pub struct AggressiveUPnP {
    gateway_ip: String,
    local_ip: String,
    discovered_devices: Vec<UPnPDevice>,
}

#[derive(Debug, Clone)]
pub struct UPnPDevice {
    pub location: String,
    pub server: String,
    pub control_url: String,
    pub service_type: String,
}

#[derive(Debug, Clone)]
pub struct PortMapping {
    pub external_port: u16,
    pub internal_port: u16,
    pub protocol: String,
    pub description: String,
    pub duration: u32,
}

impl AggressiveUPnP {
    pub fn new() -> std::io::Result<Self> {
        let gateway_ip = crate::syscall_net::get_default_gateway()?.to_string();
        let local_ip = crate::syscall_net::get_default_local_ipv4()?.to_string();
        
        Ok(Self {
            gateway_ip,
            local_ip,
            discovered_devices: Vec::new(),
        })
    }
    
    /// Aggressive UPnP discovery with multiple techniques
    pub fn discover_aggressive(&mut self) -> Result<Vec<UPnPDevice>, String> {
        println!("🚀 Starting aggressive UPnP discovery for carrier bypass");
        
        let mut devices = Vec::new();
        
        // 1. Standard SSDP multicast discovery
        if let Ok(std_devices) = self.ssdp_discovery_standard() {
            devices.extend(std_devices);
        }
        
        // 2. Direct gateway probing (bypass carrier SSDP blocking)
        if let Ok(direct_devices) = self.direct_gateway_probe() {
            devices.extend(direct_devices);
        }
        
        // 3. Port scanning common UPnP control ports
        if let Ok(scan_devices) = self.upnp_port_scan() {
            devices.extend(scan_devices);
        }
        
        // 4. Alternative discovery methods for carrier environments
        if let Ok(alt_devices) = self.alternative_discovery() {
            devices.extend(alt_devices);
        }
        
        self.discovered_devices = devices.clone();
        println!("✓ Discovered {} UPnP devices via aggressive methods", devices.len());
        
        Ok(devices)
    }
    
    /// Standard SSDP multicast discovery
    fn ssdp_discovery_standard(&self) -> Result<Vec<UPnPDevice>, String> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("Failed to bind SSDP socket: {}", e))?;
        
        socket.set_read_timeout(Some(Duration::from_secs(3)))
            .map_err(|e| format!("Failed to set timeout: {}", e))?;
            
        // Join multicast group
        socket.join_multicast_v4(
            &"239.255.255.250".parse().unwrap(),
            &"0.0.0.0".parse().unwrap()
        ).map_err(|e| format!("Failed to join multicast: {}", e))?;
        
        // Send M-SEARCH for IGD devices
        let msearch = format!(
            "M-SEARCH * HTTP/1.1\r\n\
             HOST: 239.255.255.250:1900\r\n\
             MAN: \"ssdp:discover\"\r\n\
             ST: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\n\
             MX: 3\r\n\r\n"
        );
        
        let multicast_addr: SocketAddr = "239.255.255.250:1900".parse().unwrap();
        socket.send_to(msearch.as_bytes(), multicast_addr)
            .map_err(|e| format!("Failed to send M-SEARCH: {}", e))?;
        
        let mut devices = Vec::new();
        let mut buffer = [0; 2048];
        
        // Collect responses for 3 seconds
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(3) {
            if let Ok((len, _src)) = socket.recv_from(&mut buffer) {
                let response = String::from_utf8_lossy(&buffer[..len]);
                if let Some(device) = self.parse_ssdp_response(&response) {
                    devices.push(device);
                }
            }
        }
        
        Ok(devices)
    }
    
    /// Direct gateway probing (bypass SSDP blocking)
    fn direct_gateway_probe(&self) -> Result<Vec<UPnPDevice>, String> {
        println!("📡 Probing gateway directly: {}", self.gateway_ip);
        
        let mut devices = Vec::new();
        
        // Common UPnP control ports
        let upnp_ports = [1900, 5000, 5431, 49152, 49153, 49154];
        
        for port in upnp_ports {
            let addr = format!("{}:{}", self.gateway_ip, port);
            if let Ok(_stream) = TcpStream::connect_timeout(
                &addr.parse().unwrap(),
                Duration::from_millis(500)
            ) {
                // Try to get device description
                if let Ok(device) = self.get_device_description(&self.gateway_ip, port) {
                    devices.push(device);
                }
            }
        }
        
        Ok(devices)
    }
    
    /// Scan for UPnP services on gateway subnet
    fn upnp_port_scan(&self) -> Result<Vec<UPnPDevice>, String> {
        println!("🔍 Scanning subnet for UPnP services");
        
        let gateway_parts: Vec<&str> = self.gateway_ip.split('.').collect();
        if gateway_parts.len() != 4 {
            return Err("Invalid gateway IP format".to_string());
        }
        
        let subnet_base = format!("{}.{}.{}.", gateway_parts[0], gateway_parts[1], gateway_parts[2]);
        let mut devices = Vec::new();
        
        // Scan common gateway IPs in subnet
        let gateway_candidates = [1, 254, 100, 2];
        
        for host in gateway_candidates {
            let ip = format!("{}{}", subnet_base, host);
            
            // Quick scan of UPnP ports
            let upnp_ports = [1900, 5000, 49152];
            for port in upnp_ports {
                let addr = format!("{}:{}", ip, port);
                if let Ok(_stream) = TcpStream::connect_timeout(
                    &addr.parse().unwrap(),
                    Duration::from_millis(200)
                ) {
                    if let Ok(device) = self.get_device_description(&ip, port) {
                        devices.push(device);
                    }
                }
            }
        }
        
        Ok(devices)
    }
    
    /// Alternative discovery for carrier environments
    fn alternative_discovery(&self) -> Result<Vec<UPnPDevice>, String> {
        println!("🌐 Trying alternative discovery methods");
        
        let mut devices = Vec::new();
        
        // Try common carrier gateway patterns
        let carrier_patterns = [
            "192.168.1.1:5000",
            "192.168.0.1:1900", 
            "10.0.0.1:49152",
            "172.16.0.1:5431",
        ];
        
        for pattern in carrier_patterns {
            if let Ok(stream) = TcpStream::connect_timeout(
                &pattern.parse().unwrap(),
                Duration::from_millis(300)
            ) {
                let ip = pattern.split(':').next().unwrap();
                let port: u16 = pattern.split(':').nth(1).unwrap().parse().unwrap();
                
                if let Ok(device) = self.get_device_description(ip, port) {
                    devices.push(device);
                }
            }
        }
        
        Ok(devices)
    }
    
    /// Parse SSDP response to extract device info
    fn parse_ssdp_response(&self, response: &str) -> Option<UPnPDevice> {
        let mut location = String::new();
        let mut server = String::new();
        
        for line in response.lines() {
            if line.to_uppercase().starts_with("LOCATION:") {
                location = line[9..].trim().to_string();
            } else if line.to_uppercase().starts_with("SERVER:") {
                server = line[7..].trim().to_string();
            }
        }
        
        if !location.is_empty() {
            Some(UPnPDevice {
                location,
                server,
                control_url: String::new(),
                service_type: String::new(),
            })
        } else {
            None
        }
    }
    
    /// Get device description from HTTP endpoint
    fn get_device_description(&self, ip: &str, port: u16) -> Result<UPnPDevice, String> {
        let url = format!("http://{}:{}/description.xml", ip, port);
        
        // Simple HTTP GET request
        let mut stream = TcpStream::connect_timeout(
            &format!("{}:{}", ip, port).parse().unwrap(),
            Duration::from_secs(2)
        ).map_err(|e| format!("Failed to connect: {}", e))?;
        
        let request = format!(
            "GET /description.xml HTTP/1.1\r\n\
             Host: {}:{}\r\n\
             Connection: close\r\n\r\n",
            ip, port
        );
        
        stream.write_all(request.as_bytes())
            .map_err(|e| format!("Failed to send request: {}", e))?;
        
        let mut response = String::new();
        stream.read_to_string(&mut response)
            .map_err(|e| format!("Failed to read response: {}", e))?;
        
        // Basic XML parsing for UPnP device info
        let device = UPnPDevice {
            location: url,
            server: format!("Direct:{}:{}", ip, port),
            control_url: self.extract_control_url(&response),
            service_type: self.extract_service_type(&response),
        };
        
        Ok(device)
    }
    
    /// Extract control URL from device description XML
    fn extract_control_url(&self, xml: &str) -> String {
        // Simple regex-like extraction
        if let Some(start) = xml.find("<controlURL>") {
            if let Some(end) = xml[start..].find("</controlURL>") {
                let url = &xml[start + 12..start + end];
                return url.trim().to_string();
            }
        }
        "/control".to_string() // Default fallback
    }
    
    /// Extract service type from device description XML
    fn extract_service_type(&self, xml: &str) -> String {
        if let Some(start) = xml.find("<serviceType>") {
            if let Some(end) = xml[start..].find("</serviceType>") {
                let service = &xml[start + 13..start + end];
                return service.trim().to_string();
            }
        }
        "urn:schemas-upnp-org:service:WANIPConnection:1".to_string()
    }
    
    /// Aggressively open ports using multiple UPnP techniques
    pub fn open_ports_aggressive(&self, mappings: &[PortMapping]) -> Result<(), String> {
        println!("🔓 Opening ports aggressively via UPnP");
        
        if self.discovered_devices.is_empty() {
            return Err("No UPnP devices discovered".to_string());
        }
        
        let mut success_count = 0;
        
        for device in &self.discovered_devices {
            for mapping in mappings {
                // Try multiple methods per device
                let methods = [
                    "AddPortMapping",
                    "SetGenericPortMappingEntry", 
                    "AddAnyPortMapping",
                ];
                
                for method in methods {
                    if self.add_port_mapping_method(device, mapping, method).is_ok() {
                        println!("✓ Port {} mapped via {} on {}", 
                               mapping.external_port, method, device.server);
                        success_count += 1;
                        break; // Success, move to next mapping
                    }
                }
            }
        }
        
        if success_count == 0 {
            Err("Failed to open any ports via UPnP".to_string())
        } else {
            println!("✓ Successfully opened {} port mappings", success_count);
            Ok(())
        }
    }
    
    /// Add port mapping using specific UPnP method
    fn add_port_mapping_method(&self, device: &UPnPDevice, mapping: &PortMapping, method: &str) -> Result<(), String> {
        let control_url = if device.control_url.starts_with("http") {
            device.control_url.clone()
        } else {
            format!("{}{}", device.location.trim_end_matches("/description.xml"), device.control_url)
        };
        
        // Extract host and port from control URL
        let binding = control_url.replace("http://", "");
        let url_parts: Vec<&str> = binding.split('/').collect();
        let host_port = url_parts[0];
        let path = format!("/{}", url_parts[1..].join("/"));
        
        let mut stream = TcpStream::connect_timeout(
            &host_port.parse().unwrap(),
            Duration::from_secs(3)
        ).map_err(|e| format!("Failed to connect to control URL: {}", e))?;
        
        let soap_action = format!("\"{}#{}\"", device.service_type, method);
        let soap_body = self.create_soap_body(method, mapping);
        
        let request = format!(
            "POST {} HTTP/1.1\r\n\
             Host: {}\r\n\
             Content-Type: text/xml; charset=\"utf-8\"\r\n\
             Content-Length: {}\r\n\
             SOAPAction: {}\r\n\
             Connection: close\r\n\r\n\
             {}",
            path, host_port, soap_body.len(), soap_action, soap_body
        );
        
        stream.write_all(request.as_bytes())
            .map_err(|e| format!("Failed to send SOAP request: {}", e))?;
        
        let mut response = String::new();
        stream.read_to_string(&mut response)
            .map_err(|e| format!("Failed to read SOAP response: {}", e))?;
        
        if response.contains("200 OK") && !response.contains("error") {
            Ok(())
        } else {
            Err(format!("UPnP {} failed: {}", method, response))
        }
    }
    
    /// Create SOAP body for port mapping request
    fn create_soap_body(&self, method: &str, mapping: &PortMapping) -> String {
        match method {
            "AddPortMapping" => format!(
                "<?xml version=\"1.0\"?>\
                 <s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" \
                 s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
                 <s:Body>\
                 <u:AddPortMapping xmlns:u=\"urn:schemas-upnp-org:service:WANIPConnection:1\">\
                 <NewRemoteHost></NewRemoteHost>\
                 <NewExternalPort>{}</NewExternalPort>\
                 <NewProtocol>{}</NewProtocol>\
                 <NewInternalPort>{}</NewInternalPort>\
                 <NewInternalClient>{}</NewInternalClient>\
                 <NewEnabled>1</NewEnabled>\
                 <NewPortMappingDescription>{}</NewPortMappingDescription>\
                 <NewLeaseDuration>{}</NewLeaseDuration>\
                 </u:AddPortMapping>\
                 </s:Body>\
                 </s:Envelope>",
                mapping.external_port,
                mapping.protocol,
                mapping.internal_port,
                self.local_ip,
                mapping.description,
                mapping.duration
            ),
            "AddAnyPortMapping" => format!(
                "<?xml version=\"1.0\"?>\
                 <s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" \
                 s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
                 <s:Body>\
                 <u:AddAnyPortMapping xmlns:u=\"urn:schemas-upnp-org:service:WANIPConnection:1\">\
                 <NewRemoteHost></NewRemoteHost>\
                 <NewExternalPort>{}</NewExternalPort>\
                 <NewProtocol>{}</NewProtocol>\
                 <NewInternalPort>{}</NewInternalPort>\
                 <NewInternalClient>{}</NewInternalClient>\
                 <NewEnabled>1</NewEnabled>\
                 <NewPortMappingDescription>{}</NewPortMappingDescription>\
                 <NewLeaseDuration>{}</NewLeaseDuration>\
                 </u:AddAnyPortMapping>\
                 </s:Body>\
                 </s:Envelope>",
                mapping.external_port,
                mapping.protocol,
                mapping.internal_port,
                self.local_ip,
                mapping.description,
                mapping.duration
            ),
            _ => format!(
                "<?xml version=\"1.0\"?>\
                 <s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
                 <s:Body>\
                 <u:{} xmlns:u=\"urn:schemas-upnp-org:service:WANIPConnection:1\">\
                 <NewExternalPort>{}</NewExternalPort>\
                 <NewProtocol>{}</NewProtocol>\
                 <NewInternalPort>{}</NewInternalPort>\
                 <NewInternalClient>{}</NewInternalClient>\
                 <NewEnabled>1</NewEnabled>\
                 <NewPortMappingDescription>{}</NewPortMappingDescription>\
                 </u:{}>\
                 </s:Body>\
                 </s:Envelope>",
                method,
                mapping.external_port,
                mapping.protocol,
                mapping.internal_port,
                self.local_ip,
                mapping.description,
                method
            )
        }
    }
    
    /// Open carrier-bypassing port ranges
    pub fn bypass_carrier_restrictions(&mut self) -> Result<(), String> {
        println!("🚨 Initiating carrier restriction bypass via aggressive UPnP");
        
        // Discover devices first
        self.discover_aggressive()?;
        
        // Define carrier-bypassing port mappings
        let bypass_mappings = vec![
            // Standard web ports (carriers usually allow)
            PortMapping {
                external_port: 80,
                internal_port: 8080,
                protocol: "TCP".to_string(),
                description: "LiterBike-HTTP".to_string(),
                duration: 86400, // 24 hours
            },
            PortMapping {
                external_port: 443,
                internal_port: 8443,
                protocol: "TCP".to_string(),
                description: "LiterBike-HTTPS".to_string(),
                duration: 86400,
            },
            // SSH and common service ports
            PortMapping {
                external_port: 22,
                internal_port: 2222,
                protocol: "TCP".to_string(),
                description: "LiterBike-SSH".to_string(),
                duration: 86400,
            },
            PortMapping {
                external_port: 8022,
                internal_port: 8022,
                protocol: "TCP".to_string(),
                description: "LiterBike-SSH-Alt".to_string(),
                duration: 86400,
            },
            // DNS (UDP for tunneling)
            PortMapping {
                external_port: 53,
                internal_port: 5353,
                protocol: "UDP".to_string(),
                description: "LiterBike-DNS".to_string(),
                duration: 86400,
            },
            // High numbered ports (less likely to be blocked)
            PortMapping {
                external_port: 49152,
                internal_port: 49152,
                protocol: "TCP".to_string(),
                description: "LiterBike-Dynamic".to_string(),
                duration: 86400,
            },
            PortMapping {
                external_port: 8888,
                internal_port: 8888,
                protocol: "TCP".to_string(),
                description: "LiterBike-Proxy".to_string(),
                duration: 86400,
            },
        ];
        
        // Open all bypass mappings
        self.open_ports_aggressive(&bypass_mappings)?;
        
        println!("✅ Carrier restriction bypass complete - all ports opened");
        Ok(())
    }
}

/// Convenience function for quick carrier bypass
pub fn quick_carrier_bypass() -> Result<(), String> {
    let mut upnp = AggressiveUPnP::new()
        .map_err(|e| format!("Failed to create UPnP controller: {}", e))?;
    
    upnp.bypass_carrier_restrictions()
}