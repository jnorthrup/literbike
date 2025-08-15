// Bonjour/mDNS Discovery Module
// Network service discovery and advertising via multicast DNS

use std::net::{UdpSocket, SocketAddr, Ipv4Addr};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::thread;
use std::sync::{Arc, Mutex};

/// Bonjour/mDNS service discovery and advertising
pub struct BonjourDiscovery {
    socket: UdpSocket,
    services: Arc<Mutex<HashMap<String, ServiceRecord>>>,
    running: Arc<Mutex<bool>>,
}

/// mDNS service record
#[derive(Debug, Clone)]
pub struct ServiceRecord {
    pub name: String,
    pub service_type: String,
    pub domain: String,
    pub port: u16,
    pub txt_records: HashMap<String, String>,
    pub ip_address: Ipv4Addr,
    pub ttl: u32,
    pub last_seen: u64,
}

/// mDNS query types
#[derive(Debug, Clone)]
pub enum QueryType {
    A = 1,
    PTR = 12,
    TXT = 16,
    SRV = 33,
    ANY = 255,
}

impl BonjourDiscovery {
    /// Create new Bonjour discovery instance
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        
        // Join mDNS multicast group
        socket.join_multicast_v4(&Ipv4Addr::new(224, 0, 0, 251), &Ipv4Addr::UNSPECIFIED)?;
        socket.set_multicast_loop_v4(false)?;
        socket.set_multicast_ttl_v4(255)?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;
        
        Ok(BonjourDiscovery {
            socket,
            services: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
        })
    }
    
    /// Start discovery service
    pub fn start_discovery(&self) -> Result<(), Box<dyn std::error::Error>> {
        *self.running.lock().unwrap() = true;
        
        let socket = self.socket.try_clone()?;
        let services = Arc::clone(&self.services);
        let running = Arc::clone(&self.running);
        
        thread::spawn(move || {
            let mut buffer = [0u8; 1500];
            
            while *running.lock().unwrap() {
                if let Ok((len, addr)) = socket.recv_from(&mut buffer) {
                    if let Ok(packet) = Self::parse_mdns_packet(&buffer[..len]) {
                        Self::process_mdns_response(&services, packet, addr);
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Discover specific service type
    pub fn discover_service(&self, service_type: &str) -> Result<(), Box<dyn std::error::Error>> {
        let query = self.build_mdns_query(service_type, QueryType::PTR)?;
        let mdns_addr: SocketAddr = "224.0.0.251:5353".parse()?;
        
        self.socket.send_to(&query, mdns_addr)?;
        
        // Send additional queries for common services
        if service_type == "_services._dns-sd._udp.local" {
            self.discover_common_services()?;
        }
        
        Ok(())
    }
    
    /// Discover common network services
    fn discover_common_services(&self) -> Result<(), Box<dyn std::error::Error>> {
        let common_services = [
            "_http._tcp.local",
            "_https._tcp.local", 
            "_ssh._tcp.local",
            "_ftp._tcp.local",
            "_smb._tcp.local",
            "_afpovertcp._tcp.local",
            "_airplay._tcp.local",
            "_raop._tcp.local",
            "_printer._tcp.local",
            "_ipp._tcp.local",
            "_scanner._tcp.local",
            "_workstation._tcp.local",
            "_device-info._tcp.local",
        ];
        
        for service in &common_services {
            let query = self.build_mdns_query(service, QueryType::PTR)?;
            let mdns_addr: SocketAddr = "224.0.0.251:5353".parse()?;
            self.socket.send_to(&query, mdns_addr)?;
            
            // Small delay between queries
            thread::sleep(Duration::from_millis(10));
        }
        
        Ok(())
    }
    
    /// Advertise a service
    pub fn advertise_service(&self, service: ServiceRecord) -> Result<(), Box<dyn std::error::Error>> {
        let response = self.build_mdns_response(&service)?;
        let mdns_addr: SocketAddr = "224.0.0.251:5353".parse()?;
        
        // Send advertisement
        self.socket.send_to(&response, mdns_addr)?;
        
        // Store in local registry
        let mut services = self.services.lock().unwrap();
        services.insert(service.name.clone(), service);
        
        Ok(())
    }
    
    /// Get discovered services
    pub fn get_services(&self) -> HashMap<String, ServiceRecord> {
        self.services.lock().unwrap().clone()
    }
    
    /// Get services by type
    pub fn get_services_by_type(&self, service_type: &str) -> Vec<ServiceRecord> {
        self.services
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.service_type == service_type)
            .cloned()
            .collect()
    }
    
    /// Stop discovery
    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }
    
    /// Build mDNS query packet
    fn build_mdns_query(&self, name: &str, query_type: QueryType) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut packet = Vec::new();
        
        // mDNS header
        packet.extend_from_slice(&[0x00, 0x00]); // Transaction ID
        packet.extend_from_slice(&[0x00, 0x00]); // Flags (standard query)
        packet.extend_from_slice(&[0x00, 0x01]); // Questions: 1
        packet.extend_from_slice(&[0x00, 0x00]); // Answer RRs: 0
        packet.extend_from_slice(&[0x00, 0x00]); // Authority RRs: 0
        packet.extend_from_slice(&[0x00, 0x00]); // Additional RRs: 0
        
        // Question section
        Self::encode_name(&mut packet, name);
        packet.extend_from_slice(&(query_type as u16).to_be_bytes()); // QTYPE
        packet.extend_from_slice(&[0x00, 0x01]); // QCLASS (IN)
        
        Ok(packet)
    }
    
    /// Build mDNS response packet
    fn build_mdns_response(&self, service: &ServiceRecord) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut packet = Vec::new();
        
        // mDNS header
        packet.extend_from_slice(&[0x00, 0x00]); // Transaction ID
        packet.extend_from_slice(&[0x84, 0x00]); // Flags (response, authoritative)
        packet.extend_from_slice(&[0x00, 0x00]); // Questions: 0
        packet.extend_from_slice(&[0x00, 0x01]); // Answer RRs: 1
        packet.extend_from_slice(&[0x00, 0x00]); // Authority RRs: 0
        packet.extend_from_slice(&[0x00, 0x00]); // Additional RRs: 0
        
        // Answer section - PTR record
        Self::encode_name(&mut packet, &service.service_type);
        packet.extend_from_slice(&[0x00, 0x0C]); // TYPE (PTR)
        packet.extend_from_slice(&[0x00, 0x01]); // CLASS (IN)
        packet.extend_from_slice(&service.ttl.to_be_bytes()); // TTL
        
        // PTR RDATA
        let ptr_data = format!("{}.{}", service.name, service.service_type);
        let ptr_data_encoded = Self::encode_name_to_vec(&ptr_data);
        packet.extend_from_slice(&(ptr_data_encoded.len() as u16).to_be_bytes()); // RDLENGTH
        packet.extend_from_slice(&ptr_data_encoded);
        
        Ok(packet)
    }
    
    /// Encode DNS name
    fn encode_name(packet: &mut Vec<u8>, name: &str) {
        for label in name.split('.') {
            if !label.is_empty() {
                packet.push(label.len() as u8);
                packet.extend_from_slice(label.as_bytes());
            }
        }
        packet.push(0); // Root label
    }
    
    /// Encode DNS name to vector
    fn encode_name_to_vec(name: &str) -> Vec<u8> {
        let mut encoded = Vec::new();
        Self::encode_name(&mut encoded, name);
        encoded
    }
    
    /// Parse mDNS packet
    fn parse_mdns_packet(data: &[u8]) -> Result<MdnsPacket, Box<dyn std::error::Error>> {
        if data.len() < 12 {
            return Err("Packet too short".into());
        }
        
        let transaction_id = u16::from_be_bytes([data[0], data[1]]);
        let flags = u16::from_be_bytes([data[2], data[3]]);
        let questions = u16::from_be_bytes([data[4], data[5]]);
        let answers = u16::from_be_bytes([data[6], data[7]]);
        
        Ok(MdnsPacket {
            transaction_id,
            flags,
            questions,
            answers,
            data: data.to_vec(),
        })
    }
    
    /// Process mDNS response
    fn process_mdns_response(
        services: &Arc<Mutex<HashMap<String, ServiceRecord>>>, 
        packet: MdnsPacket,
        addr: SocketAddr,
    ) {
        // Basic response processing - would need full DNS parsing for production
        if packet.answers > 0 && (packet.flags & 0x8000) != 0 {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            // Create a basic service record from the response
            if let Some(ip) = Self::extract_ip_from_addr(addr) {
                let service = ServiceRecord {
                    name: format!("discovered-{}", ip),
                    service_type: "_unknown._tcp.local".to_string(),
                    domain: "local".to_string(),
                    port: addr.port(),
                    txt_records: HashMap::new(),
                    ip_address: ip,
                    ttl: 3600,
                    last_seen: timestamp,
                };
                
                let mut services_guard = services.lock().unwrap();
                services_guard.insert(service.name.clone(), service);
            }
        }
    }
    
    /// Extract IPv4 from SocketAddr
    fn extract_ip_from_addr(addr: SocketAddr) -> Option<Ipv4Addr> {
        match addr {
            SocketAddr::V4(v4_addr) => Some(*v4_addr.ip()),
            SocketAddr::V6(_) => None,
        }
    }
}

/// Parsed mDNS packet
#[derive(Debug)]
struct MdnsPacket {
    transaction_id: u16,
    flags: u16,
    questions: u16,
    answers: u16,
    data: Vec<u8>,
}

/// Convenience functions for common operations
impl BonjourDiscovery {
    /// Discover all services on the network
    pub fn discover_all_services(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.discover_service("_services._dns-sd._udp.local")?;
        Ok(())
    }
    
    /// Find HTTP servers
    pub fn find_http_servers(&self) -> Vec<ServiceRecord> {
        self.get_services_by_type("_http._tcp.local")
    }
    
    /// Find SSH servers
    pub fn find_ssh_servers(&self) -> Vec<ServiceRecord> {
        self.get_services_by_type("_ssh._tcp.local")
    }
    
    /// Find printers
    pub fn find_printers(&self) -> Vec<ServiceRecord> {
        let mut printers = self.get_services_by_type("_printer._tcp.local");
        printers.extend(self.get_services_by_type("_ipp._tcp.local"));
        printers
    }
    
    /// Clean up old services
    pub fn cleanup_old_services(&self, max_age_seconds: u64) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut services = self.services.lock().unwrap();
        services.retain(|_, service| {
            current_time - service.last_seen < max_age_seconds
        });
    }
}

/// Create a new Bonjour discovery instance
pub fn create_bonjour_discovery() -> Result<BonjourDiscovery, Box<dyn std::error::Error>> {
    BonjourDiscovery::new()
}