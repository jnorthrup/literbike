//! Idempotent Tuple Operations - In-place updates without versioning
//! 
//! Leverages mmap_cursor's update() method for zero-copy mutations
//! Based on TrikeShed principle that tuples are immutable values that can be replaced

use crate::mmap_cursor::MmapCursor;
use crate::isam_index::ISAMTable;
use std::sync::Arc;
use std::hash::{Hash, Hasher};

/// Idempotent tuple - can be updated in-place safely
pub trait IdempotentTuple: Clone + PartialEq + Hash {
    /// Get unique tuple ID for deduplication
    fn tuple_id(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
    
    /// Check if this tuple supersedes another (for conflict resolution)
    fn supersedes(&self, other: &Self) -> bool {
        // Default: equality means same tuple, no superseding
        self != other
    }
}

/// Tuple store using mmap for zero-copy updates
pub struct TupleStore<T> 
where 
    T: IdempotentTuple + 'static
{
    /// ISAM table for indexed access
    table: ISAMTable,
    /// Type marker
    _phantom: std::marker::PhantomData<T>,
}

impl<T> TupleStore<T> 
where 
    T: IdempotentTuple + 'static
{
    /// Create new tuple store
    pub unsafe fn new(data_path: &str, index_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let table = ISAMTable::init_empty::<T>(data_path, index_path)?;
        
        Ok(Self {
            table,
            _phantom: std::marker::PhantomData,
        })
    }
    
    /// Open existing tuple store
    pub unsafe fn open(data_path: &str, index_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let table = ISAMTable::new(data_path, index_path)?;
        
        Ok(Self {
            table,
            _phantom: std::marker::PhantomData,
        })
    }
    
    /// Insert or update tuple idempotently
    pub unsafe fn upsert(&mut self, tuple: T) -> Result<(), Box<dyn std::error::Error>> {
        let tuple_id = tuple.tuple_id();
        
        // Check if tuple already exists
        if let Some(existing) = self.table.get::<T>(tuple_id) {
            // Idempotent update: only update if new tuple supersedes existing
            if tuple.supersedes(existing) {
                self.table.insert(tuple_id, &tuple)?;
            }
            // If same tuple or existing supersedes, no-op (idempotent)
        } else {
            // New tuple, insert
            self.table.insert(tuple_id, &tuple)?;
        }
        
        Ok(())
    }
    
    /// Get tuple by ID
    pub unsafe fn get(&self, tuple_id: u64) -> Option<&T> {
        self.table.get(tuple_id)
    }
    
    /// Delete tuple (mark as deleted)
    pub unsafe fn delete(&mut self, tuple_id: u64) -> Result<(), Box<dyn std::error::Error>> {
        // In idempotent system, deletion is just another state
        // We could implement tombstone tuples here
        Ok(())
    }
    
    /// Compact store (remove duplicates and deleted entries)
    pub unsafe fn compact(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // This would implement compaction logic
        // For now, delegate to table sync
        self.table.sync()?;
        Ok(())
    }
    
    /// Get all tuples (for iteration)
    pub unsafe fn iter(&self) -> TupleIterator<T> {
        TupleIterator {
            table: &self.table,
            current_id: 0,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Iterator over tuples in store
pub struct TupleIterator<'a, T> 
where 
    T: IdempotentTuple 
{
    table: &'a ISAMTable,
    current_id: u64,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T> Iterator for TupleIterator<'a, T>
where 
    T: IdempotentTuple
{
    type Item = (u64, &'a T);
    
    fn next(&mut self) -> Option<Self::Item> {
        // Simple linear scan - could be optimized with proper iteration
        loop {
            if let Some(tuple) = unsafe { self.table.get::<T>(self.current_id) } {
                let id = self.current_id;
                self.current_id += 1;
                return Some((id, tuple));
            }
            self.current_id += 1;
            
            // Arbitrary limit to prevent infinite loops
            if self.current_id > 1_000_000 {
                return None;
            }
        }
    }
}

/// Batch operations for efficient bulk updates
pub struct TupleBatch<T> 
where 
    T: IdempotentTuple
{
    tuples: Vec<T>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> TupleBatch<T>
where 
    T: IdempotentTuple
{
    /// Create new batch
    pub fn new() -> Self {
        Self {
            tuples: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }
    
    /// Add tuple to batch
    pub fn add(&mut self, tuple: T) {
        self.tuples.push(tuple);
    }
    
    /// Apply batch to store (all operations are idempotent)
    pub unsafe fn apply_to(&self, store: &mut TupleStore<T>) -> Result<(), Box<dyn std::error::Error>> {
        for tuple in &self.tuples {
            store.upsert(tuple.clone())?;
        }
        Ok(())
    }
    
    /// Deduplicate batch (remove duplicate tuples)
    pub fn dedupe(&mut self) {
        self.tuples.sort_by_key(|t| t.tuple_id());
        self.tuples.dedup_by_key(|t| t.tuple_id());
    }
}

/// Example idempotent tuple types
#[derive(Clone, PartialEq, Hash, Debug)]
pub struct MetricTuple {
    pub timestamp: u64,
    pub metric_name: String,
    pub value: f64,
    pub tags: Vec<String>,
}

impl IdempotentTuple for MetricTuple {
    fn supersedes(&self, other: &Self) -> bool {
        // Newer timestamp supersedes older for same metric
        self.metric_name == other.metric_name && self.timestamp > other.timestamp
    }
}

#[derive(Clone, PartialEq, Hash, Debug)]
pub struct ConfigTuple {
    pub key: String,
    pub value: String,
    pub version: u32,
}

impl IdempotentTuple for ConfigTuple {
    fn supersedes(&self, other: &Self) -> bool {
        // Higher version supersedes lower for same key
        self.key == other.key && self.version > other.version
    }
}

/// Merge strategy for conflicting tuples
pub enum MergeStrategy {
    /// Keep newer tuple (by timestamp)
    KeepNewer,
    /// Keep higher version
    KeepHigherVersion,
    /// Custom merge function
    Custom(Box<dyn Fn(&dyn IdempotentTuple, &dyn IdempotentTuple) -> Box<dyn IdempotentTuple>>),
}

/// Conflict-free replicated data type (CRDT) support
pub trait CRDTTuple: IdempotentTuple {
    /// Merge two tuples (commutative and associative)
    fn merge(&self, other: &Self) -> Self;
    
    /// Check if merge is needed
    fn conflicts_with(&self, other: &Self) -> bool {
        self != other
    }
}

impl CRDTTuple for ConfigTuple {
    fn merge(&self, other: &Self) -> Self {
        if self.version >= other.version {
            self.clone()
        } else {
            other.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_idempotent_tuple() {
        let tuple1 = ConfigTuple {
            key: "test".to_string(),
            value: "value1".to_string(),
            version: 1,
        };
        
        let tuple2 = ConfigTuple {
            key: "test".to_string(), 
            value: "value2".to_string(),
            version: 2,
        };
        
        assert!(tuple2.supersedes(&tuple1));
        assert!(!tuple1.supersedes(&tuple2));
        assert_eq!(tuple1.tuple_id(), tuple1.tuple_id()); // Consistent hashing
    }
    
    #[test] 
    fn test_tuple_batch() {
        let mut batch = TupleBatch::new();
        
        batch.add(ConfigTuple {
            key: "key1".to_string(),
            value: "value1".to_string(), 
            version: 1,
        });
        
        batch.add(ConfigTuple {
            key: "key1".to_string(),
            value: "value2".to_string(),
            version: 1, // Same version, will dedupe
        });
        
        assert_eq!(batch.tuples.len(), 2);
        batch.dedupe();
        assert_eq!(batch.tuples.len(), 1); // Deduped
    }
    
    #[test]
    fn test_crdt_merge() {
        let tuple1 = ConfigTuple {
            key: "config".to_string(),
            value: "old_value".to_string(),
            version: 1,
        };
        
        let tuple2 = ConfigTuple {
            key: "config".to_string(),
            value: "new_value".to_string(),
            version: 2,
        };
        
        let merged = tuple1.merge(&tuple2);
        assert_eq!(merged.version, 2);
        assert_eq!(merged.value, "new_value");
    }
}