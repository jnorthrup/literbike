/// Shared utility functions for model handling.

/// Returns the maximum context window that ModelMux should advertise for
/// newly cached models.  Clients may set the environment variable
/// `MODELMUX_MAX_CONTEXT_WINDOW` to a large number (e.g. 2_000_000) when
/// experimenting with huge contexts; otherwise the default is 128 000 tokens.

pub fn max_context_window() -> u64 {
    std::env::var("MODELMUX_MAX_CONTEXT_WINDOW")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(128_000)
}
