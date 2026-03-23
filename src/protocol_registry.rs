//! Protocol registry - minimal stub for protocol detection
//! Replaced by CCEK protocol module (ccek/agent8888/protocol/)

use std::any::Any;

pub trait ProtocolDetector: Send + Sync + 'static {
    fn detect(&self, data: &[u8]) -> Option<ProtocolDetectionResult>;
    fn protocol_name(&self) -> &'static str {
        "unknown"
    }
    fn confidence_threshold(&self) -> u8 {
        200
    }
}

#[derive(Debug, Clone)]
pub struct ProtocolDetectionResult;

pub struct ProtocolRegistry;

impl ProtocolRegistry {
    pub fn new() -> Self {
        ProtocolRegistry
    }
    pub fn register_detector(&mut self, _name: &str, _detector: Box<dyn ProtocolDetector>) {}
}
