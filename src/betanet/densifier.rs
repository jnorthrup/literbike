//! Densifier helpers — small, testable register-packed examples
//!
//! Densifier insight: provide a concrete, repr(transparent) register-packed
//! two-tuple for common Indexed patterns. This file demonstrates the
//! `Join<Int, Int->T>` packing as a single 64-bit word for u32/u32 cases.
//!
//! Keep this file small and focused so it can be unit-tested and used as
//! a template for future, generically gated implementations.

use std::mem;

/// Concrete register-packed Join for (u32, u32) stored as a single u64.
/// Layout: high 32 bits = first (offset), low 32 bits = accessor index/id.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DensifiedJoinU32Fn {
    packed: u64,
}

impl DensifiedJoinU32Fn {
    /// Create a new packed join from two u32 values.
    #[inline]
    pub fn new(first: u32, accessor: u32) -> Self {
        let packed = ((first as u64) << 32) | (accessor as u64);
        Self { packed }
    }

    /// Extract the first (offset) value.
    #[inline]
    pub fn first(&self) -> u32 {
        (self.packed >> 32) as u32
    }

    /// Extract the accessor id/value.
    #[inline]
    pub fn accessor(&self) -> u32 {
        (self.packed & 0xffffffff) as u32
    }

    /// Raw packed representation (for zero-copy passing to kernel or wires)
    #[inline]
    pub fn as_u64(&self) -> u64 {
        self.packed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn densified_join_size_is_8() {
        // This asserts the packing property for the concrete type.
        assert_eq!(mem::size_of::<DensifiedJoinU32Fn>(), 8);
    }

    #[test]
    fn densified_join_roundtrip() {
        let a = DensifiedJoinU32Fn::new(0xdead_beefu32, 0xabad_cafeu32);
        assert_eq!(a.first(), 0xdead_beefu32);
        assert_eq!(a.accessor(), 0xabad_cafeu32);
        assert_eq!(a.as_u64(), ((0xdead_beefu64 << 32) | 0xabad_cafeu64));
    }

    #[test]
    fn densifier_one_liner_header_present() {
        // Slight meta-test: ensure the densifier header comment exists and
        // document the one-liner guidance; not a strict verification, but
        // keeps the file self-descriptive.
        let s = include_str!("./densifier.rs");
        assert!(s.contains("Densifier insight"));
    }

    #[test]
    fn test_densified_join_u32fn_accessors() {
        let offset = 0xDEAD_BEEF;
        let accessor = 0xFEED_FACE;
        let join = DensifiedJoinU32Fn::new(offset, accessor);
        assert_eq!(join.first(), offset);
        assert_eq!(join.accessor(), accessor);
    }
}
