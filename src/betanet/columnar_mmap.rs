//! Columnar data layout on top of mmap'd files.
//!
//! This is the Rust port of the concepts in the Kotlin `columnar` project,
//! specifically mirroring the logic from `RowMajor`, `Columnar`, and `FixedWidth`.
//! It uses raw pointers and `unsafe` to bypass the borrow checker and achieve
//! maximum performance, operating directly on kernel-managed memory pages.

use crate::mmap_cursor::MmapCursor;
use std::marker::PhantomData;
use std::mem;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::slice;

/// Represents the type of data in a column.
/// A direct port of `IOMemento` from the Kotlin project.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    Int32,
    Int64,
    Float32,
    Float64,
    Timestamp, // Nanosecond precision timestamp
}

impl ColumnType {
    pub fn size(&self) -> usize {
        match self {
            ColumnType::Int32 => mem::size_of::<i32>(),
            ColumnType::Int64 => mem::size_of::<i64>(),
            ColumnType::Float32 => mem::size_of::<f32>(),
            ColumnType::Float64 => mem::size_of::<f64>(),
            ColumnType::Timestamp => mem::size_of::<i64>(), // Stored as nanoseconds
        }
    }
}


/// Metadata for a single column.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ColumnMetadata {
    pub name: [u8; 32], // Fixed-size string for the name
    pub col_type: ColumnType,
}

/// Defines the schema for a table stored in a memory-mapped file.
/// This is analogous to the `.meta` file in the Kotlin ISAM implementation.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TableSchema {
    pub magic: [u8; 4], // e.g., "COLS"
    pub version: u32,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: [ColumnMetadata; 64], // Max 64 columns
}

impl TableSchema {
    pub fn get_row_size(&self) -> usize {
        self.columns.iter().take(self.column_count).map(|c| c.col_type.size()).sum()
    }
}


/// A handle to a memory-mapped columnar table.
/// Provides access to columns and rows without heap allocation.
pub struct MmapColumnarTable<'a> {
    cursor: MmapCursor,
    schema: &'a TableSchema,
    _marker: PhantomData<&'a ()>,
}

// Raw data pointer for a row
pub type RowBytes = *const [u8];

impl<'a> MmapColumnarTable<'a> {
    /// Creates and writes a new columnar table file.
    /// This is the Rust equivalent of `Cursor.writeISAM`.
    pub fn create(path: &str, schema: TableSchema, data: &[RowBytes]) -> Result<(), std::io::Error> {
        let mut file = OpenOptions::new().write(true).create(true).truncate(true).open(path)?;

        // Write schema
        let schema_bytes = unsafe {
            slice::from_raw_parts(
                &schema as *const _ as *const u8,
                mem::size_of::<TableSchema>(),
            )
        };
        file.write_all(schema_bytes)?;

        // Write data
        let row_size = schema.get_row_size();
        for &row_ptr in data.iter() {
            let row_slice = unsafe { slice::from_raw_parts(row_ptr as *const u8, row_size) };
            file.write_all(row_slice)?;
        }

        Ok(())
    }


    /// Opens a memory-mapped file and treats it as a columnar table.
    ///
    /// # Safety
    /// The file specified by `path` must be a valid columnar table file
    /// with a schema that matches the expected layout. The file must not
    /// be modified while the cursor is alive.
    pub unsafe fn open(path: &str) -> Result<Self, std::io::Error> {
        let cursor = MmapCursor::new(path)?;
        let schema = &*(cursor.as_ptr() as *const TableSchema);

        // Basic validation
        if &schema.magic != b"COLS" {
            // A real implementation would have better error handling
            panic!("Invalid file format");
        }

        Ok(Self {
            cursor,
            schema,
            _marker: PhantomData,
        })
    }

    #[inline]
    pub fn row_count(&self) -> usize {
        self.schema.row_count
    }

    #[inline]
    pub fn column_count(&self) -> usize {
        self.schema.column_count
    }

    /// Gets a pointer to the beginning of a specific column for a given row.
    ///
    /// # Safety
    /// `row_idx` and `col_idx` must be within bounds.
    #[inline]
    pub unsafe fn get_cell_ptr(&self, row_idx: usize, col_idx: usize) -> *const u8 {
        let row_size = self.schema.get_row_size();
        let mut col_offset = 0;
        for i in 0..col_idx {
            col_offset += self.schema.columns[i].col_type.size();
        }

        let offset = mem::size_of::<TableSchema>() + (row_idx * row_size) + col_offset;
        self.cursor.as_ptr().add(offset)
    }

    /// Reads a value from a specific cell.
    ///
    /// # Safety
    /// `row_idx` and `col_idx` must be within bounds. The type `T` must match
    /// the type stored in the column.
    #[inline]
    pub unsafe fn get_value<T: Copy>(&self, row_idx: usize, col_idx: usize) -> T {
        let ptr = self.get_cell_ptr(row_idx, col_idx) as *const T;
        *ptr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now_nanos() -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as i64
    }

    fn name_as_bytes(name: &str) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        bytes
    }

    /// This test is the Rust equivalent of `testIoInstant` in `ISAMCursorKtTest.kt`.
    #[test]
    fn test_write_read_single_timestamp() {
        let path = "/tmp/test_single_timestamp.db";
        let test_ts = now_nanos();

        // 1. Define Schema (like `scalars` in Kotlin)
        let schema = TableSchema {
            magic: *b"COLS",
            version: 1,
            row_count: 1,
            column_count: 1,
            columns: {
                let mut cols = [ColumnMetadata{ name: [0;32], col_type: ColumnType::Timestamp }; 64];
                cols[0] = ColumnMetadata { name: name_as_bytes("timestamp"), col_type: ColumnType::Timestamp };
                cols
            },
        };

        // 2. Prepare data (like `c0` cursor in Kotlin)
        let row_data = test_ts.to_le_bytes();
        let data_slice: &[RowBytes] = &[&row_data as *const _ as RowBytes];

        // 3. Write to file (like `c.writeISAM(fname)`)
        MmapColumnarTable::create(path, schema, data_slice).unwrap();

        // 4. Read back and verify
        unsafe {
            let table = MmapColumnarTable::open(path).unwrap();
            assert_eq!(table.row_count(), 1);
            assert_eq!(table.column_count(), 1);

            let ts_read: i64 = table.get_value(0, 0);
            assert_eq!(ts_read, test_ts);
        }
    }

    /// This test is the Rust equivalent of `testIoInstant3` and `testIoInstant4`
    /// in `ISAMCursorKtTest.kt`, testing mixed types.
    #[test]
    fn test_write_read_mixed_types() {
        let path = "/tmp/test_mixed_types.db";

        // 1. Define Schema
        let schema = TableSchema {
            magic: *b"COLS",
            version: 1,
            row_count: 2,
            column_count: 3,
            columns: {
                let mut cols = [ColumnMetadata{ name: [0;32], col_type: ColumnType::Timestamp }; 64];
                cols[0] = ColumnMetadata { name: name_as_bytes("id"), col_type: ColumnType::Int32 };
                cols[1] = ColumnMetadata { name: name_as_bytes("timestamp"), col_type: ColumnType::Timestamp };
                cols[2] = ColumnMetadata { name: name_as_bytes("value"), col_type: ColumnType::Float64 };
                cols
            },
        };

        // 2. Prepare data for two rows
        #[repr(C, packed)]
        struct Row {
            id: i32,
            ts: i64,
            val: f64,
        }
        let row1 = Row { id: 101, ts: now_nanos(), val: 99.9 };
        let row2 = Row { id: 102, ts: now_nanos(), val: -0.1 };

        let data_slice: &[RowBytes] = &[
            &row1 as *const _ as RowBytes,
            &row2 as *const _ as RowBytes,
        ];

        // 3. Write to file
        MmapColumnarTable::create(path, schema, data_slice).unwrap();

        // 4. Read back and verify
        unsafe {
            let table = MmapColumnarTable::open(path).unwrap();
            assert_eq!(table.row_count(), 2);
            assert_eq!(table.column_count(), 3);

            // Verify Row 1
            assert_eq!(table.get_value::<i32>(0, 0), row1.id);
            assert_eq!(table.get_value::<i64>(0, 1), row1.ts);
            assert_eq!(table.get_value::<f64>(0, 2), row1.val);

            // Verify Row 2
            assert_eq!(table.get_value::<i32>(1, 0), row2.id);
            assert_eq!(table.get_value::<i64>(1, 1), row2.ts);
            assert_eq!(table.get_value::<f64>(1, 2), row2.val);
        }
    }
}
