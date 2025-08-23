use crate::couchdb::{
    types::{Document, AttachmentInfo},
    error::{CouchError, CouchResult},
};
use std::collections::HashMap;
use serde_json::Value;
use log::{info, warn, error, debug};

/// Document operations and utilities
pub struct DocumentManager;

impl DocumentManager {
    /// Validate document structure
    pub fn validate_document(doc: &Document) -> CouchResult<()> {
        // Check document ID
        if doc.id.is_empty() {
            return Err(CouchError::bad_request("Document ID cannot be empty"));
        }
        
        // Check for invalid characters in ID
        if doc.id.contains('\0') || doc.id.starts_with('_') && !doc.id.starts_with("_design/") {
            return Err(CouchError::bad_request("Invalid document ID"));
        }
        
        // Check revision format
        if !doc.rev.is_empty() && !Self::is_valid_revision(&doc.rev) {
            return Err(CouchError::bad_request("Invalid revision format"));
        }
        
        // Validate document size (CouchDB limit is typically 4MB)
        let doc_size = serde_json::to_vec(doc)
            .map_err(|e| CouchError::bad_request(&format!("Document serialization error: {}", e)))?
            .len();
        
        if doc_size > 4 * 1024 * 1024 {
            return Err(CouchError::request_entity_too_large("Document exceeds size limit"));
        }
        
        Ok(())
    }
    
    /// Validate revision format (N-hash)
    fn is_valid_revision(rev: &str) -> bool {
        let parts: Vec<&str> = rev.split('-').collect();
        if parts.len() != 2 {
            return false;
        }
        
        // Check if first part is a number
        if parts[0].parse::<u32>().is_err() {
            return false;
        }
        
        // Check if second part is a valid hash (simplified check)
        parts[1].len() >= 16 && parts[1].chars().all(|c| c.is_ascii_hexdigit())
    }
    
    /// Merge document with existing data (for updates)
    pub fn merge_document(existing: &Document, new_doc: &Document) -> CouchResult<Document> {
        let mut merged = new_doc.clone();
        
        // Preserve system fields from existing document if not provided
        if merged.rev.is_empty() {
            merged.rev = existing.rev.clone();
        }
        
        // Merge attachments
        if let Some(ref existing_attachments) = existing.attachments {
            if merged.attachments.is_none() {
                merged.attachments = Some(HashMap::new());
            }
            
            let merged_attachments = merged.attachments.as_mut().unwrap();
            for (name, attachment) in existing_attachments {
                merged_attachments.entry(name.clone()).or_insert_with(|| attachment.clone());
            }
        }
        
        Ok(merged)
    }
    
    /// Extract attachments from document data
    pub fn extract_inline_attachments(doc: &mut Document) -> CouchResult<HashMap<String, Vec<u8>>> {
        let mut extracted = HashMap::new();
        
        if let Some(ref mut attachments) = doc.attachments {
            for (name, attachment) in attachments.iter_mut() {
                if let Some(ref data) = attachment.data {
                    // Decode base64 data
                    let decoded = base64::decode(data)
                        .map_err(|e| CouchError::bad_request(&format!("Invalid base64 data: {}", e)))?;
                    
                    // Update attachment info
                    attachment.length = decoded.len() as u64;
                    attachment.stub = Some(true);
                    attachment.data = None; // Remove inline data
                    
                    extracted.insert(name.clone(), decoded);
                }
            }
        }
        
        Ok(extracted)
    }
    
    /// Filter document for output (remove internal fields)
    pub fn filter_for_output(doc: &Document, include_attachments: bool) -> Document {
        let mut filtered = doc.clone();
        
        // Remove attachment data if not requested
        if !include_attachments {
            if let Some(ref mut attachments) = filtered.attachments {
                for attachment in attachments.values_mut() {
                    attachment.data = None;
                }
            }
        }
        
        filtered
    }
    
    /// Check if document is a design document
    pub fn is_design_document(doc: &Document) -> bool {
        doc.id.starts_with("_design/")
    }
    
    /// Check if document is a local document
    pub fn is_local_document(doc: &Document) -> bool {
        doc.id.starts_with("_local/")
    }
    
    /// Get document conflicts (simplified implementation)
    pub fn get_conflicts(doc: &Document) -> Vec<String> {
        // In a real implementation, this would check for conflicting revisions
        // For now, return empty vector
        vec![]
    }
    
    /// Generate document revision
    pub fn generate_revision(current_rev: &str, doc_data: &Value) -> String {
        let rev_num = if current_rev.is_empty() {
            1
        } else {
            let parts: Vec<&str> = current_rev.split('-').collect();
            if parts.len() >= 2 {
                parts[0].parse::<u32>().unwrap_or(0) + 1
            } else {
                1
            }
        };
        
        // Generate hash from document data
        let doc_string = serde_json::to_string(doc_data).unwrap_or_default();
        let hash = Self::simple_hash(&doc_string);
        
        format!("{}-{:x}", rev_num, hash)
    }
    
    /// Simple hash function for revision generation
    fn simple_hash(input: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        hasher.finish()
    }
    
    /// Check if document has conflicts
    pub fn has_conflicts(doc: &Document) -> bool {
        !Self::get_conflicts(doc).is_empty()
    }
    
    /// Get document size in bytes
    pub fn get_document_size(doc: &Document) -> usize {
        serde_json::to_vec(doc).map(|v| v.len()).unwrap_or(0)
    }
    
    /// Compare documents for equality (ignoring revision)
    pub fn documents_equal(doc1: &Document, doc2: &Document) -> bool {
        doc1.id == doc2.id && doc1.data == doc2.data && doc1.attachments == doc2.attachments
    }
    
    /// Create a tombstone document for deletion
    pub fn create_tombstone(doc: &Document) -> Document {
        Document {
            id: doc.id.clone(),
            rev: doc.rev.clone(),
            deleted: Some(true),
            attachments: None,
            data: Value::Object(serde_json::Map::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    fn create_test_document() -> Document {
        Document {
            id: "test_doc".to_string(),
            rev: "1-abc123".to_string(),
            deleted: None,
            attachments: None,
            data: json!({"name": "test", "value": 42}),
        }
    }
    
    #[test]
    fn test_validate_document() {
        let doc = create_test_document();
        assert!(DocumentManager::validate_document(&doc).is_ok());
        
        // Test empty ID
        let mut invalid_doc = doc.clone();
        invalid_doc.id = "".to_string();
        assert!(DocumentManager::validate_document(&invalid_doc).is_err());
        
        // Test invalid revision
        let mut invalid_doc = doc.clone();
        invalid_doc.rev = "invalid".to_string();
        assert!(DocumentManager::validate_document(&invalid_doc).is_err());
    }
    
    #[test]
    fn test_is_valid_revision() {
        assert!(DocumentManager::is_valid_revision("1-abc123def456"));
        assert!(DocumentManager::is_valid_revision("42-1234567890abcdef"));
        assert!(!DocumentManager::is_valid_revision("invalid"));
        assert!(!DocumentManager::is_valid_revision("1"));
        assert!(!DocumentManager::is_valid_revision("1-"));
        assert!(!DocumentManager::is_valid_revision("abc-123"));
    }
    
    #[test]
    fn test_generate_revision() {
        let rev1 = DocumentManager::generate_revision("", &json!({"test": "data"}));
        assert!(rev1.starts_with("1-"));
        
        let rev2 = DocumentManager::generate_revision("1-abc123", &json!({"test": "data"}));
        assert!(rev2.starts_with("2-"));
    }
    
    #[test]
    fn test_is_design_document() {
        let design_doc = Document {
            id: "_design/test".to_string(),
            rev: "1-abc".to_string(),
            deleted: None,
            attachments: None,
            data: json!({}),
        };
        
        let regular_doc = create_test_document();
        
        assert!(DocumentManager::is_design_document(&design_doc));
        assert!(!DocumentManager::is_design_document(&regular_doc));
    }
    
    #[test]
    fn test_merge_document() {
        let existing = create_test_document();
        let mut new_doc = Document {
            id: "test_doc".to_string(),
            rev: "".to_string(),
            deleted: None,
            attachments: None,
            data: json!({"name": "updated", "new_field": "value"}),
        };
        
        let merged = DocumentManager::merge_document(&existing, &new_doc).unwrap();
        assert_eq!(merged.rev, "1-abc123");
        assert_eq!(merged.data.get("new_field").unwrap(), "value");
    }
    
    #[test]
    fn test_create_tombstone() {
        let doc = create_test_document();
        let tombstone = DocumentManager::create_tombstone(&doc);
        
        assert_eq!(tombstone.id, doc.id);
        assert_eq!(tombstone.rev, doc.rev);
        assert_eq!(tombstone.deleted, Some(true));
        assert!(tombstone.attachments.is_none());
    }
}