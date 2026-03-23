// Thin columnar integration helpers (non-invasive shim)
//
// This file provides small, safe helpers that operate on DenseColumnStore
// without requiring changes to UnifiedCursor internals. It is intended as an
// interim adapter while the full Join<Indexed<T>, Position> integration is
// implemented inside `isam_core`.
//
// See also: [`src/unified_cursor/columnar.rs`](src/unified_cursor/columnar.rs:1)
// and MLIR accessors at [`src/unified_cursor/mlir_bridge.rs`](src/unified_cursor/mlir_bridge.rs:1)

use crate::unified_cursor::columnar::DenseColumnStore;

/// Read a single u64 from the provided column store at `index`.
/// Returns None if out-of-bounds.
#[inline]
pub fn read_col_u64(store: &DenseColumnStore<u64>, index: usize) -> Option<u64> {
    store.get(index).copied()
}

/// Read a batch of u64 values from `start` up to `start + len`.
/// Returns a Vec<u64> with available values (may be shorter if OOB).
#[inline]
pub fn read_col_u64_batch(store: &DenseColumnStore<u64>, start: usize, len: usize) -> Vec<u64> {
    store.get_batch(start, len).into_iter().map(|r| *r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unified_cursor::columnar::DenseColumnStore;

    #[test]
    fn shim_single_get() {
        let data = vec![11u64, 22u64, 33u64];
        let store = DenseColumnStore::new(data);
        assert_eq!(read_col_u64(&store, 0), Some(11u64));
        assert_eq!(read_col_u64(&store, 2), Some(33u64));
        assert_eq!(read_col_u64(&store, 10), None);
    }

    #[test]
    fn shim_batch_get() {
        let data: Vec<u64> = (0..30u64).collect();
        let store = DenseColumnStore::new(data.clone());
        let batch = read_col_u64_batch(&store, 5, 4);
        assert_eq!(batch, vec![5u64, 6u64, 7u64, 8u64]);
    }
}