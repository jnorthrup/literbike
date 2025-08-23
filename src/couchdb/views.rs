use crate::couchdb::{
    types::{Document, DesignDocument, ViewDefinition, ViewQuery, ViewResult, ViewRow},
    error::{CouchError, CouchResult},
    database::DatabaseInstance,
};
use std::collections::{HashMap, BTreeMap};
use std::sync::{Arc, RwLock};
use serde_json::{Value, json};
use log::{info, warn, error, debug};
use uuid::Uuid;
use chrono::Utc;

/// View server for CouchDB map/reduce functionality
pub struct ViewServer {
    views: Arc<RwLock<HashMap<String, CompiledView>>>,
    javascript_engine: Arc<RwLock<Option<JavaScriptEngine>>>,
    config: ViewServerConfig,
}

/// View server configuration
#[derive(Debug, Clone)]
pub struct ViewServerConfig {
    pub enable_javascript: bool,
    pub max_map_results: usize,
    pub max_reduce_depth: u32,
    pub timeout_ms: u64,
    pub cache_views: bool,
    pub allow_builtin_reduces: bool,
}

impl Default for ViewServerConfig {
    fn default() -> Self {
        Self {
            enable_javascript: true,
            max_map_results: 10000,
            max_reduce_depth: 10,
            timeout_ms: 30000,
            cache_views: true,
            allow_builtin_reduces: true,
        }
    }
}

/// Compiled view with cached map/reduce functions
#[derive(Debug, Clone)]
pub struct CompiledView {
    pub design_doc_id: String,
    pub view_name: String,
    pub map_function: String,
    pub reduce_function: Option<String>,
    pub compiled_at: chrono::DateTime<chrono::Utc>,
    pub last_seq: u64,
    pub index: BTreeMap<Value, Vec<ViewIndexEntry>>,
}

/// View index entry
#[derive(Debug, Clone)]
pub struct ViewIndexEntry {
    pub doc_id: String,
    pub key: Value,
    pub value: Value,
    pub doc_seq: u64,
}

/// Map result from a single document
#[derive(Debug, Clone)]
pub struct MapResult {
    pub key: Value,
    pub value: Value,
}

/// Reduce result
#[derive(Debug, Clone)]
pub struct ReduceResult {
    pub key: Option<Value>,
    pub value: Value,
}

/// JavaScript engine wrapper (simplified mock implementation)
pub struct JavaScriptEngine {
    context_id: String,
}

impl JavaScriptEngine {
    pub fn new() -> CouchResult<Self> {
        Ok(Self {
            context_id: Uuid::new_v4().to_string(),
        })
    }
    
    /// Execute map function on a document
    pub fn execute_map(&self, map_function: &str, doc: &Document) -> CouchResult<Vec<MapResult>> {
        debug!("Executing map function on document: {}", doc.id);
        
        // This is a simplified implementation
        // In a real implementation, you'd use a JavaScript engine like V8 or SpiderMonkey
        
        // For demo purposes, implement some basic map functions
        let results = match map_function.trim() {
            // Simple key-value mapping
            map_fn if map_fn.contains("emit(doc._id, doc)") => {
                vec![MapResult {
                    key: Value::String(doc.id.clone()),
                    value: doc.data.clone(),
                }]
            }
            
            // Map by document type
            map_fn if map_fn.contains("doc.type") => {
                if let Some(doc_type) = doc.data.get("type") {
                    vec![MapResult {
                        key: doc_type.clone(),
                        value: Value::String(doc.id.clone()),
                    }]
                } else {
                    vec![]
                }
            }
            
            // Map all documents
            map_fn if map_fn.contains("emit(null, 1)") => {
                vec![MapResult {
                    key: Value::Null,
                    value: json!(1),
                }]
            }
            
            // Custom date-based mapping
            map_fn if map_fn.contains("doc.created_at") => {
                if let Some(created_at) = doc.data.get("created_at") {
                    vec![MapResult {
                        key: created_at.clone(),
                        value: json!({"id": doc.id, "rev": doc.rev}),
                    }]
                } else {
                    vec![]
                }
            }
            
            _ => {
                warn!("Unsupported map function: {}", map_function);
                vec![]
            }
        };
        
        debug!("Map function produced {} results", results.len());
        Ok(results)
    }
    
    /// Execute reduce function on mapped results
    pub fn execute_reduce(&self, reduce_function: &str, keys: &[Value], values: &[Value], _rereduce: bool) -> CouchResult<ReduceResult> {
        debug!("Executing reduce function on {} values", values.len());
        
        let result = match reduce_function.trim() {
            // Count reduce
            "_count" | "function(keys, values, rereduce) { return values.length; }" => {
                ReduceResult {
                    key: None,
                    value: json!(values.len()),
                }
            }
            
            // Sum reduce
            "_sum" | reduce_fn if reduce_fn.contains("sum") => {
                let sum: f64 = values.iter()
                    .filter_map(|v| v.as_f64())
                    .sum();
                ReduceResult {
                    key: None,
                    value: json!(sum),
                }
            }
            
            // Stats reduce
            "_stats" => {
                let numbers: Vec<f64> = values.iter()
                    .filter_map(|v| v.as_f64())
                    .collect();
                
                if numbers.is_empty() {
                    ReduceResult {
                        key: None,
                        value: json!({"sum": 0, "count": 0, "min": null, "max": null, "sumsqr": 0}),
                    }
                } else {
                    let sum: f64 = numbers.iter().sum();
                    let count = numbers.len();
                    let min = numbers.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                    let max = numbers.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                    let sumsqr: f64 = numbers.iter().map(|x| x * x).sum();
                    
                    ReduceResult {
                        key: None,
                        value: json!({
                            "sum": sum,
                            "count": count,
                            "min": min,
                            "max": max,
                            "sumsqr": sumsqr
                        }),
                    }
                }
            }
            
            _ => {
                warn!("Unsupported reduce function: {}", reduce_function);
                ReduceResult {
                    key: None,
                    value: json!(null),
                }
            }
        };
        
        Ok(result)
    }
}

impl ViewServer {
    /// Create a new view server
    pub fn new(config: ViewServerConfig) -> CouchResult<Self> {
        let javascript_engine = if config.enable_javascript {
            Some(JavaScriptEngine::new()?)
        } else {
            None
        };
        
        Ok(Self {
            views: Arc::new(RwLock::new(HashMap::new())),
            javascript_engine: Arc::new(RwLock::new(javascript_engine)),
            config,
        })
    }
    
    /// Update view index for a design document
    pub fn update_view_index(&self, db: &DatabaseInstance, design_doc: &DesignDocument) -> CouchResult<()> {
        info!("Updating view index for design document: {}", design_doc.id);
        
        if let Some(ref views) = design_doc.views {
            for (view_name, view_def) in views {
                let view_key = format!("{}:{}", design_doc.id, view_name);
                
                let compiled_view = self.compile_view(db, design_doc, view_name, view_def)?;
                
                let mut views_map = self.views.write().unwrap();
                views_map.insert(view_key, compiled_view);
                
                info!("Updated view: {}/{}", design_doc.id, view_name);
            }
        }
        
        Ok(())
    }
    
    /// Compile a view and build its index
    fn compile_view(&self, db: &DatabaseInstance, design_doc: &DesignDocument, view_name: &str, view_def: &ViewDefinition) -> CouchResult<CompiledView> {
        debug!("Compiling view: {}/{}", design_doc.id, view_name);
        
        let mut index = BTreeMap::new();
        let current_seq = *db.sequence_counter.read().unwrap();
        
        // Get all documents and apply map function
        let all_docs = db.get_all_documents(&ViewQuery {
            include_docs: Some(true),
            conflicts: Some(false),
            ..Default::default()
        })?;
        
        let js_engine = self.javascript_engine.read().unwrap();
        if let Some(ref engine) = *js_engine {
            for row in all_docs.rows {
                if let Some(doc) = row.doc {
                    // Skip design documents in regular views
                    if doc.id.starts_with("_design/") {
                        continue;
                    }
                    
                    // Execute map function
                    match engine.execute_map(&view_def.map, &doc) {
                        Ok(map_results) => {
                            for map_result in map_results {
                                let entry = ViewIndexEntry {
                                    doc_id: doc.id.clone(),
                                    key: map_result.key.clone(),
                                    value: map_result.value,
                                    doc_seq: current_seq, // Simplified - should be actual doc seq
                                };
                                
                                index.entry(map_result.key)
                                    .or_insert_with(Vec::new)
                                    .push(entry);
                            }
                        }
                        Err(e) => {
                            warn!("Map function error for doc {}: {}", doc.id, e);
                        }
                    }
                }
            }
        }
        
        Ok(CompiledView {
            design_doc_id: design_doc.id.clone(),
            view_name: view_name.to_string(),
            map_function: view_def.map.clone(),
            reduce_function: view_def.reduce.clone(),
            compiled_at: Utc::now(),
            last_seq: current_seq,
            index,
        })
    }
    
    /// Query a view
    pub fn query_view(&self, design_doc_id: &str, view_name: &str, query: &ViewQuery) -> CouchResult<ViewResult> {
        let view_key = format!("{}:{}", design_doc_id, view_name);
        
        let views = self.views.read().unwrap();
        let view = views.get(&view_key)
            .ok_or_else(|| CouchError::not_found(&format!("View not found: {}/{}", design_doc_id, view_name)))?;
        
        debug!("Querying view: {}/{}", design_doc_id, view_name);
        
        // Apply query filters and limits
        let mut results = Vec::new();
        let limit = query.limit.unwrap_or(25) as usize;
        let skip = query.skip.unwrap_or(0) as usize;
        let include_docs = query.include_docs.unwrap_or(false);
        let descending = query.descending.unwrap_or(false);
        
        // Get keys in order
        let keys: Vec<&Value> = if descending {
            view.index.keys().rev().collect()
        } else {
            view.index.keys().collect()
        };
        
        // Apply key range filters
        let filtered_keys: Vec<&Value> = keys.into_iter()
            .filter(|key| {
                // Filter by startkey
                if let Some(ref startkey) = query.startkey {
                    if descending {
                        if self.compare_keys(key, startkey) == std::cmp::Ordering::Greater {
                            return false;
                        }
                    } else if self.compare_keys(key, startkey) == std::cmp::Ordering::Less {
                        return false;
                    }
                }
                
                // Filter by endkey
                if let Some(ref endkey) = query.endkey {
                    if descending {
                        if self.compare_keys(key, endkey) == std::cmp::Ordering::Less {
                            return false;
                        }
                    } else if self.compare_keys(key, endkey) == std::cmp::Ordering::Greater {
                        return false;
                    }
                }
                
                // Filter by specific key
                if let Some(ref key_filter) = query.key {
                    return self.compare_keys(key, key_filter) == std::cmp::Ordering::Equal;
                }
                
                true
            })
            .collect();
        
        // Handle reduce
        if query.reduce.unwrap_or(false) && view.reduce_function.is_some() {
            return self.execute_reduce_query(view, &filtered_keys, query);
        }
        
        // Collect matching entries
        let mut current_skip = 0;
        for key in filtered_keys {
            if let Some(entries) = view.index.get(key) {
                for entry in entries {
                    if current_skip < skip {
                        current_skip += 1;
                        continue;
                    }
                    
                    if results.len() >= limit {
                        break;
                    }
                    
                    let row = ViewRow {
                        id: Some(entry.doc_id.clone()),
                        key: entry.key.clone(),
                        value: entry.value.clone(),
                        doc: if include_docs {
                            // In a real implementation, we'd fetch the document
                            None // Simplified for demo
                        } else {
                            None
                        },
                    };
                    
                    results.push(row);
                }
                
                if results.len() >= limit {
                    break;
                }
            }
        }
        
        Ok(ViewResult {
            total_rows: view.index.values().map(|v| v.len()).sum::<usize>() as u64,
            offset: skip as u32,
            rows: results,
            update_seq: Some(view.last_seq),
            next_cursor: None, // TODO: Implement cursor support
        })
    }
    
    /// Execute reduce query
    fn execute_reduce_query(&self, view: &CompiledView, keys: &[&Value], query: &ViewQuery) -> CouchResult<ViewResult> {
        if let Some(ref reduce_function) = view.reduce_function {
            let js_engine = self.javascript_engine.read().unwrap();
            if let Some(ref engine) = *js_engine {
                let group = query.group.unwrap_or(false);
                let group_level = query.group_level;
                
                if group || group_level.is_some() {
                    // Group reduce
                    self.execute_group_reduce(engine, view, keys, reduce_function, group_level)
                } else {
                    // Global reduce
                    self.execute_global_reduce(engine, view, keys, reduce_function)
                }
            } else {
                Err(CouchError::internal_server_error("JavaScript engine not available"))
            }
        } else {
            Err(CouchError::bad_request("View does not have a reduce function"))
        }
    }
    
    /// Execute global reduce (no grouping)
    fn execute_global_reduce(&self, engine: &JavaScriptEngine, view: &CompiledView, keys: &[&Value], reduce_function: &str) -> CouchResult<ViewResult> {
        let mut all_keys = Vec::new();
        let mut all_values = Vec::new();
        
        for key in keys {
            if let Some(entries) = view.index.get(*key) {
                for entry in entries {
                    all_keys.push(entry.key.clone());
                    all_values.push(entry.value.clone());
                }
            }
        }
        
        let result = engine.execute_reduce(reduce_function, &all_keys, &all_values, false)?;
        
        let row = ViewRow {
            id: None,
            key: Value::Null,
            value: result.value,
            doc: None,
        };
        
        Ok(ViewResult {
            total_rows: 1,
            offset: 0,
            rows: vec![row],
            update_seq: Some(view.last_seq),
            next_cursor: None,
        })
    }
    
    /// Execute group reduce
    fn execute_group_reduce(&self, engine: &JavaScriptEngine, view: &CompiledView, keys: &[&Value], reduce_function: &str, group_level: Option<u32>) -> CouchResult<ViewResult> {
        let mut groups: BTreeMap<Value, (Vec<Value>, Vec<Value>)> = BTreeMap::new();
        
        for key in keys {
            if let Some(entries) = view.index.get(*key) {
                for entry in entries {
                    let group_key = if let Some(level) = group_level {
                        self.get_group_key(&entry.key, level)
                    } else {
                        entry.key.clone()
                    };
                    
                    let (group_keys, group_values) = groups.entry(group_key).or_insert_with(|| (Vec::new(), Vec::new()));
                    group_keys.push(entry.key.clone());
                    group_values.push(entry.value.clone());
                }
            }
        }
        
        let mut results = Vec::new();
        for (group_key, (group_keys, group_values)) in groups {
            let result = engine.execute_reduce(reduce_function, &group_keys, &group_values, false)?;
            
            let row = ViewRow {
                id: None,
                key: group_key,
                value: result.value,
                doc: None,
            };
            
            results.push(row);
        }
        
        Ok(ViewResult {
            total_rows: results.len() as u64,
            offset: 0,
            rows: results,
            update_seq: Some(view.last_seq),
            next_cursor: None,
        })
    }
    
    /// Get group key for a given level
    fn get_group_key(&self, key: &Value, level: u32) -> Value {
        match key {
            Value::Array(arr) => {
                let truncated: Vec<Value> = arr.iter()
                    .take(level as usize)
                    .cloned()
                    .collect();
                Value::Array(truncated)
            }
            _ => key.clone(),
        }
    }
    
    /// Compare two JSON values for ordering (same as cursor implementation)
    fn compare_keys(&self, a: &Value, b: &Value) -> std::cmp::Ordering {
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
                    match self.compare_keys(a_item, b_item) {
                        std::cmp::Ordering::Equal => continue,
                        other => return other,
                    }
                }
                a.len().cmp(&b.len())
            }
            (Value::Array(_), _) => std::cmp::Ordering::Less,
            (_, Value::Array(_)) => std::cmp::Ordering::Greater,
            
            (Value::Object(_), Value::Object(_)) => {
                a.to_string().cmp(&b.to_string())
            }
        }
    }
    
    /// Get view server statistics
    pub fn get_stats(&self) -> HashMap<String, Value> {
        let views = self.views.read().unwrap();
        let mut stats = HashMap::new();
        
        stats.insert("total_views".to_string(), json!(views.len()));
        stats.insert("javascript_enabled".to_string(), json!(self.config.enable_javascript));
        
        let total_index_size: usize = views.values()
            .map(|v| v.index.values().map(|entries| entries.len()).sum::<usize>())
            .sum();
        
        stats.insert("total_index_entries".to_string(), json!(total_index_size));
        
        stats
    }
    
    /// Clear all view caches
    pub fn clear_caches(&self) {
        let mut views = self.views.write().unwrap();
        views.clear();
        info!("Cleared all view caches");
    }
}

impl Default for ViewQuery {
    fn default() -> Self {
        Self {
            conflicts: None,
            descending: None,
            endkey: None,
            endkey_docid: None,
            group: None,
            group_level: None,
            include_docs: None,
            inclusive_end: None,
            key: None,
            keys: None,
            limit: None,
            reduce: None,
            skip: None,
            stale: None,
            startkey: None,
            startkey_docid: None,
            update_seq: None,
            cursor: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::couchdb::types::*;
    use std::collections::HashMap;
    
    fn create_test_document(id: &str, doc_type: &str, value: i32) -> Document {
        Document {
            id: id.to_string(),
            rev: "1-abc123".to_string(),
            deleted: None,
            attachments: None,
            data: json!({
                "type": doc_type,
                "value": value,
                "created_at": "2023-01-01T00:00:00Z"
            }),
        }
    }
    
    #[test]
    fn test_javascript_engine_map() {
        let engine = JavaScriptEngine::new().unwrap();
        let doc = create_test_document("doc1", "test", 42);
        
        let results = engine.execute_map("function(doc) { emit(doc._id, doc); }", &doc).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, Value::String("doc1".to_string()));
    }
    
    #[test]
    fn test_javascript_engine_reduce() {
        let engine = JavaScriptEngine::new().unwrap();
        let keys = vec![json!("key1"), json!("key2")];
        let values = vec![json!(1), json!(2)];
        
        let result = engine.execute_reduce("_count", &keys, &values, false).unwrap();
        assert_eq!(result.value, json!(2));
        
        let result = engine.execute_reduce("_sum", &keys, &values, false).unwrap();
        assert_eq!(result.value, json!(3.0));
    }
    
    #[test]
    fn test_view_server_creation() {
        let config = ViewServerConfig::default();
        let server = ViewServer::new(config).unwrap();
        
        let stats = server.get_stats();
        assert_eq!(stats["total_views"], json!(0));
        assert_eq!(stats["javascript_enabled"], json!(true));
    }
    
    #[test]
    fn test_key_comparison() {
        let server = ViewServer::new(ViewServerConfig::default()).unwrap();
        
        assert_eq!(server.compare_keys(&json!(1), &json!(2)), std::cmp::Ordering::Less);
        assert_eq!(server.compare_keys(&json!("a"), &json!("b")), std::cmp::Ordering::Less);
        assert_eq!(server.compare_keys(&json!(null), &json!(1)), std::cmp::Ordering::Less);
    }
}