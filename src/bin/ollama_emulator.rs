//! Ollama Emulator - With Quota Tracking

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::sync::Arc;

const HF_MODELS_ENDPOINT: &str = "https://huggingface.co/api/models";
const DEFAULT_HF_MODEL_LIMIT: usize = 25;
const MAX_HF_MODEL_LIMIT: usize = 100;

#[derive(Debug, Clone, Serialize)]
pub struct Model {
    id: String,
    owned_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    upstream_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pipeline_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    downloads: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    likes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    card_data: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProviderUsage {
    total_tokens: u64,
    total_cost: f64,
    request_count: u64,
}

#[derive(Debug, Clone)]
struct ProviderModel {
    id: String,
    owned_by: String,
    upstream_id: Option<String>,
    modified_at: Option<String>,
    pipeline_tag: Option<String>,
    downloads: Option<u64>,
    likes: Option<u64>,
    tags: Vec<String>,
    card_data: Option<Value>,
}

#[derive(Debug, Clone)]
struct Provider {
    id: String,
    api_key: String,
    base_url: String,
    usage: ProviderUsage,
    models: Vec<ProviderModel>,
}

#[derive(Debug, Clone)]
struct State {
    providers: Vec<Provider>,
}

#[derive(Debug, Clone, Deserialize)]
struct HuggingFaceModelRecord {
    #[serde(alias = "modelId")]
    id: String,
    author: Option<String>,
    #[serde(rename = "lastModified")]
    last_modified: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    pipeline_tag: Option<String>,
    #[serde(default)]
    downloads: Option<u64>,
    #[serde(default)]
    likes: Option<u64>,
    #[serde(rename = "cardData", default)]
    card_data: Option<Value>,
}

impl ProviderModel {
    fn synthetic(provider_id: &str, suffix: &str) -> Self {
        Self {
            id: format!("{provider_id}/{suffix}"),
            owned_by: provider_id.to_string(),
            upstream_id: None,
            modified_at: Some("2023-11-01T00:00:00Z".to_string()),
            pipeline_tag: Some("text-generation".to_string()),
            downloads: None,
            likes: None,
            tags: vec![],
            card_data: None,
        }
    }

    fn to_openai_model(&self) -> Model {
        Model {
            id: self.id.clone(),
            owned_by: self.owned_by.clone(),
            upstream_id: self.upstream_id.clone(),
            pipeline_tag: self.pipeline_tag.clone(),
            downloads: self.downloads,
            likes: self.likes,
            card_data: self.card_data.clone(),
        }
    }

    fn to_ollama_tag(&self) -> Value {
        let mut details = Map::new();
        details.insert(
            "family".to_string(),
            Value::String(
                self.pipeline_tag
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
            ),
        );
        if let Some(downloads) = self.downloads {
            details.insert("downloads".to_string(), json!(downloads));
        }
        if let Some(likes) = self.likes {
            details.insert("likes".to_string(), json!(likes));
        }
        if !self.tags.is_empty() {
            details.insert("tags".to_string(), json!(self.tags));
        }
        if let Some(card_data) = &self.card_data {
            details.insert("card_data".to_string(), card_data.clone());
            if let Some(license) = card_data.get("license").and_then(Value::as_str) {
                details.insert("license".to_string(), Value::String(license.to_string()));
            }
        }

        json!({
            "name": format!("{}:latest", self.id),
            "model": format!("{}:latest", self.id),
            "modified_at": self
                .modified_at
                .clone()
                .unwrap_or_else(|| "2023-11-01T00:00:00Z".to_string()),
            "size": 0,
            "digest": "metadata",
            "details": Value::Object(details),
        })
    }
}

impl State {
    async fn new() -> Self {
        let client = reqwest::Client::new();
        let urls = [
            ("OPENAI", "https://api.openai.com/v1"),
            ("GROQ", "https://api.groq.com/openai/v1"),
            ("DEEPSEEK", "https://api.deepseek.com"),
            ("MOONSHOT", "https://api.moonshot.cn/v1"),
            ("XAI", "https://api.x.ai/v1"),
            ("PERPLEXITY", "https://api.perplexity.ai"),
            ("OPENROUTER", "https://openrouter.ai/api/v1"),
            ("NVIDIA", "https://integrate.api.nvidia.com/v1"),
            ("CEREBRAS", "https://api.cerebras.ai/v1"),
            ("HUGGINGFACE", "https://api-inference.huggingface.co/v1"),
            ("KILO", "https://api.kilo.ai/v1"),
            ("KILOCODE", "https://api.kilocode.ai/v1"),
        ];

        let mut providers = Vec::new();
        for (env, url) in urls {
            let Ok(api_key) = std::env::var(format!("{}_API_KEY", env)) else {
                continue;
            };
            let id = env.to_lowercase();
            let models = if id == "huggingface" {
                match fetch_huggingface_models(&client, &api_key).await {
                    Ok(models) if !models.is_empty() => models,
                    Ok(_) => {
                        log::warn!("Hugging Face model listing returned no models; using synthetic fallback");
                        default_models(&id)
                    }
                    Err(err) => {
                        log::warn!("Failed to fetch Hugging Face model cards: {err}");
                        default_models(&id)
                    }
                }
            } else {
                default_models(&id)
            };

            providers.push(Provider {
                id,
                api_key,
                base_url: url.to_string(),
                usage: ProviderUsage::default(),
                models,
            });
        }

        State { providers }
    }

    fn total_usage(&self) -> (u64, f64) {
        (
            self.providers.iter().map(|p| p.usage.total_tokens).sum(),
            self.providers.iter().map(|p| p.usage.total_cost).sum(),
        )
    }
}

fn default_models(provider_id: &str) -> Vec<ProviderModel> {
    vec![
        ProviderModel::synthetic(provider_id, &format!("{provider_id}-model")),
        ProviderModel::synthetic(provider_id, "default"),
    ]
}

fn huggingface_model_limit() -> usize {
    std::env::var("HUGGINGFACE_MODEL_LIMIT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .map(|limit| limit.clamp(1, MAX_HF_MODEL_LIMIT))
        .unwrap_or(DEFAULT_HF_MODEL_LIMIT)
}

fn huggingface_owner(record: &HuggingFaceModelRecord) -> String {
    record
        .author
        .clone()
        .or_else(|| record.id.split('/').next().map(str::to_string))
        .unwrap_or_else(|| "huggingface".to_string())
}

async fn fetch_huggingface_models(
    client: &reqwest::Client,
    api_key: &str,
) -> Result<Vec<ProviderModel>, reqwest::Error> {
    let response = client
        .get(HF_MODELS_ENDPOINT)
        .bearer_auth(api_key)
        .query(&[
            ("limit", huggingface_model_limit().to_string()),
            ("full", "true".to_string()),
            ("cardData", "true".to_string()),
            ("sort", "downloads".to_string()),
            ("direction", "-1".to_string()),
        ])
        .send()
        .await?
        .error_for_status()?;

    let records = response.json::<Vec<HuggingFaceModelRecord>>().await?;
    Ok(records
        .into_iter()
        .map(|record| ProviderModel {
            id: format!("huggingface/{}", record.id),
            owned_by: huggingface_owner(&record),
            upstream_id: Some(record.id),
            modified_at: record.last_modified,
            pipeline_tag: record.pipeline_tag,
            downloads: record.downloads,
            likes: record.likes,
            tags: record.tags,
            card_data: record.card_data,
        })
        .collect())
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let port: u16 = std::env::args()
        .skip_while(|a| a != "--port")
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(8888);
    let state = Arc::new(State::new().await);
    log::info!("Ollama Emulator: {} providers", state.providers.len());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("bind");
    log::info!("Listening on 0.0.0.0:{port}");
    loop {
        let (stream, _) = listener.accept().await.expect("accept");
        tokio::spawn(handle(stream, Arc::clone(&state)));
    }
}

async fn handle(mut stream: tokio::net::TcpStream, state: Arc<State>) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut reader = BufReader::new(&mut stream);
    let mut line = String::new();
    if reader.read_line(&mut line).await.is_err() {
        return;
    }
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0].to_string();
    let path = parts[1].to_string();
    loop {
        line.clear();
        if reader.read_line(&mut line).await.is_err() || line.trim().is_empty() {
            break;
        }
    }

    let body = match (method.as_str(), path.as_str()) {
        ("GET", "/") => r#""Ollama is running""#.into(),
        ("GET", "/api/version") => r#"{"version": "0.1.24"}"#.into(),
        ("GET", "/api/tags") => {
            let ollama_models: Vec<Value> = state
                .providers
                .iter()
                .flat_map(|provider| provider.models.iter().map(ProviderModel::to_ollama_tag))
                .collect();
            serde_json::to_string(&json!({ "models": ollama_models })).unwrap()
        }
        ("GET", "/v1/models") | ("GET", "/models") => {
            let models: Vec<Model> = state
                .providers
                .iter()
                .flat_map(|provider| provider.models.iter().map(ProviderModel::to_openai_model))
                .collect();
            serde_json::to_string(&json!({ "object": "list", "data": models })).unwrap()
        }
        ("GET", "/quota") | ("GET", "/usage") => {
            let (tokens, cost) = state.total_usage();
            serde_json::to_string(&json!({
                "object": "quota",
                "total_tokens": tokens,
                "total_cost_usd": cost,
                "providers": state.providers.iter().map(|p| json!({
                    "id": p.id,
                    "base_url": p.base_url,
                    "configured": !p.api_key.is_empty(),
                    "model_count": p.models.len(),
                    "tokens": p.usage.total_tokens,
                    "cost_usd": p.usage.total_cost,
                    "requests": p.usage.request_count
                })).collect::<Vec<_>>()
            }))
            .unwrap()
        }
        ("GET", "/health") => {
            let (tokens, cost) = state.total_usage();
            serde_json::to_string(&json!({
                "status": "ready",
                "providers": state.providers.len(),
                "total_tokens": tokens,
                "total_cost_usd": cost
            }))
            .unwrap()
        }
        _ => r#"{"error":"not found"}"#.into(),
    };
    let _ = stream
        .write_all(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .as_bytes(),
        )
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huggingface_limit_defaults_and_caps() {
        unsafe {
            std::env::remove_var("HUGGINGFACE_MODEL_LIMIT");
        }
        assert_eq!(huggingface_model_limit(), DEFAULT_HF_MODEL_LIMIT);

        unsafe {
            std::env::set_var("HUGGINGFACE_MODEL_LIMIT", "999");
        }
        assert_eq!(huggingface_model_limit(), MAX_HF_MODEL_LIMIT);

        unsafe {
            std::env::set_var("HUGGINGFACE_MODEL_LIMIT", "3");
        }
        assert_eq!(huggingface_model_limit(), 3);
    }

    #[test]
    fn parses_huggingface_model_cards_and_stats() {
        let record: HuggingFaceModelRecord = serde_json::from_value(json!({
            "id": "openai/gpt-oss-20b",
            "author": "openai",
            "lastModified": "2026-03-01T12:00:00.000Z",
            "pipeline_tag": "text-generation",
            "downloads": 123456,
            "likes": 789,
            "tags": ["text-generation", "safetensors"],
            "cardData": {
                "license": "apache-2.0",
                "language": ["en"]
            }
        }))
        .unwrap();

        assert_eq!(record.id, "openai/gpt-oss-20b");
        assert_eq!(record.author.as_deref(), Some("openai"));
        assert_eq!(record.downloads, Some(123456));
        assert_eq!(
            record
                .card_data
                .as_ref()
                .and_then(|v| v.get("license"))
                .and_then(Value::as_str),
            Some("apache-2.0")
        );
    }
}
