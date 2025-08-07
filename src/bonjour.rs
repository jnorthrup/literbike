use mdns_sd::{ServiceDaemon, ServiceInfo};
use log::{debug, info};

const SERVICE_TYPE: &str = "_litebike._tcp.local.";

pub struct BonjourDiscovery {
    daemon: ServiceDaemon,
    service_info: ServiceInfo,
}

impl BonjourDiscovery {
    pub fn new() -> Result<Self, String> {
        let daemon = ServiceDaemon::new().map_err(|e| e.to_string())?;
        let service_type = SERVICE_TYPE;
        let instance_name = "litebike-instance";
        let host_name = "litebike.local.";
        let port = 8080;
        let properties = [("version", "1.0")];

        let service_info = ServiceInfo::new(
            service_type,
            instance_name,
            host_name,
            "", // Will be replaced with actual IP
            port,
            &properties[..],
        ).map_err(|e| e.to_string())?;

        Ok(Self { daemon, service_info })
    }

    pub fn register_service(&self) -> Result<(), String> {
        self.daemon.register(self.service_info.clone()).map_err(|e| e.to_string())?;
        info!("Registered Bonjour service: {:?}", self.service_info);
        Ok(())
    }

    pub fn discover_peers(&self) -> Vec<ServiceInfo> {
        info!("Discovering LiteBike peers...");
        let receiver = self.daemon.browse(SERVICE_TYPE).unwrap();
        let mut services = Vec::new();
        // In a real application, you'd want to listen for events over time
        // For this example, we'll just collect initial services
        while let Ok(event) = receiver.recv() {
            match event {
                mdns_sd::ServiceEvent::ServiceFound(fullname, subtype) => {
                    debug!("Found service: {} (subtype: {})", fullname, subtype);
                    // You might want to resolve the service here to get its details
                    // For now, we'll just push a dummy ServiceInfo or resolve it later
                    // This requires a more complex handling of ServiceEvent
                    // For simplicity, we'll just collect resolved services
                },
                mdns_sd::ServiceEvent::ServiceResolved(info) => {
                    debug!("Resolved service: {:?}", info);
                    services.push(info);
                },
                _ => (),
            }
        }
        services
    }
}

mod tests {
    use super::BonjourDiscovery;

    #[test]
    fn test_bonjour_discovery_creation() {
        let bonjour = BonjourDiscovery::new();
        assert!(bonjour.is_ok());
    }
}