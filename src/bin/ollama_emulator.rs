//! Ollama Emulator - Discovers real models from providers

use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)] pub struct Model { id: String, owned_by: Option<String> }
#[derive(Debug, Clone)] struct State { providers: Vec<Provider> }
#[derive(Debug, Clone)] struct Provider { id: String, api_key: String, base_url: String }

#[tokio::main]
async fn main() {
    env_logger::init();
    let port: u16 = std::env::args().skip_while(|a| a != "--port").nth(1).and_then(|p| p.parse().ok()).unwrap_or(8888);
    
    // Provider base URLs
    let urls = [
        ("OPENAI", "https://api.openai.com/v1"),
        ("GROQ", "https://api.groq.com/openai/v1"),
        ("DEEPSEEK", "https://api.deepseek.com"),
        ("MOONSHOT", "https://api.moonshot.cn/v1"),
        ("MOONSHOTAI", "https://api.moonshot.cn/v1"),
        ("KIMI", "https://api.moonshot.cn/v1"),
        ("XAI", "https://api.x.ai/v1"),
        ("GROK", "https://api.x.ai/v1"),
        ("PERPLEXITY", "https://api.perplexity.ai"),
        ("OPENROUTER", "https://openrouter.ai/api/v1"),
        ("NVIDIA", "https://integrate.api.nvidia.com/v1"),
        ("CEREBRAS", "https://api.cerebras.ai/v1"),
        ("HUGGINGFACE", "https://api-inference.huggingface.co/v1"),
        ("KILO", "https://api.kilo.ai/v1"),
        ("KILOAI", "https://api.kilo.ai/v1"),
        ("KILOCODE", "https://api.kilocode.ai/v1"),
    ];
    
    let providers: Vec<Provider> = urls.iter()
        .filter_map(|(env, url)| {
            let key_var = format!("{}_API_KEY", env);
            std::env::var(&key_var).ok().map(|api_key| Provider {
                id: env.to_lowercase(),
                api_key,
                base_url: url.to_string(),
            })
        })
        .collect();
    
    let state = Arc::new(State { providers });
    log::info!("Ollama Emulator: {} providers configured", state.providers.len());
    
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
    let method = parts[0].to_string();
    let path = parts[1].to_string();
    
    loop { line.clear(); if reader.read_line(&mut line).await.is_err() || line.trim().is_empty() { break; } }
    
    let client = reqwest::Client::new();
    let body = match (method.as_str(), path.as_str()) {
        ("GET", "/v1/models") | ("GET", "/models") => {
            // Call each provider's /models endpoint and aggregate
            let mut all_models = Vec::new();
            for provider in &state.providers {
                let url = format!("{}/models", provider.base_url);
                if let Ok(resp) = client.get(&url)
                    .header("Authorization", format!("Bearer {}", provider.api_key))
                    .send()
                    .await
                {
                    if resp.status().is_success() {
                        if let Ok(data) = resp.json::<serde_json::Value>().await {
                            if let Some(models) = data.get("data").and_then(|v| v.as_array()) {
                                for model in models {
                                    if let Some(id) = model.get("id").and_then(|v| v.as_str()) {
                                        all_models.push(Model {
                                            id: format!("{}/{}", provider.id, id),
                                            owned_by: Some(provider.id.clone()),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
            serde_json::to_string(&serde_json::json!({ "object": "list", "data": all_models })).unwrap()
        }
        ("GET", "/health") => serde_json::to_string(&serde_json::json!({
            "status": "ready",
            "providers": state.providers.len()
        })).unwrap(),
        _ => r#"{"error":"not found"}"#.into(),
    };
    
    let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).as_bytes()).await;
}
