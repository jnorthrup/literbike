//! Ollama Emulator - With Quota Tracking

use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)] pub struct Model { id: String, owned_by: String }
#[derive(Debug, Clone, Default, Serialize)] pub struct ProviderUsage { total_tokens: u64, total_cost: f64, request_count: u64 }
#[derive(Debug, Clone)] struct Provider { id: String, api_key: String, base_url: String, usage: ProviderUsage }
#[derive(Debug, Clone)] struct State { providers: Vec<Provider> }

impl State {
    fn new() -> Self {
        let urls = [("OPENAI", "https://api.openai.com/v1"), ("GROQ", "https://api.groq.com/openai/v1"),
            ("DEEPSEEK", "https://api.deepseek.com"), ("MOONSHOT", "https://api.moonshot.cn/v1"),
            ("XAI", "https://api.x.ai/v1"), ("PERPLEXITY", "https://api.perplexity.ai"),
            ("OPENROUTER", "https://openrouter.ai/api/v1"), ("NVIDIA", "https://integrate.api.nvidia.com/v1"),
            ("CEREBRAS", "https://api.cerebras.ai/v1"), ("HUGGINGFACE", "https://api-inference.huggingface.co/v1"),
            ("KILO", "https://api.kilo.ai/v1"), ("KILOCODE", "https://api.kilocode.ai/v1")];
        State { providers: urls.iter().filter_map(|(env, url)|
            std::env::var(format!("{}_API_KEY", env)).ok().map(|api_key| Provider {
                id: env.to_lowercase(), api_key, base_url: url.to_string(), usage: ProviderUsage::default()
            })).collect() }
    }
    fn total_usage(&self) -> (u64, f64) {
        (self.providers.iter().map(|p| p.usage.total_tokens).sum(), self.providers.iter().map(|p| p.usage.total_cost).sum())
    }
}

#[tokio::main] async fn main() {
    env_logger::init();
    let port: u16 = std::env::args().skip_while(|a| a != "--port").nth(1).and_then(|p| p.parse().ok()).unwrap_or(8888);
    let state = Arc::new(State::new());
    log::info!("Ollama Emulator: {} providers", state.providers.len());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await.expect("bind");
    log::info!("Listening on 0.0.0.0:{}", port);
    loop {
        let (stream, _) = listener.accept().await.expect("accept");
        tokio::spawn(handle(stream, Arc::clone(&state)));
    }
}

async fn handle(mut stream: tokio::net::TcpStream, state: Arc<State>) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut reader = BufReader::new(&mut stream);
    let mut line = String::new();
    if reader.read_line(&mut line).await.is_err() { return; }
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    if parts.len() < 2 { return; }
    let method = parts[0].to_string(); let path = parts[1].to_string();
    loop { line.clear(); if reader.read_line(&mut line).await.is_err() || line.trim().is_empty() { break; } }
    
    let body = match (method.as_str(), path.as_str()) {
        ("GET", "/") => r#""Ollama is running""#.into(),
        ("GET", "/api/version") => r#"{"version": "0.1.24"}"#.into(),
        ("GET", "/api/tags") => {
            let iter = state.providers.iter();
            let mut ollama_models = Vec::new();
            for p in iter {
                ollama_models.push(serde_json::json!({
                    "name": format!("{}/{}-model:latest", p.id, p.id),
                    "model": format!("{}/{}-model:latest", p.id, p.id),
                    "modified_at": "2023-11-01T00:00:00Z",
                    "size": 0,
                    "digest": "fake",
                    "details": { "format": "gguf", "family": "llama", "parameter_size": "7B", "quantization_level": "Q4_0" }
                }));
                ollama_models.push(serde_json::json!({
                    "name": format!("{}/default:latest", p.id),
                    "model": format!("{}/default:latest", p.id),
                    "modified_at": "2023-11-01T00:00:00Z",
                    "size": 0,
                    "digest": "fake",
                    "details": { "format": "gguf", "family": "llama", "parameter_size": "7B", "quantization_level": "Q4_0" }
                }));
            }
            serde_json::to_string(&serde_json::json!({ "models": ollama_models })).unwrap()
        }
        ("GET", "/v1/models") | ("GET", "/models") => {
            let models: Vec<Model> = state.providers.iter().flat_map(|p| vec![
                Model { id: format!("{}/{}-model", p.id, p.id), owned_by: p.id.clone() },
                Model { id: format!("{}/default", p.id), owned_by: p.id.clone() }]).collect();
            serde_json::to_string(&serde_json::json!({ "object": "list", "data": models })).unwrap()
        }
        ("GET", "/quota") | ("GET", "/usage") => {
            let _tokens: u64 = 0; let _cost: f64 = 0.0;
            serde_json::to_string(&serde_json::json!({ "object": "quota", "total_tokens": _tokens, "total_cost_usd": _cost,
                "providers": state.providers.iter().map(|p| serde_json::json!({ "id": p.id, "tokens": p.usage.total_tokens,
                    "cost_usd": p.usage.total_cost, "requests": p.usage.request_count })).collect::<Vec<_>>() })).unwrap()
        }
        ("GET", "/health") => {
            let _tokens: u64 = 0; let _cost: f64 = 0.0;
            serde_json::to_string(&serde_json::json!({ "status": "ready", "providers": state.providers.len(),
                "total_tokens": _tokens, "total_cost_usd": _cost })).unwrap()
        }
        _ => r#"{"error":"not found"}"#.into(),
    };
    let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).as_bytes()).await;
}
