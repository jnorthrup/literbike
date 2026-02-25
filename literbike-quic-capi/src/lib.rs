use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::slice;

#[repr(C)]
pub struct LBQuicConnection {
    host: String,
    port: u16,
    timeout_ms: u32,
}

#[repr(C)]
pub struct LBQuicResponse {
    status: u16,
    body: Vec<u8>,
}

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(
        CString::new("literbike-quic-capi: no error").expect("static error message has no NUL")
    );
}

fn set_last_error(message: impl AsRef<str>) {
    let sanitized = message.as_ref().replace('\0', " ");
    let cstr = CString::new(sanitized).unwrap_or_else(|_| {
        CString::new("literbike-quic-capi: failed to set error message")
            .expect("fallback error has no NUL")
    });
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() = cstr;
    });
}

fn read_cstr(ptr: *const c_char, field_name: &str) -> Result<String, ()> {
    if ptr.is_null() {
        set_last_error(format!("{field_name} pointer is null"));
        return Err(());
    }
    let s = unsafe { CStr::from_ptr(ptr) };
    match s.to_str() {
        Ok(v) => Ok(v.to_string()),
        Err(_) => {
            set_last_error(format!("{field_name} is not valid UTF-8"));
            Err(())
        }
    }
}

fn make_stub_response() -> *mut LBQuicResponse {
    let body =
        br#"{"error":"quic_request_not_implemented","transport":"literbike-quic-capi"}"#.to_vec();
    Box::into_raw(Box::new(LBQuicResponse { status: 501, body }))
}

#[no_mangle]
pub extern "C" fn quic_connect(
    host: *const c_char,
    port: u16,
    timeout_ms: u32,
) -> *mut LBQuicConnection {
    let host = match read_cstr(host, "host") {
        Ok(host) => host,
        Err(()) => return ptr::null_mut(),
    };

    if host.is_empty() {
        set_last_error("host must not be empty");
        return ptr::null_mut();
    }

    set_last_error("literbike-quic-capi: no error");
    Box::into_raw(Box::new(LBQuicConnection {
        host,
        port,
        timeout_ms,
    }))
}

#[no_mangle]
pub extern "C" fn quic_request(
    conn: *mut LBQuicConnection,
    method: *const c_char,
    path: *const c_char,
    headers_json: *const c_char,
    body_ptr: *const u8,
    body_len: usize,
    _timeout_ms: u32,
) -> *mut LBQuicResponse {
    if conn.is_null() {
        set_last_error("connection handle is null");
        return ptr::null_mut();
    }

    let _method = match read_cstr(method, "method") {
        Ok(v) => v,
        Err(()) => return ptr::null_mut(),
    };
    let _path = match read_cstr(path, "path") {
        Ok(v) => v,
        Err(()) => return ptr::null_mut(),
    };

    if !headers_json.is_null() {
        if read_cstr(headers_json, "headers_json").is_err() {
            return ptr::null_mut();
        }
    }

    if body_ptr.is_null() && body_len != 0 {
        set_last_error("body_ptr is null but body_len is non-zero");
        return ptr::null_mut();
    }

    if !body_ptr.is_null() && body_len != 0 {
        let _body = unsafe { slice::from_raw_parts(body_ptr, body_len) };
    }

    // Real QUIC transport request path is intentionally not wired yet.
    // This stub returns a structured response so ctypes callers can complete
    // handle/status/body ownership flows while retaining fallback logic.
    set_last_error("quic_request transport path is not implemented yet");
    make_stub_response()
}

#[no_mangle]
pub extern "C" fn quic_response_status(resp: *const LBQuicResponse) -> u16 {
    if resp.is_null() {
        set_last_error("response handle is null");
        return 0;
    }
    unsafe { (*resp).status }
}

#[no_mangle]
pub extern "C" fn quic_response_body_ptr(resp: *const LBQuicResponse) -> *const u8 {
    if resp.is_null() {
        set_last_error("response handle is null");
        return ptr::null();
    }
    unsafe { (*resp).body.as_ptr() }
}

#[no_mangle]
pub extern "C" fn quic_response_body_len(resp: *const LBQuicResponse) -> usize {
    if resp.is_null() {
        set_last_error("response handle is null");
        return 0;
    }
    unsafe { (*resp).body.len() }
}

#[no_mangle]
pub extern "C" fn quic_response_free(resp: *mut LBQuicResponse) {
    if resp.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(resp));
    }
}

#[no_mangle]
pub extern "C" fn quic_close(conn: *mut LBQuicConnection) {
    if conn.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(conn));
    }
}

#[no_mangle]
pub extern "C" fn quic_last_error_message() -> *const c_char {
    LAST_ERROR.with(|slot| slot.borrow().as_ptr())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn connect_and_stub_request_roundtrip() {
        let host = CString::new("127.0.0.1").unwrap();
        let method = CString::new("GET").unwrap();
        let path = CString::new("/api/v1/health").unwrap();
        let headers = CString::new("{}").unwrap();

        let conn = quic_connect(host.as_ptr(), 8080, 1000);
        assert!(!conn.is_null());

        let resp = quic_request(
            conn,
            method.as_ptr(),
            path.as_ptr(),
            headers.as_ptr(),
            ptr::null(),
            0,
            1000,
        );
        assert!(!resp.is_null());
        assert_eq!(quic_response_status(resp), 501);
        assert!(quic_response_body_len(resp) > 0);

        quic_response_free(resp);
        quic_close(conn);
    }

    #[test]
    fn null_connection_request_sets_error() {
        let method = CString::new("GET").unwrap();
        let path = CString::new("/").unwrap();
        let resp = quic_request(
            ptr::null_mut(),
            method.as_ptr(),
            path.as_ptr(),
            ptr::null(),
            ptr::null(),
            0,
            1000,
        );
        assert!(resp.is_null());

        let msg = unsafe { CStr::from_ptr(quic_last_error_message()) }
            .to_str()
            .unwrap()
            .to_string();
        assert!(msg.contains("connection handle is null"));
    }
}
