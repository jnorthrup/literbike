use literbike::couchdb::{
    types::*,
    error::CouchError,
    documents::DocumentManager,
    attachments::AttachmentManager,
    cursor::{CursorManager, PaginationHelper},
};
use serde_json::json;
use std::collections::HashMap;

#[test]
fn test_document_validation() {
    let doc = Document {
        id: "valid_doc".to_string(),
        rev: "1-abc123def456".to_string(),
        deleted: None,
        attachments: None,
        data: json!({"name": "test"}),
    };
    
    assert!(DocumentManager::validate_document(&doc).is_ok());
    
    // Test empty ID
    let mut invalid_doc = doc.clone();
    invalid_doc.id = "".to_string();
    assert!(DocumentManager::validate_document(&invalid_doc).is_err());
    
    // Test invalid revision format
    let mut invalid_doc = doc.clone();
    invalid_doc.rev = "invalid-revision".to_string();
    assert!(DocumentManager::validate_document(&invalid_doc).is_err());
}

#[test]
fn test_revision_generation() {
    let data = json!({"test": "data"});
    
    let rev1 = DocumentManager::generate_revision("", &data);
    assert!(rev1.starts_with("1-"));
    
    let rev2 = DocumentManager::generate_revision("1-abc123", &data);
    assert!(rev2.starts_with("2-"));
    
    let rev3 = DocumentManager::generate_revision("5-def456", &data);
    assert!(rev3.starts_with("6-"));
}

#[test]
fn test_document_type_detection() {
    let design_doc = Document {
        id: "_design/test".to_string(),
        rev: "1-abc".to_string(),
        deleted: None,
        attachments: None,
        data: json!({}),
    };
    
    let local_doc = Document {
        id: "_local/test".to_string(),
        rev: "1-abc".to_string(),
        deleted: None,
        attachments: None,
        data: json!({}),
    };
    
    let regular_doc = Document {
        id: "regular_doc".to_string(),
        rev: "1-abc".to_string(),
        deleted: None,
        attachments: None,
        data: json!({}),
    };
    
    assert!(DocumentManager::is_design_document(&design_doc));
    assert!(!DocumentManager::is_design_document(&local_doc));
    assert!(!DocumentManager::is_design_document(&regular_doc));
    
    assert!(DocumentManager::is_local_document(&local_doc));
    assert!(!DocumentManager::is_local_document(&design_doc));
    assert!(!DocumentManager::is_local_document(&regular_doc));
}

#[test]
fn test_document_merging() {
    let existing = Document {
        id: "test_doc".to_string(),
        rev: "1-abc123".to_string(),
        deleted: None,
        attachments: Some({
            let mut attachments = HashMap::new();
            attachments.insert("file1.txt".to_string(), AttachmentInfo {
                content_type: "text/plain".to_string(),
                length: 100,
                digest: "md5-abc".to_string(),
                stub: Some(true),
                revpos: Some(1),
                data: None,
            });
            attachments
        }),
        data: json!({"name": "original", "type": "test"}),
    };
    
    let new_doc = Document {
        id: "test_doc".to_string(),
        rev: "".to_string(),
        deleted: None,
        attachments: None,
        data: json!({"name": "updated", "value": 42}),
    };
    
    let merged = DocumentManager::merge_document(&existing, &new_doc).unwrap();
    
    assert_eq!(merged.rev, "1-abc123"); // Should preserve existing rev
    assert_eq!(merged.data["name"], json!("updated"));
    assert_eq!(merged.data["value"], json!(42));
    assert!(merged.attachments.is_some()); // Should preserve attachments
    assert!(merged.attachments.unwrap().contains_key("file1.txt"));
}

#[test]
fn test_tombstone_creation() {
    let doc = Document {
        id: "test_doc".to_string(),
        rev: "2-def456".to_string(),
        deleted: None,
        attachments: Some(HashMap::new()),
        data: json!({"name": "test", "value": 123}),
    };
    
    let tombstone = DocumentManager::create_tombstone(&doc);
    
    assert_eq!(tombstone.id, doc.id);
    assert_eq!(tombstone.rev, doc.rev);
    assert_eq!(tombstone.deleted, Some(true));
    assert!(tombstone.attachments.is_none());
    assert!(tombstone.data.as_object().unwrap().is_empty());
}

#[test]
fn test_attachment_validation() {
    let data = b"test attachment data";
    let content_type = "text/plain";
    let name = "test.txt";
    
    assert!(AttachmentManager::validate_attachment(name, data, content_type).is_ok());
    
    // Test empty name
    assert!(AttachmentManager::validate_attachment("", data, content_type).is_err());
    
    // Test invalid characters in name
    assert!(AttachmentManager::validate_attachment("test/file.txt", data, content_type).is_err());
    assert!(AttachmentManager::validate_attachment("test\\file.txt", data, content_type).is_err());
    
    // Test empty content type
    assert!(AttachmentManager::validate_attachment(name, data, "").is_err());
}

#[test]
fn test_attachment_digest_calculation() {
    let data1 = b"identical data";
    let data2 = b"identical data";
    let data3 = b"different data";
    
    let digest1 = AttachmentManager::calculate_digest(data1);
    let digest2 = AttachmentManager::calculate_digest(data2);
    let digest3 = AttachmentManager::calculate_digest(data3);
    
    assert_eq!(digest1, digest2);
    assert_ne!(digest1, digest3);
    assert!(digest1.starts_with("md5-"));
}

#[test]
fn test_attachment_info_creation() {
    let data = b"test data for attachment";
    let content_type = "text/plain";
    
    let info = AttachmentManager::create_attachment_info(data, content_type).unwrap();
    
    assert_eq!(info.content_type, content_type);
    assert_eq!(info.length, data.len() as u64);
    assert!(info.digest.starts_with("md5-"));
    assert_eq!(info.stub, Some(false));
    assert_eq!(info.revpos, Some(1));
    assert!(info.data.is_none());
}

#[test]
fn test_mime_type_detection() {
    assert_eq!(AttachmentManager::mime_type_from_extension("document.txt"), "text/plain");
    assert_eq!(AttachmentManager::mime_type_from_extension("webpage.html"), "text/html");
    assert_eq!(AttachmentManager::mime_type_from_extension("style.css"), "text/css");
    assert_eq!(AttachmentManager::mime_type_from_extension("script.js"), "text/javascript");
    assert_eq!(AttachmentManager::mime_type_from_extension("data.json"), "application/json");
    assert_eq!(AttachmentManager::mime_type_from_extension("image.jpg"), "image/jpeg");
    assert_eq!(AttachmentManager::mime_type_from_extension("image.png"), "image/png");
    assert_eq!(AttachmentManager::mime_type_from_extension("video.mp4"), "video/mp4");
    assert_eq!(AttachmentManager::mime_type_from_extension("unknown.xyz"), "application/octet-stream");
}

#[test]
fn test_content_type_support() {
    assert!(AttachmentManager::is_supported_content_type("text/plain"));
    assert!(AttachmentManager::is_supported_content_type("image/jpeg"));
    assert!(AttachmentManager::is_supported_content_type("application/json"));
    assert!(AttachmentManager::is_supported_content_type("text/custom"));
    assert!(AttachmentManager::is_supported_content_type("application/custom"));
    
    // Empty content type should not be supported
    assert!(!AttachmentManager::is_supported_content_type(""));
}

#[test]
fn test_base64_encoding_decoding() {
    let original_data = b"Hello, World! This is test data with special chars: !@#$%^&*()";
    
    let encoded = AttachmentManager::encode_base64(original_data);
    assert!(!encoded.is_empty());
    
    let decoded = AttachmentManager::decode_base64(&encoded).unwrap();
    assert_eq!(original_data.to_vec(), decoded);
    
    // Test invalid base64
    assert!(AttachmentManager::decode_base64("invalid base64!!!").is_err());
}

#[test]
fn test_inline_attachment_creation() {
    let data = b"inline attachment data";
    let content_type = "text/plain";
    
    let attachment = AttachmentManager::create_inline_attachment(data, content_type).unwrap();
    
    assert_eq!(attachment.content_type, content_type);
    assert_eq!(attachment.length, data.len() as u64);
    assert_eq!(attachment.stub, Some(false));
    assert!(attachment.data.is_some());
    
    // Verify the data can be decoded back
    let encoded_data = attachment.data.unwrap();
    let decoded = AttachmentManager::decode_base64(&encoded_data).unwrap();
    assert_eq!(data.to_vec(), decoded);
}

#[test]
fn test_stub_attachment_creation() {
    let stub = AttachmentManager::create_stub_attachment(
        1024,
        "application/pdf",
        "md5-abcdef123456",
        3
    );
    
    assert_eq!(stub.length, 1024);
    assert_eq!(stub.content_type, "application/pdf");
    assert_eq!(stub.digest, "md5-abcdef123456");
    assert_eq!(stub.revpos, Some(3));
    assert_eq!(stub.stub, Some(true));
    assert!(stub.data.is_none());
}

#[test]
fn test_attachment_merging() {
    let mut existing = HashMap::new();
    existing.insert("file1.txt".to_string(), AttachmentManager::create_stub_attachment(
        100, "text/plain", "md5-1", 1
    ));
    existing.insert("file2.txt".to_string(), AttachmentManager::create_stub_attachment(
        200, "text/plain", "md5-2", 1
    ));
    
    let mut new_attachments = HashMap::new();
    new_attachments.insert("file3.txt".to_string(), AttachmentManager::create_stub_attachment(
        300, "text/plain", "md5-3", 2
    ));
    new_attachments.insert("file1.txt".to_string(), AttachmentManager::create_stub_attachment(
        150, "text/plain", "md5-1-updated", 2
    ));
    
    let merged = AttachmentManager::merge_attachments(&Some(existing), &Some(new_attachments)).unwrap();
    
    assert_eq!(merged.len(), 3);
    assert!(merged.contains_key("file1.txt"));
    assert!(merged.contains_key("file2.txt"));
    assert!(merged.contains_key("file3.txt"));
    
    // file1.txt should be updated with new version
    assert_eq!(merged["file1.txt"].length, 150);
    assert_eq!(merged["file1.txt"].digest, "md5-1-updated");
}

#[test]
fn test_attachment_integrity_validation() {
    let data = b"test attachment content";
    let attachment = AttachmentManager::create_attachment_info(data, "text/plain").unwrap();
    
    // Valid data should pass
    assert!(AttachmentManager::validate_integrity(&attachment, data).unwrap());
    
    // Wrong data should fail
    let wrong_data = b"different content";
    assert!(!AttachmentManager::validate_integrity(&attachment, wrong_data).unwrap());
    
    // Wrong length should fail
    let mut wrong_length_attachment = attachment.clone();
    wrong_length_attachment.length = 999;
    assert!(!AttachmentManager::validate_integrity(&wrong_length_attachment, data).unwrap());
}

#[test]
fn test_attachment_security() {
    // Safe content
    let safe_html = b"<html><body><h1>Safe Content</h1></body></html>";
    let security = AttachmentManager::get_security_info("text/html", safe_html);
    assert!(security.is_safe);
    assert!(!security.is_executable);
    assert!(!security.contains_scripts);
    
    // HTML with scripts
    let script_html = b"<html><body><script>alert('xss')</script></body></html>";
    let security = AttachmentManager::get_security_info("text/html", script_html);
    assert!(!security.is_safe);
    assert!(!security.is_executable);
    assert!(security.contains_scripts);
    
    // JavaScript file
    let js_content = b"function test() { console.log('test'); }";
    let security = AttachmentManager::get_security_info("text/javascript", js_content);
    assert!(!security.is_safe);
    assert!(!security.is_executable);
    assert!(security.contains_scripts);
    
    // Plain text (safe)
    let text_content = b"This is just plain text content.";
    let security = AttachmentManager::get_security_info("text/plain", text_content);
    assert!(security.is_safe);
    assert!(!security.is_executable);
    assert!(!security.contains_scripts);
}

#[test]
fn test_cursor_manager() {
    let manager = CursorManager::new();
    
    // Test cursor creation
    let cursor_id = manager.create_cursor(json!("test_key"), Some("doc1".to_string()), 0);
    assert!(!cursor_id.is_empty());
    
    // Test cursor retrieval
    let cursor = manager.get_cursor(&cursor_id).unwrap();
    assert_eq!(cursor.key, json!("test_key"));
    assert_eq!(cursor.doc_id, Some("doc1".to_string()));
    assert_eq!(cursor.skip, 0);
    
    // Test cursor update
    manager.update_cursor(&cursor_id, json!("new_key"), Some("doc2".to_string()), 5).unwrap();
    let updated_cursor = manager.get_cursor(&cursor_id).unwrap();
    assert_eq!(updated_cursor.key, json!("new_key"));
    assert_eq!(updated_cursor.doc_id, Some("doc2".to_string()));
    assert_eq!(updated_cursor.skip, 5);
    
    // Test cursor deletion
    assert!(manager.delete_cursor(&cursor_id));
    assert!(manager.get_cursor(&cursor_id).is_err());
    
    // Test non-existent cursor
    assert!(!manager.delete_cursor("nonexistent"));
}

#[test]
fn test_cursor_encoding_decoding() {
    let cursor = Cursor::new(json!("test_key"), Some("doc123".to_string()), 10);
    
    let encoded = cursor.encode().unwrap();
    assert!(!encoded.is_empty());
    
    let decoded = Cursor::decode(&encoded).unwrap();
    assert_eq!(cursor.key, decoded.key);
    assert_eq!(cursor.doc_id, decoded.doc_id);
    assert_eq!(cursor.skip, decoded.skip);
    
    // Test invalid encoded cursor
    assert!(Cursor::decode("invalid_base64!!!").is_err());
}

#[test]
fn test_pagination_helper_key_comparison() {
    // Test null comparison
    assert_eq!(PaginationHelper::compare_keys(&json!(null), &json!(null)), std::cmp::Ordering::Equal);
    assert_eq!(PaginationHelper::compare_keys(&json!(null), &json!(1)), std::cmp::Ordering::Less);
    assert_eq!(PaginationHelper::compare_keys(&json!(1), &json!(null)), std::cmp::Ordering::Greater);
    
    // Test boolean comparison
    assert_eq!(PaginationHelper::compare_keys(&json!(false), &json!(true)), std::cmp::Ordering::Less);
    assert_eq!(PaginationHelper::compare_keys(&json!(true), &json!(false)), std::cmp::Ordering::Greater);
    assert_eq!(PaginationHelper::compare_keys(&json!(true), &json!(true)), std::cmp::Ordering::Equal);
    
    // Test number comparison
    assert_eq!(PaginationHelper::compare_keys(&json!(1), &json!(2)), std::cmp::Ordering::Less);
    assert_eq!(PaginationHelper::compare_keys(&json!(2.5), &json!(2.5)), std::cmp::Ordering::Equal);
    assert_eq!(PaginationHelper::compare_keys(&json!(10), &json!(5)), std::cmp::Ordering::Greater);
    
    // Test string comparison
    assert_eq!(PaginationHelper::compare_keys(&json!("apple"), &json!("banana")), std::cmp::Ordering::Less);
    assert_eq!(PaginationHelper::compare_keys(&json!("zebra"), &json!("apple")), std::cmp::Ordering::Greater);
    assert_eq!(PaginationHelper::compare_keys(&json!("test"), &json!("test")), std::cmp::Ordering::Equal);
    
    // Test array comparison
    assert_eq!(PaginationHelper::compare_keys(&json!([1, 2]), &json!([1, 3])), std::cmp::Ordering::Less);
    assert_eq!(PaginationHelper::compare_keys(&json!([1, 2, 3]), &json!([1, 2])), std::cmp::Ordering::Greater);
    assert_eq!(PaginationHelper::compare_keys(&json!([1, 2]), &json!([1, 2])), std::cmp::Ordering::Equal);
    
    // Test type precedence (null < bool < number < string < array < object)
    assert_eq!(PaginationHelper::compare_keys(&json!(null), &json!(false)), std::cmp::Ordering::Less);
    assert_eq!(PaginationHelper::compare_keys(&json!(false), &json!(1)), std::cmp::Ordering::Less);
    assert_eq!(PaginationHelper::compare_keys(&json!(1), &json!("string")), std::cmp::Ordering::Less);
    assert_eq!(PaginationHelper::compare_keys(&json!("string"), &json!([1, 2])), std::cmp::Ordering::Less);
    assert_eq!(PaginationHelper::compare_keys(&json!([1, 2]), &json!({"key": "value"})), std::cmp::Ordering::Less);
}

#[test]
fn test_couch_error_creation() {
    let error = CouchError::bad_request("Invalid parameter");
    assert_eq!(error.error, "bad_request");
    assert_eq!(error.reason, "Invalid parameter");
    assert_eq!(error.status_code(), 400);
    
    let error = CouchError::not_found("Document not found");
    assert_eq!(error.error, "not_found");
    assert_eq!(error.reason, "Document not found");
    assert_eq!(error.status_code(), 404);
    
    let error = CouchError::conflict("Document conflict");
    assert_eq!(error.error, "conflict");
    assert_eq!(error.reason, "Document conflict");
    assert_eq!(error.status_code(), 409);
    
    let error = CouchError::internal_server_error("Database error");
    assert_eq!(error.error, "internal_server_error");
    assert_eq!(error.reason, "Database error");
    assert_eq!(error.status_code(), 500);
}

#[test]
fn test_couch_error_from_conversions() {
    // Test JSON error conversion
    let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
    let couch_err = CouchError::from(json_err);
    assert_eq!(couch_err.error, "bad_request");
    assert!(couch_err.reason.contains("JSON parsing error"));
    
    // Test UUID error conversion (though UUID errors are rare)
    // We'll test this conceptually
    let uuid_err = uuid::Error::Simple { length: 0 };
    let couch_err = CouchError::from(uuid_err);
    assert_eq!(couch_err.error, "bad_request");
    assert!(couch_err.reason.contains("UUID error"));
}