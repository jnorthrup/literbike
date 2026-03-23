//! Core ISAM Index and Mmap Management
//!
//! This module combines the ISAM index structure with direct mmap management
//! for zero-allocation, high-performance data access.

use std::os::unix::io::RawFd;
use std::ptr;
use memmap2::MmapMut;
use std::sync::atomic::{AtomicUsize, AtomicU64};

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

pub use crate::mmap_cursor::ISAMHeader;

/// ENDGAME: Lock-free ISAM index structure
/// Combines IsamIndex from `museum/mini-literbike-stub/src/trikeshedcouch/cursor.rs`
/// with MmapCursor's raw pointer management and columnar offsets.
#[repr(C, align(64))]
pub struct IsamIndex {
    // Raw pointer to mmap'd data region
    data_base: *mut u8,
    // Raw pointer to mmap'd index region
    index_base: *mut u64,
    // Total data length
    data_len: usize,
    // Index entry count (from MmapCursor)
    index_len: usize,
    // File descriptor for data
    data_fd: RawFd,
    // File descriptor for index
    index_fd: RawFd,
    // Keep mmap alive (but don't use its API)
    _data_mmap: MmapMut,
    _index_mmap: MmapMut,

    // ISAM specific fields
    pages: *mut IndexPage,
    page_count: usize,
    total_entries: AtomicUsize,
    search_cycles: AtomicU64,

    // Columnar offset tracking (from mmap_cursor.rs -> MmapDataFrame)
    column_offsets: Vec<(usize, usize)>, // (start, end) for each column
}

/// ENDGAME: Cache-aligned index page (64KB)
#[repr(C, align(65536))]
struct IndexPage {
    entry_count: u16,
    level: u8,
    _pad: [u8; 5],
    entries: [IndexEntry; 511], // 511 * 128 bytes = ~64KB
}

/// ENDGAME: Fixed-size index entry for SIMD with columnar metadata
#[repr(C, align(128))]
struct IndexEntry {
    key: [u8; 64],      // Fixed 64-byte keys
    key_len: u16,       // Actual key length
    offset: u64,        // Data offset
    child_page: u32,    // Child page number (for non-leaf)
    column_idx: u16,    // Column index for columnar storage
    _pad: [u8; 48],     // Adjusted padding
}

/// Unified Cursor combining features from IsamCursor and MmapCursor with columnar support
pub struct UnifiedCursor {
    /// Backing mmap cursor (owns the mmaps)
    pub mmap_cursor: crate::mmap_cursor::MmapCursor,

    // IsamCursor's traversal state
    current_page: *const IndexPage,
    current_entry: u16,
    stack: [(*const IndexPage, u16); 32],
    stack_depth: u8,
    traversal_cycles: u64,

    // Columnar tracking directly from mmap header
    column_offsets: Vec<(usize, usize)>,

    // Integrated dense column stores (supports heterogeneous types via Arc<dyn Any>)
    // This allows for full columnar integration with different column types.
    columns: Vec<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
}

impl IsamIndex {
    /// Return raw pages pointer (for low-level ops / diagnostics)
    pub fn pages_ptr(&self) -> *mut IndexPage {
        self.pages
    }

    /// Return number of pages allocated for this index
    pub fn page_count(&self) -> usize {
        self.page_count
    }

    /// Get column offsets for a given column index
    pub fn column_offset(&self, idx: usize) -> Option<(usize, usize)> {
        self.column_offsets.get(idx).copied()
    }
}

impl UnifiedCursor {
    // ... (existing methods unchanged)

    /// Get column data slice for current position
    pub fn current_column_data(&self, col_idx: usize) -> Option<&[u8]> {
        unsafe {
            if self.current_page.is_null() || col_idx >= self.column_offsets.len() {
                return None;
            }

            let entry = &(*self.current_page).entries[self.current_entry as usize];
            let (start, end) = self.column_offsets[col_idx];
            let ptr = self.mmap_cursor.seek(entry.offset)?;
            Some(std::slice::from_raw_parts(ptr.add(start), end - start))
        }
    }

    /// Attach a column store to this cursor instance.
    /// Columns are addressed by insertion order (0..).
    pub fn add_column<T: 'static + Send + Sync>(&mut self, store: crate::unified_cursor::columnar::DenseColumnStore<T>) {
        self.columns.push(std::sync::Arc::new(store));
    }

    /// Read a value of type T from a previously attached column.
    /// Returns None if column index or element index is out of bounds, or if type mismatch.
    pub fn read_column<T: 'static + Clone + Send + Sync>(&self, col_idx: usize, index: usize) -> Option<T> {
        if col_idx >= self.columns.len() {
            return None;
        }
        let any_store = &self.columns[col_idx];
        any_store.downcast_ref::<crate::unified_cursor::columnar::DenseColumnStore<T>>()
            .and_then(|store| store.get(index))
    }

    /// Read a batch of values of type T from an attached column.
    /// Returns a Vec<T> containing available values (may be shorter than len if OOB or type mismatch).
    pub fn read_column_batch<T: 'static + Clone + Send + Sync>(&self, col_idx: usize, start: usize, len: usize) -> Vec<T> {
        if col_idx >= self.columns.len() {
            return Vec::new();
        }
        let any_store = &self.columns[col_idx];
        any_store.downcast_ref::<crate::unified_cursor::columnar::DenseColumnStore<T>>()
            .map_or_else(Vec::new, |store| store.get_batch(start, len))
    }
}

// ... (remaining implementations unchanged)
// Columnar integration helpers (minimal shim to wire DenseColumnStore into isam core)
//
// This file appends a tiny, safe adapter that higher-level code can use to read
// column values without exposing internal columnar types across the crate boundary.
//
// Note: this is intentionally small and conservative so it can be extended by the full integration later.
pub mod columnar_integration {
    /// Read a u64 value from a DenseColumnStore at index `i`.
    /// Returns None if out-of-bounds.
    #[inline]
    pub fn read_col_u64(
        store: &crate::unified_cursor::columnar::DenseColumnStore<u64>,
        index: usize,
    ) -> Option<u64> {
        store.get(index).copied()
    }

    /// Read a batch of u64 values from start..start+len.
    /// Returns a Vec<u64> with available values (may be shorter than requested if OOB).
    pub fn read_col_u64_batch(
        store: &crate::unified_cursor::columnar::DenseColumnStore<u64>,
        start: usize,
        len: usize,
    ) -> Vec<u64> {
        store
            .get_batch(start, len)
            .into_iter()
            .map(|r| *r)
            .collect::<Vec<u64>>()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::unified_cursor::columnar::DenseColumnStore;

        #[test]
        fn integration_read_single() {
            let data = vec![100u64, 200u64, 300u64];
            let store = DenseColumnStore::new(data);
            assert_eq!(read_col_u64(&store, 0), Some(100u64));
            assert_eq!(read_col_u64(&store, 2), Some(300u64));
            assert_eq!(read_col_u64(&store, 10), None);
        }

        #[test]
        fn integration_read_batch() {
            let data: Vec<u64> = (0..50u64).collect();
            let store = DenseColumnStore::new(data.clone());
            let batch = read_col_u64_batch(&store, 10, 5);
            assert_eq!(batch, vec![10u64, 11u64, 12u64, 13u64, 14u64]);
        }
    }
}
#[cfg(test)]
mod cursor_traversal_tests {
    use super::*;
    use crate::mmap_cursor::MmapCursor;

    // Minimal fake MmapCursor for unit testing without OS mmaps.
    struct FakeMmapCursor {
        data: Vec<u8>,
    }

    impl FakeMmapCursor {
        fn new(data: Vec<u8>) -> Self {
            Self { data }
        }

        fn as_mmap_cursor(self) -> crate::mmap_cursor::MmapCursor {
            // Test-only shim: construct a minimal, zeroed MmapCursor for unit tests.
            // Safety: creating a zeroed instance bypasses real mmap initialization.
            // Tests must avoid calling into MmapCursor internals that dereference mmap handles.
            // This allows traversal unit tests to construct a UnifiedCursor instance without
            // requiring OS mmap setup.
            unsafe { std::mem::zeroed() }
        }
    }

    #[test]
    fn traversal_first_next_with_shim() {
        // Construct a UnifiedCursor using the test-only MmapCursor shim.
        // This allows testing traversal logic without requiring real mmaps.
        let fake_mmap = FakeMmapCursor::new(vec![0u8; 128]).as_mmap_cursor();
        let mut cursor = UnifiedCursor {
            mmap_cursor: fake_mmap,
            current_page: std::ptr::null(),
            current_entry: 0,
            stack: [(std::ptr::null(), 0); 32],
            stack_depth: 0,
            traversal_cycles: 0,
            column_offsets: Vec::new(),
            columns: Vec::new(), // Initialize with the new heterogeneous column vector
        };

        // Setup a minimal in-memory ISAM structure for testing traversal.
        let mut page = IndexPage {
            entry_count: 2,
            level: 0,
            _pad: [0; 5],
            entries: [
                IndexEntry {
                    key: [1; 64],
                    key_len: 1,
                    offset: 100,
                    child_page: 0,
                    column_idx: 0,
                    _pad: [0; 48],
                },
                IndexEntry {
                    key: [2; 64],
                    key_len: 1,
                    offset: 200,
                    child_page: 0,
                    column_idx: 0,
                    _pad: [0; 48],
                },
                // Fill remaining entries with zeros
                IndexEntry {
                    key: [0; 64],
                    key_len: 0,
                    offset: 0,
                    child_page: 0,
                    column_idx: 0,
                    _pad: [0; 48],
                }; 509
            ],
        };

        // Assign the test page to the cursor's current_page.
        // This is unsafe and only for testing purposes with a controlled, static page.
        cursor.current_page = &mut page as *mut IndexPage;
        cursor.current_entry = 0; // Start at the first entry

        // Test first() and next() traversal.
        // For this minimal setup, first() should succeed and point to the first entry.
        // next() should then move to the second entry.
        assert!(cursor.first(), "first() should succeed for a valid page");
        assert_eq!(cursor.current_entry, 0);
        assert_eq!(unsafe { &*cursor.current_page }.entries[cursor.current_entry as usize].offset, 100);

        assert!(cursor.next(), "next() should succeed for the second entry");
        assert_eq!(cursor.current_entry, 1);
        assert_eq!(unsafe { &*cursor.current_page }.entries[cursor.current_entry as usize].offset, 200);

        assert!(!cursor.next(), "next() should fail after the last entry");
    }
}