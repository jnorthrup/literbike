//! modelmux — desktop AI model proxy
//!
//! Reads API keys from environment, routes to real providers.
//! Replaces opencode/claude shell with a proxified model layer.
//!
//! Usage:
//!   cargo run --bin modelmux
//!   MODELMUX_PORT=8888 cargo run --bin modelmux
//!
//! Providers activated by env vars:
//!   ANTHROPIC_API_KEY, OPENAI_API_KEY, GOOGLE_API_KEY,
//!   GROQ_API_KEY, OPENROUTER_API_KEY, MISTRAL_API_KEY,
//!   XAI_API_KEY, CEREBRAS_API_KEY

use literbike::modelmux::proxy::{ModelProxy, ProxyConfig};
use log::info;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let port: u16 = std::env::var("MODELMUX_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8888);

    let config = ProxyConfig {
        bind_address: "127.0.0.1".to_string(),
        port,
        enable_streaming: true,
        enable_caching: true,
        default_model: std::env::var("MODELMUX_DEFAULT_MODEL").ok(),
        fallback_model: std::env::var("MODELMUX_FALLBACK_MODEL")
            .ok()
            .or_else(|| {
                // auto-select first available key as fallback
                [
                    ("ANTHROPIC_API_KEY", "anthropic/claude-haiku-4-5-20251001"),
                    ("OPENAI_API_KEY",    "openai/gpt-4o-mini"),
                    ("GROQ_API_KEY",      "groq/llama-3.1-8b-instant"),
                    ("OPENROUTER_API_KEY","openrouter/meta-llama/llama-3.1-8b-instruct:free"),
                ]
                .iter()
                .find(|(k, _)| std::env::var(k).is_ok())
                .map(|(_, m)| m.to_string())
            }),
        request_timeout_secs: 120,
        max_retries: 2,
    };

    let mut proxy = ModelProxy::new(config);
    proxy.init_from_env(None).await.unwrap_or_else(|e| {
        log::warn!("env init: {}", e);
    });

    // Log which providers are live
    let keys = [
        ("ANTHROPIC_API_KEY",  "anthropic"),
        ("OPENAI_API_KEY",     "openai"),
        ("GOOGLE_API_KEY",     "google"),
        ("GROQ_API_KEY",       "groq"),
        ("OPENROUTER_API_KEY", "openrouter"),
        ("MISTRAL_API_KEY",    "mistral"),
        ("XAI_API_KEY",        "xai"),
        ("CEREBRAS_API_KEY",   "cerebras"),
    ];
    let active: Vec<&str> = keys.iter()
        .filter(|(k, _)| std::env::var(k).is_ok())
        .map(|(_, p)| *p)
        .collect();

    info!("modelmux starting on http://127.0.0.1:{}", port);
    info!("active providers: {}", if active.is_empty() { "none (set API key env vars)".to_string() } else { active.join(", ") });
    info!("opencode literbike provider: http://127.0.0.1:{}", port);

    if let Err(e) = proxy.start_server().await {
        log::error!("modelmux server error: {}", e);
        std::process::exit(1);
    }
}
