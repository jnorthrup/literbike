// Comprehensive Protocol Detection Unit Tests
// Tests all protocol detectors for accuracy, edge cases, and false positives

use literbike::protocol_registry::{ProtocolDetectionResult, ProtocolDetector};
use literbike::protocol_handlers::{
    HttpDetector, Socks5Detector, TlsDetector, DohDetector
};

mod http_detection_tests {
    use super::*;

    #[test]
    fn test_http_methods_detection() {
        let detector = HttpDetector::new();
        
        let test_cases = vec![
            ("GET / HTTP/1.1\r\n", "http", true),
            ("POST /api HTTP/1.1\r\n", "http", true),
            ("PUT /data HTTP/1.1\r\n", "http", true),
            ("DELETE /item HTTP/1.1\r\n", "http", true),
            ("HEAD /info HTTP/1.1\r\n", "http", true),
            ("OPTIONS * HTTP/1.1\r\n", "http", true),
            ("CONNECT example.com:443 HTTP/1.1\r\n", "http", true),
            ("PATCH /update HTTP/1.1\r\n", "http", true),
        ];
        
        for (input, expected_protocol, should_detect) in test_cases {
            let result = detector.detect(input.as_bytes());
            if should_detect {
                assert_eq!(result.protocol_name, expected_protocol);
                assert!(result.confidence >= detector.confidence_threshold());
            } else {
                assert_eq!(result.protocol_name, "unknown");
            }
        }
    }
    
    #[test]
    fn test_http_version_confidence_scaling() {
        let detector = HttpDetector::new();
        
        // HTTP/1.1 should have higher confidence than bare method
        let http11_request = b"GET /test HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let bare_method = b"GET /test \r\n";
        
        let result_http11 = detector.detect(http11_request);
        let result_bare = detector.detect(bare_method);
        
        assert_eq!(result_http11.protocol_name, "http");
        assert_eq!(result_bare.protocol_name, "http");
        assert!(result_http11.confidence > result_bare.confidence);
    }
    
    #[test]
    fn test_http_edge_cases() {
        let detector = HttpDetector::new();
        
        let edge_cases = vec![
            // Empty request
            ("", false),
            // Too short
            ("G", false),
            ("GE", false),
            ("GET", false),
            // Invalid UTF-8 should not panic
            (b"\xFF\xFE\xFD", false),
            // Case sensitivity
            ("get / HTTP/1.1\r\n", false), // Should be case-sensitive
            ("GET", false), // No space after method
            // Malformed but detectable
            ("GET /\r\n", true),
            ("POST /api", true),
        ];
        
        for (input, should_detect) in edge_cases {
            let input_bytes = match input {
                s if s.is_ascii() => s.as_bytes(),
                _ => input as &[u8],
            };
            let result = detector.detect(input_bytes);
            
            if should_detect {
                assert_eq!(result.protocol_name, "http");
            } else {
                assert_eq!(result.protocol_name, "unknown");
            }
        }
    }
    
    #[test]
    fn test_http_false_positives() {
        let detector = HttpDetector::new();
        
        // These should NOT be detected as HTTP
        let false_positives = vec![
            b"GETS / HTTP/1.1\r\n", // Invalid method
            b"GET\x00/test HTTP/1.1\r\n", // Null byte in method  
            b"GETDATA /api", // Not a valid HTTP method
            b"EMAIL: test@example.com", // Looks like method but isn't
        ];
        
        for input in false_positives {
            let result = detector.detect(input);
            assert_eq!(result.protocol_name, "unknown", 
                      "False positive for: {:?}", String::from_utf8_lossy(input));
        }
    }
    
    #[test]
    fn test_http_metadata_extraction() {
        let detector = HttpDetector::new();
        
        let request = b"GET /api/v1/users HTTP/1.1\r\nHost: api.example.com\r\n\r\n";
        let result = detector.detect(request);
        
        assert_eq!(result.protocol_name, "http");
        assert!(result.metadata.is_some());
        let metadata = result.metadata.unwrap();
        assert!(metadata.contains("GET /api/v1/users HTTP/1.1"));
    }
}

mod socks5_detection_tests {
    use super::*;

    #[test]
    fn test_socks5_valid_handshakes() {
        let detector = Socks5Detector::new();
        
        let test_cases = vec![
            // Complete handshakes
            (vec![0x05, 0x01, 0x00], true, 3), // SOCKS5, 1 method, no auth
            (vec![0x05, 0x02, 0x00, 0x02], true, 4), // SOCKS5, 2 methods
            (vec![0x05, 0x03, 0x00, 0x01, 0x02], true, 5), // SOCKS5, 3 methods
            
            // Partial handshakes (should still detect)
            (vec![0x05, 0x02, 0x00], false, 3), // Incomplete method list
            (vec![0x05, 0x01], false, 2), // Missing methods
            
            // Invalid
            (vec![0x04, 0x01, 0x00], false, 0), // SOCKS4
            (vec![0x05], false, 0), // Too short
            (vec![], false, 0), // Empty
            (vec![0x06, 0x01, 0x00], false, 0), // Invalid version
        ];
        
        for (input, should_complete, expected_bytes) in test_cases {
            let result = detector.detect(&input);
            
            if should_complete {
                assert_eq!(result.protocol_name, "socks5");
                assert!(result.confidence >= detector.confidence_threshold());
                assert_eq!(result.bytes_consumed, expected_bytes);
            } else if input.len() >= 2 && input[0] == 0x05 {
                // Partial but valid SOCKS5
                assert_eq!(result.protocol_name, "socks5");
                assert!(result.confidence >= 200); // Should still be confident
            } else {
                assert_eq!(result.protocol_name, "unknown");
            }
        }
    }
    
    #[test]
    fn test_socks5_confidence_levels() {
        let detector = Socks5Detector::new();
        
        // Complete handshake should have higher confidence
        let complete = vec![0x05, 0x01, 0x00];
        let partial = vec![0x05, 0x02, 0x00]; // Missing one method byte
        
        let result_complete = detector.detect(&complete);
        let result_partial = detector.detect(&partial);
        
        assert_eq!(result_complete.protocol_name, "socks5");
        assert_eq!(result_partial.protocol_name, "socks5");
        assert!(result_complete.confidence > result_partial.confidence);
    }
    
    #[test]
    fn test_socks5_malformed_data() {
        let detector = Socks5Detector::new();
        
        let malformed_cases = vec![
            vec![0x05, 0xFF, 0x00], // Invalid method count (255)
            vec![0x05, 0x00], // Zero methods
            vec![0x05, 0x01, 0x00, 0x01], // Too many bytes for method count
        ];
        
        for input in malformed_cases {
            let result = detector.detect(&input);
            // These should either be unknown or have low confidence
            if result.protocol_name == "socks5" {
                assert!(result.confidence < 250, "Malformed SOCKS5 should have lower confidence");
            }
        }
    }
}

mod tls_detection_tests {
    use super::*;

    #[test]
    fn test_tls_version_detection() {
        let detector = TlsDetector::new();
        
        let test_cases = vec![
            // Valid TLS handshakes
            (vec![0x16, 0x03, 0x01], "tls", 200), // TLS 1.0
            (vec![0x16, 0x03, 0x02], "tls", 210), // TLS 1.1
            (vec![0x16, 0x03, 0x03], "tls", 230), // TLS 1.2
            (vec![0x16, 0x03, 0x04], "tls", 240), // TLS 1.3
            (vec![0x16, 0x03, 0x05], "tls", 150), // Unknown TLS version
            
            // Invalid
            (vec![0x15, 0x03, 0x03], "unknown", 0), // Not handshake
            (vec![0x16, 0x02, 0x03], "unknown", 0), // Not TLS version
            (vec![0x16, 0x03], "unknown", 0), // Too short
            (vec![0x16], "unknown", 0), // Too short
            (vec![], "unknown", 0), // Empty
        ];
        
        for (input, expected_protocol, min_confidence) in test_cases {
            let result = detector.detect(&input);
            assert_eq!(result.protocol_name, expected_protocol);
            
            if expected_protocol == "tls" {
                assert!(result.confidence >= min_confidence);
                assert_eq!(result.bytes_consumed, 3);
                assert!(result.metadata.is_some());
            }
        }
    }
    
    #[test]
    fn test_tls_metadata_extraction() {
        let detector = TlsDetector::new();
        
        let tls12_handshake = vec![0x16, 0x03, 0x03];
        let result = detector.detect(&tls12_handshake);
        
        assert_eq!(result.protocol_name, "tls");
        assert!(result.metadata.is_some());
        let metadata = result.metadata.unwrap();
        assert!(metadata.contains("TLS version: 1.3"));
    }
}

mod doh_detection_tests {
    use super::*;

    #[test]
    fn test_doh_path_detection() {
        let detector = DohDetector::new();
        
        let test_cases = vec![
            // Standard DoH requests
            ("POST /dns-query HTTP/1.1\r\n", "doh", 230),
            ("GET /dns-query?dns=AAAB HTTP/1.1\r\n", "doh", 230),
            
            // Content-type based detection
            ("POST /api HTTP/1.1\r\nContent-Type: application/dns-message\r\n", "doh", 250),
            ("GET /resolve HTTP/1.1\r\nAccept: application/dns-message\r\n", "doh", 250),
            
            // Invalid
            ("GET / HTTP/1.1\r\n", "unknown", 0),
            ("POST /api HTTP/1.1\r\n", "unknown", 0),
            ("GET /dns HTTP/1.1\r\n", "unknown", 0), // Close but not exact
        ];
        
        for (input, expected_protocol, min_confidence) in test_cases {
            let result = detector.detect(input.as_bytes());
            assert_eq!(result.protocol_name, expected_protocol);
            
            if expected_protocol == "doh" {
                assert!(result.confidence >= min_confidence);
            }
        }
    }
    
    #[test]
    fn test_doh_priority_over_http() {
        let http_detector = HttpDetector::new();
        let doh_detector = DohDetector::new();
        
        let doh_request = b"POST /dns-query HTTP/1.1\r\nHost: example.com\r\n\r\n";
        
        let http_result = http_detector.detect(doh_request);
        let doh_result = doh_detector.detect(doh_request);
        
        // Both should detect, but DoH should have higher confidence
        assert_eq!(http_result.protocol_name, "http");
        assert_eq!(doh_result.protocol_name, "doh");
        assert!(doh_result.confidence > http_result.confidence);
    }
    
    #[test]
    fn test_doh_content_type_variants() {
        let detector = DohDetector::new();
        
        let content_type_tests = vec![
            "Content-Type: application/dns-message",
            "content-type: application/dns-message", // Case insensitive
            "Accept: application/dns-message",
            "ACCEPT: APPLICATION/DNS-MESSAGE", // Case insensitive
        ];
        
        for content_type in content_type_tests {
            let request = format!("POST /api HTTP/1.1\r\n{}\r\n\r\n", content_type);
            let result = detector.detect(request.as_bytes());
            
            assert_eq!(result.protocol_name, "doh");
            assert!(result.confidence >= 200);
        }
    }
}

mod protocol_detection_performance {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_detection_performance_regression() {
        let http_detector = HttpDetector::new();
        let socks5_detector = Socks5Detector::new();
        let tls_detector = TlsDetector::new();
        
        let test_data = vec![
            b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(),
            vec![0x05, 0x01, 0x00],
            vec![0x16, 0x03, 0x03, 0x00, 0x01, 0x02],
        ];
        
        let iterations = 10000;
        
        for (detector_name, detector, data) in vec![
            ("HTTP", &http_detector as &dyn ProtocolDetector, &test_data[0]),
            ("SOCKS5", &socks5_detector as &dyn ProtocolDetector, &test_data[1]),
            ("TLS", &tls_detector as &dyn ProtocolDetector, &test_data[2]),
        ] {
            let start = Instant::now();
            
            for _ in 0..iterations {
                let _ = detector.detect(data);
            }
            
            let duration = start.elapsed();
            let per_detection = duration / iterations;
            
            // Each detection should be very fast (< 1μs)
            assert!(per_detection.as_nanos() < 1000, 
                   "{} detection too slow: {:?} per detection", detector_name, per_detection);
        }
    }
    
    #[test]
    fn test_false_positive_rate() {
        let detectors: Vec<(&str, Box<dyn ProtocolDetector>)> = vec![
            ("HTTP", Box::new(HttpDetector::new())),
            ("SOCKS5", Box::new(Socks5Detector::new())),
            ("TLS", Box::new(TlsDetector::new())),
            ("DoH", Box::new(DohDetector::new())),
        ];
        
        // Random data that shouldn't match any protocol
        let random_data = vec![
            vec![0x12, 0x34, 0x56, 0x78],
            vec![0xFF, 0xFE, 0xFD, 0xFC],
            b"random text that isnt a protocol".to_vec(),
            vec![0x00; 32], // All zeros
            vec![0xFF; 32], // All ones
        ];
        
        for (detector_name, detector) in &detectors {
            let mut false_positives = 0;
            
            for data in &random_data {
                let result = detector.detect(data);
                if result.protocol_name != "unknown" {
                    false_positives += 1;
                }
            }
            
            // Should have very low false positive rate
            let false_positive_rate = false_positives as f64 / random_data.len() as f64;
            assert!(false_positive_rate < 0.1, 
                   "{} has high false positive rate: {:.2}%", 
                   detector_name, false_positive_rate * 100.0);
        }
    }
}

mod confidence_scoring_tests {
    use super::*;

    #[test]
    fn test_confidence_threshold_consistency() {
        let detectors: Vec<Box<dyn ProtocolDetector>> = vec![
            Box::new(HttpDetector::new()),
            Box::new(Socks5Detector::new()),
            Box::new(TlsDetector::new()),
            Box::new(DohDetector::new()),
        ];
        
        for detector in detectors {
            let threshold = detector.confidence_threshold();
            
            // Thresholds should be reasonable (not too low or too high)
            assert!(threshold >= 100, "Threshold too low for {}", detector.protocol_name());
            assert!(threshold <= 250, "Threshold too high for {}", detector.protocol_name());
        }
    }
    
    #[test]
    fn test_confidence_never_exceeds_255() {
        let detectors: Vec<Box<dyn ProtocolDetector>> = vec![
            Box::new(HttpDetector::new()),
            Box::new(Socks5Detector::new()),
            Box::new(TlsDetector::new()),
            Box::new(DohDetector::new()),
        ];
        
        // Test with various inputs that should trigger high confidence
        let high_confidence_inputs = vec![
            b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(),
            vec![0x05, 0x01, 0x00],
            vec![0x16, 0x03, 0x04], // TLS 1.3
            b"POST /dns-query HTTP/1.1\r\nContent-Type: application/dns-message\r\n\r\n".to_vec(),
        ];
        
        for detector in detectors {
            for input in &high_confidence_inputs {
                let result = detector.detect(input);
                assert!(result.confidence <= 255, 
                       "Confidence overflow for {} with input: {:?}", 
                       detector.protocol_name(), input);
            }
        }
    }
}

mod bytes_consumed_tests {
    use super::*;

    #[test]
    fn test_bytes_consumed_accuracy() {
        let http_detector = HttpDetector::new();
        
        let request = b"GET /test HTTP/1.1\r\nHost: example.com\r\n\r\nBody content";
        let result = http_detector.detect(request);
        
        assert_eq!(result.protocol_name, "http");
        // Should only consume the first line for HTTP detection
        assert!(result.bytes_consumed <= "GET /test HTTP/1.1".len());
        assert!(result.bytes_consumed > 0);
    }
    
    #[test]
    fn test_bytes_consumed_consistency() {
        let socks5_detector = Socks5Detector::new();
        
        let handshake = vec![0x05, 0x02, 0x00, 0x01];
        let result = socks5_detector.detect(&handshake);
        
        assert_eq!(result.protocol_name, "socks5");
        assert_eq!(result.bytes_consumed, 4); // Version + nmethods + 2 methods
    }
    
    #[test]
    fn test_bytes_consumed_never_exceeds_input() {
        let detectors: Vec<Box<dyn ProtocolDetector>> = vec![
            Box::new(HttpDetector::new()),
            Box::new(Socks5Detector::new()),
            Box::new(TlsDetector::new()),
            Box::new(DohDetector::new()),
        ];
        
        let test_inputs = vec![
            b"short".to_vec(),
            b"medium length input".to_vec(),
            vec![0x05, 0x01],
            vec![0x16, 0x03],
        ];
        
        for detector in detectors {
            for input in &test_inputs {
                let result = detector.detect(input);
                assert!(result.bytes_consumed <= input.len(),
                       "Bytes consumed exceeds input for {}: {} > {}",
                       detector.protocol_name(), result.bytes_consumed, input.len());
            }
        }
    }
}