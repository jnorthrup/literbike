//! Baby Pandas main entrypoint with MLIR schema coordination
//! Phase: MLIR densification and DataFrame orchestration
//! - Current: MLIR schema coordination, DataFrame creation, cursor pattern usage
//! - TODO: Add streaming DataFrame support, integrate with HTX protocol
//! - TODO: Add CLI for batch DataFrame ops
//! - TODO: Document orchestration flow and error handling

use betanet::{BabyDataFrame, ColumnMeta, CursorOps, WindowOps};
use betanet::mlir_schema::{MLIRCoordination, MLIRTensor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🐼 Baby Pandas - Economical DataFrames with MLIR Schema Coordination");
    
    // Create sample data without ISAM overhead
    let data = vec![
        vec![Some("1.5".to_string()), Some("A".to_string()), Some("100".to_string())],
        vec![Some("2.3".to_string()), Some("B".to_string()), Some("200".to_string())],
        vec![Some("3.1".to_string()), Some("A".to_string()), Some("150".to_string())],
        vec![Some("4.7".to_string()), Some("C".to_string()), Some("300".to_string())],
        vec![Some("5.2".to_string()), Some("B".to_string()), Some("250".to_string())],
    ];
    
    let columns = vec![
        ColumnMeta::new("value", "f64", false),
        ColumnMeta::new("category", "object", false),
        ColumnMeta::new("amount", "int64", false),
    ];
    
    let df = BabyDataFrame::new(data, columns);
    println!("Created DataFrame: {:?}", df);
    println!("Columns: {:?}", df.columns());
    
    // Demonstrate cursor operations without ISAM persistence
    println!("\n🔄 Cursor Operations (No ISAM):");
    
    let filtered = df.filter(|i| i % 2 == 0);
    println!("Filtered (even indices): {:?}", filtered);
    
    let taken = df.take(3);
    println!("Take first 3: {:?}", taken);
    
    let resampled = df.resample(3);
    println!("Resampled to 3 rows: {:?}", resampled);
    
    // Window operations
    println!("\n📊 Window Operations:");
    let rolling_sum = WindowOps::rolling_sum(&df, 3);
    println!("Rolling sum (window=3): {:?}", rolling_sum);
    
    let rolling_mean = WindowOps::rolling_mean(&df, 2);
    println!("Rolling mean (window=2): {:?}", rolling_mean);
    
    // Groupby operations  
    println!("\n📈 Group Operations:");
    let grouped_count = df.groupby("category").count();
    println!("Group by category (count): {:?}", grouped_count);
    
    let grouped_sum = df.groupby("category").sum();
    println!("Group by category (sum): {:?}", grouped_sum);
    
    // MLIR schema coordination
    println!("\n🏗️  MLIR Schema Coordination:");
    let tensor = df.to_mlir_tensor();
    println!("MLIR Tensor: {:?}", tensor);
    
    let optimized = df.optimize_for_mlir();
    println!("MLIR Optimized: {:?}", optimized);
    
    // Chain operations economically
    println!("\n⛓️  Chained Operations:");
    let result = df
        .filter(|i| i < 4)
        .take(2)
        .fillna("missing")
        .select(&["value", "category"]);
    println!("Chained result: {:?}", result);
    
    // Lazy evaluation demonstration
    println!("\n💤 Lazy Evaluation:");
    let lazy_mapped = df.lazy_map(|i| format!("processed_{}", i));
    println!("Lazy mapped: {:?}", lazy_mapped);
    
    println!("\n✅ Baby Pandas demo completed successfully!");
    println!("🚀 All operations used zero-allocation Trikeshed cursor patterns");
    println!("🔧 MLIR schema coordination active for compilation optimization");
    
    Ok(())
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[test]
    fn test_full_pipeline() {
        let data = vec![
            vec![Some("1.0".to_string()), Some("X".to_string())],
            vec![Some("2.0".to_string()), Some("Y".to_string())],
            vec![Some("3.0".to_string()), Some("X".to_string())],
        ];
        
        let columns = vec![
            ColumnMeta::new("val", "f64", false),
            ColumnMeta::new("cat", "object", false),
        ];
        
        let df = BabyDataFrame::new(data, columns);
        
        // Test chained operations
        let result = df
            .filter(|i| i < 3)
            .take(2)
            .fillna("default");
        
        assert_eq!(result.len(), 2);
        
        // Test MLIR coordination
        let tensor = df.to_mlir_tensor();
        assert_eq!(tensor.shape, vec![3, 2]);
        
        let optimized = df.optimize_for_mlir();
        assert_eq!(optimized.len(), 3);
    }
    
    #[test]
    fn test_window_operations() {
        let data = vec![
            vec![Some("1".to_string())],
            vec![Some("2".to_string())],
            vec![Some("3".to_string())],
            vec![Some("4".to_string())],
        ];
        
        let columns = vec![ColumnMeta::new("values", "int64", true)];
        let df = BabyDataFrame::new(data, columns);
        
        let rolling = WindowOps::rolling_sum(&df, 3);
        assert_eq!(rolling.len(), 2);
        
        let rolling_mean = WindowOps::rolling_mean(&df, 2);
        assert_eq!(rolling_mean.len(), 3);
    }
}