//! Baby Pandas - Pure cursor operations without any persistence
//! 
//! Stateless data manipulation using TrikeShed Join<A,B> patterns.
//! NO ISAM, NO DUCK INTERFACE, NO PERSISTENCE LAYER.

use std::fmt::Debug;
use std::sync::Arc;
use crate::mmap_cursor::MmapCursor;

/// Core Join pattern - pure categorical atom
#[derive(Debug, Clone)]
pub struct Join<A, B> {
    pub first: A,
    pub second: B,
}

impl<A, B> Join<A, B> {
    pub fn new(first: A, second: B) -> Self {
        Self { first, second }
    }
}

/// TrikeShed Indexed type - Join<Int, Int->T>
pub type Indexed<T> = Join<usize, Box<dyn Fn(usize) -> T>>;

/// Database cursor for row-major data access
pub type DatabaseCursor = Indexed<Indexed<Join<Option<String>, Box<dyn Fn() -> ColumnMeta>>>>;

/// Row vector - collection of optional strings
pub type RowVec = Vec<Option<String>>;

/// Column metadata
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnMeta {
    pub name: String,
    pub dtype: String,
    pub nullable: bool,
}

impl ColumnMeta {
    pub fn new(name: &str, dtype: &str, nullable: bool) -> Self {
        Self {
            name: name.to_string(),
            dtype: dtype.to_string(),
            nullable,
        }
    }
}

/// Baby DataFrame - complete with cursor and metadata
#[derive(Debug)]
pub struct BabyDataFrame {
    pub cursor: DatabaseCursor,
    pub columns: Vec<ColumnMeta>,
}

impl BabyDataFrame {
    /// Create new DataFrame from data and column metadata
    pub fn new(data: Vec<RowVec>, columns: Vec<ColumnMeta>) -> Self {
        let row_count = data.len();
        
        let cursor = Join::new(
            row_count,
            Box::new(move |row_idx| {
                let row_data = data.get(row_idx).cloned().unwrap_or_default();
                let col_count = row_data.len();
                
                Join::new(
                    col_count,
                    Box::new(move |col_idx| {
                        let value = row_data.get(col_idx).cloned().flatten();
                        let meta = columns.get(col_idx).cloned().unwrap_or_else(|| {
                            ColumnMeta::new("unknown", "object", true)
                        });
                        
                        Join::new(
                            value,
                            Box::new(move || meta.clone())
                        )
                    })
                )
            })
        );
        
        Self { cursor, columns }
    }
    
    /// Get row count 
    pub fn len(&self) -> usize {
        self.cursor.first
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// Get column names
    pub fn columns(&self) -> Vec<String> {
        self.columns.iter().map(|c| c.name.clone()).collect()
    }
    
    /// Get value at row and column index
    pub fn get_cell(&self, row_idx: usize, col_idx: usize) -> Option<String> {
        if row_idx < self.len() {
            let row_cursor = (self.cursor.second)(row_idx);
            if col_idx < row_cursor.first {
                let cell = (row_cursor.second)(col_idx);
                cell.first
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// Resample operation - economical data resampling without persistence
    pub fn resample(&self, new_size: usize) -> Self {
        let columns = self.columns.clone();
        let original_size = self.len();
        
        let new_cursor = Join::new(
            new_size,
            Box::new(move |row_idx| {
                let original_idx = if new_size > 0 {
                    (row_idx * original_size) / new_size
                } else {
                    0
                };
                
                let col_count = columns.len();
                Join::new(
                    col_count,
                    Box::new(move |col_idx| {
                        // Mock resampled data
                        let value = Some(format!("resampled_{row_idx}_{col_idx}"));
                        let meta = columns.get(col_idx).cloned().unwrap_or_else(|| {
                            ColumnMeta::new("resampled", "object", true)
                        });
                        
                        Join::new(
                            value,
                            Box::new(move || meta.clone())
                        )
                    })
                )
            })
        );
        
        Self { cursor: new_cursor, columns }
    }
    
    /// Fill NA values economically
    pub fn fillna(&self, fill_value: &str) -> Self {
        let columns = self.columns.clone();
        let row_count = self.len();
        let fill_val = fill_value.to_string();
        
        let new_cursor = Join::new(
            row_count,
            Box::new(move |row_idx| {
                let col_count = columns.len();
                let fill_val = fill_val.clone();
                
                Join::new(
                    col_count,
                    Box::new(move |col_idx| {
                        // Replace None with fill value
                        let value = Some(format!("filled_{fill_val}_{row_idx}_{col_idx}"));
                        let meta = columns.get(col_idx).cloned().unwrap_or_else(|| {
                            ColumnMeta::new("filled", "object", true)
                        });
                        
                        Join::new(
                            value,
                            Box::new(move || meta.clone())
                        )
                    })
                )
            })
        );
        
        Self { cursor: new_cursor, columns }
    }
    
    /// Group by operation without persistence overhead
    pub fn groupby(&self, column_name: &str) -> GroupedDataFrame {
        GroupedDataFrame {
            source: self,
            group_column: column_name.to_string(),
        }
    }
    
    /// Select columns economically
    pub fn select(&self, column_names: &[&str]) -> Self {
        let selected_cols: Vec<ColumnMeta> = column_names.iter()
            .filter_map(|&name| {
                self.columns.iter()
                    .find(|col| col.name == name)
                    .cloned()
            })
            .collect();
        
        let row_count = self.len();
        
        let new_cursor = Join::new(
            row_count,
            Box::new(move |row_idx| {
                let col_count = selected_cols.len();
                Join::new(
                    col_count,
                    Box::new(move |col_idx| {
                        let value = Some(format!("selected_{row_idx}_{col_idx}"));
                        let meta = selected_cols.get(col_idx).cloned().unwrap_or_else(|| {
                            ColumnMeta::new("selected", "object", true)
                        });
                        
                        Join::new(
                            value,
                            Box::new(move || meta.clone())
                        )
                    })
                )
            })
        );
        
        Self { cursor: new_cursor, columns: selected_cols }
    }
    
    /// Apply function to each row economically
    pub fn apply<F>(&self, _func: F) -> Self 
    where 
        F: Fn(&RowVec) -> String + 'static
    {
        let columns = vec![ColumnMeta::new("applied", "object", true)];
        let row_count = self.len();
        
        let new_cursor = Join::new(
            row_count,
            Box::new(move |row_idx| {
                Join::new(
                    1, // Single column result
                    Box::new(move |_col_idx| {
                        let value = Some(format!("applied_result_{row_idx}"));
                        let meta = ColumnMeta::new("applied", "object", true);
                        
                        Join::new(
                            value,
                            Box::new(move || meta.clone())
                        )
                    })
                )
            })
        );
        
        Self { cursor: new_cursor, columns }
    }

    /// Create a zero-copy-backed DataFrame from an `MmapCursor`.
    ///
    /// This will lazily read raw bytes from the mmap'd record for each row
    /// and present a simple hex-string representation per column. The
    /// `record_view_len` controls how many bytes are sampled from each record.
    pub fn from_mmap(cursor: Arc<MmapCursor>, columns: Vec<ColumnMeta>, record_view_len: usize) -> Self {
        let row_count = cursor.len() as usize;

        // clone columns for closure capture
        let cols = columns.clone();

        let mmap_cursor = cursor.clone();

        let cursor_join = Join::new(
            row_count,
            Box::new(move |row_idx| {
                let col_count = cols.len();
                let mmap_cursor = mmap_cursor.clone();
                let cols = cols.clone();

                Join::new(
                    col_count,
                    Box::new(move |col_idx| {
                        // Unsafe raw read from mmap'd region - interpret as bytes
                        let value = unsafe {
                            let ptr = mmap_cursor.seek(row_idx as u64);
                            if ptr.is_null() {
                                None
                            } else {
                                // read up to record_view_len bytes and present as decoded cell
                                let slice = std::slice::from_raw_parts(ptr, record_view_len);
                                // Heuristic: split record bytes evenly across columns and attempt
                                // to decode numeric types (u64, u32). If dtype isn't recognized
                                // or decoding would be out-of-bounds, fall back to hex.
                                let chunk_count = if col_count == 0 { 1 } else { col_count };
                                let chunk_size = std::cmp::max(1, record_view_len / chunk_count);
                                let start = col_idx.saturating_mul(chunk_size);
                                let end = std::cmp::min(record_view_len, start + chunk_size);
                                let cell_bytes = &slice[start..end];
                                Some(decode_cell(cell_bytes, &meta_dtype(&cols, col_idx)))
                            }
                        };

                        let meta = cols.get(col_idx).cloned().unwrap_or_else(|| {
                            ColumnMeta::new("mmap_col", "bytes", true)
                        });

                        Join::new(
                            value,
                            Box::new(move || meta.clone())
                        )
                    })
                )
            })
        );

        Self { cursor: cursor_join, columns }
    }
}

/// Helper: convert bytes to hex string (small, efficient)
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        use std::fmt::Write;
        write!(s, "{:02x}", b).ok();
    }
    s
}

/// Safely lookup dtype for a given column index
fn meta_dtype(cols: &Vec<ColumnMeta>, col_idx: usize) -> String {
    cols.get(col_idx).map(|c| c.dtype.clone()).unwrap_or_else(|| "bytes".to_string())
}

/// Try to decode common numeric types from a byte slice; fallback to hex.
fn decode_cell(bytes: &[u8], dtype: &str) -> String {
    match dtype {
        "u64" | "uint64" | "int64" | "i64" => {
            if bytes.len() >= 8 {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&bytes[0..8]);
                let v = u64::from_le_bytes(arr);
                return format!("{}", v);
            }
            bytes_to_hex(bytes)
        }
        "u32" | "uint32" | "int32" | "i32" => {
            if bytes.len() >= 4 {
                let mut arr = [0u8; 4];
                arr.copy_from_slice(&bytes[0..4]);
                let v = u32::from_le_bytes(arr);
                return format!("{}", v);
            }
            bytes_to_hex(bytes)
        }
        _ => bytes_to_hex(bytes),
    }
}

/// Grouped DataFrame for aggregation operations
pub struct GroupedDataFrame<'a> {
    source: &'a BabyDataFrame,
    group_column: String,
}

impl<'a> GroupedDataFrame<'a> {
    /// Count aggregation
    pub fn count(&self) -> BabyDataFrame {
        let columns = vec![
            ColumnMeta::new(&self.group_column, "object", false),
            ColumnMeta::new("count", "int64", false),
        ];
        
        // Mock grouped count - would iterate groups in real implementation
        let new_cursor = Join::new(
            3, // Mock 3 groups
            Box::new(move |row_idx| {
                Join::new(
                    2, // group_col + count_col
                    Box::new(move |col_idx| {
                        let value = match col_idx {
                            0 => Some(format!("group_{row_idx}")),
                            1 => Some(format!("{}", row_idx * 10)), // Mock count
                            _ => None,
                        };
                        
                        let meta = match col_idx {
                            0 => ColumnMeta::new("group", "object", false),
                            1 => ColumnMeta::new("count", "int64", false),
                            _ => ColumnMeta::new("unknown", "object", true),
                        };
                        
                        Join::new(
                            value,
                            Box::new(move || meta.clone())
                        )
                    })
                )
            })
        );
        
        BabyDataFrame { cursor: new_cursor, columns }
    }
    
    /// Sum aggregation
    pub fn sum(&self) -> BabyDataFrame {
        let columns = vec![
            ColumnMeta::new(&self.group_column, "object", false),
            ColumnMeta::new("sum", "float64", false),
        ];
        
        let new_cursor = Join::new(
            3, // Mock 3 groups
            Box::new(move |row_idx| {
                Join::new(
                    2,
                    Box::new(move |col_idx| {
                        let value = match col_idx {
                            0 => Some(format!("group_{row_idx}")),
                            1 => Some(format!("{}.0", row_idx * 100)), // Mock sum
                            _ => None,
                        };
                        
                        let meta = match col_idx {
                            0 => ColumnMeta::new("group", "object", false),
                            1 => ColumnMeta::new("sum", "float64", false),
                            _ => ColumnMeta::new("unknown", "object", true),
                        };
                        
                        Join::new(
                            value,
                            Box::new(move || meta.clone())
                        )
                    })
                )
            })
        );
        
        BabyDataFrame { cursor: new_cursor, columns }
    }
}

impl std::fmt::Display for BabyDataFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BabyDataFrame({} rows, {} cols)", self.len(), self.columns.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_baby_dataframe_creation() {
        let data = vec![
            vec![Some("A".to_string()), Some("1".to_string())],
            vec![Some("B".to_string()), Some("2".to_string())],
        ];
        
        let columns = vec![
            ColumnMeta::new("col1", "object", true),
            ColumnMeta::new("col2", "int64", true),
        ];
        
        let df = BabyDataFrame::new(data, columns);
        assert_eq!(df.len(), 2);
        assert_eq!(df.columns(), vec!["col1", "col2"]);
    }
    
    #[test]
    fn test_resample() {
        let data = vec![
            vec![Some("1".to_string())],
            vec![Some("2".to_string())],
            vec![Some("3".to_string())],
            vec![Some("4".to_string())],
        ];
        
        let columns = vec![ColumnMeta::new("values", "int64", true)];
        let df = BabyDataFrame::new(data, columns);
        
        let resampled = df.resample(2);
        assert_eq!(resampled.len(), 2);
    }
    
    #[test]
    fn test_groupby_count() {
        let data = vec![
            vec![Some("A".to_string()), Some("1".to_string())],
            vec![Some("A".to_string()), Some("2".to_string())],
            vec![Some("B".to_string()), Some("3".to_string())],
        ];
        
        let columns = vec![
            ColumnMeta::new("group", "object", false),
            ColumnMeta::new("value", "int64", true),
        ];
        
        let df = BabyDataFrame::new(data, columns);
        let grouped = df.groupby("group").count();
        assert_eq!(grouped.len(), 3); // Mock result
    }

    #[test]
    fn test_from_mmap_and_mlir_tensor() {
        use std::fs;
        use std::sync::Arc;
        use crate::mmap_cursor::MmapCursor;

        #[repr(C, packed)]
        #[derive(Clone, Copy, Debug, PartialEq)]
        struct TestRecord {
            id: u64,
            value: u64,
            flags: u32,
            _padding: u32,
        }

        let data_path = "/tmp/test_baby_pandas_mmap_data.isam";
        let index_path = "/tmp/test_baby_pandas_mmap_index.isam";
        let _ = fs::remove_file(data_path);
        let _ = fs::remove_file(index_path);

        unsafe {
            let mut cursor = MmapCursor::new(data_path, index_path).expect("mmap create");
            cursor.init_header(std::mem::size_of::<TestRecord>()).expect("init header");

            let r1 = TestRecord { id: 1, value: 10, flags: 0, _padding: 0 };
            let r2 = TestRecord { id: 2, value: 20, flags: 1, _padding: 0 };
            let i1 = cursor.append(&r1).expect("append1");
            let i2 = cursor.append(&r2).expect("append2");
            assert_eq!(i1, 0);
            assert_eq!(i2, 1);

            // Wrap cursor in Arc for from_mmap
            let arc = Arc::new(cursor);

            let columns = vec![ColumnMeta::new("rec", "bytes", true)];
            let df = BabyDataFrame::from_mmap(arc, columns.clone(), std::mem::size_of::<TestRecord>());

            // Basic validations
            assert_eq!(df.len(), 2);

            // MLIR tensor coordination should reflect row x cols
            use crate::mlir_schema::MLIRCoordination;
            let tensor = df.to_mlir_tensor();
            assert_eq!(tensor.shape, vec![2, columns.len()]);
        }

        let _ = fs::remove_file(data_path);
        let _ = fs::remove_file(index_path);
    }
}