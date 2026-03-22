//! RBcurse - Recursive Byte Cursor
//!
//! Protocol recognition engine with SIMD acceleration and MLIR JIT support.

pub mod rbcursive;

pub use rbcursive::{
    AddrPack, CachedResult, CompiledMatcher, Indexed, MlirJitEngine, NetTuple, PatternAnalysis,
    PatternMatcher, PatternType, PortProto, Protocol, RbCursor, RbCursorConfig, RbCursorImpl,
    Signal, TargetMachine,
};
