//! Reactor CCEK Context Integration

use crate::concurrency::ccek::ContextElement;
use std::sync::Arc;

/// CCEK Context Element for Reactor Service
pub struct ReactorService {
    pub id: String,
    config: ReactorConfig,
}

#[derive(Debug, Clone)]
pub struct ReactorConfig {
    pub select_timeout_ms: u64,
    pub stats_enabled: bool,
}

impl Default for ReactorConfig {
    fn default() -> Self {
        Self {
            select_timeout_ms: 100,
            stats_enabled: true,
        }
    }
}

impl ReactorService {
    pub fn new() -> Self {
        Self {
            id: format!("reactor-{}", std::process::id()),
            config: ReactorConfig::default(),
        }
    }
    
    pub fn with_config(config: ReactorConfig) -> Self {
        Self {
            id: format!("reactor-{}", std::process::id()),
            config,
        }
    }
}

impl Default for ReactorService {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextElement for ReactorService {
    fn key(&self) -> &'static str {
        "ReactorService"
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Clone for ReactorService {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::concurrency::EmptyContext;
    
    #[test]
    fn test_reactor_service_key() {
        assert_eq!(ReactorService::new().key(), "ReactorService");
    }
    
    #[test]
    fn test_context_composition() {
        let service = Arc::new(ReactorService::new());
        let ctx = EmptyContext + service as Arc<dyn ContextElement>;
        assert!(ctx.contains("ReactorService"));
    }
}
