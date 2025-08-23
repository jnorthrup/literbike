use std::env;
use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub bind_addr: IpAddr,
    pub bind_port: u16,
    pub interface: String,
    pub log_level: String,
    pub features: Vec<String>,
    pub egress_interface: Option<String>,
    pub egress_bind_ip: Option<IpAddr>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0".parse().unwrap(),
            bind_port: 8888,
            interface: "swlan0".to_string(),
            log_level: "info".to_string(),
            features: vec![],
            egress_interface: None,
            egress_bind_ip: None,
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        let mut cfg = Config::default();

        if let Ok(v) = env::var("LITEBIKE_BIND_PORT") {
            if let Ok(p) = v.parse() {
                cfg.bind_port = p;
            }
        }
        if let Ok(v) = env::var("LITEBIKE_INTERFACE") {
            if !v.trim().is_empty() {
                cfg.interface = v;
            }
        }
        if let Ok(v) = env::var("LITEBIKE_BIND_ADDR") {
            if let Ok(ip) = v.parse() {
                cfg.bind_addr = ip;
            }
        }
        if let Ok(v) = env::var("LITEBIKE_LOG") {
            if !v.trim().is_empty() {
                cfg.log_level = v;
            }
        }
        if let Ok(v) = env::var("LITEBIKE_FEATURES") {
            let parts = v
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<String>>();
            if !parts.is_empty() {
                cfg.features = parts;
            }
        }

        if let Ok(v) = env::var("EGRESS_INTERFACE") {
            if !v.trim().is_empty() {
                cfg.egress_interface = Some(v);
            }
        }
        if let Ok(v) = env::var("EGRESS_BIND_IP") {
            if let Ok(ip) = v.parse() {
                cfg.egress_bind_ip = Some(ip);
            }
        }

        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::env;

    #[test]
    fn from_env_applies_bind_addr_and_interface_independently() {
        let orig_interface = env::var("LITEBIKE_INTERFACE").ok();
        let orig_bind_addr = env::var("LITEBIKE_BIND_ADDR").ok();

        env::set_var("LITEBIKE_INTERFACE", "eth0");
        env::set_var("LITEBIKE_BIND_ADDR", "127.0.0.2");

        let cfg = Config::from_env();

        assert_eq!(cfg.interface, "eth0");
        assert_eq!(cfg.bind_addr.to_string(), "127.0.0.2");

        if let Some(v) = orig_interface {
            env::set_var("LITEBIKE_INTERFACE", v);
        } else {
            env::remove_var("LITEBIKE_INTERFACE");
        }
        if let Some(v) = orig_bind_addr {
            env::set_var("LITEBIKE_BIND_ADDR", v);
        } else {
            env::remove_var("LITEBIKE_BIND_ADDR");
        }
    }
}
