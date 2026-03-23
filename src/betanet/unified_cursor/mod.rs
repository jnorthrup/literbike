#![doc = "Unified ISAM Cursor API\n\nThis module provides a consolidated, high-performance cursor implementation \
combining ISAM indexing, direct mmap access, SIMD acceleration, and columnar support."]

pub mod isam_core;
pub mod simd_ops;
pub mod columnar;
pub mod columnar_integration;
pub mod mlir_bridge;

// Re-export core types for easier access
pub use isam_core::{UnifiedCursor, ISAMHeader, IsamIndex};
pub use simd_ops::SimdOps;
pub use columnar::DenseColumnStore;
pub use mlir_bridge::safe_accessors;
// columnar_integration is now internal, as its functionality is absorbed into UnifiedCursor

/// Create a UnifiedCursor from an existing MmapCursor
pub fn from_mmap_cursor(cursor: crate::mmap_cursor::MmapCursor) -> UnifiedCursor {
    cursor.into()
}