//! mmap-based cursor - TrikeShed port to Rust
//! 
//! Zero borrow checker, all raw pointers, direct kernel memory access
//! Idempotent tuples = no versioning complexity, just replace in place

use std::os::unix::io::RawFd;
use std::ptr::{self, NonNull};
use std::marker::PhantomData;
use memmap2::MmapMut;
use std::mem;

/// Raw pointer-based cursor over mmap'd data files
/// NO heap allocations, NO borrow checking, pure kernel memory
pub struct MmapCursor {
    /// Raw pointer to mmap'd data region
    data_base: *mut u8,
    /// Raw pointer to mmap'd index region  
    index_base: *mut u64,
    /// Total data length
    data_len: usize,
    /// Index entry count
    index_len: usize,
    /// File descriptor for data
    data_fd: RawFd,
    /// File descriptor for index
    index_fd: RawFd,
    /// Keep mmap alive (but don't use its API)
    _data_mmap: MmapMut,
    _index_mmap: MmapMut,
}

/// ISAM header for mmap'd files
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ISAMHeader {
    /// Magic number for format validation
    pub magic: u64,
    /// Total record count
    pub record_count: u64,
    /// Fixed record size in bytes
    pub record_size: u64,
    /// Index entry count
    pub index_count: u64,
    /// Offset to first data record
    pub data_offset: u64,
    /// Reserved for future use
    pub reserved: [u64; 3],
}

impl MmapCursor {
    /// Create new mmap cursor from file paths
    /// UNSAFE: Caller must ensure files exist and are valid ISAM format
    pub unsafe fn new(data_path: &str, index_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let data_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(data_path)?;
            
        let index_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(index_path)?;

        // Ensure minimum file sizes
        data_file.set_len(std::cmp::max(data_file.metadata()?.len(), mem::size_of::<ISAMHeader>() as u64))?;
        index_file.set_len(std::cmp::max(index_file.metadata()?.len(), 8 * 1024))?; // Min 8KB index

        let data_mmap = MmapMut::map_mut(&data_file)?;
        let index_mmap = MmapMut::map_mut(&index_file)?;

        let data_base = data_mmap.as_ptr() as *mut u8;
        let index_base = index_mmap.as_ptr() as *mut u64;
        let data_len = data_mmap.len();
        let index_len = index_mmap.len() / 8; // 8 bytes per u64

        use std::os::unix::io::AsRawFd;
        let data_fd = data_file.as_raw_fd();
        let index_fd = index_file.as_raw_fd();

        Ok(Self {
            data_base,
            index_base,
            data_len,
            index_len,
            data_fd,
            index_fd,
            _data_mmap: data_mmap,
            _index_mmap: index_mmap,
        })
    }

    /// Initialize ISAM header in data file
    /// UNSAFE: Raw pointer manipulation, no bounds checking
    pub unsafe fn init_header(&mut self, record_size: usize) -> Result<(), &'static str> {
        if self.data_len < mem::size_of::<ISAMHeader>() {
            return Err("Data file too small for ISAM header");
        }

        let header = self.data_base as *mut ISAMHeader;
        (*header) = ISAMHeader {
            magic: 0xDEADBEEF_CAFEBABE, // TrikeShed-style magic
            record_count: 0,
            record_size: record_size as u64,
            index_count: 0,
            data_offset: mem::size_of::<ISAMHeader>() as u64,
            reserved: [0; 3],
        };

        Ok(())
    }

    /// Get ISAM header
    /// UNSAFE: Raw pointer dereference, no validation
    pub unsafe fn header(&self) -> &ISAMHeader {
        &*(self.data_base as *const ISAMHeader)
    }

    /// Get mutable ISAM header
    /// UNSAFE: Raw pointer dereference, no validation
    pub unsafe fn header_mut(&mut self) -> &mut ISAMHeader {
        &mut *(self.data_base as *mut ISAMHeader)
    }

    /// Seek to record by index (zero-based)
    /// UNSAFE: No bounds checking, raw pointer arithmetic
    /// Returns raw pointer to record data
    pub unsafe fn seek(&self, index: u64) -> *const u8 {
        let header = self.header();
        if index >= header.record_count {
            return ptr::null();
        }

        // Direct offset calculation - no indirection
        let offset = header.data_offset + (index * header.record_size);
        self.data_base.add(offset as usize)
    }

    /// Seek to mutable record
    /// UNSAFE: No bounds checking, raw pointer arithmetic
    pub unsafe fn seek_mut(&mut self, index: u64) -> *mut u8 {
        let header = self.header();
        if index >= header.record_count {
            return ptr::null_mut();
        }

        let offset = header.data_offset + (index * header.record_size);
        self.data_base.add(offset as usize) as *mut u8
    }

    /// Append new record (idempotent tuple)
    /// UNSAFE: Raw pointer manipulation, no bounds checking
    /// Returns index of appended record
    pub unsafe fn append<T>(&mut self, record: &T) -> Result<u64, &'static str> {
        let header = self.header_mut();
        
        if mem::size_of::<T>() != header.record_size as usize {
            return Err("Record size mismatch");
        }

        let new_index = header.record_count;
        let record_offset = header.data_offset + (new_index * header.record_size);
        
        // Check if we have space
        if record_offset + header.record_size > self.data_len as u64 {
            return Err("Data file full");
        }

        // Copy record directly to mmap'd memory
        let dest = self.data_base.add(record_offset as usize) as *mut T;
        ptr::copy_nonoverlapping(record, dest, 1);

        // Update header
        header.record_count += 1;

        Ok(new_index)
    }

    /// Update record in place (leverages idempotent tuples)
    /// UNSAFE: Raw pointer manipulation
    pub unsafe fn update<T>(&mut self, index: u64, record: &T) -> Result<(), &'static str> {
        let dest = self.seek_mut(index);
        if dest.is_null() {
            return Err("Index out of bounds");
        }

        let header = self.header();
        if mem::size_of::<T>() != header.record_size as usize {
            return Err("Record size mismatch");
        }

        // Direct replacement - idempotent tuples make this safe
        ptr::copy_nonoverlapping(record, dest as *mut T, 1);
        Ok(())
    }

    /// Get record as typed reference
    /// UNSAFE: No type checking, raw pointer cast
    pub unsafe fn get<T>(&self, index: u64) -> Option<&T> {
        let ptr = self.seek(index);
        if ptr.is_null() {
            None
        } else {
            Some(&*(ptr as *const T))
        }
    }

    /// Get mutable record reference
    /// UNSAFE: No type checking, raw pointer cast
    pub unsafe fn get_mut<T>(&mut self, index: u64) -> Option<&mut T> {
        let ptr = self.seek_mut(index);
        if ptr.is_null() {
            None
        } else {
            Some(&mut *(ptr as *mut T))
        }
    }

    /// Scan all records sequentially (cache-optimal)
    /// UNSAFE: Raw pointer iteration
    pub unsafe fn scan<T>(&self) -> MmapRecordIter<T> {
        let header = self.header();
        MmapRecordIter {
            cursor: self,
            current: 0,
            count: header.record_count,
            _phantom: PhantomData,
        }
    }

    /// Force sync to disk (bypass kernel page cache)
    pub fn sync(&mut self) -> Result<(), std::io::Error> {
        use std::os::unix::io::FromRawFd;
        
        unsafe {
            let data_file = std::fs::File::from_raw_fd(self.data_fd);
            let index_file = std::fs::File::from_raw_fd(self.index_fd);
            
            data_file.sync_all()?;
            index_file.sync_all()?;
            
            // Don't close the files - we borrowed them
            std::mem::forget(data_file);
            std::mem::forget(index_file);
        }

        Ok(())
    }

    /// Compact data file (remove gaps, defragment)
    /// UNSAFE: Massive raw pointer manipulation
    pub unsafe fn compact(&mut self) -> Result<(), &'static str> {
        // For idempotent tuples, compaction is just removing nulls/deleted markers
        // Implementation depends on specific tuple format
        // This is a placeholder for the general pattern
        
        let header = self.header();
        let record_size = header.record_size as usize;
        let mut write_index = 0u64;
        
        // Sequential scan and compact
        for read_index in 0..header.record_count {
            let src = self.seek(read_index);
            if !src.is_null() && self.is_record_valid(src) {
                if read_index != write_index {
                    let dst = self.data_base.add(
                        (header.data_offset + write_index * header.record_size) as usize
                    );
                    ptr::copy_nonoverlapping(src, dst, record_size);
                }
                write_index += 1;
            }
        }
        
        // Update record count
        self.header_mut().record_count = write_index;
        
        Ok(())
    }

    /// Check if record is valid (not deleted/null)
    /// UNSAFE: Raw pointer dereference
    /// Override this for specific tuple formats
    unsafe fn is_record_valid(&self, ptr: *const u8) -> bool {
        // Default: check if first 8 bytes are non-zero
        // Real implementation depends on tuple schema
        let first_u64 = *(ptr as *const u64);
        first_u64 != 0
    }

    /// Get current record count
    pub fn len(&self) -> u64 {
        unsafe { self.header().record_count }
    }

    /// Check if cursor is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Iterator over mmap'd records
/// UNSAFE: No bounds checking, raw pointer arithmetic
pub struct MmapRecordIter<'a, T> {
    cursor: &'a MmapCursor,
    current: u64,
    count: u64,
    _phantom: PhantomData<T>,
}

impl<'a, T> Iterator for MmapRecordIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.count {
            return None;
        }

        unsafe {
            let ptr = self.cursor.seek(self.current);
            if ptr.is_null() {
                None
            } else {
                self.current += 1;
                Some(&*(ptr as *const T))
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.count - self.current) as usize;
        (remaining, Some(remaining))
    }
}

impl<'a, T> ExactSizeIterator for MmapRecordIter<'a, T> {}

// Make cursor Send/Sync - we're managing raw pointers manually
unsafe impl Send for MmapCursor {}
unsafe impl Sync for MmapCursor {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[repr(C, packed)]
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct TestRecord {
        id: u64,
        value: u64,
        flags: u32,
        _padding: u32,
    }

    #[test]
    fn test_mmap_cursor_basic() {
        let data_path = "/tmp/test_mmap_data.isam";
        let index_path = "/tmp/test_mmap_index.isam";
        
        // Clean up any existing files
        let _ = fs::remove_file(data_path);
        let _ = fs::remove_file(index_path);

        unsafe {
            let mut cursor = MmapCursor::new(data_path, index_path).unwrap();
            cursor.init_header(std::mem::size_of::<TestRecord>()).unwrap();

            // Test append
            let record1 = TestRecord { id: 1, value: 100, flags: 0, _padding: 0 };
            let index1 = cursor.append(&record1).unwrap();
            assert_eq!(index1, 0);

            let record2 = TestRecord { id: 2, value: 200, flags: 1, _padding: 0 };
            let index2 = cursor.append(&record2).unwrap();
            assert_eq!(index2, 1);

            // Test get
            let retrieved1: &TestRecord = cursor.get(0).unwrap();
            assert_eq!(*retrieved1, record1);

            let retrieved2: &TestRecord = cursor.get(1).unwrap();
            assert_eq!(*retrieved2, record2);

            // Test update (idempotent)
            let updated_record = TestRecord { id: 1, value: 150, flags: 2, _padding: 0 };
            cursor.update(0, &updated_record).unwrap();

            let retrieved_updated: &TestRecord = cursor.get(0).unwrap();
            assert_eq!(*retrieved_updated, updated_record);

            // Test iteration
            let records: Vec<TestRecord> = cursor.scan::<TestRecord>().copied().collect();
            assert_eq!(records.len(), 2);
            assert_eq!(records[0], updated_record);
            assert_eq!(records[1], record2);

            assert_eq!(cursor.len(), 2);
        }

        // Clean up
        let _ = fs::remove_file(data_path);
        let _ = fs::remove_file(index_path);
    }
}