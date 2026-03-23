use crate::couchdb::{
    types::Cursor,
    error::{CouchError, CouchResult},
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};
use log::{debug, warn};

/// Manages cursor-based pagination for efficient data retrieval
pub struct CursorManager {
    cursors: Arc<RwLock<HashMap<String, Cursor>>>,
    cursor_ttl: Duration,
}

impl CursorManager {
    pub fn new() -> Self {
        Self {
            cursors: Arc::new(RwLock::new(HashMap::new())),
            cursor_ttl: Duration::minutes(30), // 30 minute TTL
        }
    }
    
    pub fn new_with_ttl(ttl_minutes: i64) -> Self {
        Self {
            cursors: Arc::new(RwLock::new(HashMap::new())),
            cursor_ttl: Duration::minutes(ttl_minutes),
        }
    }
    
    /// Create a new cursor
    pub fn create_cursor(&self, key: serde_json::Value, doc_id: Option<String>, skip: u32) -> String {
        let cursor_id = Uuid::new_v4().to_string();
        let cursor = Cursor::new(key, doc_id, skip);
        
        let mut cursors = self.cursors.write().unwrap();
        cursors.insert(cursor_id.clone(), cursor);
        
        debug!("Created cursor: {}", cursor_id);
        cursor_id
    }
    
    /// Get cursor by ID
    pub fn get_cursor(&self, cursor_id: &str) -> CouchResult<Cursor> {
        let cursors = self.cursors.read().unwrap();
        
        match cursors.get(cursor_id) {
            Some(cursor) => {
                // Check if cursor has expired
                if Utc::now() - cursor.timestamp > self.cursor_ttl {
                    return Err(CouchError::not_found("Cursor has expired"));
                }
                Ok(cursor.clone())
            }
            None => Err(CouchError::not_found("Cursor not found")),
        }
    }
    
    /// Encode cursor to base64 string for client
    pub fn encode_cursor(&self, cursor_id: &str) -> CouchResult<String> {
        let cursor = self.get_cursor(cursor_id)?;
        cursor.encode().map_err(|e| {
            CouchError::internal_server_error(&format!("Failed to encode cursor: {}", e))
        })
    }
    
    /// Decode cursor from base64 string
    pub fn decode_cursor(&self, encoded: &str) -> CouchResult<Cursor> {
        Cursor::decode(encoded).map_err(|e| {
            CouchError::bad_request(&format!("Invalid cursor format: {}", e))
        })
    }
    
    /// Store decoded cursor and return ID
    pub fn store_decoded_cursor(&self, encoded: &str) -> CouchResult<String> {
        let cursor = self.decode_cursor(encoded)?;
        let cursor_id = Uuid::new_v4().to_string();
        
        let mut cursors = self.cursors.write().unwrap();
        cursors.insert(cursor_id.clone(), cursor);
        
        Ok(cursor_id)
    }
    
    /// Update cursor position
    pub fn update_cursor(&self, cursor_id: &str, new_key: serde_json::Value, new_doc_id: Option<String>, new_skip: u32) -> CouchResult<()> {
        let mut cursors = self.cursors.write().unwrap();
        
        match cursors.get_mut(cursor_id) {
            Some(cursor) => {
                cursor.key = new_key;
                cursor.doc_id = new_doc_id;
                cursor.skip = new_skip;
                cursor.timestamp = Utc::now();
                Ok(())
            }
            None => Err(CouchError::not_found("Cursor not found")),
        }
    }
    
    /// Delete cursor
    pub fn delete_cursor(&self, cursor_id: &str) -> bool {
        let mut cursors = self.cursors.write().unwrap();
        cursors.remove(cursor_id).is_some()
    }
    
    /// Clean up expired cursors
    pub fn cleanup_expired(&self) {
        let now = Utc::now();
        let mut cursors = self.cursors.write().unwrap();
        
        let expired_keys: Vec<String> = cursors
            .iter()
            .filter(|(_, cursor)| now - cursor.timestamp > self.cursor_ttl)
            .map(|(key, _)| key.clone())
            .collect();
        
        for key in expired_keys {
            cursors.remove(&key);
        }
        
        if !cursors.is_empty() {
            debug!("Cleaned up {} expired cursors", cursors.len());
        }
    }
    
    /// Get cursor statistics
    pub fn get_stats(&self) -> CursorStats {
        let cursors = self.cursors.read().unwrap();
        let now = Utc::now();
        
        let mut expired_count = 0;
        let mut active_count = 0;
        
        for cursor in cursors.values() {
            if now - cursor.timestamp > self.cursor_ttl {
                expired_count += 1;
            } else {
                active_count += 1;
            }
        }
        
        CursorStats {
            total_cursors: cursors.len(),
            active_cursors: active_count,
            expired_cursors: expired_count,
            ttl_minutes: self.cursor_ttl.num_minutes(),
        }
    }

}

/// Cursor statistics
#[derive(Debug, Clone)]
pub struct CursorStats {
    pub total_cursors: usize,
    pub active_cursors: usize,
    pub expired_cursors: usize,
    pub ttl_minutes: i64,
}

/// Cursor-based pagination helper
pub struct PaginationHelper;

impl PaginationHelper {
    /// Apply cursor-based pagination to view results
    pub fn apply_cursor_pagination(
        results: &mut Vec<serde_json::Value>,
        cursor: Option<&Cursor>,
        limit: usize,
        descending: bool,
    ) -> Option<serde_json::Value> {
        if let Some(cursor) = cursor {
            // Find the starting position based on cursor
            let start_pos = Self::find_cursor_position(results, cursor, descending);
            
            // Apply skip from cursor
            let skip_pos = start_pos + cursor.skip as usize;
            
            if skip_pos < results.len() {
                results.drain(0..skip_pos);
            } else {
                results.clear();
                return None;
            }
        }
        
        // Apply limit
        if results.len() > limit {
            results.truncate(limit);
            
            // Create next cursor if there are more results
            if let Some(last_result) = results.last() {
                return Some(last_result.clone());
            }
        }
        
        None
    }
    
    /// Find position of cursor in results
    fn find_cursor_position(
        results: &[serde_json::Value],
        cursor: &Cursor,
        descending: bool,
    ) -> usize {
        for (i, result) in results.iter().enumerate() {
            if let Some(key) = result.get("key") {
                let comparison = Self::compare_keys(&cursor.key, key);
                
                let found = if descending {
                    comparison >= std::cmp::Ordering::Equal
                } else {
                    comparison <= std::cmp::Ordering::Equal
                };
                
                if found {
                    // If we have a doc_id, check for exact match
                    if let Some(ref cursor_doc_id) = cursor.doc_id {
                        if let Some(result_id) = result.get("id") {
                            if result_id.as_str() == Some(cursor_doc_id) {
                                return i;
                            }
                        }
                    } else {
                        return i;
                    }
                }
            }
        }
        
        0
    }
    
    /// Compare two JSON values for ordering
    fn compare_keys(a: &serde_json::Value, b: &serde_json::Value) -> std::cmp::Ordering {
        use serde_json::Value;
        
        match (a, b) {
            (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
            (Value::Null, _) => std::cmp::Ordering::Less,
            (_, Value::Null) => std::cmp::Ordering::Greater,
            
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            (Value::Bool(_), _) => std::cmp::Ordering::Less,
            (_, Value::Bool(_)) => std::cmp::Ordering::Greater,
            
            (Value::Number(a), Value::Number(b)) => {
                a.as_f64().partial_cmp(&b.as_f64()).unwrap_or(std::cmp::Ordering::Equal)
            }
            (Value::Number(_), _) => std::cmp::Ordering::Less,
            (_, Value::Number(_)) => std::cmp::Ordering::Greater,
            
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::String(_), _) => std::cmp::Ordering::Less,
            (_, Value::String(_)) => std::cmp::Ordering::Greater,
            
            (Value::Array(a), Value::Array(b)) => {
                for (a_item, b_item) in a.iter().zip(b.iter()) {
                    match Self::compare_keys(a_item, b_item) {
                        std::cmp::Ordering::Equal => continue,
                        other => return other,
                    }
                }
                a.len().cmp(&b.len())
            }
            (Value::Array(_), _) => std::cmp::Ordering::Less,
            (_, Value::Array(_)) => std::cmp::Ordering::Greater,
            
            (Value::Object(_), Value::Object(_)) => {
                // For objects, compare as strings (simplified)
                a.to_string().cmp(&b.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_cursor_creation() {
        let manager = CursorManager::new();
        let cursor_id = manager.create_cursor(json!("test_key"), Some("doc1".to_string()), 0);
        
        let cursor = manager.get_cursor(&cursor_id).unwrap();
        assert_eq!(cursor.key, json!("test_key"));
        assert_eq!(cursor.doc_id, Some("doc1".to_string()));
        assert_eq!(cursor.skip, 0);
    }
    
    #[test]
    fn test_cursor_encoding() {
        let cursor = Cursor::new(json!("test"), Some("doc1".to_string()), 5);
        let encoded = cursor.encode().unwrap();
        let decoded = Cursor::decode(&encoded).unwrap();
        
        assert_eq!(cursor.key, decoded.key);
        assert_eq!(cursor.doc_id, decoded.doc_id);
        assert_eq!(cursor.skip, decoded.skip);
    }
    
    #[test]
    fn test_key_comparison() {
        use PaginationHelper as PH;
        
        assert_eq!(PH::compare_keys(&json!(null), &json!(null)), std::cmp::Ordering::Equal);
        assert_eq!(PH::compare_keys(&json!(1), &json!(2)), std::cmp::Ordering::Less);
        assert_eq!(PH::compare_keys(&json!("a"), &json!("b")), std::cmp::Ordering::Less);
        assert_eq!(PH::compare_keys(&json!([1, 2]), &json!([1, 3])), std::cmp::Ordering::Less);
    }
}
