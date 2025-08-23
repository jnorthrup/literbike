/// Minimal ProtocolDetector trait used by tests and detectors.
/// Implementations in the repo should implement this trait; this shim
/// provides the type/signature expected by tests so they can compile.
pub trait ProtocolDetector: Send + Sync {
    /// Inspect the provided bytes and return a ProtocolDetectionResult.
    fn detect(&self, data: &[u8]) -> ProtocolDetectionResult;

    /// Human-readable name of the protocol this detector recognizes.
    /// Default implementation returns "unknown"; real detectors should
    /// override to return a static name (e.g. "http").
    fn protocol_name(&self) -> &'static str {
        "unknown"
    }

    /// A heuristic confidence threshold used by tests.
    fn confidence_threshold(&self) -> u8 {
        200
    }
}

/// A tiny ProtocolRegistry placeholder. The real project may provide
/// a richer implementation; tests only reference the type for imports.
pub struct ProtocolRegistry;

impl ProtocolRegistry {
    pub fn new() -> Self {
        ProtocolRegistry
    }

    /// Register a detector (no-op in shim)
    pub fn register_detector(&mut self, _name: &str, _detector: Box<dyn ProtocolDetector>) {}
}

// ProtocolDetectionResult is available from crate::types; tests import it via
// literbike::protocol_registry::ProtocolDetectionResult, so provide a small
// type alias here.
pub type ProtocolDetectionResult = crate::types::ProtocolDetectionResult;
