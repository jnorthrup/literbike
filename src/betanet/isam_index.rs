//! ISAM Index - Direct offset-based indexing
//! 
//! No B-trees, no hash tables, just direct array indexing
//! TrikeShed simplicity: offset[key] = data location

use crate::mmap_cursor::MmapCursor;
use std::mem;
use std::ptr;

/// Direct offset index - simple array of file positions
/// Each index entry is a u64 offset into the data file
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct IndexEntry {
    /// File offset to record
    pub offset: u64,
    /// Record key for validation
    pub key: u64,
}

/// ISAM index over mmap'd files
/// NO complex data structures, just linear arrays
pub struct ISAMIndex {
    /// Raw pointer to index entries
    entries: *mut IndexEntry,
    /// Current entry count
    count: usize,
    /// Maximum entries (file size / entry size)
    capacity: usize,
    /// Associated data cursor
    data_cursor: *mut MmapCursor,
}

impl ISAMIndex {
    /// Create new ISAM index backed by mmap cursor
    /// UNSAFE: Raw pointer manipulation, no bounds checking
    pub unsafe fn new(data_cursor: &mut MmapCursor, index_cursor: &mut MmapCursor) -> Result<Self, &'static str> {
        // Validate index file has correct record size
        let header = index_cursor.header();
        if header.record_size != mem::size_of::<IndexEntry>() as u64 {
            return Err("Index file has wrong record size for IndexEntry");
        }

        let entries = index_cursor.data_base.add(header.data_offset as usize) as *mut IndexEntry;
        let capacity = ((index_cursor.data_len - header.data_offset as usize) / mem::size_of::<IndexEntry>()).min(u32::MAX as usize);
        let count = header.record_count as usize;

        Ok(Self {
            entries,
            count,
            capacity,
            data_cursor: data_cursor as *mut MmapCursor,
        })
    }

    /// Initialize empty index
    /// UNSAFE: Raw pointer initialization
    pub unsafe fn init_empty(data_cursor: &mut MmapCursor, index_cursor: &mut MmapCursor) -> Result<Self, &'static str> {
        index_cursor.init_header(mem::size_of::<IndexEntry>())?;
        Self::new(data_cursor, index_cursor)
    }

    /// Direct key lookup - O(1) if keys are dense
    /// UNSAFE: No bounds checking, raw array access
    pub unsafe fn get(&self, key: u64) -> Option<*const u8> {
        if key >= self.count as u64 {
            return None;
        }

        let entry = &*self.entries.add(key as usize);
        if entry.key != key {
            // Key mismatch - sparse or deleted entry
            return None;
        }

        let data_cursor = &*self.data_cursor;
        Some(data_cursor.data_base.add(entry.offset as usize))
    }

    /// Insert or update index entry
    /// UNSAFE: Raw pointer manipulation, no bounds checking
    pub unsafe fn put(&mut self, key: u64, data_offset: u64) -> Result<(), &'static str> {
        if key >= self.capacity as u64 {
            return Err("Key exceeds index capacity");
        }

        let entry_ptr = self.entries.add(key as usize);
        let entry = &mut *entry_ptr;
        entry.key = key;
        entry.offset = data_offset;

        // Extend count if necessary
        if key >= self.count as u64 {
            self.count = (key + 1) as usize;
        }

        Ok(())
    }

    /// Linear scan for non-dense keys (fallback)
    /// UNSAFE: Raw pointer iteration
    pub unsafe fn scan_for_key(&self, target_key: u64) -> Option<*const u8> {
        for i in 0..self.count {
            let entry = &*self.entries.add(i);
            if entry.key == target_key {
                let data_cursor = &*self.data_cursor;
                return Some(data_cursor.data_base.add(entry.offset as usize));
            }
        }
        None
    }

    /// Get all entries as iterator
    /// UNSAFE: Raw pointer iteration
    pub unsafe fn entries(&self) -> IndexIterator {
        IndexIterator {
            entries: self.entries,
            current: 0,
            count: self.count,
            data_cursor: self.data_cursor,
        }
    }

    /// Compact index by removing gaps
    /// UNSAFE: Massive pointer manipulation
    pub unsafe fn compact(&mut self) -> Result<(), &'static str> {
        let mut write_idx = 0;

        for read_idx in 0..self.count {
            let entry = &*self.entries.add(read_idx);
            if entry.offset != 0 { // Non-null entry
                if read_idx != write_idx {
                    let dst = self.entries.add(write_idx);
                    *dst = *entry;
                }
                write_idx += 1;
            }
        }

        self.count = write_idx;
        Ok(())
    }

    /// Sort index by key for better cache locality
    /// UNSAFE: Raw pointer manipulation
    pub unsafe fn sort(&mut self) {
        if self.count <= 1 {
            return;
        }

        // Simple insertion sort for now (good enough for ISAM)
        for i in 1..self.count {
            let current = *self.entries.add(i);
            let mut j = i;

            while j > 0 {
                let prev = *self.entries.add(j - 1);
                if prev.key <= current.key {
                    break;
                }
                *self.entries.add(j) = prev;
                j -= 1;
            }

            *self.entries.add(j) = current;
        }
    }

    /// Binary search for sorted index
    /// UNSAFE: Raw pointer arithmetic
    pub unsafe fn binary_search(&self, key: u64) -> Option<*const u8> {
        if self.count == 0 {
            return None;
        }

        let mut left = 0;
        let mut right = self.count - 1;

        while left <= right {
            let mid = (left + right) / 2;
            let entry = &*self.entries.add(mid);

            match entry.key.cmp(&key) {
                std::cmp::Ordering::Equal => {
                    let data_cursor = &*self.data_cursor;
                    return Some(data_cursor.data_base.add(entry.offset as usize));
                }
                std::cmp::Ordering::Less => left = mid + 1,
                std::cmp::Ordering::Greater => {
                    if mid == 0 { break; }
                    right = mid - 1;
                }
            }
        }

        None
    }

    /// Get current entry count
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get index capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Iterator over index entries
/// UNSAFE: Raw pointer iteration, no bounds checking
pub struct IndexIterator {
    entries: *const IndexEntry,
    current: usize,
    count: usize,
    data_cursor: *const MmapCursor,
}

impl Iterator for IndexIterator {
    type Item = (u64, *const u8); // (key, data_pointer)

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.count {
            return None;
        }

        unsafe {
            let entry = &*self.entries.add(self.current);
            let data_cursor = &*self.data_cursor;
            let data_ptr = data_cursor.data_base.add(entry.offset as usize);

            self.current += 1;
            Some((entry.key, data_ptr))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.current;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for IndexIterator {}

/// High-level interface combining data cursor and index
pub struct ISAMTable {
    data_cursor: MmapCursor,
    index_cursor: MmapCursor,
    index: ISAMIndex,
}

impl ISAMTable {
    /// Create new ISAM table with data and index files
    /// UNSAFE: File operations and raw pointer manipulation
    pub unsafe fn new(data_path: &str, index_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut data_cursor = MmapCursor::new(data_path, &format!("{}.tmp", data_path))?;
        let mut index_cursor = MmapCursor::new(index_path, &format!("{}.tmp", index_path))?;

        let index = ISAMIndex::new(&mut data_cursor, &mut index_cursor)?;

        Ok(Self {
            data_cursor,
            index_cursor,
            index,
        })
    }

    /// Initialize empty table with specific record size
    /// UNSAFE: Raw initialization
    pub unsafe fn init_empty<T>(data_path: &str, index_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut data_cursor = MmapCursor::new(data_path, &format!("{}.tmp", data_path))?;
        let mut index_cursor = MmapCursor::new(index_path, &format!("{}.tmp", index_path))?;

        data_cursor.init_header(mem::size_of::<T>())?;
        let index = ISAMIndex::init_empty(&mut data_cursor, &mut index_cursor)?;

        Ok(Self {
            data_cursor,
            index_cursor,
            index,
        })
    }

    /// Insert record with automatic indexing
    /// UNSAFE: Raw operations, idempotent tuple assumption
    pub unsafe fn insert<T>(&mut self, key: u64, record: &T) -> Result<(), Box<dyn std::error::Error>> {
        // Check if key already exists (idempotent update)
        if let Some(_existing) = self.index.get(key) {
            // Update existing record
            let header = self.data_cursor.header();
            let data_offset = header.data_offset + (key * header.record_size);
            let dest = self.data_cursor.data_base.add(data_offset as usize) as *mut T;
            ptr::copy_nonoverlapping(record, dest, 1);
        } else {
            // Append new record
            let record_index = self.data_cursor.append(record)
                .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, e)) as Box<dyn std::error::Error>)?;

            let header = self.data_cursor.header();
            let data_offset = header.data_offset + (record_index * header.record_size);

            self.index.put(key, data_offset)
                .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, e)) as Box<dyn std::error::Error>)?;
        }

        Ok(())
    }

    /// Get record by key
    /// UNSAFE: Raw pointer cast
    pub unsafe fn get<T>(&self, key: u64) -> Option<&T> {
        self.index.get(key).map(|ptr| &*(ptr as *const T))
    }

    /// Get mutable record by key
    /// UNSAFE: Raw pointer cast
    pub unsafe fn get_mut<T>(&mut self, key: u64) -> Option<&mut T> {
        self.index.get(key).map(|ptr| &mut *(ptr as *mut T))
    }

    /// Sync both data and index to disk
    pub fn sync(&mut self) -> Result<(), std::io::Error> {
        self.data_cursor.sync()?;
        self.index_cursor.sync()?;
        Ok(())
    }
}

// Make everything Send/Sync since we manage raw pointers
unsafe impl Send for ISAMIndex {}
unsafe impl Sync for ISAMIndex {}
unsafe impl Send for IndexIterator {}
unsafe impl Sync for IndexIterator {}
unsafe impl Send for ISAMTable {}
unsafe impl Sync for ISAMTable {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[repr(C, packed)]
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct TestRecord {
        id: u64,
        value: u64,
    }

    #[test]
    fn test_isam_index_basic() {
        let data_path = "/tmp/test_isam_data";
        let index_path = "/tmp/test_isam_index";

        // Clean up
        let _ = fs::remove_file(data_path);
        let _ = fs::remove_file(index_path);
        let _ = fs::remove_file(&format!("{}.tmp", data_path));
        let _ = fs::remove_file(&format!("{}.tmp", index_path));

        unsafe {
            let mut table = ISAMTable::init_empty::<TestRecord>(data_path, index_path).unwrap();

            // Insert records
            let record1 = TestRecord { id: 42, value: 1000 };
            let record2 = TestRecord { id: 84, value: 2000 };

            table.insert(42, &record1).unwrap();
            table.insert(84, &record2).unwrap();

            // Retrieve records
            let retrieved1: &TestRecord = table.get(42).unwrap();
            assert_eq!(*retrieved1, record1);

            let retrieved2: &TestRecord = table.get(84).unwrap();
            assert_eq!(*retrieved2, record2);

            // Test idempotent update
            let updated_record = TestRecord { id: 42, value: 1500 };
            table.insert(42, &updated_record).unwrap();

            let retrieved_updated: &TestRecord = table.get(42).unwrap();
            assert_eq!(retrieved_updated.value, 1500);

            // Sync to disk
            table.sync().unwrap();
        }

        // Clean up
        let _ = fs::remove_file(data_path);
        let _ = fs::remove_file(index_path);
        let _ = fs::remove_file(&format!("{}.tmp", data_path));
        let _ = fs::remove_file(&format!("{}.tmp", index_path));
    }

    #[test]
    fn test_index_binary_search() {
        let data_path = "/tmp/test_search_data";
        let index_path = "/tmp/test_search_index";

        let _ = fs::remove_file(data_path);
        let _ = fs::remove_file(index_path);
        let _ = fs::remove_file(&format!("{}.tmp", data_path));
        let _ = fs::remove_file(&format!("{}.tmp", index_path));

        unsafe {
            let mut table = ISAMTable::init_empty::<TestRecord>(data_path, index_path).unwrap();

            // Insert sorted records
            for i in 0..10 {
                let record = TestRecord { id: i * 10, value: i * 100 };
                table.insert(i * 10, &record).unwrap();
            }

            // Sort the index
            table.index.sort();

            // Test binary search
            let found: &TestRecord = table.get(50).unwrap();
            assert_eq!(found.value, 500);

            let not_found = table.get::<TestRecord>(55);
            assert!(not_found.is_none());
        }

        // Clean up
        let _ = fs::remove_file(data_path);
        let _ = fs::remove_file(index_path);
        let _ = fs::remove_file(&format!("{}.tmp", data_path));
        let _ = fs::remove_file(&format!("{}.tmp", index_path));
    }
}