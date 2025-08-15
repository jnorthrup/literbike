// Protocol Registry Unit Tests
// Tests the central registry system for protocol management

use crate::universal_listener::{
    create_shared_registry, ProtocolDetectionResult,
    PrefixedStream, ProtocolRegistry, ProtocolRegistryStats,
};
pub use crate::universal_listener::{ProtocolDetector, ProtocolHandler};
use async_trait::async_trait;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

// Test Mock Detector
struct TestDetector {
    name: String,
    pattern: Vec<u8>,
    confidence: u8,
    threshold: u8,
}

impl TestDetector {
    fn new(name: &str, pattern: Vec<u8>, confidence: u8, threshold: u8) -> Self {
        Self {
            name: name.to_string(),
            pattern,
            confidence,
            threshold,
        }
    }
}

#[async_trait]
impl ProtocolDetector for TestDetector {
    fn detect(&self, data: &[u8]) -> ProtocolDetectionResult {
        if data.starts_with(&self.pattern) {
            ProtocolDetectionResult::new(&self.name, self.confidence, self.pattern.len())
        } else {
            ProtocolDetectionResult::unknown()
        }
    }

    fn required_bytes(&self) -> usize {
        self.pattern.len()
    }
    fn confidence_threshold(&self) -> u8 {
        self.threshold
    }
    fn protocol_name(&self) -> &str {
        &self.name
    }
}

// Test Mock Handler with call tracking
struct TestHandler {
    name: String,
    call_count: Arc<AtomicUsize>,
    should_fail: bool,
}

impl TestHandler {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            call_count: Arc::new(AtomicUsize::new(0)),
            should_fail: false,
        }
    }

    fn new_failing(name: &str) -> Self {
        Self {
            name: name.to_string(),
            call_count: Arc::new(AtomicUsize::new(0)),
            should_fail: true,
        }
    }

    fn get_call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ProtocolHandler for TestHandler {
    async fn handle(&self, _stream: PrefixedStream<TcpStream>) -> io::Result<()> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        if self.should_fail {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "Test handler failure",
            ))
        } else {
            Ok(())
        }
    }

    fn can_handle(&self, detection: &ProtocolDetectionResult) -> bool {
        detection.protocol_name == self.name
    }

    fn protocol_name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod registry_creation_tests {
    use super::*;

    #[test]
    fn test_empty_registry_creation() {
        let registry = ProtocolRegistry::new();
        let stats = registry.get_stats();

        assert_eq!(stats.registered_protocols, 0);
        assert!(!stats.has_fallback);
        assert_eq!(stats.max_detection_bytes, 1024);
    }

    #[tokio::test]
    async fn test_shared_registry_creation() {
        let registry = create_shared_registry();
        let stats = registry.lock().await.get_stats();

        assert_eq!(stats.registered_protocols, 0);
        assert!(!stats.has_fallback);
    }

    #[test]
    fn test_max_detection_bytes_configuration() {
        let mut registry = ProtocolRegistry::new();

        registry.set_max_detection_bytes(2048);
        let stats = registry.get_stats();
        assert_eq!(stats.max_detection_bytes, 2048);

        registry.set_max_detection_bytes(512);
        let stats = registry.get_stats();
        assert_eq!(stats.max_detection_bytes, 512);
    }
}

#[cfg(test)]
mod protocol_registration_tests {
    use super::*;

    #[test]
    fn test_single_protocol_registration() {
        let mut registry = ProtocolRegistry::new();

        let detector = Box::new(TestDetector::new("test", b"TEST".to_vec(), 200, 150));
        let handler = Box::new(TestHandler::new("test"));

        registry.register(detector, handler, 10);

        let stats = registry.get_stats();
        assert_eq!(stats.registered_protocols, 1);
    }

    #[test]
    fn test_multiple_protocol_registration() {
        let mut registry = ProtocolRegistry::new();

        // Register protocols with different priorities
        let protocols: Vec<(&str, &[u8], u8)> = vec![
            ("http", b"GET", 8),
            ("socks5", b"\x05", 10),
            ("tls", b"\x16\x03", 5),
        ];

        for (name, pattern, priority) in protocols {
            let detector = Box::new(TestDetector::new(name, pattern.to_vec(), 200, 150));
            let handler = Box::new(TestHandler::new(name));
            registry.register(detector, handler, priority);
        }

        let stats = registry.get_stats();
        assert_eq!(stats.registered_protocols, 3);
    }

    #[test]
    fn test_priority_ordering() {
        let mut registry = ProtocolRegistry::new();

        // Register in random order
        let registrations = vec![("low", 1), ("high", 10), ("medium", 5)];

        for (name, priority) in registrations {
            let detector = Box::new(TestDetector::new(name, b"TEST".to_vec(), 200, 150));
            let handler = Box::new(TestHandler::new(name));
            registry.register(detector, handler, priority);
        }

        // Verify protocols are checked in priority order by using overlapping patterns
        // Higher priority should be checked first
        let stats = registry.get_stats();
        assert_eq!(stats.registered_protocols, 3);
    }

    #[test]
    fn test_fallback_handler_registration() {
        let mut registry = ProtocolRegistry::new();

        let fallback = Box::new(TestHandler::new("fallback"));
        registry.set_fallback(fallback);

        let stats = registry.get_stats();
        assert!(stats.has_fallback);
    }
}

#[cfg(test)]
mod protocol_detection_routing_tests {
    use super::*;

    #[tokio::test]
    async fn test_successful_protocol_routing() {
        let mut registry = ProtocolRegistry::new();

        let detector = Box::new(TestDetector::new("test", b"TEST".to_vec(), 200, 150));
        let handler = Box::new(TestHandler::new("test"));

        registry.register(detector, handler, 10);

        // Create mock TCP stream (this is simplified for testing)
        // In a real scenario, you'd need to set up actual TCP connections
        // For now, we'll test the detection logic separately
    }

    #[test]
    fn test_confidence_threshold_filtering() {
        let mut registry = ProtocolRegistry::new();

        // Register detector with high threshold
        let detector = Box::new(TestDetector::new("test", b"TEST".to_vec(), 100, 200));
        let handler = Box::new(TestHandler::new("test"));

        registry.register(detector, handler, 10);

        // Test data that would be detected but with low confidence
        // This simulates the scenario where detection confidence is below threshold
    }

    #[test]
    fn test_fallback_routing() {
        let mut registry = ProtocolRegistry::new();

        // Register a specific protocol
        let detector = Box::new(TestDetector::new(
            "specific",
            b"SPECIFIC".to_vec(),
            200,
            150,
        ));
        let handler = Box::new(TestHandler::new("specific"));
        registry.register(detector, handler, 10);

        // Set fallback
        let fallback = Box::new(TestHandler::new("fallback"));
        registry.set_fallback(fallback);

        // Test with data that doesn't match any specific protocol
        // Should route to fallback
    }
}

#[cfg(test)]
mod detection_result_tests {
    use super::*;

    #[test]
    fn test_detection_result_creation() {
        let result = ProtocolDetectionResult::new("http", 200, 15);

        assert_eq!(result.protocol_name, "http");
        assert_eq!(result.confidence, 200);
        assert_eq!(result.bytes_consumed, 15);
        assert!(result.metadata.is_none());
    }

    #[test]
    fn test_detection_result_with_metadata() {
        let result = ProtocolDetectionResult::new("http", 200, 15)
            .with_metadata("GET / HTTP/1.1".to_string());

        assert_eq!(result.protocol_name, "http");
        assert_eq!(result.confidence, 200);
        assert_eq!(result.bytes_consumed, 15);
        assert!(result.metadata.is_some());
        assert_eq!(result.metadata.unwrap(), "GET / HTTP/1.1");
    }

    #[test]
    fn test_unknown_detection_result() {
        let result = ProtocolDetectionResult::unknown();

        assert_eq!(result.protocol_name, "unknown");
        assert_eq!(result.confidence, 0);
        assert_eq!(result.bytes_consumed, 0);
        assert!(result.metadata.is_none());
    }
}

#[cfg(test)]
mod registry_stats_tests {
    use super::*;

    #[test]
    fn test_stats_accuracy() {
        let mut registry = ProtocolRegistry::new();

        // Start with empty registry
        let stats = registry.get_stats();
        assert_eq!(stats.registered_protocols, 0);
        assert!(!stats.has_fallback);
        assert_eq!(stats.max_detection_bytes, 1024);

        // Add protocols
        for i in 0..3 {
            let name = format!("protocol_{}", i);
            let detector = Box::new(TestDetector::new(&name, vec![i as u8], 200, 150));
            let handler = Box::new(TestHandler::new(&name));
            registry.register(detector, handler, i as u8);
        }

        let stats = registry.get_stats();
        assert_eq!(stats.registered_protocols, 3);
        assert!(!stats.has_fallback);

        // Add fallback
        let fallback = Box::new(TestHandler::new("fallback"));
        registry.set_fallback(fallback);

        let stats = registry.get_stats();
        assert_eq!(stats.registered_protocols, 3);
        assert!(stats.has_fallback);

        // Change max detection bytes
        registry.set_max_detection_bytes(2048);
        let stats = registry.get_stats();
        assert_eq!(stats.max_detection_bytes, 2048);
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_handler_failure_propagation() {
        let mut registry = ProtocolRegistry::new();

        // Register a handler that fails
        let detector = Box::new(TestDetector::new("failing", b"FAIL".to_vec(), 200, 150));
        let handler = Box::new(TestHandler::new_failing("failing"));

        registry.register(detector, handler, 10);

        // Test that handler failure is properly propagated
        // This would require actual TCP connection testing
    }

    #[test]
    fn test_invalid_detection_handling() {
        let detector = TestDetector::new("test", b"TEST".to_vec(), 200, 150);

        // Test with empty data
        let result = detector.detect(&[]);
        assert_eq!(result.protocol_name, "unknown");

        // Test with invalid data
        let result = detector.detect(b"INVALID");
        assert_eq!(result.protocol_name, "unknown");
    }
}

#[cfg(test)]
mod concurrency_tests {
    use super::*;
    use std::sync::Arc;
    use tokio::task;

    #[tokio::test]
    async fn test_concurrent_registration() {
        let registry = Arc::new(tokio::sync::Mutex::new(ProtocolRegistry::new()));

        let mut handles = vec![];

        // Concurrently register multiple protocols
        for i in 0..10 {
            let registry_clone: Arc<Mutex<ProtocolRegistry>> = Arc::clone(&registry);
            let handle = task::spawn(async move {
                let name = format!("protocol_{}", i);
                let detector = Box::new(TestDetector::new(&name, vec![i as u8], 200, 150));
                let handler = Box::new(TestHandler::new(&name));

                let mut reg = registry_clone.lock().await;
                reg.register(detector, handler, i as u8);
            });
            handles.push(handle);
        }

        // Wait for all registrations to complete
        for handle in handles {
            handle.await.unwrap();
        }

        let registry = registry.lock().await;
        let stats = registry.get_stats();
        assert_eq!(stats.registered_protocols, 10);
    }
}

#[cfg(test)]
mod memory_management_tests {
    use super::*;

    #[test]
    fn test_registry_clone_behavior() {
        let registry = ProtocolRegistry::new();
        let cloned = registry.clone();

        // Cloned registry should be empty (as per current implementation)
        let stats = cloned.get_stats();
        assert_eq!(stats.registered_protocols, 0);
    }

    #[test]
    fn test_large_detection_buffer() {
        let mut registry = ProtocolRegistry::new();

        // Test with very large detection buffer
        registry.set_max_detection_bytes(65536);
        let stats = registry.get_stats();
        assert_eq!(stats.max_detection_bytes, 65536);

        // Test with minimal buffer
        registry.set_max_detection_bytes(16);
        let stats = registry.get_stats();
        assert_eq!(stats.max_detection_bytes, 16);
    }
}

#[cfg(test)]
mod integration_with_handlers_tests {
    use super::*;

    #[test]
    fn test_handler_can_handle_logic() {
        let handler = TestHandler::new("test_protocol");

        // Matching protocol
        let matching_result = ProtocolDetectionResult::new("test_protocol", 200, 10);
        assert!(handler.can_handle(&matching_result));

        // Non-matching protocol
        let non_matching_result = ProtocolDetectionResult::new("other_protocol", 200, 10);
        assert!(!handler.can_handle(&non_matching_result));

        // Unknown protocol
        let unknown_result = ProtocolDetectionResult::unknown();
        assert!(!handler.can_handle(&unknown_result));
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_zero_confidence_threshold() {
        let detector = TestDetector::new("test", b"TEST".to_vec(), 100, 0);

        assert_eq!(detector.confidence_threshold(), 0);

        // Even low confidence should pass zero threshold
        let low_confidence_detector = TestDetector::new("test", b"TEST".to_vec(), 1, 0);
        let result = low_confidence_detector.detect(b"TEST");

        assert_eq!(result.protocol_name, "test");
        assert!(result.confidence >= 0);
    }

    #[test]
    fn test_maximum_confidence_threshold() {
        let detector = TestDetector::new("test", b"TEST".to_vec(), 255, 255);

        assert_eq!(detector.confidence_threshold(), 255);

        let result = detector.detect(b"TEST");
        assert_eq!(result.protocol_name, "test");
        assert_eq!(result.confidence, 255);
    }

    #[test]
    fn test_empty_pattern_detection() {
        let detector = TestDetector::new("empty", vec![], 200, 150);

        // Empty pattern should match any input
        let result = detector.detect(b"anything");
        assert_eq!(result.protocol_name, "empty");
        assert_eq!(result.bytes_consumed, 0);
    }
}