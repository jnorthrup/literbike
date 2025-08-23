// Small densifier shim: register-packed Join and Indexed types
// This is a lightweight, cross-platform implementation used for tests and
// to demonstrate zero-allocation register-packing ideas from the Densifier axioms.

use std::marker::PhantomData;

/// A tiny register-packed pair stored in a u64 when possible.
#[repr(transparent)]
pub struct Join<A, B> {
    packed: u64,
    _phantom: PhantomData<(A, B)>,
}

impl Join<u32, u32> {
    /// Pack two u32 values into a single u64.
    pub fn pack(a: u32, b: u32) -> Self {
        let packed = ((a as u64) << 32) | (b as u64);
        Self { packed, _phantom: PhantomData }
    }

    pub fn unpack(&self) -> (u32, u32) {
        let a = (self.packed >> 32) as u32;
        let b = (self.packed & 0xffff_ffff) as u32;
        (a, b)
    }
}

/// Indexed<T> = Join<u32, fn(u32) -> T> would not be directly representable in stable Rust,
/// so provide a small ergonomic wrapper for the common case of indexing into a slice.
pub struct Indexed<'a, T> {
    offset: u32,
    slice: &'a [T],
}

impl<'a, T> Indexed<'a, T> {
    pub fn new(offset: u32, slice: &'a [T]) -> Self {
        Self { offset, slice }
    }

    pub fn get(&self) -> Option<&T> {
        self.slice.get(self.offset as usize)
    }
}

/// Projection type alias example (not register-packed, but demonstrates API)
pub struct Projection<X, T> {
    domain: X,
    projector: fn(&X) -> T,
}

impl<X, T> Projection<X, T> {
    pub fn new(domain: X, projector: fn(&X) -> T) -> Self {
        Self { domain, projector }
    }

    pub fn project(&self) -> T {
        (self.projector)(&self.domain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_pack_unpack() {
        let j = Join::pack(0x1122_3344, 0x5566_7788);
        let (a, b) = j.unpack();
        assert_eq!(a, 0x1122_3344);
        assert_eq!(b, 0x5566_7788);
    }

    #[test]
    fn indexed_get() {
        let data = [10u8, 20, 30];
        let idx = Indexed::new(1, &data);
        assert_eq!(idx.get(), Some(&20u8));

        let idx_oob = Indexed::new(10, &data);
        assert_eq!(idx_oob.get(), None);
    }

    #[test]
    fn projection_project() {
        let p = Projection::new(5u32, |x: &u32| x + 3);
        assert_eq!(p.project(), 8u32);
    }
}
