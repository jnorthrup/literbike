pub mod oroboros_slsa;
pub mod mmap_cursor;
pub mod isam_index;
pub mod columnar_mmap;
pub mod adaptive_typing;

//! Baby Pandas - Economical DataFrame library
//! Phase: MLIR densification and cursor pattern integration
//! - Current: MLIR schema coordination, zero-allocation sequences, cursor_ops
//! - TODO: Expand MLIR tensor ops, add SIMD cursor patterns, document schema evolution
//! - TODO: Integrate with HTX protocol for streaming dataframes
//! - TODO: Add property-based tests for cursor edge cases

pub mod baby_pandas;
pub mod cursor_ops;
pub mod mmap_dataframe;
pub mod indexed;
pub mod idempotent_tuples;
pub mod densifier;
pub mod tensor_core;
pub mod anchor;
pub mod capabilities;
pub mod simd_match;
pub mod mlir_mock;
pub mod detector_pipeline;
pub mod detector_pipeline;

// High-performance ISAM implementation
pub mod isam_format;
pub mod dayjobtest;

pub use baby_pandas::*;
pub use cursor_ops::*;

/// MLIR schema coordination for baby pandas operations
pub mod mlir_schema {
    use crate::{BabyDataFrame, ColumnMeta};
    
    /// MLIR tensor coordination types
    #[derive(Debug, Clone)]
    pub struct MLIRTensor {
        pub shape: Vec<usize>,
        pub dtype: String,
        pub strides: Vec<usize>,
    }
    
    impl MLIRTensor {
        pub fn from_dataframe(df: &BabyDataFrame) -> Self {
            Self {
                shape: vec![df.len(), df.columns().len()],
                dtype: "f64".to_string(),
                strides: vec![df.columns().len(), 1],
            }
        }
    }
    
    /// Coordinate with MLIR compilation pipeline
    pub trait MLIRCoordination {
        fn to_mlir_tensor(&self) -> MLIRTensor;
        fn optimize_for_mlir(&self) -> Self;
    }
    
    impl MLIRCoordination for BabyDataFrame {
        fn to_mlir_tensor(&self) -> MLIRTensor {
            MLIRTensor::from_dataframe(self)
        }
        
        fn optimize_for_mlir(&self) -> Self {
            // Optimize data layout for MLIR compilation
            let columns = vec![ColumnMeta::new("mlir_optimized", "f64", false)];
            let row_count = self.len();
            
            let cursor = crate::Join::new(
                row_count,
                Box::new(move |row_idx| {
                    crate::Join::new(
                        1,
                        Box::new(move |_col_idx| {
                            let value = Some(format!("mlir_opt_{row_idx}"));
                            let meta = ColumnMeta::new("mlir_optimized", "f64", false);
                            
                            crate::Join::new(
                                value,
                                Box::new(move || meta.clone())
                            )
                        })
                    )
                })
            );
            
            Self { cursor, columns }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlir_schema::*;
    
    #[test]
    fn test_mlir_coordination() {
        let data = vec![
            vec![Some("1.0".to_string()), Some("2.0".to_string())],
            vec![Some("3.0".to_string()), Some("4.0".to_string())],
        ];
        
        let columns = vec![
            ColumnMeta::new("x", "f64", false),
            ColumnMeta::new("y", "f64", false),
        ];
        
        let df = BabyDataFrame::new(data, columns);
        let tensor = df.to_mlir_tensor();
        
        assert_eq!(tensor.shape, vec![2, 2]);
        assert_eq!(tensor.dtype, "f64");
        
        let optimized = df.optimize_for_mlir();
        assert_eq!(optimized.len(), 2);
    }
    
        #[test]
        fn test_hello_world() {
            println!("hello world");
            assert_eq!(2 + 2, 4);
        }
}
