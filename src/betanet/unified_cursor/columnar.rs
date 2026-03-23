//! Columnar dense store utilities for UnifiedCursor
//!
//! This mirrors the `DenseColumnStore` concept from `platform/couchduck` but is scoped
//! to the unified cursor module to avoid cross‑crate dependencies.

use std::sync::Arc;

/// Core `Join` type – categorical composition of two values.
pub type Join<A, B> = (A, B);

/// `Indexed<T>` – size + accessor function.
pub type Indexed<T> = Join<usize, Box<dyn Fn(usize) -> T + Send + Sync>>;

/// Columnar cursor with position tracking.
pub type ColumnarCursor<T> = Join<Indexed<T>, usize>; // (indexed_data, current_position)

/// Dense column store for bulk operations.
/// `loader` receives a slice of row indices and returns the corresponding values.
pub struct DenseColumnStore<T> {
    /// The underlying columnar cursor.
    pub cursor: ColumnarCursor<T>,
    /// Bulk loader for fetching many rows at once.
    pub loader: Box<dyn Fn(&[usize]) -> Vec<T> + Send + Sync>,
}

impl<T: Clone + Send + Sync + 'static> DenseColumnStore<T> {
    /// Create a new dense column store from an indexed data source.
    ///
    /// * `size` – total number of rows.
    /// * `accessor` – function that returns the value for a given row index.
    /// * `loader` – bulk loader that can fetch many rows in a single call.
    pub fn new<F, G>(size: usize, accessor: F, loader: G) -> Self
    where
        F: Fn(usize) -> T + Send + Sync + 'static,
        G: Fn(&[usize]) -> Vec<T> + Send + Sync + 'static,
    {
        let indexed = (size, Box::new(accessor) as Box<dyn Fn(usize) -> T + Send + Sync>);
        DenseColumnStore {
            cursor: (indexed, 0),
            loader: Box::new(loader),
        }
    }

    /// Convenience: construct a DenseColumnStore directly from a Vec<T>.
    /// Creates an accessor and a bulk loader that reference the owned vector.
    pub fn from_vec(data: Vec<T>) -> Self {
        let size = data.len();
        let arc = Arc::new(data);
        let accessor_arc = arc.clone();
        let loader_arc = arc.clone();
        let accessor = move |idx: usize| accessor_arc[idx].clone();
        let loader = move |indices: &[usize]| -> Vec<T> {
            indices.iter().filter_map(|&i| loader_arc.get(i).cloned()).collect()
        };
        DenseColumnStore::new(size, accessor, loader)
    }

    /// Retrieve a single value at the given row index.
    pub fn get(&self, idx: usize) -> Option<T> {
        let (size, accessor) = &self.cursor.0;
        if idx >= *size {
            None
        } else {
            Some(accessor(idx))
        }
    }

    /// Retrieve a batch of values using the bulk loader by explicit index slice.
    pub fn get_batch_indices(&self, indices: &[usize]) -> Vec<T> {
        (self.loader)(indices)
    }

    /// Retrieve a batch of values from start..start+len.
    /// This convenience matches existing call-sites that use (start, len).
    pub fn get_batch(&self, start: usize, len: usize) -> Vec<T> {
        let (size, _) = &self.cursor.0;
        if start >= *size || len == 0 {
            return Vec::new();
        }
        let end = std::cmp::min(start + len, *size);
        let indices: Vec<usize> = (start..end).collect();
        self.get_batch_indices(&indices)
    }

    /// Advance the cursor by `n` positions.
    /// Returns the new position.
    pub fn advance(&mut self, n: usize) -> usize {
        let new_pos = self.cursor.1.saturating_add(n);
        self.cursor.1 = new_pos;
        new_pos
    }

    /// Reset the cursor to the beginning.
    pub fn reset(&mut self) {
        self.cursor.1 = 0;
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_column_basic_get() {
        // Construct a simple column and verify single-get semantics.
        let data: Vec<u32> = vec![10, 20, 30, 40];
        let col = DenseColumnStore::from_vec(data.clone());
        assert_eq!(col.get(0).copied(), Some(10u32));
        assert_eq!(col.get(2).copied(), Some(30u32));
        assert_eq!(col.get(4), None);
    }

    #[test]
    fn dense_column_get_batch() {
        // Verify get_batch(start,len) convenience returns expected range.
        let data: Vec<u64> = (0..100u64).collect();
        let col = DenseColumnStore::from_vec(data.clone());
        let batch = col.get_batch(10, 5);
        let expected: Vec<u64> = (10..15u64).collect();
        assert_eq!(batch, expected);
    }

    #[test]
    fn dense_column_empty_batch_and_bounds() {
        let col: DenseColumnStore<i32> = DenseColumnStore::from_vec(vec![]);
        assert_eq!(col.get(0), None);
        let batch = col.get_batch(0, 10);
        assert!(batch.is_empty());
    }

    #[test]
    fn dense_column_from_vec_with_oob_indices() {
        let data = vec![10, 20, 30];
        let col = DenseColumnStore::from_vec(data);
        let indices = vec![0, 1, 5, 2]; // 5 is out of bounds
        let batch = col.get_batch_indices(&indices);
        assert_eq!(batch, vec![10, 20, 30]); // Expect only in-bounds values
    }
}