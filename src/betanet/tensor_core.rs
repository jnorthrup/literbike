//! Lightweight tensor view utilities for prototype experiments.
//! No external dependencies. Focus: zero-copy mapping of byte buffers into multi-dim tensors
//! and a tiny contraction operation over a specified axis. This is intentionally minimal.

use std::marker::PhantomData;
use std::convert::TryInto;

/// Userspace backend selection for emulations.
#[derive(Debug, PartialEq, Eq)]
pub enum UserspaceBackend {
    Mlir,
    Mlcore,
    Default,
}

impl UserspaceBackend {
    /// Choose a backend by inspecting a filesystem or module path.
    /// - If the path contains "mlir" (case-insensitive) -> Mlir
    /// - If the path contains "mlcore" (case-insensitive) -> Mlcore
    /// - Otherwise -> Default
    pub fn from_path(path: &str) -> Self {
        let lower = path.to_ascii_lowercase();
        if lower.contains("mlir") { UserspaceBackend::Mlir }
        else if lower.contains("mlcore") { UserspaceBackend::Mlcore }
        else { UserspaceBackend::Default }
    }
}

/// A simple, zero-copy view into a contiguous buffer interpreted as a D-dimensional tensor of T.
/// Currently supports only element types that are POD and can be transmuted from bytes (u8,u16,u32,u64).
#[derive(Debug)]
pub struct TensorView<'a, T> {
    data: &'a [T],
    dims: Vec<usize>,
    strides: Vec<usize>,
    _phantom: PhantomData<&'a T>,
}

impl<'a, T> TensorView<'a, T> {
    /// Create a tensor view from a typed slice and dimensions.
    /// dims.len() must match the logical rank and product(dims) == data.len()
    pub fn from_slice(data: &'a [T], dims: Vec<usize>) -> Option<Self> {
        let prod: usize = dims.iter().product();
        if prod != data.len() {
            return None;
        }
        let mut strides = vec![1usize; dims.len()];
        for i in (0..dims.len()).rev() {
            if i + 1 < dims.len() {
                strides[i] = strides[i + 1] * dims[i + 1];
            }
        }
        Some(Self { data, dims, strides, _phantom: PhantomData })
    }

    /// Rank of the tensor
    pub fn rank(&self) -> usize { self.dims.len() }

    /// Get raw element by multi-index
    pub fn get(&self, idx: &[usize]) -> Option<&T> {
        if idx.len() != self.rank() { return None; }
        let mut linear = 0usize;
        for (i, &v) in idx.iter().enumerate() {
            if v >= self.dims[i] { return None; }
            linear += v * self.strides[i];
        }
        self.data.get(linear)
    }
}

impl<'a> TensorView<'a, u64> {
    /// Contract (reduce) along an axis by summing u64 elements, returning a lower-rank TensorView backed by a Vec<u64>.
    /// This allocates a new Vec for the result (small, explicit allocation for prototype).
    pub fn contract_axis_sum(&self, axis: usize) -> Option<OwnedTensor<u64>> {
        if axis >= self.rank() { return None; }
        let mut out_dims = self.dims.clone();
        out_dims.remove(axis);
        let out_len: usize = out_dims.iter().product();
        let mut out = vec![0u64; out_len];

        // iterate over every element in the input and accumulate
        // compute multi-index by linear iteration
        let total = self.data.len();
        for linear in 0..total {
            // convert linear -> multi-index
            let mut rem = linear;
            let mut idx = vec![0usize; self.rank()];
            for i in 0..self.rank() {
                let s = self.strides[i];
                idx[i] = rem / s;
                rem = rem % s;
            }
            // compute output linear index by skipping the axis
            let mut out_linear = 0usize;
            let mut mul = 1usize;
            for i in (0..self.rank()).rev() {
                if i == axis { continue; }
                let out_pos = if i < axis { idx[i] } else { idx[i] };
                let od = if i < axis { self.dims[i] } else { self.dims[i] };
                // compute position using out_dims and strides implicitly
                // accumulate from the back
                out_linear += out_pos * mul;
                mul *= od;
            }
            out[out_linear] = out[out_linear].wrapping_add(self.data[linear]);
        }

        OwnedTensor::from_vec(out, out_dims)
    }
}

/// Owned tensor returned from contraction (owns its backing Vec)
#[derive(Debug)]
pub struct OwnedTensor<T> {
    data: Vec<T>,
    dims: Vec<usize>,
    strides: Vec<usize>,
}

impl<T> OwnedTensor<T> {
    pub fn from_vec(data: Vec<T>, dims: Vec<usize>) -> Option<Self> {
        let prod: usize = dims.iter().product();
        if prod != data.len() { return None; }
        let mut strides = vec![1usize; dims.len()];
        for i in (0..dims.len()).rev() {
            if i + 1 < dims.len() {
                strides[i] = strides[i + 1] * dims[i + 1];
            }
        }
        Some(Self { data, dims, strides })
    }

    pub fn as_view(&self) -> TensorView<'_, T> where T: 'static {
        // SAFETY: we return a view with lifetime tied to &self
        TensorView { data: &self.data, dims: self.dims.clone(), strides: self.strides.clone(), _phantom: PhantomData }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensorview_from_slice() {
        let data: Vec<u64> = (0..24u64).collect();
        let tv = TensorView::from_slice(&data, vec![2,3,4]).expect("create view");
        assert_eq!(tv.rank(), 3);
        assert_eq!(*tv.get(&[1,2,3]).unwrap(), 1*12 + 2*4 + 3);
    }

    #[test]
    fn test_contract_axis_sum() {
        // 2 x 3 x 4 tensor, contract axis 1 (middle)
        let data: Vec<u64> = (0..24u64).collect();
        let tv = TensorView::from_slice(&data, vec![2,3,4]).unwrap();
        let out = tv.contract_axis_sum(1).expect("contract");
        // out dims should be [2,4]
        assert_eq!(out.dims, vec![2,4]);
        // compute expected: sum over middle axis
        for i in 0..2 {
            for k in 0..4 {
                let mut expected = 0u64;
                for j in 0..3 {
                    let linear = i*12 + j*4 + k;
                    expected += data[linear];
                }
                let out_idx = i*4 + k;
                assert_eq!(out.data[out_idx], expected);
            }
        }
    }

    #[test]
    fn test_backend_from_path() {
        use super::UserspaceBackend;
        assert_eq!(UserspaceBackend::from_path("/some/mlir/module"), UserspaceBackend::Mlir);
        assert_eq!(UserspaceBackend::from_path("path/to/MLCORE/lib"), UserspaceBackend::Mlcore);
        assert_eq!(UserspaceBackend::from_path("unknown/path"), UserspaceBackend::Default);
    }
}
