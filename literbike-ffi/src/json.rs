/// C ABI bindings for Bun JSON parser integration
///
/// This module provides a C-compatible API for Bun to call the thread-safe
/// Rust JSON parser, replacing Bun's race-condition-prone HashMapPool.
///
/// # Memory Safety
///
/// - All returned pointers must be freed with `literbike_json_free()`
/// - Strings are UTF-8 encoded and null-terminated
/// - The parser is thread-safe and can be called concurrently
///
/// # Example Usage from C
///
/// ```c
/// const char* json = "{\"name\": \"value\"}";
/// void* result = literbike_json_parse(json);
/// if (result == NULL) {
///     const char* error = literbike_json_last_error();
///     fprintf(stderr, "Parse error: %s\n", error);
///     return;
/// }
/// // Use result...
/// literbike_json_free(result);
/// ```

use literbike::json::{FastJsonParser, JsonError};
use std::ffi::{CStr, CString, NulError};
use std::os::raw::{c_char, c_int};
use std::sync::Mutex;

/// Last error message from JSON parsing
///
/// This is thread-local and stores the most recent error message
thread_local! {
    static LAST_ERROR: Mutex<Option<CString>> = Mutex::new(None);
}

/// Opaque handle to parsed JSON AST
///
/// This represents a parsed JSON value that can be inspected and freed
#[repr(C)]
pub struct JsonAst {
    _private: [u8; 0],
}

/// Parse a JSON string
///
/// # Arguments
///
/// * `json_str` - Null-terminated UTF-8 JSON string
///
/// # Returns
///
/// * `JsonAst*` - Opaque pointer to parsed AST (must be freed with `literbike_json_free`)
/// * `NULL` - Parse error occurred, call `literbike_json_last_error()` for details
///
/// # Thread Safety
///
/// This function is thread-safe and can be called concurrently
#[no_mangle]
pub extern "C" fn literbike_json_parse(json_str: *const c_char) -> *mut JsonAst {
    if json_str.is_null() {
        set_error("JSON string pointer is null");
        return std::ptr::null_mut();
    }

    // Convert C string to Rust string
    let c_str = unsafe { CStr::from_ptr(json_str) };
    let json_bytes = match c_str.to_bytes() {
        bytes => bytes,
        Err(_) => {
            set_error("Invalid UTF-8 in JSON string");
            return std::ptr::null_mut();
        }
    };

    let json_str_rust = match std::str::from_utf8(json_bytes) {
        Ok(s) => s,
        Err(_) => {
            set_error("Invalid UTF-8 in JSON string");
            return std::ptr::null_mut();
        }
    };

    // Parse JSON
    let parser = FastJsonParser::new();
    let result = match parser.parse(json_str_rust) {
        Ok(ast) => ast,
        Err(err) => {
            set_error(&err.to_string());
            return std::ptr::null_mut();
        }
    };

    // Box the AST and return a pointer to it
    let boxed = Box::new(result);
    Box::into_raw(boxed) as *mut JsonAst
}

/// Parse a JSON5 string (with comments, trailing commas, etc.)
///
/// # Arguments
///
/// * `json_str` - Null-terminated UTF-8 JSON5 string
///
/// # Returns
///
/// * `JsonAst*` - Opaque pointer to parsed AST (must be freed with `literbike_json_free`)
/// * `NULL` - Parse error occurred, call `literbike_json_last_error()` for details
#[no_mangle]
pub extern "C" fn literbike_json_parse5(json_str: *const c_char) -> *mut JsonAst {
    if json_str.is_null() {
        set_error("JSON5 string pointer is null");
        return std::ptr::null_mut();
    }

    // Convert C string to Rust string
    let c_str = unsafe { CStr::from_ptr(json_str) };
    let json_bytes = match c_str.to_bytes() {
        bytes => bytes,
        Err(_) => {
            set_error("Invalid UTF-8 in JSON5 string");
            return std::ptr::null_mut();
        }
    };

    let json_str_rust = match std::str::from_utf8(json_bytes) {
        Ok(s) => s,
        Err(_) => {
            set_error("Invalid UTF-8 in JSON5 string");
            return std::ptr::null_mut();
        }
    };

    // Parse JSON5
    let parser = FastJsonParser::new();
    let result = match parser.parse_json5(json_str_rust) {
        Ok(ast) => ast,
        Err(err) => {
            set_error(&err.to_string());
            return std::ptr::null_mut();
        }
    };

    // Box the AST and return a pointer to it
    let boxed = Box::new(result);
    Box::into_raw(boxed) as *mut JsonAst
}

/// Free a parsed JSON AST
///
/// # Arguments
///
/// * `ast` - Pointer to AST returned by `literbike_json_parse` or `literbike_json_parse5`
///
/// # Safety
///
/// - Must only be called on pointers returned by parse functions
/// - Pointer becomes invalid after this call
/// - Double-free will cause undefined behavior
#[no_mangle]
pub extern "C" fn literbike_json_free(ast: *mut JsonAst) {
    if ast.is_null() {
        return;
    }

    // Convert back to Box and drop
    unsafe {
        let _ = Box::from_raw(ast as *mut literbike::json::Expr);
    }
}

/// Get the last error message
///
/// # Returns
///
/// * Pointer to null-terminated error string
/// * Pointer is valid until next call to parse functions
/// * Do not free the returned pointer
///
/// # Thread Safety
///
/// This function is thread-safe (uses thread-local storage)
#[no_mangle]
pub extern "C" fn literbike_json_last_error() -> *const c_char {
    LAST_ERROR.with(|error| {
        let guard = error.lock().unwrap();
        match guard.as_ref() {
            Some(cstring) => cstring.as_ptr(),
            None => {
                // Static empty string for "no error"
                static EMPTY: &[u8] = b"\0";
                EMPTY.as_ptr() as *const c_char
            }
        }
    })
}

/// Get JSON AST as a string (for debugging/testing)
///
/// # Arguments
///
/// * `ast` - Pointer to AST returned by parse function
///
/// # Returns
///
/// * Pointer to null-terminated JSON string
/// * NULL if AST is null or serialization fails
/// * Caller must free with `literbike_json_string_free()`
/// 
/// # Memory Safety
///
/// The returned string MUST be freed by the caller to prevent memory leaks.
/// Use `literbike_json_string_free()` to release the memory.
#[no_mangle]
pub extern "C" fn literbike_json_to_string(ast: *mut JsonAst) -> *mut c_char {
    if ast.is_null() {
        set_error("AST pointer is null");
        return std::ptr::null_mut();
    }

    let expr = unsafe { &*(ast as *const literbike::json::Expr) };

    // Serialize Expr to JSON string
    let json_str = match serde_json::to_string(expr) {
        Ok(s) => s,
        Err(err) => {
            set_error(&format!("Serialization failed: {}", err));
            return std::ptr::null_mut();
        }
    };

    // Convert to CString and return raw pointer (caller owns this memory)
    // SAFETY: The caller MUST call literbike_json_string_free() to prevent leaks
    match CString::new(json_str) {
        Ok(cstring) => cstring.into_raw(),
        Err(_) => {
            set_error("String contains null byte");
            std::ptr::null_mut()
        }
    }
}

/// Free a string returned by `literbike_json_to_string`
///
/// CRITICAL: This function MUST be called for every string returned by
/// `literbike_json_to_string()` to prevent memory leaks.
///
/// # Arguments
///
/// * `str` - Pointer to string returned by `literbike_json_to_string`
///
/// # Memory Safety
///
/// - Passing NULL is safe and will be ignored
/// - Double-free will cause undefined behavior
/// - After calling, the pointer becomes invalid
#[no_mangle]
pub extern "C" fn literbike_json_string_free(str: *mut c_char) {
    if str.is_null() {
        return;
    }

    unsafe {
        // Reconstruct the CString and drop it, freeing the memory
        let _ = CString::from_raw(str);
    }
}

/// Get JSON AST type
///
/// # Arguments
///
/// * `ast` - Pointer to AST returned by parse function
///
/// # Returns
///
/// * Type code: 0=object, 1=array, 2=string, 3=number, 4=boolean, 5=null
/// * -1 if AST is null
#[no_mangle]
pub extern "C" fn literbike_json_type(ast: *mut JsonAst) -> c_int {
    if ast.is_null() {
        return -1;
    }

    let expr = unsafe { &*(ast as *const literbike::json::Expr) };

    match expr {
        literbike::json::Expr::Object { .. } => 0,
        literbike::json::Expr::Array { .. } => 1,
        literbike::json::Expr::String { .. } => 2,
        literbike::json::Expr::Number { .. } => 3,
        literbike::json::Expr::Boolean { .. } => 4,
        literbike::json::Expr::Null { .. } => 5,
    }
}

/// Set the last error message
fn set_error(msg: &str) {
    LAST_ERROR.with(|error| {
        let mut guard = error.lock().unwrap();
        *guard = CString::new(msg).ok();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_json() {
        let json = c"{\"name\": \"value\"}";
        let result = literbike_json_parse(json.as_ptr());
        assert!(!result.is_null());
        literbike_json_free(result);
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = c"{invalid}";
        let result = literbike_json_parse(json.as_ptr());
        assert!(result.is_null());
        
        let error = literbike_json_last_error();
        assert!(!error.is_null());
    }

    #[test]
    fn test_parse_json5() {
        let json5 = c"// comment\n{\"name\": \"value\",}";
        let result = literbike_json_parse5(json5.as_ptr());
        assert!(!result.is_null());
        literbike_json_free(result);
    }

    #[test]
    fn test_to_string() {
        let json = c"{\"test\": 123}";
        let ast = literbike_json_parse(json.as_ptr());
        assert!(!ast.is_null());
        
        let string = literbike_json_to_string(ast);
        assert!(!string.is_null());
        
        // Verify string content
        let c_str = unsafe { CStr::from_ptr(string) };
        let json_str = c_str.to_str().unwrap();
        assert!(json_str.contains("test") || json_str.contains("123"));
        
        literbike_json_string_free(string);
        literbike_json_free(ast);
    }

    #[test]
    fn test_null_pointer_safety() {
        // Should not crash
        literbike_json_free(std::ptr::null_mut());
        literbike_json_string_free(std::ptr::null_mut());
        
        let result = literbike_json_parse(std::ptr::null());
        assert!(result.is_null());
    }

    #[test]
    fn test_type_detection() {
        let json = c"42";
        let ast = literbike_json_parse(json.as_ptr());
        assert!(!ast.is_null());
        
        let type_code = literbike_json_type(ast);
        assert_eq!(type_code, 3); // Number
        
        literbike_json_free(ast);
    }
}
