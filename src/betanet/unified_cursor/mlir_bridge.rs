// MLIR/JIT accessor generator - expanded stub and safe wrappers.
//
// This file provides:
//  - A macro to generate simple, safe typed accessors from a raw byte pointer.
//  - Small safe wrapper helpers that operate on raw pointers (no strong coupling to UnifiedCursor internals).
//  - Example usage and extension notes for integrating a real MLIR/JIT pipeline.
//
// Example (documentation):
// Use the generated accessor from another module like this:
// [`src/unified_cursor/mlir_bridge.rs`](src/unified_cursor/mlir_bridge.rs:1)
// 
// // assume `ptr: *const u8` points to an array of u64 values
// if let Some(v) = mlir_generated::get_u64_at(ptr, 3) {
//     // use v
// }
//
// Notes:
// - This is intentionally conservative: it only depends on raw pointers and read_unaligned,
//   so it is portable and safe to include as a stub. A real JIT/MLIR backend can replace
//   the generated functions with optimized pointers to JIT'd code and the public API remains the same.
// - Keep these functions thin to minimize ABI surface and make replacement by JIT easy.
//
// Macro: generate a typed accessor that reads from a raw pointer at element index `i`.
// The accessor returns Option<T> where None represents a null pointer or out-of-bounds caller decision.
// The macro emits functions in the `mlir_generated` module to avoid polluting the crate root.
#[macro_export]
macro_rules! mlir_generate_accessor {
    // $fn_name: identifier for the generated function
    // $ty: the concrete type to read (e.g. u64, i32, f32)
    // generates: pub fn $fn_name(ptr: *const u8, index: usize) -> Option<$ty>
    ($fn_name:ident, $ty:ty) => {
        #[allow(dead_code)]
        pub mod mlir_generated {
            use core::mem;
            use core::ptr;

            #[inline(always)]
            pub fn $fn_name(ptr: *const u8, index: usize) -> Option<$ty> {
                if ptr.is_null() {
                    return None;
                }
                // compute byte offset for index
                let size = mem::size_of::<$ty>();
                // Defensive: avoid overflow in index*size
                let offset = match index.checked_mul(size) {
                    Some(o) => o,
                    None => return None,
                };
                unsafe {
                    // SAFETY: caller must ensure ptr points to valid memory for the requested element.
                    // We use read_unaligned to be compatible with packed layouts and mmap-backed regions.
                    let src = ptr.add(offset) as *const $ty;
                    // Use read_unaligned to avoid alignment assumptions on mmap.
                    Some(ptr::read_unaligned(src))
                }
            }
        }
    };
}

// Instantiate a few common typed accessors as practical stubs for MLIR/JIT replacement.
mlir_generate_accessor!(get_u64_at, u64);
mlir_generate_accessor!(get_u32_at, u32);
mlir_generate_accessor!(get_i64_at, i64);
mlir_generate_accessor!(get_f64_at, f64);

// Lightweight safe helpers that accept a raw pointer and an element offset (in elements).
// These are convenience thin wrappers that callers (or test scaffolding) can use.
#[allow(dead_code)]
pub mod safe_accessors {
    use super::mlir_generated;
    use core::ptr;

    #[inline]
    pub fn read_u64(ptr: *const u8, index: usize) -> Option<u64> {
        mlir_generated::get_u64_at(ptr, index)
    }

    #[inline]
    pub fn read_u32(ptr: *const u8, index: usize) -> Option<u32> {
        mlir_generated::get_u32_at(ptr, index)
    }

    #[inline]
    pub fn read_i64(ptr: *const u8, index: usize) -> Option<i64> {
        mlir_generated::get_i64_at(ptr, index)
    }

    #[inline]
    pub fn read_f64(ptr: *const u8, index: usize) -> Option<f64> {
        mlir_generated::get_f64_at(ptr, index)
    }

    // Example adapter: read element at byte offset instead of index (useful for columnar store offsets)
    #[inline]
    pub fn read_u64_at_byte_offset(ptr: *const u8, byte_offset: usize) -> Option<u64> {
        if ptr.is_null() {
            return None;
        }
        // treat the byte_offset as element index for u64 when divisible by size_of::<u64>()
        let size = core::mem::size_of::<u64>();
        if byte_offset % size != 0 {
            // caller provided an unaligned element offset; still attempt to read as unaligned u64
            unsafe {
                let src = ptr.add(byte_offset) as *const u64;
                return Some(core::ptr::read_unaligned(src));
            }
        }
        let index = byte_offset / size;
        read_u64(ptr, index)
    }
}

// Extension point: a small trait that a JIT implementation may implement to provide pointers
// to optimized accessors. This trait intentionally uses raw pointer types to avoid tying to
// internal cursor types; the integration layer can bridge types to pointers.
#[allow(dead_code)]
pub trait JitAccessorProvider {
    // Return a function pointer for the requested typed accessor.
    // The function takes (*const u8, usize) and returns an Option encoded as usize (0 == None).
    // This is a compact ABI that a generated JIT function can conform to.
    fn get_accessor(&self, ty_id: u32) -> Option<extern "C" fn(*const u8, usize) -> usize>;
}

// Replacement strategy notes (for docs/PR):
// - When integrating a real MLIR/LLVM pipeline, compile generated accessor functions with
//   extern "C" ABI and return a simple integer-coded Option (e.g., 0 for None, non-zero for Some(ptr or value))
//   or write results into a caller-provided out-parameter. Keep the surface identical to the macro-generated helpers.
// - Provide a small shim that translates the JIT accessor signature to the safe helpers above. This keeps the rest
//   of the codebase using read_u64/read_f64 etc. unchanged and allows runtime swapping of implementations.
//
// Safety model summary:
// - These helpers use unsafe pointer reads; callers must ensure the pointer is valid for the requested index.
// - Use read_unaligned to avoid undefined behavior on potentially unaligned mmap-backed memory.
// - Encapsulate all unsafe behavior in this module so higher-level code can remain safe or use explicit unsafe blocks.
//
// End of MLIR bridge expanded stub.
#[cfg(test)]
mod tests {
    use super::safe_accessors;

    #[test]
    fn test_read_u64_basic() {
        let data: [u64; 4] = [10u64, 20u64, 30u64, 40u64];
        let ptr = data.as_ptr() as *const u8;
        assert_eq!(safe_accessors::read_u64(ptr, 0), Some(10u64));
        assert_eq!(safe_accessors::read_u64(ptr, 1), Some(20u64));
        assert_eq!(safe_accessors::read_u64(ptr, 3), Some(40u64));
    }

    #[test]
    fn test_read_u32_basic() {
        let data: [u32; 3] = [1u32, 2u32, 3u32];
        let ptr = data.as_ptr() as *const u8;
        assert_eq!(safe_accessors::read_u32(ptr, 0), Some(1u32));
        assert_eq!(safe_accessors::read_u32(ptr, 2), Some(3u32));
    }

    #[test]
    fn test_read_u64_unaligned_byte_offset() {
        // Create a byte buffer where reading an unaligned u64 at offset 1 should still succeed via read_unaligned.
        let bytes: [u8; 16] = [
            0xAA, // offset 0
            0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // little-endian u64 value 0x10 at offset 1
            0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA, 0x99,
        ];
        let ptr = bytes.as_ptr();
        let v = safe_accessors::read_u64_at_byte_offset(ptr, 1);
        assert!(v.is_some());
    }

    #[test]
    fn test_null_pointer_returns_none() {
        let null_ptr: *const u8 = core::ptr::null();
        assert_eq!(safe_accessors::read_u64(null_ptr, 0), None);
        assert_eq!(safe_accessors::read_u32(null_ptr, 0), None);
    }
}