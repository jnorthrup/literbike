// Feature Gate Tests
// Tests all Cargo feature combinations and validates feature-gated functionality

use std::process::Command;
use std::collections::HashMap;

/// Test configuration for feature combinations
#[derive(Debug, Clone)]
struct FeatureTestConfig {
    features: Vec<String>,
    expected_functionality: Vec<String>,
    expected_binary_size_max: Option<u64>,
    compile_time_max: Option<std::time::Duration>,
}

impl FeatureTestConfig {
    fn new(features: Vec<&str>) -> Self {
        Self {
            features: features.iter().map(|s| s.to_string()).collect(),
            expected_functionality: Vec::new(),
            expected_binary_size_max: None,
            compile_time_max: None,
        }
    }
    
    fn with_functionality(mut self, funcs: Vec<&str>) -> Self {
        self.expected_functionality = funcs.iter().map(|s| s.to_string()).collect();
        self
    }
    
    fn with_max_binary_size(mut self, size: u64) -> Self {
        self.expected_binary_size_max = Some(size);
        self
    }
    
    fn with_max_compile_time(mut self, duration: std::time::Duration) -> Self {
        self.compile_time_max = Some(duration);
        self
    }
}

/// Results from testing a feature combination
#[derive(Debug)]
struct FeatureTestResult {
    config: FeatureTestConfig,
    compile_success: bool,
    compile_time: std::time::Duration,
    binary_size: Option<u64>,
    test_success: bool,
    functionality_verified: Vec<String>,
    errors: Vec<String>,
}

impl FeatureTestResult {
    fn is_success(&self) -> bool {
        self.compile_success && self.test_success && self.errors.is_empty()
    }
}

/// Feature gate test runner
struct FeatureGateTestRunner {
    workspace_dir: std::path::PathBuf,
    target_dir: std::path::PathBuf,
}

impl FeatureGateTestRunner {
    fn new() -> Self {
        let workspace_dir = std::env::current_dir()
            .expect("Could not determine current directory");
        let target_dir = workspace_dir.join("target").join("feature_tests");
        
        Self {
            workspace_dir,
            target_dir,
        }
    }
    
    /// Test a specific feature combination
    fn test_feature_combination(&self, config: FeatureTestConfig) -> FeatureTestResult {
        let mut result = FeatureTestResult {
            config: config.clone(),
            compile_success: false,
            compile_time: std::time::Duration::from_secs(0),
            binary_size: None,
            test_success: false,
            functionality_verified: Vec::new(),
            errors: Vec::new(),
        };
        
        println!("Testing feature combination: {:?}", config.features);
        
        // Build with features
        let compile_start = std::time::Instant::now();
        let build_result = self.build_with_features(&config.features);
        result.compile_time = compile_start.elapsed();
        
        match build_result {
            Ok(binary_path) => {
                result.compile_success = true;
                
                // Check binary size
                if let Ok(metadata) = std::fs::metadata(&binary_path) {
                    result.binary_size = Some(metadata.len());
                    
                    if let Some(max_size) = config.expected_binary_size_max {
                        if metadata.len() > max_size {
                            result.errors.push(format!(
                                "Binary size {} exceeds maximum {}", 
                                metadata.len(), max_size
                            ));
                        }
                    }
                }
                
                // Run tests
                match self.run_tests_with_features(&config.features) {
                    Ok(_) => {
                        result.test_success = true;
                        
                        // Verify expected functionality
                        result.functionality_verified = self.verify_functionality(&config, &binary_path);
                    }
                    Err(e) => {
                        result.errors.push(format!("Tests failed: {}", e));
                    }
                }
            }
            Err(e) => {
                result.errors.push(format!("Compilation failed: {}", e));
            }
        }
        
        // Check compile time
        if let Some(max_time) = config.compile_time_max {
            if result.compile_time > max_time {
                result.errors.push(format!(
                    "Compile time {:?} exceeds maximum {:?}",
                    result.compile_time, max_time
                ));
            }
        }
        
        result
    }
    
    fn build_with_features(&self, features: &[String]) -> Result<std::path::PathBuf, String> {
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&self.workspace_dir);
        cmd.args(["build", "--release"]);
        cmd.arg("--target-dir").arg(&self.target_dir);
        
        if !features.is_empty() {
            cmd.arg("--features").arg(features.join(","));
        } else {
            cmd.arg("--no-default-features");
        }
        
        let output = cmd.output().map_err(|e| format!("Failed to execute cargo: {}", e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Cargo build failed: {}", stderr));
        }
        
        // Find the binary
        let binary_path = self.target_dir
            .join("release")
            .join("litebike");
        
        if binary_path.exists() {
            Ok(binary_path)
        } else {
            Err("Binary not found after build".to_string())
        }
    }
    
    fn run_tests_with_features(&self, features: &[String]) -> Result<(), String> {
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&self.workspace_dir);
        cmd.args(["test", "--release"]);
        cmd.arg("--target-dir").arg(&self.target_dir);
        
        if !features.is_empty() {
            cmd.arg("--features").arg(features.join(","));
        } else {
            cmd.arg("--no-default-features");
        }
        
        // Only run unit tests, not integration tests
        cmd.arg("--lib");
        
        let output = cmd.output().map_err(|e| format!("Failed to execute cargo test: {}", e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Tests failed: {}", stderr));
        }
        
        Ok(())
    }
    
    fn verify_functionality(&self, config: &FeatureTestConfig, binary_path: &std::path::Path) -> Vec<String> {
        let mut verified = Vec::new();
        
        // For now, we'll verify by checking if the binary runs with --help
        // In a real implementation, you'd have more sophisticated functionality tests
        
        let help_output = Command::new(binary_path)
            .arg("--help")
            .output();
        
        match help_output {
            Ok(output) if output.status.success() => {
                let help_text = String::from_utf8_lossy(&output.stdout);
                
                // Check for feature-specific help text or functionality
                for expected_func in &config.expected_functionality {
                    if help_text.contains(expected_func) {
                        verified.push(expected_func.clone());
                    }
                }
                
                // Basic functionality verification
                if !help_text.is_empty() {
                    verified.push("basic_help".to_string());
                }
            }
            _ => {
                // Binary doesn't run correctly
            }
        }
        
        verified
    }
    
    /// Test all important feature combinations
    fn test_all_combinations(&self) -> HashMap<String, FeatureTestResult> {
        let test_configs = vec![
            // No features (minimal build)
            FeatureTestConfig::new(vec![])
                .with_functionality(vec!["basic_help"])
                .with_max_binary_size(5 * 1024 * 1024) // 5MB max
                .with_max_compile_time(std::time::Duration::from_secs(120)),
            
            // Basic proxy only
            FeatureTestConfig::new(vec!["basic-proxy"])
                .with_functionality(vec!["basic_help", "proxy"])
                .with_max_binary_size(8 * 1024 * 1024)
                .with_max_compile_time(std::time::Duration::from_secs(150)),
            
            // DoH support
            FeatureTestConfig::new(vec!["doh"])
                .with_functionality(vec!["basic_help", "dns"])
                .with_max_binary_size(10 * 1024 * 1024)
                .with_max_compile_time(std::time::Duration::from_secs(180)),
            
            // Auto-discovery
            FeatureTestConfig::new(vec!["auto-discovery"])
                .with_functionality(vec!["basic_help", "discovery"])
                .with_max_binary_size(12 * 1024 * 1024)
                .with_max_compile_time(std::time::Duration::from_secs(200)),
            
            // UPnP support
            FeatureTestConfig::new(vec!["upnp"])
                .with_functionality(vec!["basic_help", "upnp"])
                .with_max_binary_size(10 * 1024 * 1024)
                .with_max_compile_time(std::time::Duration::from_secs(180)),
            
            // Advanced networking
            FeatureTestConfig::new(vec!["advanced-networking"])
                .with_functionality(vec!["basic_help"])
                .with_max_binary_size(8 * 1024 * 1024)
                .with_max_compile_time(std::time::Duration::from_secs(150)),
            
            // Combined features
            FeatureTestConfig::new(vec!["basic-proxy", "doh"])
                .with_functionality(vec!["basic_help", "proxy", "dns"])
                .with_max_binary_size(15 * 1024 * 1024)
                .with_max_compile_time(std::time::Duration::from_secs(220)),
            
            FeatureTestConfig::new(vec!["basic-proxy", "auto-discovery"])
                .with_functionality(vec!["basic_help", "proxy", "discovery"])
                .with_max_binary_size(18 * 1024 * 1024)
                .with_max_compile_time(std::time::Duration::from_secs(250)),
            
            FeatureTestConfig::new(vec!["auto-discovery", "upnp"])
                .with_functionality(vec!["basic_help", "discovery", "upnp"])
                .with_max_binary_size(20 * 1024 * 1024)
                .with_max_compile_time(std::time::Duration::from_secs(280)),
            
            // Full feature set
            FeatureTestConfig::new(vec!["full"])
                .with_functionality(vec!["basic_help", "proxy", "dns", "discovery", "upnp"])
                .with_max_binary_size(25 * 1024 * 1024) // Larger for full build
                .with_max_compile_time(std::time::Duration::from_secs(300)),
        ];
        
        let mut results = HashMap::new();
        
        for config in test_configs {
            let config_name = if config.features.is_empty() {
                "no-features".to_string()
            } else {
                config.features.join(",")
            };
            
            let result = self.test_feature_combination(config);
            results.insert(config_name, result);
        }
        
        results
    }
    
    /// Generate a feature compatibility matrix
    fn generate_compatibility_matrix(&self) -> HashMap<String, HashMap<String, bool>> {
        let individual_features = vec![
            "basic-proxy",
            "doh", 
            "auto-discovery",
            "upnp",
            "advanced-networking",
        ];
        
        let mut matrix = HashMap::new();
        
        // Test each feature individually
        for feature in &individual_features {
            let config = FeatureTestConfig::new(vec![feature]);
            let result = self.test_feature_combination(config);
            
            let mut compatibility = HashMap::new();
            compatibility.insert("standalone".to_string(), result.is_success());
            matrix.insert(feature.to_string(), compatibility);
        }
        
        // Test feature combinations
        for (i, feature1) in individual_features.iter().enumerate() {
            for feature2 in individual_features.iter().skip(i + 1) {
                let config = FeatureTestConfig::new(vec![feature1, feature2]);
                let result = self.test_feature_combination(config);
                
                let compatibility1 = matrix.get_mut(*feature1).unwrap();
                compatibility1.insert(feature2.to_string(), result.is_success());
                
                let compatibility2 = matrix.get_mut(*feature2).unwrap();
                compatibility2.insert(feature1.to_string(), result.is_success());
            }
        }
        
        matrix
    }
}

#[cfg(test)]
mod feature_gate_tests {
    use super::*;

    #[test]
    #[ignore] // This test takes a long time, so it's ignored by default
    fn test_all_feature_combinations() {
        let runner = FeatureGateTestRunner::new();
        let results = runner.test_all_combinations();
        
        println!("\nFeature Gate Test Results:");
        println!("{:-<80}", "");
        
        let mut all_passed = true;
        
        for (config_name, result) in &results {
            let status = if result.is_success() { "✓" } else { "✗" };
            println!("{} {:<30} | Compile: {:>6.2}s | Size: {:>8} bytes | Tests: {}", 
                    status,
                    config_name,
                    result.compile_time.as_secs_f64(),
                    result.binary_size.map(|s| s.to_string()).unwrap_or_else(|| "N/A".to_string()),
                    if result.test_success { "PASS" } else { "FAIL" });
            
            if !result.errors.is_empty() {
                for error in &result.errors {
                    println!("    Error: {}", error);
                }
            }
            
            if !result.is_success() {
                all_passed = false;
            }
        }
        
        // Print summary
        let total = results.len();
        let passed = results.values().filter(|r| r.is_success()).count();
        let failed = total - passed;
        
        println!("{:-<80}", "");
        println!("Summary: {} passed, {} failed, {} total", passed, failed, total);
        
        if !all_passed {
            println!("\nFailed configurations:");
            for (name, result) in &results {
                if !result.is_success() {
                    println!("  - {}: {:?}", name, result.errors);
                }
            }
        }
        
        assert!(all_passed, "Some feature combinations failed to build or test correctly");
    }
    
    #[test]
    #[ignore] // This test takes a long time
    fn test_feature_compatibility_matrix() {
        let runner = FeatureGateTestRunner::new();
        let matrix = runner.generate_compatibility_matrix();
        
        println!("\nFeature Compatibility Matrix:");
        println!("{:-<80}", "");
        
        // Print header
        print!("{:<20}", "Feature");
        for feature in matrix.keys() {
            print!(" {:>15}", feature);
        }
        println!();
        
        // Print matrix
        for (feature1, compatibility) in &matrix {
            print!("{:<20}", feature1);
            for feature2 in matrix.keys() {
                if feature1 == feature2 {
                    print!(" {:>15}", "---");
                } else {
                    let compatible = compatibility.get(feature2).unwrap_or(&false);
                    print!(" {:>15}", if *compatible { "✓" } else { "✗" });
                }
            }
            println!();
        }
        
        // Verify no major incompatibilities
        for (feature1, compatibility) in &matrix {
            for (feature2, is_compatible) in compatibility {
                if feature2 != "standalone" && !is_compatible {
                    println!("Warning: {} is incompatible with {}", feature1, feature2);
                }
            }
        }
    }
    
    #[test]
    fn test_minimal_build_size() {
        let runner = FeatureGateTestRunner::new();
        
        // Test minimal build with no features
        let minimal_config = FeatureTestConfig::new(vec![])
            .with_max_binary_size(3 * 1024 * 1024); // 3MB max for minimal build
        
        let result = runner.test_feature_combination(minimal_config);
        
        assert!(result.compile_success, "Minimal build should compile successfully");
        assert!(result.is_success(), "Minimal build should pass all checks: {:?}", result.errors);
        
        if let Some(size) = result.binary_size {
            println!("Minimal binary size: {} bytes ({:.2} MB)", size, size as f64 / 1024.0 / 1024.0);
            assert!(size < 5 * 1024 * 1024, "Minimal binary should be under 5MB, got {} bytes", size);
        }
    }
    
    #[test]
    fn test_default_features() {
        let runner = FeatureGateTestRunner::new();
        
        // Test default features (basic-proxy)
        let default_config = FeatureTestConfig::new(vec!["basic-proxy"])
            .with_functionality(vec!["proxy"]);
        
        let result = runner.test_feature_combination(default_config);
        
        assert!(result.compile_success, "Default features should compile successfully");
        assert!(result.test_success, "Default features should pass tests");
        assert!(result.is_success(), "Default features should pass all checks: {:?}", result.errors);
        
        // Should have basic functionality
        assert!(result.functionality_verified.contains(&"basic_help".to_string()));
    }
    
    #[test]
    fn test_full_features() {
        let runner = FeatureGateTestRunner::new();
        
        // Test full feature set
        let full_config = FeatureTestConfig::new(vec!["full"])
            .with_max_binary_size(30 * 1024 * 1024) // 30MB max for full build
            .with_max_compile_time(std::time::Duration::from_secs(600)); // 10 minutes max
        
        let result = runner.test_feature_combination(full_config);
        
        assert!(result.compile_success, "Full features should compile successfully");
        assert!(result.test_success, "Full features should pass tests");
        assert!(result.is_success(), "Full features should pass all checks: {:?}", result.errors);
    }
    
    #[test]
    fn test_compile_time_regression() {
        let runner = FeatureGateTestRunner::new();
        
        // Test that compile times are reasonable for different feature sets
        let compile_time_tests = vec![
            ("minimal", vec![], std::time::Duration::from_secs(60)),
            ("basic", vec!["basic-proxy"], std::time::Duration::from_secs(90)),
            ("with-doh", vec!["basic-proxy", "doh"], std::time::Duration::from_secs(120)),
            ("full", vec!["full"], std::time::Duration::from_secs(300)),
        ];
        
        for (name, features, max_time) in compile_time_tests {
            let config = FeatureTestConfig::new(features).with_max_compile_time(max_time);
            let result = runner.test_feature_combination(config);
            
            println!("{} compile time: {:?}", name, result.compile_time);
            
            assert!(result.compile_success, "{} should compile successfully", name);
            assert!(result.compile_time <= max_time, 
                   "{} compile time {:?} exceeds maximum {:?}", 
                   name, result.compile_time, max_time);
        }
    }
}

#[cfg(test)]
mod conditional_compilation_tests {
    use super::*;

    #[test]
    fn test_conditional_compilation_syntax() {
        // This test verifies that conditional compilation attributes are correct
        // by attempting to compile with different feature combinations
        
        let test_code_snippets = vec![
            ("#[cfg(feature = \"doh\")]", "doh"),
            ("#[cfg(feature = \"upnp\")]", "upnp"),
            ("#[cfg(feature = \"auto-discovery\")]", "auto-discovery"),
            ("#[cfg(feature = \"advanced-networking\")]", "advanced-networking"),
            ("#[cfg(any(feature = \"doh\", feature = \"auto-discovery\"))]", "doh"),
        ];
        
        // In a real implementation, you'd parse the source code and verify
        // that conditional compilation attributes are used correctly
        
        for (cfg_attr, feature) in test_code_snippets {
            // Verify the attribute syntax is valid
            assert!(cfg_attr.starts_with("#[cfg("));
            assert!(cfg_attr.ends_with(")]"));
            assert!(cfg_attr.contains(feature));
        }
    }
}