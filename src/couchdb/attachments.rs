use crate::couchdb::{
    types::{AttachmentInfo, AttachmentDigest},
    error::{CouchError, CouchResult},
};
use std::collections::HashMap;
use log::{info, warn, error, debug};

/// Attachment manager for handling document attachments
pub struct AttachmentManager;

impl AttachmentManager {
    /// Validate attachment data
    pub fn validate_attachment(name: &str, data: &[u8], content_type: &str) -> CouchResult<()> {
        // Validate attachment name
        if name.is_empty() {
            return Err(CouchError::bad_request("Attachment name cannot be empty"));
        }
        
        // Check for invalid characters in name
        if name.contains('/') || name.contains('\\') || name.contains('\0') {
            return Err(CouchError::bad_request("Invalid characters in attachment name"));
        }
        
        // Check size limits (CouchDB typically allows up to 1GB attachments)
        if data.len() > 1_073_741_824 {
            return Err(CouchError::request_entity_too_large("Attachment exceeds size limit"));
        }
        
        // Validate content type
        if content_type.is_empty() {
            return Err(CouchError::bad_request("Content type is required"));
        }
        
        Ok(())
    }
    
    /// Create attachment info from data
    pub fn create_attachment_info(data: &[u8], content_type: &str) -> CouchResult<AttachmentInfo> {
        let digest = Self::calculate_digest(data);
        
        Ok(AttachmentInfo {
            content_type: content_type.to_string(),
            length: data.len() as u64,
            digest,
            stub: Some(false),
            revpos: Some(1),
            data: None,
        })
    }
    
    /// Calculate MD5 digest for attachment
    pub fn calculate_digest(data: &[u8]) -> AttachmentDigest {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        // In a real implementation, we'd use MD5 or SHA
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        format!("md5-{:x}", hasher.finish())
    }
    
    /// Encode attachment data as base64
    pub fn encode_base64(data: &[u8]) -> String {
        base64::encode(data)
    }
    
    /// Decode base64 attachment data
    pub fn decode_base64(encoded: &str) -> CouchResult<Vec<u8>> {
        base64::decode(encoded)
            .map_err(|e| CouchError::bad_request(&format!("Invalid base64 data: {}", e)))
    }
    
    /// Check if content type is supported
    pub fn is_supported_content_type(content_type: &str) -> bool {
        // List of commonly supported content types
        let supported_types = [
            "text/plain",
            "text/html",
            "text/css",
            "text/javascript",
            "application/json",
            "application/xml",
            "application/pdf",
            "application/octet-stream",
            "image/jpeg",
            "image/png",
            "image/gif",
            "image/svg+xml",
            "audio/mpeg",
            "audio/wav",
            "video/mp4",
            "video/mpeg",
        ];
        
        // Check exact match
        if supported_types.contains(&content_type) {
            return true;
        }
        
        // Check wildcard patterns
        content_type.starts_with("text/") ||
        content_type.starts_with("image/") ||
        content_type.starts_with("audio/") ||
        content_type.starts_with("video/") ||
        content_type.starts_with("application/")
    }
    
    /// Compress attachment data (simplified)
    pub fn compress_data(data: &[u8], _compression_type: &str) -> CouchResult<Vec<u8>> {
        // In a real implementation, we'd use gzip, deflate, etc.
        // For simplicity, just return the original data
        Ok(data.to_vec())
    }
    
    /// Decompress attachment data (simplified)
    pub fn decompress_data(data: &[u8], _compression_type: &str) -> CouchResult<Vec<u8>> {
        // In a real implementation, we'd decompress based on type
        // For simplicity, just return the original data
        Ok(data.to_vec())
    }
    
    /// Get MIME type from file extension
    pub fn mime_type_from_extension(filename: &str) -> &'static str {
        let extension = filename.split('.').last().unwrap_or("").to_lowercase();
        
        match extension.as_str() {
            "txt" => "text/plain",
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "js" => "text/javascript",
            "json" => "application/json",
            "xml" => "application/xml",
            "pdf" => "application/pdf",
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "mp4" => "video/mp4",
            "mpeg" | "mpg" => "video/mpeg",
            "zip" => "application/zip",
            "tar" => "application/x-tar",
            "gz" => "application/gzip",
            _ => "application/octet-stream",
        }
    }
    
    /// Create inline attachment (with base64 data)
    pub fn create_inline_attachment(data: &[u8], content_type: &str) -> CouchResult<AttachmentInfo> {
        let digest = Self::calculate_digest(data);
        let encoded_data = Self::encode_base64(data);
        
        Ok(AttachmentInfo {
            content_type: content_type.to_string(),
            length: data.len() as u64,
            digest,
            stub: Some(false),
            revpos: Some(1),
            data: Some(encoded_data),
        })
    }
    
    /// Create stub attachment (reference only)
    pub fn create_stub_attachment(length: u64, content_type: &str, digest: &str, revpos: u32) -> AttachmentInfo {
        AttachmentInfo {
            content_type: content_type.to_string(),
            length,
            digest: digest.to_string(),
            stub: Some(true),
            revpos: Some(revpos),
            data: None,
        }
    }
    
    /// Merge attachment collections
    pub fn merge_attachments(
        existing: &Option<HashMap<String, AttachmentInfo>>,
        new: &Option<HashMap<String, AttachmentInfo>>,
    ) -> Option<HashMap<String, AttachmentInfo>> {
        match (existing, new) {
            (None, None) => None,
            (Some(existing), None) => Some(existing.clone()),
            (None, Some(new)) => Some(new.clone()),
            (Some(existing), Some(new)) => {
                let mut merged = existing.clone();
                for (name, attachment) in new {
                    merged.insert(name.clone(), attachment.clone());
                }
                Some(merged)
            }
        }
    }
    
    /// Get attachment statistics
    pub fn get_attachment_stats(attachments: &Option<HashMap<String, AttachmentInfo>>) -> AttachmentStats {
        match attachments {
            None => AttachmentStats::default(),
            Some(attachments) => {
                let count = attachments.len();
                let total_size: u64 = attachments.values().map(|a| a.length).sum();
                let content_types: Vec<String> = attachments
                    .values()
                    .map(|a| a.content_type.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                
                AttachmentStats {
                    count,
                    total_size,
                    content_types,
                }
            }
        }
    }
    
    /// Validate attachment integrity
    pub fn validate_integrity(attachment: &AttachmentInfo, data: &[u8]) -> CouchResult<bool> {
        // Check length
        if attachment.length != data.len() as u64 {
            return Ok(false);
        }
        
        // Check digest
        let calculated_digest = Self::calculate_digest(data);
        Ok(attachment.digest == calculated_digest)
    }
    
    /// Get attachment security info
    pub fn get_security_info(content_type: &str, data: &[u8]) -> AttachmentSecurity {
        let is_executable = Self::is_executable_type(content_type);
        let contains_scripts = Self::contains_scripts(content_type, data);
        let is_safe = !is_executable && !contains_scripts;
        
        AttachmentSecurity {
            is_safe,
            is_executable,
            contains_scripts,
            content_type: content_type.to_string(),
        }
    }
    
    /// Check if content type is executable
    fn is_executable_type(content_type: &str) -> bool {
        matches!(content_type, 
            "application/x-executable" |
            "application/x-msdos-program" |
            "application/x-msdownload" |
            "application/x-sh" |
            "application/x-csh" |
            "application/x-ksh"
        )
    }
    
    /// Check if content contains scripts
    fn contains_scripts(content_type: &str, data: &[u8]) -> bool {
        if content_type == "text/html" {
            let content = String::from_utf8_lossy(data).to_lowercase();
            return content.contains("<script") || content.contains("javascript:");
        }
        
        if content_type == "text/javascript" || content_type == "application/javascript" {
            return true;
        }
        
        false
    }
}

/// Attachment statistics
#[derive(Debug, Clone, Default)]
pub struct AttachmentStats {
    pub count: usize,
    pub total_size: u64,
    pub content_types: Vec<String>,
}

/// Attachment security information
#[derive(Debug, Clone)]
pub struct AttachmentSecurity {
    pub is_safe: bool,
    pub is_executable: bool,
    pub contains_scripts: bool,
    pub content_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_attachment() {
        let data = b"test data";
        let content_type = "text/plain";
        let name = "test.txt";
        
        assert!(AttachmentManager::validate_attachment(name, data, content_type).is_ok());
        
        // Test empty name
        assert!(AttachmentManager::validate_attachment("", data, content_type).is_err());
        
        // Test invalid name
        assert!(AttachmentManager::validate_attachment("test/file.txt", data, content_type).is_err());
        
        // Test empty content type
        assert!(AttachmentManager::validate_attachment(name, data, "").is_err());
    }
    
    #[test]
    fn test_create_attachment_info() {
        let data = b"test data";
        let content_type = "text/plain";
        
        let info = AttachmentManager::create_attachment_info(data, content_type).unwrap();
        
        assert_eq!(info.content_type, content_type);
        assert_eq!(info.length, data.len() as u64);
        assert!(info.digest.starts_with("md5-"));
        assert_eq!(info.stub, Some(false));
    }
    
    #[test]
    fn test_calculate_digest() {
        let data1 = b"test data";
        let data2 = b"test data";
        let data3 = b"different data";
        
        let digest1 = AttachmentManager::calculate_digest(data1);
        let digest2 = AttachmentManager::calculate_digest(data2);
        let digest3 = AttachmentManager::calculate_digest(data3);
        
        assert_eq!(digest1, digest2);
        assert_ne!(digest1, digest3);
        assert!(digest1.starts_with("md5-"));
    }
    
    #[test]
    fn test_mime_type_from_extension() {
        assert_eq!(AttachmentManager::mime_type_from_extension("test.txt"), "text/plain");
        assert_eq!(AttachmentManager::mime_type_from_extension("image.jpg"), "image/jpeg");
        assert_eq!(AttachmentManager::mime_type_from_extension("page.html"), "text/html");
        assert_eq!(AttachmentManager::mime_type_from_extension("data.json"), "application/json");
        assert_eq!(AttachmentManager::mime_type_from_extension("unknown.xyz"), "application/octet-stream");
    }
    
    #[test]
    fn test_is_supported_content_type() {
        assert!(AttachmentManager::is_supported_content_type("text/plain"));
        assert!(AttachmentManager::is_supported_content_type("image/jpeg"));
        assert!(AttachmentManager::is_supported_content_type("application/json"));
        assert!(AttachmentManager::is_supported_content_type("text/custom"));
        assert!(!AttachmentManager::is_supported_content_type(""));
    }
    
    #[test]
    fn test_base64_encoding() {
        let data = b"test data";
        let encoded = AttachmentManager::encode_base64(data);
        let decoded = AttachmentManager::decode_base64(&encoded).unwrap();
        
        assert_eq!(data.to_vec(), decoded);
    }
    
    #[test]
    fn test_create_inline_attachment() {
        let data = b"test data";
        let content_type = "text/plain";
        
        let attachment = AttachmentManager::create_inline_attachment(data, content_type).unwrap();
        
        assert_eq!(attachment.stub, Some(false));
        assert!(attachment.data.is_some());
        assert_eq!(attachment.length, data.len() as u64);
    }
    
    #[test]
    fn test_create_stub_attachment() {
        let attachment = AttachmentManager::create_stub_attachment(
            100, 
            "text/plain", 
            "md5-abc123", 
            1
        );
        
        assert_eq!(attachment.stub, Some(true));
        assert!(attachment.data.is_none());
        assert_eq!(attachment.length, 100);
        assert_eq!(attachment.digest, "md5-abc123");
    }
    
    #[test]
    fn test_merge_attachments() {
        let mut existing = HashMap::new();
        existing.insert("file1.txt".to_string(), AttachmentManager::create_stub_attachment(100, "text/plain", "md5-1", 1));
        
        let mut new = HashMap::new();
        new.insert("file2.txt".to_string(), AttachmentManager::create_stub_attachment(200, "text/plain", "md5-2", 1));
        
        let merged = AttachmentManager::merge_attachments(&Some(existing), &Some(new)).unwrap();
        
        assert_eq!(merged.len(), 2);
        assert!(merged.contains_key("file1.txt"));
        assert!(merged.contains_key("file2.txt"));
    }
    
    #[test]
    fn test_validate_integrity() {
        let data = b"test data";
        let attachment = AttachmentManager::create_attachment_info(data, "text/plain").unwrap();
        
        assert!(AttachmentManager::validate_integrity(&attachment, data).unwrap());
        
        let wrong_data = b"wrong data";
        assert!(!AttachmentManager::validate_integrity(&attachment, wrong_data).unwrap());
    }
    
    #[test]
    fn test_security_info() {
        let safe_content = b"<html><body>Safe content</body></html>";
        let security = AttachmentManager::get_security_info("text/html", safe_content);
        assert!(security.is_safe);
        assert!(!security.contains_scripts);
        
        let script_content = b"<html><script>alert('test')</script></html>";
        let security = AttachmentManager::get_security_info("text/html", script_content);
        assert!(!security.is_safe);
        assert!(security.contains_scripts);
    }
}