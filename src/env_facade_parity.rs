// Environment facade parity — unified config across deployment contexts
// (Docker env vars, systemd, .env files, sidecar inheritance)

use std::collections::HashMap;
use std::env;

pub struct EnvironmentFacade {
    vars: HashMap<String, String>,
}

impl EnvironmentFacade {
    pub fn new() -> Self {
        Self { vars: HashMap::new() }
    }
    pub fn from_env() -> Self {
        Self { vars: env::vars().collect() }
    }
    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(|s| s.as_str())
    }
    pub fn get_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.vars.get(key).map(|s| s.as_str()).unwrap_or(default)
    }
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.vars.insert(key.into(), value.into());
    }
    pub fn ensure_parity(&self, required: &[&str]) -> Result<(), String> {
        let missing: Vec<_> = required.iter().filter(|k| !self.vars.contains_key(**k)).copied().collect();
        if missing.is_empty() { Ok(()) } else { Err(format!("Missing env vars: {:?}", missing)) }
    }
}

impl Default for EnvironmentFacade {
    fn default() -> Self { Self::from_env() }
}

#[derive(Debug, Clone)]
pub struct ConfigSource {
    pub name: String,
    pub priority: i32,
    pub available: bool,
}

impl ConfigSource {
    pub fn env() -> Self {
        Self { name: "env".into(), priority: 10, available: true }
    }
    pub fn sidecar() -> Self {
        Self { name: "sidecar".into(), priority: 20, available: env::var("SIDECAR_MODE").is_ok() }
    }
    pub fn file(path: &str) -> Self {
        Self {
            name: format!("file:{}", path),
            priority: 5,
            available: std::path::Path::new(path).exists(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn get_or_default() {
        let f = EnvironmentFacade::new();
        assert_eq!(f.get_or("MISSING", "default"), "default");
    }
    #[test]
    fn parity_check_fails_on_missing() {
        let f = EnvironmentFacade::new();
        assert!(f.ensure_parity(&["DEFINITELY_NOT_SET_XYZ"]).is_err());
    }
}
