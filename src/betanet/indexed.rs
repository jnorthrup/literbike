//! Indexed<T> = Join<Int, Int->T> - TrikeShed's core abstraction
//! 
//! Zero-allocation sequences using categorical composition with mmap offsets

use crate::baby_pandas::Join;
use crate::mmap_cursor::MmapCursor;
use std::sync::Arc;

/// TrikeShed Indexed type - primary abstraction for zero-copy sequences
pub type Indexed<T> = Join<usize, Box<dyn Fn(usize) -> T>>;

/// Mmap-backed indexed sequence for true zero-copy
pub struct MmapIndexed<T> {
    /// Memory-mapped cursor for data access
    cursor: Arc<MmapCursor>,
    /// Record size for offset calculations
    record_size: usize,
    /// Type marker
    _phantom: std::marker::PhantomData<T>,
}

impl<T> MmapIndexed<T> {
    /// Create new mmap-backed indexed sequence
    pub fn new(cursor: Arc<MmapCursor>, record_size: usize) -> Self {
        Self {
            cursor,
            record_size,
            _phantom: std::marker::PhantomData,
        }
    }
    
    /// Get length from mmap cursor
    pub fn len(&self) -> usize {
        self.cursor.len() as usize
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// Convert to TrikeShed Indexed<T> for compatibility
    pub fn to_indexed<F>(self, mapper: F) -> Indexed<T>
    where 
        F: Fn(&[u8]) -> T + 'static,
        T: 'static,
    {
        let len = self.len();
        let cursor = self.cursor.clone();
        let record_size = self.record_size;
        
        Join::new(
            len,
            Box::new(move |index| {
                unsafe {
                    let ptr = cursor.seek(index as u64);
                    if ptr.is_null() {
                        // Return default - would be better with proper error handling
                        panic!("Invalid index: {}", index);
                    } else {
                        let slice = std::slice::from_raw_parts(ptr, record_size);
                        mapper(slice)
                    }
                }
            })
        )
    }
}

/// Indexed operations for categorical composition
pub struct IndexedOps;

impl IndexedOps {
    /// Map function over indexed sequence (lazy)
    pub fn map<T, U, F>(indexed: Indexed<T>, func: F) -> Indexed<U>
    where
        F: Fn(T) -> U + 'static,
        T: 'static,
        U: 'static,
    {
        Join::new(
            indexed.first,
            Box::new(move |index| {
                let value = (indexed.second)(index);
                func(value)
            })
        )
    }
    
    /// Filter indexed sequence (lazy)
    pub fn filter<T, P>(indexed: Indexed<T>, predicate: P) -> Indexed<Option<T>>
    where
        P: Fn(&T) -> bool + 'static,
        T: Clone + 'static,
    {
        Join::new(
            indexed.first,
            Box::new(move |index| {
                let value = (indexed.second)(index);
                if predicate(&value) {
                    Some(value)
                } else {
                    None
                }
            })
        )
    }
    
    /// Take first n elements
    pub fn take<T>(indexed: Indexed<T>, n: usize) -> Indexed<T>
    where
        T: 'static,
    {
        let take_count = n.min(indexed.first);
        Join::new(
            take_count,
            indexed.second
        )
    }
    
    /// Skip first n elements  
    pub fn skip<T>(indexed: Indexed<T>, n: usize) -> Indexed<T>
    where
        T: 'static,
    {
        let remaining = indexed.first.saturating_sub(n);
        Join::new(
            remaining,
            Box::new(move |index| {
                (indexed.second)(index + n)
            })
        )
    }
    
    /// Fold over indexed sequence
    pub fn fold<T, Acc, F>(indexed: Indexed<T>, init: Acc, func: F) -> Acc
    where
        F: Fn(Acc, T) -> Acc,
        T: 'static,
    {
        let mut accumulator = init;
        for i in 0..indexed.first {
            let value = (indexed.second)(i);
            accumulator = func(accumulator, value);
        }
        accumulator
    }
    
    /// Create indexed from mmap with byte interpretation
    pub fn from_mmap_bytes(cursor: Arc<MmapCursor>, record_size: usize) -> Indexed<Vec<u8>> {
        let len = cursor.len() as usize;
        
        Join::new(
            len,
            Box::new(move |index| {
                unsafe {
                    let ptr = cursor.seek(index as u64);
                    if ptr.is_null() {
                        Vec::new()
                    } else {
                        let slice = std::slice::from_raw_parts(ptr, record_size);
                        slice.to_vec()
                    }
                }
            })
        )
    }
    
    /// Create indexed from mmap with hex string interpretation  
    pub fn from_mmap_hex(cursor: Arc<MmapCursor>, record_size: usize) -> Indexed<String> {
        let len = cursor.len() as usize;
        
        Join::new(
            len,
            Box::new(move |index| {
                unsafe {
                    let ptr = cursor.seek(index as u64);
                    if ptr.is_null() {
                        String::new()
                    } else {
                        let slice = std::slice::from_raw_parts(ptr, record_size);
                        crate::baby_pandas::bytes_to_hex(slice)
                    }
                }
            })
        )
    }
    
    /// Create indexed from function (pure TrikeShed style)
    pub fn from_fn<T, F>(size: usize, func: F) -> Indexed<T>
    where
        F: Fn(usize) -> T + 'static,
        T: 'static,
    {
        Join::new(size, Box::new(func))
    }
    
    /// Zip two indexed sequences
    pub fn zip<T, U>(left: Indexed<T>, right: Indexed<U>) -> Indexed<(T, U)>
    where
        T: 'static,
        U: 'static,
    {
        let min_len = left.first.min(right.first);
        
        Join::new(
            min_len,
            Box::new(move |index| {
                let left_val = (left.second)(index);
                let right_val = (right.second)(index);
                (left_val, right_val)
            })
        )
    }
    
    /// Chain two indexed sequences
    pub fn chain<T>(first: Indexed<T>, second: Indexed<T>) -> Indexed<T>
    where
        T: 'static,
    {
        let total_len = first.first + second.first;
        let first_len = first.first;
        
        Join::new(
            total_len,
            Box::new(move |index| {
                if index < first_len {
                    (first.second)(index)
                } else {
                    (second.second)(index - first_len)
                }
            })
        )
    }
}

/// Extension trait for working with Indexed sequences
pub trait IndexedExt<T> {
    /// Convert to vector (materialization)
    fn collect(self) -> Vec<T>;
    
    /// Get element at index
    fn get(&self, index: usize) -> Option<T>;
    
    /// Apply operation and return new indexed
    fn map_indexed<U, F>(self, func: F) -> Indexed<U> 
    where 
        F: Fn(T) -> U + 'static,
        U: 'static;
}

impl<T> IndexedExt<T> for Indexed<T> 
where 
    T: Clone + 'static 
{
    fn collect(self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.first);
        for i in 0..self.first {
            result.push((self.second)(i));
        }
        result
    }
    
    fn get(&self, index: usize) -> Option<T> {
        if index < self.first {
            Some((self.second)(index))
        } else {
            None
        }
    }
    
    fn map_indexed<U, F>(self, func: F) -> Indexed<U>
    where 
        F: Fn(T) -> U + 'static,
        U: 'static
    {
        IndexedOps::map(self, func)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmap_cursor::MmapCursor;
    use std::fs;

    #[test]  
    fn test_indexed_from_fn() {
        let indexed = IndexedOps::from_fn(5, |i| i * 2);
        
        assert_eq!(indexed.first, 5);
        assert_eq!((indexed.second)(0), 0);
        assert_eq!((indexed.second)(2), 4);
        assert_eq!((indexed.second)(4), 8);
    }
    
    #[test]
    fn test_indexed_map() {
        let indexed = IndexedOps::from_fn(3, |i| i + 1);
        let mapped = IndexedOps::map(indexed, |x| x * 10);
        
        assert_eq!(mapped.first, 3);
        assert_eq!((mapped.second)(0), 10);  // (0 + 1) * 10
        assert_eq!((mapped.second)(1), 20);  // (1 + 1) * 10
        assert_eq!((mapped.second)(2), 30);  // (2 + 1) * 10
    }
    
    #[test]
    fn test_indexed_operations() {
        let indexed = IndexedOps::from_fn(5, |i| i);
        
        // Test take
        let taken = IndexedOps::take(indexed.clone(), 3);
        assert_eq!(taken.first, 3);
        
        // Test skip  
        let skipped = IndexedOps::skip(indexed.clone(), 2);
        assert_eq!(skipped.first, 3);
        assert_eq!((skipped.second)(0), 2); // originally index 2
        
        // Test fold
        let sum = IndexedOps::fold(indexed, 0, |acc, x| acc + x);
        assert_eq!(sum, 0 + 1 + 2 + 3 + 4); // sum of 0..5
    }
    
    #[test]
    fn test_indexed_ext_trait() {
        let indexed = IndexedOps::from_fn(3, |i| i * 2);
        
        // Test collect
        let collected = indexed.collect();
        assert_eq!(collected, vec![0, 2, 4]);
    }
    
    #[test] 
    fn test_indexed_zip_chain() {
        let left = IndexedOps::from_fn(2, |i| i);
        let right = IndexedOps::from_fn(2, |i| i * 10);
        
        // Test zip
        let zipped = IndexedOps::zip(left.clone(), right);
        assert_eq!(zipped.first, 2);
        assert_eq!((zipped.second)(0), (0, 0));
        assert_eq!((zipped.second)(1), (1, 10));
        
        // Test chain
        let another = IndexedOps::from_fn(2, |i| i + 100);
        let chained = IndexedOps::chain(left, another);
        assert_eq!(chained.first, 4);
        assert_eq!((chained.second)(0), 0);   // from first
        assert_eq!((chained.second)(2), 100); // from second (index 0 + 100)
    }
}