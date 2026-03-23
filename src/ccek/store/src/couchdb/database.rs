use crate::couchdb::{
    types::*,
    error::{CouchError, CouchResult},
    cursor::CursorManager,
};
use sled::{Db, Tree};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;
use chrono::Utc;
use log::{info, warn, error, debug};

/// Core database manager for CouchDB emulation
pub struct DatabaseManager {
    sled_db: Db,
    databases: Arc<RwLock<HashMap<String, DatabaseInstance>>>,
    cursor_manager: CursorManager,
    server_uuid: String,
}

/// Individual database instance
pub struct DatabaseInstance {
    name: String,
    tree: Tree,
    sequence_counter: Arc<RwLock<u64>>,
    doc_count: Arc<RwLock<u64>>,
    deleted_count: Arc<RwLock<u64>>,
    design_docs: Arc<RwLock<HashMap<String, DesignDocument>>>,
}

impl DatabaseManager {
    pub fn new(data_dir: &str) -> CouchResult<Self> {
        let sled_db = sled::open(data_dir)
            .map_err(|e| CouchError::internal_server_error(&format!("Failed to open database: {}", e)))?;
        
        let server_uuid = Uuid::new_v4().to_string();
        
        Ok(Self {
            sled_db,
            databases: Arc::new(RwLock::new(HashMap::new())),
            cursor_manager: CursorManager::new(),
            server_uuid,
        })
    }
    
    /// Create a new database
    pub fn create_database(&self, name: &str) -> CouchResult<DatabaseInfo> {
        if !is_valid_db_name(name) {
            return Err(CouchError::bad_request("Invalid database name"));
        }
        
        let mut databases = self.databases.write().unwrap();
        
        if databases.contains_key(name) {
            return Err(CouchError::conflict("Database already exists"));
        }
        
        let tree = self.sled_db.open_tree(name)
            .map_err(|e| CouchError::internal_server_error(&format!("Failed to create database tree: {}", e)))?;
        
        let db_instance = DatabaseInstance {
            name: name.to_string(),
            tree,
            sequence_counter: Arc::new(RwLock::new(0)),
            doc_count: Arc::new(RwLock::new(0)),
            deleted_count: Arc::new(RwLock::new(0)),
            design_docs: Arc::new(RwLock::new(HashMap::new())),
        };
        
        databases.insert(name.to_string(), db_instance);
        info!("Created database: {}", name);
        
        self.get_database_info(name)
    }
    
    /// Delete a database
    pub fn delete_database(&self, name: &str) -> CouchResult<serde_json::Value> {
        let mut databases = self.databases.write().unwrap();
        
        if !databases.contains_key(name) {
            return Err(CouchError::not_found("Database does not exist"));
        }
        
        // Remove from sled
        self.sled_db.drop_tree(name)
            .map_err(|e| CouchError::internal_server_error(&format!("Failed to drop database tree: {}", e)))?;
        
        databases.remove(name);
        info!("Deleted database: {}", name);
        
        Ok(serde_json::json!({"ok": true}))
    }
    
    /// Get database information
    pub fn get_database_info(&self, name: &str) -> CouchResult<DatabaseInfo> {
        let databases = self.databases.read().unwrap();

        let db_instance = databases.get(name)
            .ok_or_else(|| CouchError::not_found("Database does not exist"))?;

        let doc_count = *db_instance.doc_count.read().unwrap();
        let deleted_count = *db_instance.deleted_count.read().unwrap();
        let sequence = *db_instance.sequence_counter.read().unwrap();

        // sled does not expose a size_on_disk() API; use data_size (entry count)
        // as a truthful proxy for disk_size to preserve the response shape.
        let data_size = db_instance.tree.len() as u64;

        Ok(DatabaseInfo {
            db_name: name.to_string(),
            doc_count,
            doc_del_count: deleted_count,
            update_seq: sequence,
            purge_seq: 0,
            compact_running: false,
            disk_size: data_size,
            data_size,
            instance_start_time: "1970-01-01T00:00:00.000000Z".to_string(),
            disk_format_version: 8,
            committed_update_seq: sequence,
        })
    }
    
    /// List all databases
    pub fn list_databases(&self) -> CouchResult<AllDbsResponse> {
        let databases = self.databases.read().unwrap();
        let mut db_names: Vec<String> = databases.keys().cloned().collect();
        db_names.sort();
        Ok(AllDbsResponse(db_names))
    }
    
    /// Check if database exists
    pub fn database_exists(&self, name: &str) -> bool {
        let databases = self.databases.read().unwrap();
        databases.contains_key(name)
    }
    
    /// Get database instance
    ///
    /// Returns an error unconditionally: a shared reference into an RwLock
    /// cannot be returned without holding the guard for its lifetime. Use
    /// [`get_database_clone`] instead, which is the preferred safe API.
    pub fn get_database(&self, _name: &str) -> CouchResult<&DatabaseInstance> {
        Err(CouchError::internal_server_error(
            "get_database is not supported; call get_database_clone instead",
        ))
    }
    
    /// Get cloned database instance (safer alternative)
    pub fn get_database_clone(&self, name: &str) -> CouchResult<DatabaseInstance> {
        let databases = self.databases.read().unwrap();
        databases.get(name)
            .ok_or_else(|| CouchError::not_found("Database does not exist"))
            .map(|db| DatabaseInstance {
                name: db.name.clone(),
                tree: db.tree.clone(),
                sequence_counter: Arc::clone(&db.sequence_counter),
                doc_count: Arc::clone(&db.doc_count),
                deleted_count: Arc::clone(&db.deleted_count),
                design_docs: Arc::clone(&db.design_docs),
            })
    }
    
    /// Get server information
    pub fn get_server_info(&self) -> ServerInfo {
        ServerInfo {
            couchdb: "Welcome".to_string(),
            uuid: self.server_uuid.clone(),
            version: "1.7.2".to_string(),
            vendor: ServerVendor {
                name: "Literbike CouchDB Emulator".to_string(),
                version: "0.1.0".to_string(),
            },
            features: vec![
                "attachments".to_string(),
                "httpd".to_string(),
                "ipfs".to_string(),
                "m2m".to_string(),
                "tensor".to_string(),
                "cursor_pagination".to_string(),
            ],
            git_sha: "unknown".to_string(),
        }
    }
    
    /// Initialize with default databases
    pub fn initialize_defaults(&self) -> CouchResult<()> {
        // Create system databases
        let system_dbs = vec!["_users", "_replicator"];
        
        for db_name in system_dbs {
            if !self.database_exists(db_name) {
                self.create_database(db_name)?;
                info!("Created system database: {}", db_name);
            }
        }
        
        Ok(())
    }
    
    /// Compact database (placeholder implementation)
    pub fn compact_database(&self, name: &str) -> CouchResult<serde_json::Value> {
        if !self.database_exists(name) {
            return Err(CouchError::not_found("Database does not exist"));
        }
        
        info!("Compaction triggered for database: {}", name);
        // In a real implementation, this would trigger background compaction
        Ok(serde_json::json!({"ok": true}))
    }
}

impl DatabaseInstance {
    /// Put a document
    pub fn put_document(&self, doc: &Document) -> CouchResult<(String, String)> {
        let doc_key = format!("doc:{}", doc.id);
        let serialized = serde_json::to_vec(doc)?;
        
        // Check if document exists for conflict detection
        if let Ok(existing) = self.tree.get(&doc_key) {
            if let Some(existing_data) = existing {
                let existing_doc: Document = serde_json::from_slice(&existing_data)?;
                if existing_doc.rev != doc.rev {
                    return Err(CouchError::conflict("Document update conflict"));
                }
            }
        }
        
        // Generate new revision
        let new_rev = generate_revision(&doc.rev);
        let mut updated_doc = doc.clone();
        updated_doc.rev = new_rev.clone();
        
        let updated_serialized = serde_json::to_vec(&updated_doc)?;
        self.tree.insert(&doc_key, updated_serialized)?;
        
        // Update counters
        let mut seq_counter = self.sequence_counter.write().unwrap();
        *seq_counter += 1;
        
        if doc.deleted.unwrap_or(false) {
            let mut deleted_count = self.deleted_count.write().unwrap();
            *deleted_count += 1;
        } else {
            let mut doc_count = self.doc_count.write().unwrap();
            *doc_count += 1;
        }
        
        debug!("Stored document: {} with revision: {}", doc.id, new_rev);
        Ok((doc.id.clone(), new_rev))
    }
    
    /// Get a document
    pub fn get_document(&self, id: &str) -> CouchResult<Document> {
        let doc_key = format!("doc:{}", id);
        
        match self.tree.get(&doc_key)? {
            Some(data) => {
                let doc: Document = serde_json::from_slice(&data)?;
                if doc.deleted.unwrap_or(false) {
                    Err(CouchError::not_found("Document is deleted"))
                } else {
                    Ok(doc)
                }
            }
            None => Err(CouchError::not_found("Document not found")),
        }
    }
    
    /// Delete a document
    pub fn delete_document(&self, id: &str, rev: &str) -> CouchResult<(String, String)> {
        let mut doc = self.get_document(id)?;
        
        if doc.rev != rev {
            return Err(CouchError::conflict("Document revision conflict"));
        }
        
        doc.deleted = Some(true);
        self.put_document(&doc)
    }
    
    /// Check if document exists
    pub fn document_exists(&self, id: &str) -> bool {
        let doc_key = format!("doc:{}", id);
        self.tree.contains_key(&doc_key).unwrap_or(false)
    }
    
    /// Get all documents with pagination
    pub fn get_all_documents(&self, query: &ViewQuery) -> CouchResult<ViewResult> {
        let mut results = Vec::new();
        let limit = query.limit.unwrap_or(25) as usize;
        let skip = query.skip.unwrap_or(0) as usize;
        let include_docs = query.include_docs.unwrap_or(false);
        
        let iter = self.tree.scan_prefix("doc:");
        let mut total_count = 0;
        let mut current_skip = 0;
        
        for item in iter {
            let (key, value) = item?;
            total_count += 1;
            
            if current_skip < skip {
                current_skip += 1;
                continue;
            }
            
            if results.len() >= limit {
                break;
            }
            
            let key_str = String::from_utf8_lossy(&key);
            let doc_id = key_str.strip_prefix("doc:").unwrap_or("");
            
            let doc: Document = serde_json::from_slice(&value)?;
            
            // Skip deleted documents unless explicitly requested
            if doc.deleted.unwrap_or(false) && !query.conflicts.unwrap_or(false) {
                continue;
            }
            
            let row = ViewRow {
                id: Some(doc_id.to_string()),
                key: serde_json::Value::String(doc_id.to_string()),
                value: serde_json::json!({"rev": doc.rev}),
                doc: if include_docs { Some(doc) } else { None },
            };
            
            results.push(row);
        }
        
        Ok(ViewResult {
            total_rows: total_count,
            offset: skip as u32,
            rows: results,
            update_seq: Some(*self.sequence_counter.read().unwrap()),
            next_cursor: None, // TODO: Implement cursor logic
        })
    }
    
    /// Store design document
    pub fn put_design_document(&self, ddoc: &DesignDocument) -> CouchResult<(String, String)> {
        let mut design_docs = self.design_docs.write().unwrap();
        design_docs.insert(ddoc.id.clone(), ddoc.clone());
        
        // Also store in tree for persistence
        let ddoc_key = format!("design:{}", ddoc.id);
        let serialized = serde_json::to_vec(ddoc)?;
        self.tree.insert(&ddoc_key, serialized)?;
        
        info!("Stored design document: {}", ddoc.id);
        Ok((ddoc.id.clone(), ddoc.rev.clone()))
    }
    
    /// Get design document
    pub fn get_design_document(&self, id: &str) -> CouchResult<DesignDocument> {
        let design_docs = self.design_docs.read().unwrap();
        design_docs.get(id)
            .cloned()
            .ok_or_else(|| CouchError::not_found("Design document not found"))
    }
}

/// Validate database name according to CouchDB rules
fn is_valid_db_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 127 {
        return false;
    }
    
    // Must start with lowercase letter
    if !name.chars().next().unwrap_or(' ').is_lowercase() {
        return false;
    }
    
    // Only lowercase letters, digits, and special chars
    name.chars().all(|c| c.is_lowercase() || c.is_numeric() || "_$()+-/".contains(c))
}

/// Generate new document revision
fn generate_revision(current_rev: &str) -> String {
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
    
    let hash = format!("{:x}", Utc::now().timestamp_nanos());
    format!("{}-{}", rev_num, &hash[..16])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_db_name_validation() {
        assert!(is_valid_db_name("test"));
        assert!(is_valid_db_name("test_123"));
        assert!(is_valid_db_name("my-database"));
        assert!(!is_valid_db_name("Test")); // uppercase
        assert!(!is_valid_db_name("")); // empty
        assert!(!is_valid_db_name("123abc")); // starts with number
    }
    
    #[test]
    fn test_revision_generation() {
        let rev1 = generate_revision("");
        assert!(rev1.starts_with("1-"));
        
        let rev2 = generate_revision("1-abc123");
        assert!(rev2.starts_with("2-"));
    }
    
    #[test]
    fn test_get_database_returns_error_not_panic() {
        let temp_dir = tempdir().unwrap();
        let db_manager = DatabaseManager::new(temp_dir.path().to_str().unwrap()).unwrap();
        db_manager.create_database("mydb").unwrap();

        // Must not panic; must return Err
        let result = db_manager.get_database("mydb");
        assert!(result.is_err(), "get_database must return Err, not panic");
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.error, "internal_server_error");
        assert!(err.reason.contains("get_database_clone"));

        // Also returns Err for a nonexistent database
        let result2 = db_manager.get_database("no_such_db");
        assert!(result2.is_err());
    }

    #[tokio::test]
    async fn test_database_creation() {
        let temp_dir = tempdir().unwrap();
        let db_manager = DatabaseManager::new(temp_dir.path().to_str().unwrap()).unwrap();
        
        let db_info = db_manager.create_database("test_db").unwrap();
        assert_eq!(db_info.db_name, "test_db");
        assert_eq!(db_info.doc_count, 0);
        
        // Test duplicate creation
        assert!(db_manager.create_database("test_db").is_err());
    }
}