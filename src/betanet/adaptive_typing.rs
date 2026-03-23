//! Adaptive typing system - IoMemento + Evidence counters
//! 
//! Port of TrikeShed's adaptive typing with statistical evidence
//! for automatic type inference and optimization

use std::collections::HashMap;
use std::any::TypeId;
use std::mem;
use serde::{Serialize, Deserialize};

/// IoMemento - runtime type representation with evidence tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IoMemento {
    IoString,
    IoInt,
    IoLong,
    IoFloat,
    IoDouble,
    IoBoolean,
    IoInstant,
    IoLocalDate,
    IoBytes,
    IoUuid,
}

impl IoMemento {
    /// Network size for serialization (None = variable length)
    pub fn network_size(&self) -> Option<usize> {
        match self {
            IoMemento::IoString => None,  // Variable length
            IoMemento::IoInt => Some(4),
            IoMemento::IoLong => Some(8), 
            IoMemento::IoFloat => Some(4),
            IoMemento::IoDouble => Some(8),
            IoMemento::IoBoolean => Some(1),
            IoMemento::IoInstant => Some(8),  // Unix timestamp
            IoMemento::IoLocalDate => Some(4), // Days since epoch
            IoMemento::IoBytes => None,   // Variable length
            IoMemento::IoUuid => Some(16),
        }
    }

    /// Rust TypeId mapping
    pub fn type_id(&self) -> TypeId {
        match self {
            IoMemento::IoString => TypeId::of::<String>(),
            IoMemento::IoInt => TypeId::of::<i32>(),
            IoMemento::IoLong => TypeId::of::<i64>(),
            IoMemento::IoFloat => TypeId::of::<f32>(),
            IoMemento::IoDouble => TypeId::of::<f64>(),
            IoMemento::IoBoolean => TypeId::of::<bool>(),
            IoMemento::IoInstant => TypeId::of::<u64>(), // Unix timestamp as u64
            IoMemento::IoLocalDate => TypeId::of::<u32>(), // Days as u32
            IoMemento::IoBytes => TypeId::of::<Vec<u8>>(),
            IoMemento::IoUuid => TypeId::of::<[u8; 16]>(),
        }
    }

    /// SIMD-friendly types for autovec optimization
    pub fn is_simd_friendly(&self) -> bool {
        matches!(self, 
            IoMemento::IoInt | 
            IoMemento::IoLong | 
            IoMemento::IoFloat | 
            IoMemento::IoDouble
        )
    }

    /// Check if type supports direct mmap casting
    pub fn is_mmap_safe(&self) -> bool {
        // Fixed-size types can be cast directly from mmap'd memory
        self.network_size().is_some()
    }
}

/// Evidence counter for adaptive type inference
#[derive(Debug, Clone)]
pub struct Evidence {
    /// Type evidence counters
    type_counts: HashMap<IoMemento, u64>,
    /// Total observations
    total_observations: u64,
    /// Confidence threshold for type promotion
    confidence_threshold: f64,
    /// Current best guess
    current_type: Option<IoMemento>,
    /// Type promotion history
    promotion_history: Vec<(IoMemento, IoMemento, u64)>, // from, to, observation_count
}

impl Evidence {
    /// Create new evidence tracker
    pub fn new() -> Self {
        Self {
            type_counts: HashMap::new(),
            total_observations: 0,
            confidence_threshold: 0.8, // 80% confidence
            current_type: None,
            promotion_history: Vec::new(),
        }
    }

    /// Create with custom confidence threshold
    pub fn with_threshold(threshold: f64) -> Self {
        Self {
            confidence_threshold: threshold,
            ..Self::new()
        }
    }

    /// Add evidence for a specific type
    pub fn add_evidence(&mut self, memento: IoMemento) -> bool {
        *self.type_counts.entry(memento).or_insert(0) += 1;
        self.total_observations += 1;

        let previous_type = self.current_type;
        self.update_current_type();

        // Check if type was promoted
        if let (Some(old), Some(new)) = (previous_type, self.current_type) {
            if old != new {
                self.promotion_history.push((old, new, self.total_observations));
                return true; // Type was promoted
            }
        }

        false
    }

    /// Get current best type with confidence
    pub fn current_type_with_confidence(&self) -> Option<(IoMemento, f64)> {
        self.current_type.map(|t| {
            let count = *self.type_counts.get(&t).unwrap_or(&0);
            let confidence = count as f64 / self.total_observations as f64;
            (t, confidence)
        })
    }

    /// Check if we have sufficient evidence for the current type
    pub fn is_confident(&self) -> bool {
        if let Some((_, confidence)) = self.current_type_with_confidence() {
            confidence >= self.confidence_threshold
        } else {
            false
        }
    }

    /// Get type distribution for debugging
    pub fn type_distribution(&self) -> Vec<(IoMemento, f64)> {
        let mut dist: Vec<_> = self.type_counts.iter()
            .map(|(&memento, &count)| {
                let percentage = count as f64 / self.total_observations as f64;
                (memento, percentage)
            })
            .collect();
        
        dist.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        dist
    }

    /// Update current type based on evidence
    fn update_current_type(&mut self) {
        if self.total_observations == 0 {
            self.current_type = None;
            return;
        }

        // Find type with highest count
        let best_type = self.type_counts.iter()
            .max_by_key(|(_, count)| *count)
            .map(|(memento, _)| *memento);

        self.current_type = best_type;
    }

    /// Suggest optimal SIMD strategy based on evidence
    pub fn simd_strategy(&self) -> SIMDStrategy {
        if let Some((memento, confidence)) = self.current_type_with_confidence() {
            if confidence >= self.confidence_threshold && memento.is_simd_friendly() {
                match memento {
                    IoMemento::IoInt => SIMDStrategy::AVX2_I32,
                    IoMemento::IoLong => SIMDStrategy::AVX2_I64,
                    IoMemento::IoFloat => SIMDStrategy::AVX2_F32,
                    IoMemento::IoDouble => SIMDStrategy::AVX2_F64,
                    _ => SIMDStrategy::Scalar,
                }
            } else {
                SIMDStrategy::Scalar
            }
        } else {
            SIMDStrategy::Adaptive // Keep collecting evidence
        }
    }

    /// Reset evidence (useful for schema evolution)
    pub fn reset(&mut self) {
        self.type_counts.clear();
        self.total_observations = 0;
        self.current_type = None;
        // Keep promotion history for analysis
    }
}

/// SIMD strategy based on evidence
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SIMDStrategy {
    Scalar,      // Use scalar operations
    AVX2_I32,    // 8x i32 operations  
    AVX2_I64,    // 4x i64 operations
    AVX2_F32,    // 8x f32 operations
    AVX2_F64,    // 4x f64 operations
    Adaptive,    // Keep collecting evidence
}

impl SIMDStrategy {
    /// Get register width for this strategy
    pub fn register_width(&self) -> usize {
        match self {
            SIMDStrategy::Scalar => 1,
            SIMDStrategy::AVX2_I32 | SIMDStrategy::AVX2_F32 => 8,
            SIMDStrategy::AVX2_I64 | SIMDStrategy::AVX2_F64 => 4,
            SIMDStrategy::Adaptive => 1,
        }
    }

    /// Get element size in bytes
    pub fn element_size(&self) -> usize {
        match self {
            SIMDStrategy::Scalar | SIMDStrategy::Adaptive => 8, // Assume worst case
            SIMDStrategy::AVX2_I32 | SIMDStrategy::AVX2_F32 => 4,
            SIMDStrategy::AVX2_I64 | SIMDStrategy::AVX2_F64 => 8,
        }
    }
}

/// Adaptive column that evolves based on evidence
pub struct AdaptiveColumn {
    /// Current evidence about this column's type
    evidence: Evidence,
    /// Raw data buffer (reinterpreted based on current type)
    raw_data: Vec<u8>,
    /// Number of values stored
    value_count: usize,
    /// Current storage strategy
    storage_strategy: StorageStrategy,
}

/// Storage strategy for adaptive columns
#[derive(Debug, Clone, Copy)]
enum StorageStrategy {
    /// Store as variant enum (flexible but slower)
    Variant,
    /// Store as fixed-size records (fast but requires known type)
    FixedSize(usize),
    /// Store with SIMD optimization
    SIMDOptimized(SIMDStrategy),
}

impl AdaptiveColumn {
    /// Create new adaptive column
    pub fn new() -> Self {
        Self {
            evidence: Evidence::new(),
            raw_data: Vec::new(),
            value_count: 0,
            storage_strategy: StorageStrategy::Variant,
        }
    }

    /// Add value and update evidence
    pub fn push_value(&mut self, value: &dyn std::any::Any) -> Result<(), &'static str> {
        // Infer type from Any (simplified - would need proper type detection)
        let memento = self.infer_memento_from_any(value)?;
        
        // Add evidence and check for type promotion
        let promoted = self.evidence.add_evidence(memento);
        
        if promoted {
            self.update_storage_strategy();
        }

        // Store the value (implementation depends on current strategy)
        self.store_value(value, memento)?;
        self.value_count += 1;

        Ok(())
    }

    /// Get SIMD-optimized access to values
    /// UNSAFE: Returns raw pointers for SIMD operations
    pub unsafe fn simd_data(&self) -> Option<(*const u8, usize, SIMDStrategy)> {
        if let StorageStrategy::SIMDOptimized(strategy) = self.storage_strategy {
            Some((
                self.raw_data.as_ptr(),
                self.value_count,
                strategy,
            ))
        } else {
            None
        }
    }

    /// Check if column is ready for SIMD processing
    pub fn is_simd_ready(&self) -> bool {
        matches!(self.storage_strategy, StorageStrategy::SIMDOptimized(_)) &&
        self.evidence.is_confident()
    }

    /// Get current type evidence
    pub fn type_evidence(&self) -> &Evidence {
        &self.evidence
    }

    /// Force type promotion (useful for schema migration)
    pub fn promote_to_type(&mut self, target: IoMemento) -> Result<(), &'static str> {
        if !self.can_promote_to(target) {
            return Err("Cannot promote to target type");
        }

        // Convert existing data to new type
        self.convert_storage_to_type(target)?;
        
        // Update evidence to strongly favor new type
        for _ in 0..100 {
            self.evidence.add_evidence(target);
        }

        self.update_storage_strategy();
        Ok(())
    }

    // Private implementation methods

    fn infer_memento_from_any(&self, value: &dyn std::any::Any) -> Result<IoMemento, &'static str> {
        // This is a simplified version - real implementation would use TypeId matching
        let type_id = value.type_id();
        
        if type_id == TypeId::of::<String>() {
            Ok(IoMemento::IoString)
        } else if type_id == TypeId::of::<i32>() {
            Ok(IoMemento::IoInt)
        } else if type_id == TypeId::of::<i64>() {
            Ok(IoMemento::IoLong)
        } else if type_id == TypeId::of::<f32>() {
            Ok(IoMemento::IoFloat)
        } else if type_id == TypeId::of::<f64>() {
            Ok(IoMemento::IoDouble)
        } else if type_id == TypeId::of::<bool>() {
            Ok(IoMemento::IoBoolean)
        } else {
            Err("Unsupported type")
        }
    }

    fn update_storage_strategy(&mut self) {
        let simd_strategy = self.evidence.simd_strategy();
        
        self.storage_strategy = match simd_strategy {
            SIMDStrategy::Scalar => {
                if let Some((memento, _)) = self.evidence.current_type_with_confidence() {
                    if let Some(size) = memento.network_size() {
                        StorageStrategy::FixedSize(size)
                    } else {
                        StorageStrategy::Variant
                    }
                } else {
                    StorageStrategy::Variant
                }
            },
            SIMDStrategy::Adaptive => StorageStrategy::Variant,
            strategy => StorageStrategy::SIMDOptimized(strategy),
        };
    }

    fn store_value(&mut self, value: &dyn std::any::Any, memento: IoMemento) -> Result<(), &'static str> {
        match self.storage_strategy {
            StorageStrategy::Variant => {
                // Store as tagged union (simplified)
                self.raw_data.push(memento as u8);
                // Would serialize value here
            },
            StorageStrategy::FixedSize(size) => {
                // Direct memory copy for fixed-size types
                unsafe {
                    let value_ptr = value as *const dyn std::any::Any as *const u8;
                    let current_len = self.raw_data.len();
                    self.raw_data.resize(current_len + size, 0);
                    std::ptr::copy_nonoverlapping(
                        value_ptr,
                        self.raw_data.as_mut_ptr().add(current_len),
                        size,
                    );
                }
            },
            StorageStrategy::SIMDOptimized(_) => {
                // Store in SIMD-friendly layout
                let size = memento.network_size().unwrap_or(8);
                unsafe {
                    let value_ptr = value as *const dyn std::any::Any as *const u8;
                    let current_len = self.raw_data.len();
                    self.raw_data.resize(current_len + size, 0);
                    std::ptr::copy_nonoverlapping(
                        value_ptr,
                        self.raw_data.as_mut_ptr().add(current_len),
                        size,
                    );
                }
            },
        }
        Ok(())
    }

    fn can_promote_to(&self, target: IoMemento) -> bool {
        // Check if current data can be losslessly converted to target type
        if let Some((current, _)) = self.evidence.current_type_with_confidence() {
            match (current, target) {
                // Numeric promotions
                (IoMemento::IoInt, IoMemento::IoLong) => true,
                (IoMemento::IoInt, IoMemento::IoFloat) => true,
                (IoMemento::IoInt, IoMemento::IoDouble) => true,
                (IoMemento::IoFloat, IoMemento::IoDouble) => true,
                // Same type
                (a, b) if a == b => true,
                // String can represent anything
                (_, IoMemento::IoString) => true,
                _ => false,
            }
        } else {
            true // No current type, can promote to anything
        }
    }

    fn convert_storage_to_type(&mut self, _target: IoMemento) -> Result<(), &'static str> {
        // Implementation would convert existing data to new format
        // This is complex and depends on current and target types
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evidence_accumulation() {
        let mut evidence = Evidence::new();
        
        // Initially no evidence
        assert_eq!(evidence.current_type_with_confidence(), None);
        assert!(!evidence.is_confident());
        
        // Add evidence for integers
        for _ in 0..80 {
            evidence.add_evidence(IoMemento::IoInt);
        }
        
        // Add some noise
        for _ in 0..20 {
            evidence.add_evidence(IoMemento::IoString);
        }
        
        // Should be confident about IoInt
        let (current_type, confidence) = evidence.current_type_with_confidence().unwrap();
        assert_eq!(current_type, IoMemento::IoInt);
        assert_eq!(confidence, 0.8);
        assert!(evidence.is_confident());
        
        // SIMD strategy should be optimized
        assert_eq!(evidence.simd_strategy(), SIMDStrategy::AVX2_I32);
    }

    #[test]
    fn test_adaptive_column() {
        let mut column = AdaptiveColumn::new();
        
        // Add integer values
        for i in 0..100 {
            let val = i as i32;
            column.push_value(&val).unwrap();
        }
        
        // Should become SIMD-optimized
        assert!(column.is_simd_ready());
        
        let evidence = column.type_evidence();
        let (memento, confidence) = evidence.current_type_with_confidence().unwrap();
        assert_eq!(memento, IoMemento::IoInt);
        assert!(confidence >= 0.8);
    }

    #[test]
    fn test_memento_properties() {
        assert_eq!(IoMemento::IoInt.network_size(), Some(4));
        assert_eq!(IoMemento::IoString.network_size(), None);
        assert!(IoMemento::IoFloat.is_simd_friendly());
        assert!(!IoMemento::IoString.is_simd_friendly());
        assert!(IoMemento::IoLong.is_mmap_safe());
        assert!(!IoMemento::IoBytes.is_mmap_safe());
    }
}