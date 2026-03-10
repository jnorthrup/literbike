use literbike::dht::{DhtService, PeerId, PeerInfo};
use literbike::quic::quic_engine::Role;
use literbike::quic::quic_protocol::{
    deserialize_decoded_packet_with_dcid_len, ConnectionId, ConnectionState, QuicConnectionState,
    TransportParameters,
};
use literbike::quic::QuicEngine;
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::net::{SocketAddr, ToSocketAddrs};
use std::os::raw::c_char;
use std::ptr;
use std::slice;
use std::sync::Arc;
use std::time::{Duration, Instant};

const QUIC_REQUEST_PROTOCOL_AUTO: u32 = 0;
const QUIC_REQUEST_PROTOCOL_HTTP1_OVER_QUIC: u32 = 1;
const QUIC_REQUEST_PROTOCOL_HTTP3: u32 = 2;

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

fn sanitize_json_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn make_error_response(status: u16, code: &str, detail: &str) -> *mut LBQuicResponse {
    let body = format!(
        "{{\"error\":\"{}\",\"detail\":\"{}\",\"transport\":\"literbike-quic-capi\"}}",
        sanitize_json_str(code),
        sanitize_json_str(detail)
    )
    .into_bytes();
    Box::into_raw(Box::new(LBQuicResponse { status, body }))
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

fn build_client_state() -> QuicConnectionState {
    QuicConnectionState {
        local_connection_id: ConnectionId {
            bytes: vec![0x10, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
        },
        remote_connection_id: ConnectionId {
            bytes: vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x10],
        },
        version: 1,
        transport_params: TransportParameters::default(),
        streams: Vec::new(),
        sent_packets: Vec::new(),
        received_packets: Vec::new(),
        next_packet_number: 0,
        next_stream_id: 1, // client-initiated bidi stream
        congestion_window: 14720,
        bytes_in_flight: 0,
        rtt: 100,
        connection_state: ConnectionState::Handshaking,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestProtocolMode {
    Http1OverQuic,
    Http3,
}

impl RequestProtocolMode {
    fn from_abi(mode: u32) -> Result<Self, String> {
        match mode {
            QUIC_REQUEST_PROTOCOL_AUTO | QUIC_REQUEST_PROTOCOL_HTTP1_OVER_QUIC => {
                Ok(Self::Http1OverQuic)
            }
            QUIC_REQUEST_PROTOCOL_HTTP3 => Ok(Self::Http3),
            other => Err(format!("unsupported protocol_mode: {other}")),
        }
    }
}

fn parse_header_map(headers_json: Option<&str>) -> Result<Vec<(String, String)>, String> {
    let mut headers = Vec::new();
    let Some(headers_json) = headers_json else {
        return Ok(headers);
    };
    if headers_json.trim().is_empty() {
        return Ok(headers);
    }
    let value: serde_json::Value =
        serde_json::from_str(headers_json).map_err(|e| format!("invalid headers_json: {e}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "headers_json must be a JSON object".to_string())?;
    for (k, v) in obj {
        let value_str = match v {
            serde_json::Value::String(s) => s.clone(),
            _ => v.to_string(),
        };
        headers.push((k.clone(), value_str));
    }
    Ok(headers)
}

fn build_http1_over_quic_payload(
    conn: &LBQuicConnection,
    method: &str,
    path: &str,
    headers_json: Option<&str>,
    body: &[u8],
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let normalized_path = if path.is_empty() { "/" } else { path };
    let normalized_path = if normalized_path.starts_with('/') {
        normalized_path.to_string()
    } else {
        format!("/{normalized_path}")
    };

    out.extend_from_slice(method.as_bytes());
    out.extend_from_slice(b" ");
    out.extend_from_slice(normalized_path.as_bytes());
    out.extend_from_slice(b" HTTP/1.1\r\n");

    let mut headers = parse_header_map(headers_json)?;
    let mut has_host = false;
    let mut has_content_length = false;
    for (k, _) in &headers {
        if k.eq_ignore_ascii_case("host") {
            has_host = true;
        }
        if k.eq_ignore_ascii_case("content-length") {
            has_content_length = true;
        }
    }

    if !has_host {
        headers.push(("Host".to_string(), format!("{}:{}", conn.host, conn.port)));
    }
    if !has_content_length {
        headers.push(("Content-Length".to_string(), body.len().to_string()));
    }

    for (k, v) in headers {
        if k.contains('\r') || k.contains('\n') || v.contains('\r') || v.contains('\n') {
            return Err("header keys/values must not contain CR/LF".into());
        }
        out.extend_from_slice(k.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(v.as_bytes());
        out.extend_from_slice(b"\r\n");
    }

    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(body);
    Ok(out)
}

fn parse_http_status(body: &[u8]) -> Option<u16> {
    if !body.starts_with(b"HTTP/") {
        return None;
    }
    let line_end = body.iter().position(|&b| b == b'\n').unwrap_or(body.len());
    let line = std::str::from_utf8(&body[..line_end]).ok()?;
    let mut parts = line.split_whitespace();
    let _version = parts.next()?;
    let status = parts.next()?.parse::<u16>().ok()?;
    Some(status)
}

async fn execute_quic_request(
    conn: &LBQuicConnection,
    method: &str,
    path: &str,
    headers_json: Option<&str>,
    body: &[u8],
    timeout_ms: u32,
    protocol_mode: RequestProtocolMode,
) -> Result<LBQuicResponse, (u16, String, String)> {
    let remote_addr = (conn.host.as_str(), conn.port)
        .to_socket_addrs()
        .map_err(|e| (502, "resolve_failed".to_string(), e.to_string()))?
        .find(|addr| matches!(addr, SocketAddr::V4(_) | SocketAddr::V6(_)))
        .ok_or_else(|| {
            (
                502,
                "resolve_failed".to_string(),
                "no socket address resolved".to_string(),
            )
        })?;

    let socket = tokio::net::UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| (502, "udp_bind_failed".to_string(), e.to_string()))?;
    let socket = Arc::new(socket);

    let state = build_client_state();
    let local_cid_len = state.local_connection_id.bytes.len();
    let ctx = literbike::concurrency::ccek::CoroutineContext::new();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        state,
        socket.clone(),
        remote_addr,
        vec![],
        ctx,
    ));
    let stream_id = engine.create_stream();

    let payload = match protocol_mode {
        RequestProtocolMode::Http1OverQuic => {
            build_http1_over_quic_payload(conn, method, path, headers_json, body)
                .map_err(|e| (400, "invalid_request".to_string(), e))?
        }
        RequestProtocolMode::Http3 => {
            return Err((
                501,
                "http3_not_implemented".to_string(),
                "HTTP/3 framing/QPACK path is not implemented yet; use protocol_mode=1 (HTTP/1.1-over-QUIC stream)"
                    .to_string(),
            ));
        }
    };

    let timeout = Duration::from_millis(timeout_ms.max(1) as u64);
    tokio::time::timeout(timeout, engine.send_stream_data(stream_id, payload))
        .await
        .map_err(|_| {
            (
                504,
                "send_timeout".to_string(),
                "timed out sending QUIC packet".into(),
            )
        })?
        .map_err(|e| (502, "send_failed".to_string(), e.to_string()))?;

    let deadline = Instant::now() + timeout;
    let mut buf = vec![0u8; 65536];
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err((
                504,
                "receive_timeout".to_string(),
                "timed out waiting for QUIC response".into(),
            ));
        }
        let remaining = deadline.saturating_duration_since(now);
        let (len, from_addr) = tokio::time::timeout(remaining, socket.recv_from(&mut buf))
            .await
            .map_err(|_| {
                (
                    504,
                    "receive_timeout".to_string(),
                    "timed out waiting for QUIC datagram".into(),
                )
            })?
            .map_err(|e| (502, "receive_failed".to_string(), e.to_string()))?;

        if from_addr != remote_addr {
            continue;
        }

        let decoded_packet =
            deserialize_decoded_packet_with_dcid_len(&buf[..len], Some(local_cid_len))
                .map_err(|e| (502, "decode_failed".to_string(), e.to_string()))?;
        engine
            .process_decoded_packet(decoded_packet)
            .await
            .map_err(|e| (502, "process_failed".to_string(), e.to_string()))?;

        if let Some(stream) = engine.get_stream(stream_id) {
            if !stream.receive_buffer.is_empty() {
                let status = parse_http_status(&stream.receive_buffer).unwrap_or(200);
                return Ok(LBQuicResponse {
                    status,
                    body: stream.receive_buffer,
                });
            }
        }
    }
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
    timeout_ms: u32,
) -> *mut LBQuicResponse {
    quic_request_ex(
        conn,
        method,
        path,
        headers_json,
        body_ptr,
        body_len,
        timeout_ms,
        QUIC_REQUEST_PROTOCOL_AUTO,
    )
}

#[no_mangle]
pub extern "C" fn quic_request_ex(
    conn: *mut LBQuicConnection,
    method: *const c_char,
    path: *const c_char,
    headers_json: *const c_char,
    body_ptr: *const u8,
    body_len: usize,
    timeout_ms: u32,
    protocol_mode: u32,
) -> *mut LBQuicResponse {
    if conn.is_null() {
        set_last_error("connection handle is null");
        return ptr::null_mut();
    }

    let method = match read_cstr(method, "method") {
        Ok(v) => v,
        Err(()) => return ptr::null_mut(),
    };
    let path = match read_cstr(path, "path") {
        Ok(v) => v,
        Err(()) => return ptr::null_mut(),
    };

    let headers_json = if headers_json.is_null() {
        None
    } else {
        match read_cstr(headers_json, "headers_json") {
            Ok(v) => Some(v),
            Err(()) => return ptr::null_mut(),
        }
    };

    if body_ptr.is_null() && body_len != 0 {
        set_last_error("body_ptr is null but body_len is non-zero");
        return ptr::null_mut();
    }

    let body = if !body_ptr.is_null() && body_len != 0 {
        unsafe { slice::from_raw_parts(body_ptr, body_len) }.to_vec()
    } else {
        Vec::new()
    };

    let conn_ref = unsafe { &*conn };
    let effective_timeout_ms = if timeout_ms == 0 {
        conn_ref.timeout_ms
    } else {
        timeout_ms
    };
    let protocol_mode = match RequestProtocolMode::from_abi(protocol_mode) {
        Ok(mode) => mode,
        Err(msg) => {
            set_last_error(&msg);
            return make_error_response(400, "invalid_protocol_mode", &msg);
        }
    };

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let msg = format!("failed to create Tokio runtime: {e}");
            set_last_error(&msg);
            return make_error_response(500, "runtime_init_failed", &msg);
        }
    };

    match runtime.block_on(execute_quic_request(
        conn_ref,
        &method,
        &path,
        headers_json.as_deref(),
        &body,
        effective_timeout_ms,
        protocol_mode,
    )) {
        Ok(resp) => {
            set_last_error("literbike-quic-capi: no error");
            Box::into_raw(Box::new(resp))
        }
        Err((status, code, detail)) => {
            let msg = format!("{code}: {detail}");
            set_last_error(&msg);
            make_error_response(status, &code, &detail)
        }
    }
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

#[no_mangle]
pub extern "C" fn quic_protocol_mode_http1_over_quic() -> u32 {
    QUIC_REQUEST_PROTOCOL_HTTP1_OVER_QUIC
}

#[no_mangle]
pub extern "C" fn quic_protocol_mode_http3() -> u32 {
    QUIC_REQUEST_PROTOCOL_HTTP3
}

// ============================================================================
// Stream Management C ABI (for Agent Harness)
// ============================================================================

/// Stream priority levels for multiplexing and scheduling
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LBQuicStreamPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Opaque handle for a QUIC stream
#[repr(C)]
pub struct LBQuicStream {
    stream_id: u64,
    priority: LBQuicStreamPriority,
    engine: Arc<QuicEngine>,
    remote_addr: SocketAddr,
}

#[no_mangle]
pub extern "C" fn quic_stream_create(
    conn: *mut LBQuicConnection,
    priority: LBQuicStreamPriority,
) -> *mut LBQuicStream {
    if conn.is_null() {
        set_last_error("connection handle is null");
        return ptr::null_mut();
    }

    let conn_ref = unsafe { &*conn };
    
    // Create a temporary engine for this stream
    // In a real implementation, you'd want to maintain a connection pool
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let msg = format!("failed to create Tokio runtime: {e}");
            set_last_error(&msg);
            return ptr::null_mut();
        }
    };

    let remote_addr = match (conn_ref.host.as_str(), conn_ref.port)
        .to_socket_addrs()
        .map_err(|e| {
            set_last_error(format!("failed to resolve host: {e}"));
        })
        .and_then(|mut addrs| addrs.next().ok_or_else(|| {
            set_last_error("no socket address resolved".to_string());
        })) {
        Ok(addr) => addr,
        Err(_) => return ptr::null_mut(),
    };

    let socket = match runtime.block_on(async {
        tokio::net::UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| {
                set_last_error(format!("failed to bind UDP socket: {e}"));
            })
    }) {
        Ok(s) => Arc::new(s),
        Err(_) => return ptr::null_mut(),
    };

    let state = build_client_state();
    let ctx = literbike::concurrency::ccek::CoroutineContext::new();
    let engine = Arc::new(QuicEngine::new(
        Role::Client,
        state,
        socket,
        remote_addr,
        vec![],
        ctx,
    ));

    let stream_id = match priority {
        LBQuicStreamPriority::Critical => engine.create_stream_with_priority(
            literbike::quic::quic_protocol::StreamPriority::Critical
        ),
        LBQuicStreamPriority::High => engine.create_stream_with_priority(
            literbike::quic::quic_protocol::StreamPriority::High
        ),
        LBQuicStreamPriority::Low => engine.create_stream_with_priority(
            literbike::quic::quic_protocol::StreamPriority::Low
        ),
        _ => engine.create_stream_with_priority(
            literbike::quic::quic_protocol::StreamPriority::Normal
        ),
    };

    set_last_error("literbike-quic-capi: no error");
    Box::into_raw(Box::new(LBQuicStream {
        stream_id,
        priority,
        engine,
        remote_addr,
    }))
}

#[no_mangle]
pub extern "C" fn quic_stream_send(
    stream: *mut LBQuicStream,
    data_ptr: *const u8,
    data_len: usize,
) -> bool {
    if stream.is_null() {
        set_last_error("stream handle is null");
        return false;
    }

    if data_ptr.is_null() && data_len != 0 {
        set_last_error("data_ptr is null but data_len is non-zero");
        return false;
    }

    let stream_ref = unsafe { &*stream };
    let data = if !data_ptr.is_null() && data_len != 0 {
        unsafe { slice::from_raw_parts(data_ptr, data_len) }.to_vec()
    } else {
        Vec::new()
    };

    let priority = match stream_ref.priority {
        LBQuicStreamPriority::Critical => literbike::quic::quic_protocol::StreamPriority::Critical,
        LBQuicStreamPriority::High => literbike::quic::quic_protocol::StreamPriority::High,
        LBQuicStreamPriority::Low => literbike::quic::quic_protocol::StreamPriority::Low,
        _ => literbike::quic::quic_protocol::StreamPriority::Normal,
    };

    let engine = stream_ref.engine.clone();
    let stream_id = stream_ref.stream_id;

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let msg = format!("failed to create Tokio runtime: {e}");
            set_last_error(&msg);
            return false;
        }
    };

    match runtime.block_on(async {
        engine.send_stream_data_priority(stream_id, data, priority).await
    }) {
        Ok(()) => {
            set_last_error("literbike-quic-capi: no error");
            true
        }
        Err(e) => {
            set_last_error(format!("failed to send stream data: {e}"));
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn quic_stream_close(stream: *mut LBQuicStream) {
    if !stream.is_null() {
        unsafe {
            drop(Box::from_raw(stream));
        }
    }
}

#[no_mangle]
pub extern "C" fn quic_stream_finish(stream: *mut LBQuicStream) -> bool {
    if stream.is_null() {
        set_last_error("stream handle is null");
        return false;
    }

    let stream_ref = unsafe { &*stream };
    let engine = stream_ref.engine.clone();
    let stream_id = stream_ref.stream_id;

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let msg = format!("failed to create Tokio runtime: {e}");
            set_last_error(&msg);
            return false;
        }
    };

    match runtime.block_on(async {
        engine.send_stream_fin(stream_id).await
    }) {
        Ok(()) => {
            set_last_error("literbike-quic-capi: no error");
            true
        }
        Err(e) => {
            set_last_error(format!("failed to finish stream: {e}"));
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn quic_stream_set_priority(
    stream: *mut LBQuicStream,
    priority: LBQuicStreamPriority,
) {
    if stream.is_null() {
        set_last_error("stream handle is null");
        return;
    }

    let stream_ref = unsafe { &mut *stream };
    stream_ref.priority = priority;
    
    let quic_priority = match priority {
        LBQuicStreamPriority::Critical => literbike::quic::quic_protocol::StreamPriority::Critical,
        LBQuicStreamPriority::High => literbike::quic::quic_protocol::StreamPriority::High,
        LBQuicStreamPriority::Low => literbike::quic::quic_protocol::StreamPriority::Low,
        _ => literbike::quic::quic_protocol::StreamPriority::Normal,
    };
    
    stream_ref.engine.set_stream_priority(stream_ref.stream_id, quic_priority);
    set_last_error("literbike-quic-capi: no error");
}

#[no_mangle]
pub extern "C" fn quic_stream_get_id(stream: *const LBQuicStream) -> u64 {
    if stream.is_null() {
        set_last_error("stream handle is null");
        return 0;
    }
    unsafe { (*stream).stream_id }
}

// ============================================================================
// Connection Lifecycle Management C ABI
// ============================================================================

#[no_mangle]
pub extern "C" fn quic_connection_status(conn: *const LBQuicConnection) -> u32 {
    if conn.is_null() {
        return 0; // Disconnected
    }
    // For now, return connected status
    // In a full implementation, this would check the actual connection state
    1 // Connected
}

#[no_mangle]
pub extern "C" fn quic_idle_timeout(conn: *mut LBQuicConnection) -> bool {
    if conn.is_null() {
        return false;
    }
    // In a full implementation, this would check the engine's idle timeout
    // For now, return false (not timed out)
    false
}

#[no_mangle]
pub extern "C" fn quic_disconnect(conn: *mut LBQuicConnection) {
    if !conn.is_null() {
        unsafe {
            drop(Box::from_raw(conn));
        }
    }
}

// ============================================================================
// DHT Service C ABI
// ============================================================================

#[no_mangle]
pub extern "C" fn quic_dht_service_new(local_peer_id_b58: *const c_char) -> *mut DhtService {
    let peer_id_str = match read_cstr(local_peer_id_b58, "local_peer_id_b58") {
        Ok(s) => s,
        Err(()) => return ptr::null_mut(),
    };

    let peer_id = match PeerId::from_base58(&peer_id_str) {
        Some(id) => id,
        None => {
            set_last_error("invalid base58 for local_peer_id");
            return ptr::null_mut();
        }
    };

    set_last_error("literbike-quic-capi: no error");
    Box::into_raw(Box::new(DhtService::new(peer_id)))
}

#[no_mangle]
pub extern "C" fn quic_dht_service_free(service: *mut DhtService) {
    if !service.is_null() {
        unsafe {
            drop(Box::from_raw(service));
        }
    }
}

#[no_mangle]
pub extern "C" fn quic_dht_add_peer(service: *mut DhtService, peer_json: *const c_char) -> bool {
    if service.is_null() {
        set_last_error("dht service handle is null");
        return false;
    }

    let peer_json_str = match read_cstr(peer_json, "peer_json") {
        Ok(s) => s,
        Err(()) => return false,
    };

    let peer: PeerInfo = match serde_json::from_str(&peer_json_str) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("failed to parse peer_json: {e}"));
            return false;
        }
    };

    let service_ref = unsafe { &*service };
    service_ref.add_peer(peer);
    true
}

#[no_mangle]
pub extern "C" fn quic_dht_get_peer(
    service: *mut DhtService,
    peer_id_b58: *const c_char,
) -> *mut c_char {
    if service.is_null() {
        set_last_error("dht service handle is null");
        return ptr::null_mut();
    }

    let peer_id_str = match read_cstr(peer_id_b58, "peer_id_b58") {
        Ok(s) => s,
        Err(()) => return ptr::null_mut(),
    };

    let peer_id = match PeerId::from_base58(&peer_id_str) {
        Some(id) => id,
        None => {
            set_last_error("invalid base58 for peer_id");
            return ptr::null_mut();
        }
    };

    let service_ref = unsafe { &*service };
    if let Some(peer) = service_ref.get_peer(&peer_id) {
        let json = serde_json::to_string(&peer).unwrap_or_default();
        return CString::new(json).unwrap().into_raw();
    }

    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn quic_dht_closest_peers(
    service: *mut DhtService,
    peer_id_b58: *const c_char,
    count: usize,
) -> *mut c_char {
    if service.is_null() {
        set_last_error("dht service handle is null");
        return ptr::null_mut();
    }

    let peer_id_str = match read_cstr(peer_id_b58, "peer_id_b58") {
        Ok(s) => s,
        Err(()) => return ptr::null_mut(),
    };

    let peer_id = match PeerId::from_base58(&peer_id_str) {
        Some(id) => id,
        None => {
            set_last_error("invalid base58 for peer_id");
            return ptr::null_mut();
        }
    };

    let service_ref = unsafe { &*service };
    let peers = service_ref.closest_peers(&peer_id, count);
    let json = serde_json::to_string(&peers).unwrap_or_default();
    CString::new(json).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn quic_dht_string_free(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            drop(CString::from_raw(s));
        }
    }
}

// ============================================================================
// DHT Persistence Callback FFI
// ============================================================================

pub type DhtPersistenceCallback = extern "C" fn(op: *const c_char, json_data: *const c_char);

struct DhtFfiPersistence {
    callback: DhtPersistenceCallback,
}

impl literbike::dht::service::DhtPersistence for DhtFfiPersistence {
    fn upsert_node(&self, peer: &PeerInfo) {
        let json = serde_json::to_string(peer).unwrap_or_default();
        let op = CString::new("upsert_node").unwrap();
        let data = CString::new(json).unwrap();
        (self.callback)(op.as_ptr(), data.as_ptr());
    }

    fn load_nodes(&self) -> Vec<PeerInfo> {
        // For P0, we assume rehydration is handled by Python calling quic_dht_add_peer
        // rather than Rust requesting nodes via callback.
        Vec::new()
    }

    fn upsert_value(&self, key: &str, value: &[u8]) {
        // Serialize as simple object for FFI
        let json = serde_json::json!({
            "key": key,
            "value_hex": hex::encode(value)
        })
        .to_string();
        let op = CString::new("upsert_value").unwrap();
        let data = CString::new(json).unwrap();
        (self.callback)(op.as_ptr(), data.as_ptr());
    }
}

#[no_mangle]
pub extern "C" fn quic_dht_service_set_persistence(
    service: *mut DhtService,
    callback: DhtPersistenceCallback,
) {
    if service.is_null() {
        return;
    }
    let service_ref = unsafe { &mut *service };
    let persistence = Arc::new(DhtFfiPersistence { callback });
    service_ref.set_persistence(persistence);
}

#[cfg(test)]
mod tests {
    use super::*;
    use literbike::quic::QuicServer;
    use std::ffi::CString;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    fn spawn_echo_server() -> (SocketAddr, thread::JoinHandle<()>) {
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async move {
                let ctx = literbike::concurrency::ccek::CoroutineContext::new();
                let server = QuicServer::bind("127.0.0.1:0".parse().unwrap(), ctx)
                    .await
                    .expect("bind quic server");
                let local = tokio::task::LocalSet::new();
                local
                    .run_until(async move {
                        server.start().await.expect("start quic server");
                        let addr = server.local_addr().expect("server local addr");
                        tx.send(addr).expect("send server addr");
                        tokio::time::sleep(Duration::from_millis(700)).await;
                    })
                    .await;
            });
        });

        let addr = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("receive server addr");
        (addr, handle)
    }

    #[test]
    fn connect_and_request_returns_transport_error_response_without_server() {
        let host = CString::new("127.0.0.1").unwrap();
        let method = CString::new("GET").unwrap();
        let path = CString::new("/api/v1/health").unwrap();
        let headers = CString::new("{}").unwrap();

        let conn = quic_connect(host.as_ptr(), 9, 50);
        assert!(!conn.is_null());

        let resp = quic_request(
            conn,
            method.as_ptr(),
            path.as_ptr(),
            headers.as_ptr(),
            ptr::null(),
            0,
            50,
        );
        assert!(!resp.is_null());
        assert_eq!(quic_response_status(resp), 504);
        assert!(quic_response_body_len(resp) > 0);

        quic_response_free(resp);
        quic_close(conn);
    }

    #[test]
    fn quic_request_ex_rejects_unknown_protocol_mode() {
        let host = CString::new("127.0.0.1").unwrap();
        let method = CString::new("GET").unwrap();
        let path = CString::new("/").unwrap();
        let headers = CString::new("{}").unwrap();

        let conn = quic_connect(host.as_ptr(), 9, 50);
        assert!(!conn.is_null());

        let resp = quic_request_ex(
            conn,
            method.as_ptr(),
            path.as_ptr(),
            headers.as_ptr(),
            ptr::null(),
            0,
            50,
            999,
        );
        assert!(!resp.is_null());
        assert_eq!(quic_response_status(resp), 400);
        let body_len = quic_response_body_len(resp);
        let body_ptr = quic_response_body_ptr(resp);
        let body = unsafe { slice::from_raw_parts(body_ptr, body_len) };
        let body_text = String::from_utf8_lossy(body);
        assert!(body_text.contains("invalid_protocol_mode"));
        quic_response_free(resp);
        quic_close(conn);
    }

    #[test]
    fn quic_request_ex_http3_mode_returns_not_implemented() {
        let host = CString::new("127.0.0.1").unwrap();
        let method = CString::new("GET").unwrap();
        let path = CString::new("/").unwrap();
        let headers = CString::new("{}").unwrap();

        let conn = quic_connect(host.as_ptr(), 9, 50);
        assert!(!conn.is_null());

        let resp = quic_request_ex(
            conn,
            method.as_ptr(),
            path.as_ptr(),
            headers.as_ptr(),
            ptr::null(),
            0,
            50,
            quic_protocol_mode_http3(),
        );
        assert!(!resp.is_null());
        assert_eq!(quic_response_status(resp), 501);
        let body_len = quic_response_body_len(resp);
        let body_ptr = quic_response_body_ptr(resp);
        let body = unsafe { slice::from_raw_parts(body_ptr, body_len) };
        let body_text = String::from_utf8_lossy(body);
        assert!(body_text.contains("http3_not_implemented"));
        quic_response_free(resp);
        quic_close(conn);
    }

    #[test]
    fn connect_and_request_roundtrip_with_local_quic_echo_server() {
        let (addr, handle) = spawn_echo_server();

        let host = CString::new(addr.ip().to_string()).unwrap();
        let method = CString::new("GET").unwrap();
        let path = CString::new("/quic-echo").unwrap();
        let headers = CString::new("{\"accept\":\"application/json\"}").unwrap();

        let conn = quic_connect(host.as_ptr(), addr.port(), 500);
        assert!(!conn.is_null());

        let resp = quic_request(
            conn,
            method.as_ptr(),
            path.as_ptr(),
            headers.as_ptr(),
            ptr::null(),
            0,
            500,
        );
        assert!(!resp.is_null());
        assert_eq!(quic_response_status(resp), 200);

        let body_len = quic_response_body_len(resp);
        assert!(body_len > 0);
        let body_ptr = quic_response_body_ptr(resp);
        assert!(!body_ptr.is_null());
        let body = unsafe { slice::from_raw_parts(body_ptr, body_len) };
        let body_text = String::from_utf8_lossy(body);
        assert!(body_text.contains("GET /quic-echo HTTP/1.1"));

        quic_response_free(resp);
        quic_close(conn);

        handle.join().unwrap();
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
