//! Tensor operations and MLIR coordination

pub mod core;
pub mod mlir;
pub mod mlir_jit;

pub use core::{DType, Tensor, TensorShape};
pub use mlir::{MLIRContext, MLIRTensor};
pub use mlir_jit::{
    CompileRequest, JitError, JitExecutor, JitResult, OrcJit, TensorGraph, TensorOp,
};
