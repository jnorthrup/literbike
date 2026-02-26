use super::quic_error::QuicError;
use serde::Serialize;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
struct FailureEvent<'a> {
    ts_ms: u128,
    component: &'a str,
    category: &'a str,
    message: String,
    context: serde_json::Value,
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn log_path() -> String {
    env::var("HTX_QUIC_FAILURE_LOG").unwrap_or_else(|_| "quic_failures.jsonl".to_string())
}

fn append_line(line: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = writeln!(file, "{}", line);
    }
}

pub fn log_error(
    component: &'static str,
    category: &'static str,
    err: &QuicError,
    context: serde_json::Value,
) {
    let ev = FailureEvent {
        ts_ms: now_ms(),
        component,
        category,
        message: err.to_string(),
        context,
    };
    if let Ok(s) = serde_json::to_string(&ev) {
        append_line(&s);
    }
}

pub fn log_message(
    component: &'static str,
    category: &'static str,
    message: impl AsRef<str>,
    context: serde_json::Value,
) {
    let ev = FailureEvent {
        ts_ms: now_ms(),
        component,
        category,
        message: message.as_ref().to_string(),
        context,
    };
    if let Ok(s) = serde_json::to_string(&ev) {
        append_line(&s);
    }
}
