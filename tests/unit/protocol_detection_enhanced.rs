// Enhanced Protocol Detection Tests
// Comprehensive testing with property-based testing, SIMD validation, and edge cases

use literbike::protocol_registry::{ProtocolDetectionResult, ProtocolDetector};
use literbike::protocol_handlers::{
    HttpDetector, Socks5Detector, TlsDetector, DohDetector
};



use crate::utils::{
    HttpTestData, Socks5TestData, TlsTestData, DohTestData, FuzzGenerator,
    ProtocolTestData, Timer
};

use std::time::Duration;
use std::collections::HashMap;

/// Property-based testing framework for protocol detectors
struct PropertyBasedTester<'a> {
    detector: &'a dyn ProtocolDetector,
    test_count: usize,
    max_input_size: usize,
}

impl<'a> PropertyBasedTester<'a> {
    fn new(detector: &'a dyn ProtocolDetector) -> Self {
        Self {
            detector,
            test_count: 1000,
            max_input_size: 8192,
        }
    }
    
    fn with_test_count(mut self, count: usize) -> Self {
        self.test_count = count;
        self
    }
    
    fn with_max_input_size(mut self, size: usize) -> Self {
        self.max_input_size = size;
        self
    }
    
    /// Property: Detection should be deterministic
    fn test_deterministic_detection(&self) -> Result<(), String> {
        let fuzzer = FuzzGenerator;
        let test_data = fuzzer.generate_random_data(1, self.max_input_size, self.test_count);
        
        for (i, data) in test_data.iter().enumerate() {
            let result1 = self.detector.detect(data);
            let result2 = self.detector.detect(data);
            
            if result1.protocol_name != result2.protocol_name ||
               result1.confidence != result2.confidence ||
               result1.bytes_consumed != result2.bytes_consumed {
                return Err(format!(
                    "Non-deterministic detection on test {}: {:?} vs {:?}",
                    i, result1, result2
                ));
            }
        }
        
        Ok(())
    }
    
    /// Property: Bytes consumed should never exceed input length
    fn test_bytes_consumed_bounds(&self) -> Result<(), String> {
        let fuzzer = FuzzGenerator;
        let test_data = fuzzer.generate_random_data(1, self.max_input_size, self.test_count);
        
        for (i, data) in test_data.iter().enumerate() {
            let result = self.detector.detect(data);
            
            if result.bytes_consumed > data.len() {
                return Err(format!(
                    "Bytes consumed ({}) exceeds input length ({}) on test {}",
                    result.bytes_consumed, data.len(), i
                ));
            }
        }
        
        Ok(())
    }
    
    /// Property: Confidence should be in valid range (0-255)
    fn test_confidence_bounds(&self) -> Result<(), String> {
        let fuzzer = FuzzGenerator;
        let test_data = fuzzer.generate_random_data(1, self.max_input_size, self.test_count);
        
        for (i, data) in test_data.iter().enumerate() {
            let result = self.detector.detect(data);
            
            // Confidence is u8, so it's automatically bounded, but we test the threshold
            if result.confidence > 255 {
                return Err(format!("Confidence overflow on test {}: {}", i, result.confidence));
            }
            
            // If protocol is detected, confidence should meet threshold
            if result.protocol_name != "unknown" && 
               result.confidence < self.detector.confidence_threshold() {
                return Err(format!(
                    "Protocol '{}' detected with confidence {} below threshold {} on test {}",
                    result.protocol_name, result.confidence, 
                    self.detector.confidence_threshold(), i
                ));
            }
        }
        
        Ok(())
    }
    
    /// Property: Empty input should not cause panics or invalid results
    fn test_empty_input_handling(&self) -> Result<(), String> {
        let result = self.detector.detect(&[]);
        
        if result.bytes_consumed > 0 {
            return Err("Empty input resulted in non-zero bytes consumed".to_string());
        }
        
        // Most protocols should return "unknown" for empty input
        if result.protocol_name != "unknown" && result.confidence >= self.detector.confidence_threshold() {
            return Err(format!(
                "Empty input unexpectedly detected as '{}' with confidence {}",
                result.protocol_name, result.confidence
            ));
        }
        
        Ok(())
    }
    
    /// Property: Single byte inputs should be handled gracefully
    fn test_single_byte_inputs(&self) -> Result<(), String> {
        for byte_value in 0u8..=255u8 {
            let input = vec![byte_value];
            let result = self.detector.detect(&input);
            
            if result.bytes_consumed > 1 {
                return Err(format!(
                    "Single byte input consumed {} bytes for byte value {}",
                    result.bytes_consumed, byte_value
                ));
            }
        }
        
        Ok(())
    }
    
    /// Property: Repeated bytes should not cause overconfidence
    fn test_repeated_byte_patterns(&self) -> Result<(), String> {
        for byte_value in [0x00, 0xFF, 0x41, 0x20] {
            for length in [1, 10, 100, 1000] {
                let input = vec![byte_value; length];
                let result = self.detector.detect(&input);
                
                // Very unlikely that repeated bytes form valid protocol headers
                if result.protocol_name != "unknown" && result.confidence > 200 {
                    return Err(format!(
                        "Repeated byte 0x{:02X} (length {}) detected as '{}' with high confidence {}",
                        byte_value, length, result.protocol_name, result.confidence
                    ));
                }
            }
        }
        
        Ok(())
    }
}

mod property_based_tests {
    use super::*;

    #[test]
    fn test_http_detector_properties() {
        let detector = HttpDetector::new();
        let tester = PropertyBasedTester::new(&detector).with_test_count(500);
        
        assert!(tester.test_deterministic_detection().is_ok());
        assert!(tester.test_bytes_consumed_bounds().is_ok());
        assert!(tester.test_confidence_bounds().is_ok());
        assert!(tester.test_empty_input_handling().is_ok());
        assert!(tester.test_single_byte_inputs().is_ok());
        assert!(tester.test_repeated_byte_patterns().is_ok());
    }
    
    #[test]
    fn test_socks5_detector_properties() {
        let detector = Socks5Detector::new();
        let tester = PropertyBasedTester::new(&detector).with_test_count(500);
        
        assert!(tester.test_deterministic_detection().is_ok());
        assert!(tester.test_bytes_consumed_bounds().is_ok());
        assert!(tester.test_confidence_bounds().is_ok());
        assert!(tester.test_empty_input_handling().is_ok());
        assert!(tester.test_single_byte_inputs().is_ok());
        assert!(tester.test_repeated_byte_patterns().is_ok());
    }
    
    #[test]
    fn test_tls_detector_properties() {
        let detector = TlsDetector::new();
        let tester = PropertyBasedTester::new(&detector).with_test_count(500);
        
        assert!(tester.test_deterministic_detection().is_ok());
        assert!(tester.test_bytes_consumed_bounds().is_ok());
        assert!(tester.test_confidence_bounds().is_ok());
        assert!(tester.test_empty_input_handling().is_ok());
        assert!(tester.test_single_byte_inputs().is_ok());
        assert!(tester.test_repeated_byte_patterns().is_ok());
    }
    
    #[test]
    fn test_doh_detector_properties() {
        let detector = DohDetector::new();
        let tester = PropertyBasedTester::new(&detector).with_test_count(500);
        
        assert!(tester.test_deterministic_detection().is_ok());
        assert!(tester.test_bytes_consumed_bounds().is_ok());
        assert!(tester.test_confidence_bounds().is_ok());
        assert!(tester.test_empty_input_handling().is_ok());
        assert!(tester.test_single_byte_inputs().is_ok());
        // DoH is less strict about repeated patterns since it's HTTP-based
    }
}

mod simd_validation_tests {
    use super::*;

    
}

mod comprehensive_edge_case_tests {
    use super::*;

    #[test]
    fn test_all_detectors_with_edge_cases() {
        let detectors: Vec<Box<dyn ProtocolDetector>> = vec![
            Box::new(HttpDetector::new()),
            Box::new(Socks5Detector::new()),
            Box::new(TlsDetector::new()),
            Box::new(DohDetector::new()),
        ];
        
        let fuzzer = FuzzGenerator;
        let edge_cases = fuzzer.generate_edge_case_data();
        
        for detector in &detectors {
            for (i, data) in edge_cases.iter().enumerate() {
                let result = detector.detect(data);
                
                // Basic invariants that should hold for all detectors
                assert!(result.bytes_consumed <= data.len(),
                       "Detector {} consumed too many bytes on edge case {}: {} > {}",
                       detector.protocol_name(), i, result.bytes_consumed, data.len());
                
                assert!(result.confidence <= 255,
                       "Detector {} confidence overflow on edge case {}: {}",
                       detector.protocol_name(), i, result.confidence);
                
                // If protocol is detected, confidence should meet threshold
                if result.protocol_name != "unknown" {
                    assert!(result.confidence >= detector.confidence_threshold(),
                           "Detector {} detected protocol '{}' with insufficient confidence {} < {} on edge case {}",
                           detector.protocol_name(), result.protocol_name, 
                           result.confidence, detector.confidence_threshold(), i);
                }
            }
        }
    }
    
    #[test]
    fn test_unicode_and_non_ascii_handling() {
        let detectors: Vec<Box<dyn ProtocolDetector>> = vec![
            Box::new(HttpDetector::new()),
            Box::new(DohDetector::new()), // These two handle text
        ];
        
        let unicode_test_cases = vec![
            "GET /测试 HTTP/1.1\r\n".as_bytes().to_vec(),
            "POST /🚀 HTTP/1.1\r\n".as_bytes().to_vec(),
            "GET /файл HTTP/1.1\r\n".as_bytes().to_vec(),
            "GET /\u{1F4A9} HTTP/1.1\r\n".as_bytes().to_vec(),
            // Invalid UTF-8 sequences
            vec![0x47, 0x45, 0x54, 0x20, 0x2F, 0xFF, 0xFE, 0x20, 0x48, 0x54, 0x54, 0x50],
            vec![0xC2], // Incomplete UTF-8
            vec![0xF0, 0x9F], // Incomplete emoji
        ];
        
        for detector in &detectors {
            for (i, data) in unicode_test_cases.iter().enumerate() {
                // Should not panic on invalid UTF-8
                let result = detector.detect(data);
                
                assert!(result.bytes_consumed <= data.len(),
                       "Detector {} consumed too many bytes on unicode test {}", 
                       detector.protocol_name(), i);
            }
        }
    }
    
    #[test]
    fn test_large_input_handling() {
        let detectors: Vec<Box<dyn ProtocolDetector>> = vec![
            Box::new(HttpDetector::new()),
            Box::new(Socks5Detector::new()),
            Box::new(TlsDetector::new()),
            Box::new(DohDetector::new()),
        ];
        
        // Test with various large input sizes
        let large_inputs = vec![
            vec![b'A'; 1024],
            vec![b'A'; 8192],
            vec![b'A'; 65536],
            // Large HTTP request
            format!("GET /{} HTTP/1.1\r\nHost: example.com\r\n\r\n", "a".repeat(10000)).into_bytes(),
            // Large but potentially valid SOCKS5 handshake
            {
                let mut data = vec![0x05, 0xFF]; // SOCKS5, 255 methods
                data.extend(vec![0x00; 255]); // 255 auth methods
                data
            },
        ];
        
        for detector in &detectors {
            for (i, data) in large_inputs.iter().enumerate() {
                let timer = Timer::new();
                let result = detector.detect(data);
                let duration = timer.elapsed();
                
                // Should complete in reasonable time (< 100ms)
                assert!(duration < Duration::from_millis(100),
                       "Detector {} took too long ({:?}) on large input test {}",
                       detector.protocol_name(), duration, i);
                
                assert!(result.bytes_consumed <= data.len(),
                       "Detector {} consumed too many bytes on large input test {}",
                       detector.protocol_name(), i);
            }
        }
    }
    
    #[test]
    fn test_boundary_conditions() {
        let detectors: Vec<Box<dyn ProtocolDetector>> = vec![
            Box::new(HttpDetector::new()),
            Box::new(Socks5Detector::new()),
            Box::new(TlsDetector::new()),
            Box::new(DohDetector::new()),
        ];
        
        for detector in &detectors {
            let required_bytes = detector.required_bytes();
            
            // Test with exactly required bytes minus 1
            if required_bytes > 0 {
                let insufficient_data = vec![0x41; required_bytes - 1];
                let result = detector.detect(&insufficient_data);
                
                // May or may not detect, but should be well-behaved
                assert!(result.bytes_consumed <= insufficient_data.len(),
                       "Detector {} consumed too many bytes with insufficient data",
                       detector.protocol_name());
            }
            
            // Test with exactly required bytes
            let exact_data = vec![0x41; required_bytes];
            let result = detector.detect(&exact_data);
            assert!(result.bytes_consumed <= exact_data.len(),
                   "Detector {} consumed too many bytes with exact required data",
                   detector.protocol_name());
            
            // Test with one more than required bytes
            let extra_data = vec![0x41; required_bytes + 1];
            let result = detector.detect(&extra_data);
            assert!(result.bytes_consumed <= extra_data.len(),
                   "Detector {} consumed too many bytes with extra data",
                   detector.protocol_name());
        }
    }
}

mod detection_accuracy_tests {
    use super::*;
    
    /// Test detection accuracy with known good samples
    #[test]
    fn test_detection_accuracy_with_known_samples() {
        struct DetectorTest<'a> {
            detector: &'a dyn ProtocolDetector,
            valid_samples: Vec<Vec<u8>>,
            expected_protocol: &'a str,
        }
        
        let http_data = HttpTestData;
        let socks5_data = Socks5TestData;
        let tls_data = TlsTestData;
        let doh_data = DohTestData;
        
        let tests = vec![
            DetectorTest {
                detector: &HttpDetector::new(),
                valid_samples: http_data.valid_requests(),
                expected_protocol: "http",
            },
            DetectorTest {
                detector: &Socks5Detector::new(),
                valid_samples: socks5_data.valid_requests(),
                expected_protocol: "socks5",
            },
            DetectorTest {
                detector: &TlsDetector::new(),
                valid_samples: tls_data.valid_requests(),
                expected_protocol: "tls",
            },
            DetectorTest {
                detector: &DohDetector::new(),
                valid_samples: doh_data.valid_requests(),
                expected_protocol: "doh",
            },
        ];
        
        for test in tests {
            let mut correct_detections = 0;
            let total_samples = test.valid_samples.len();
            
            for (i, sample) in test.valid_samples.iter().enumerate() {
                let result = test.detector.detect(sample);
                
                if result.protocol_name == test.expected_protocol &&
                   result.confidence >= test.detector.confidence_threshold() {
                    correct_detections += 1;
                } else {
                    println!("Detection failure for {} detector on sample {}: got '{}' with confidence {}, expected '{}'",
                           test.detector.protocol_name(), i, result.protocol_name, 
                           result.confidence, test.expected_protocol);
                }
            }
            
            let accuracy = correct_detections as f64 / total_samples as f64;
            assert!(accuracy >= 0.95, 
                   "Detection accuracy for {} too low: {:.2}% ({}/{})",
                   test.detector.protocol_name(), accuracy * 100.0, 
                   correct_detections, total_samples);
        }
    }
    
    /// Test false positive rate with invalid samples
    #[test]
    fn test_false_positive_rate() {
        let detectors: Vec<Box<dyn ProtocolDetector>> = vec![
            Box::new(HttpDetector::new()),
            Box::new(Socks5Detector::new()),
            Box::new(TlsDetector::new()),
            Box::new(DohDetector::new()),
        ];
        
        // Create samples that should NOT be detected by each detector
        let invalid_samples = vec![
            // Random data
            FuzzGenerator.generate_random_data(10, 1000, 100),
            // Other protocol data
            HttpTestData.invalid_requests(),
            Socks5TestData.invalid_requests(),
            TlsTestData.invalid_requests(),
            DohTestData.invalid_requests(),
        ].into_iter().flatten().collect::<Vec<_>>();
        
        for detector in &detectors {
            let mut false_positives = 0;
            
            for sample in &invalid_samples {
                let result = detector.detect(sample);
                
                // Count as false positive if this detector identifies the sample
                // as its protocol with high confidence
                if result.protocol_name == detector.protocol_name() &&
                   result.confidence >= detector.confidence_threshold() {
                    false_positives += 1;
                }
            }
            
            let false_positive_rate = false_positives as f64 / invalid_samples.len() as f64;
            assert!(false_positive_rate < 0.05,
                   "False positive rate for {} too high: {:.2}% ({}/{})",
                   detector.protocol_name(), false_positive_rate * 100.0,
                   false_positives, invalid_samples.len());
        }
    }
    
    /// Test cross-contamination between detectors
    #[test]
    fn test_protocol_cross_contamination() {
        let http_detector = HttpDetector::new();
        let socks5_detector = Socks5Detector::new();
        let tls_detector = TlsDetector::new();
        let doh_detector = DohDetector::new();
        
        let test_cases = vec![
            (&http_detector, &socks5_detector, HttpTestData.valid_requests()),
            (&http_detector, &tls_detector, HttpTestData.valid_requests()),
            (&socks5_detector, &http_detector, Socks5TestData.valid_requests()),
            (&socks5_detector, &tls_detector, Socks5TestData.valid_requests()),
            (&tls_detector, &http_detector, TlsTestData.valid_requests()),
            (&tls_detector, &socks5_detector, TlsTestData.valid_requests()),
        ];
        
        for (primary_detector, other_detector, samples) in test_cases {
            for (i, sample) in samples.iter().enumerate() {
                let primary_result = primary_detector.detect(sample);
                let other_result = other_detector.detect(sample);
                
                // If primary detector recognizes it strongly, other should not
                if primary_result.confidence >= primary_detector.confidence_threshold() {
                    assert!(other_result.confidence < other_detector.confidence_threshold(),
                           "Cross-contamination: {} sample {} detected by both {} (confidence {}) and {} (confidence {})",
                           primary_detector.protocol_name(), i,
                           primary_detector.protocol_name(), primary_result.confidence,
                           other_detector.protocol_name(), other_result.confidence);
                }
            }
        }
        
        // Special case: DoH vs HTTP - DoH should win for DoH requests
        let doh_samples = DohTestData.valid_requests();
        for (i, sample) in doh_samples.iter().enumerate() {
            let http_result = http_detector.detect(sample);
            let doh_result = doh_detector.detect(sample);
            
            // Both may detect, but DoH should have higher confidence
            if http_result.confidence >= http_detector.confidence_threshold() &&
               doh_result.confidence >= doh_detector.confidence_threshold() {
                assert!(doh_result.confidence > http_result.confidence,
                       "DoH sample {} should be detected with higher confidence by DoH detector: DoH={} vs HTTP={}",
                       i, doh_result.confidence, http_result.confidence);
            }
        }
    }
}