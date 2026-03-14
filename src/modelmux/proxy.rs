//! Model Proxy for ModelMux
//!
//! Proxies requests to multiple model providers with unified OpenAI-compatible API.
//! Similar to Kilo.ai Gateway, LMStudio, and Ollama server.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::RwLock;
use log::{info, warn, error, debug};

use crate::modelmux::cache::{CachedModel, ModelCache};
use crate::modelmux::control::{GatewayControlAction, GatewayControlState, GatewayRuntimeControl};
use crate::modelmux::registry::{ModelRegistry, ProviderEntry};
use crate::modelmux::metamodel::{MetamodelCache, Metamodel};
use crate::modelmux::toolbar::{derive_toolbar_state, ToolbarAction, ToolbarState};
use crate::keymux::dsel::{DSELBuilder, RuleEngine, QuotaContainer};
use crate::keymux::cards::ModelCardStore;

/// Proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub bind_address: String,
    pub port: u16,
    pub enable_streaming: bool,
    pub enable_caching: bool,
    pub default_model: Option<String>,
    pub fallback_model: Option<String>,
    pub request_timeout_secs: u64,
    pub max_retries: u32,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 11434, // Ollama-compatible port
            enable_streaming: true,
            enable_caching: true,
            default_model: None,
            fallback_model: None,
            request_timeout_secs: 120,
            max_retries: 2,
        }
    }
}

/// Proxy route configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyRoute {
    pub path: String,
    pub method: String,
    pub handler: String,
    pub providers: Vec<String>,
}

/// Model proxy state
pub struct ModelProxy {
    config: ProxyConfig,
    registry: Arc<ModelRegistry>,
    cache: Arc<RwLock<ModelCache>>,
    rule_engine: Arc<RwLock<RuleEngine>>,
    control: Arc<RwLock<GatewayRuntimeControl>>,
    http_client: reqwest::Client,
    metacache: Arc<RwLock<crate::models::metamodel::MetamodelCache>>,
    card_store: Arc<ModelCardStore>,
}

impl ModelProxy {
    pub fn new(config: ProxyConfig) -> Self {
        let registry = Arc::new(ModelRegistry::new());
        let cache = Arc::new(RwLock::new(ModelCache::with_defaults()));
        
        // metamodel cache (CAS + replication)
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let meta_dir = home.join(".modelmux").join("metacache");
        let metacache = Arc::new(RwLock::new(
            crate::models::metamodel::MetamodelCache::new(meta_dir)
        ));
        
        // Initialize DSEL rule engine with quota management
        let rule_engine = Arc::new(RwLock::new(
            DSELBuilder::new()
                .with_quota("modelmux_pool", 10_000_000)
                .with_free_provider("kilo_code", 1_000_000, 1, 100_000, 3_000_000, 0)
                .with_free_provider("moonshot", 500_000, 2, 50_000, 1_500_000, 0)
                .with_free_provider("deepseek", 500_000, 2, 50_000, 1_500_000, 0)
                .with_free_provider("nvidia", 500_000, 2, 50_000, 1_500_000, 0)
                .with_free_provider("opencode", 250_000, 2, 25_000, 750_000, 0)
                .with_free_provider("zenmux", 500_000, 2, 50_000, 1_500_000, 0)
                .with_provider("openai", 2_000_000, 3, 5.0, false)
                .with_provider("anthropic", 2_000_000, 3, 15.0, false)
                .build_with_rule_engine()
                .unwrap_or_else(|_| RuleEngine::new())
        ));

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.request_timeout_secs))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let control = Arc::new(RwLock::new(GatewayRuntimeControl::from_config(&config)));
        let card_store = Arc::new(ModelCardStore::new());

        Self {
            config,
            registry,
            cache,
            rule_engine,
            control,
            http_client,
            metacache,
            card_store,
        }
    }

    /// Initialize proxy from .env file and environment
    pub async fn init_from_env(&mut self, env_path: Option<&str>) -> Result<(), String> {
        // Load .env file if specified
        if let Some(path) = env_path {
            self.load_env_file(path)?;
        }

        // Load models from cache
        self.load_cached_models().await;

        // load metamodels from HuggingFace if token provided
        if let Ok(hf) = std::env::var("HUGGINGFACE_API_KEY") {
            if let Ok(metas) = crate::models::metamodel::fetch_huggingface_sheet(&hf).await {
                let mut mc = self.metacache.write().await;
                for m in metas {
                    let _ = mc.insert(m);
                }
                mc.replicate_all();
            }
        }

        // Pick up default/fallback model from env (may have been loaded from .env)
        if self.config.default_model.is_none() {
            self.config.default_model = std::env::var("MODELMUX_DEFAULT_MODEL").ok().filter(|s| !s.is_empty());
        }
        if self.config.fallback_model.is_none() {
            self.config.fallback_model = std::env::var("MODELMUX_FALLBACK_MODEL").ok().filter(|s| !s.is_empty());
        }

        // Rebuild control with updated config
        *self.control.write().await = GatewayRuntimeControl::from_config(&self.config);

        // Update rule engine based on available API keys
        self.update_rule_engine_from_env().await;

        info!("ModelProxy initialized from env");
        if let Some(ref dm) = self.config.default_model {
            info!("Default model: {}", dm);
        }
        Ok(())
    }

    fn load_env_file(&self, path: &str) -> Result<(), String> {
        use std::fs;
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read .env file: {}", e))?;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(eq) = line.find('=') {
                let key = line[..eq].trim();
                let value = line[eq + 1..].trim().trim_matches('"').trim_matches('\'');
                std::env::set_var(key, value);
                debug!("Loaded env: {}={}", key, value);
            }
        }
        Ok(())
    }

    async fn load_cached_models(&self) {
        // No predefined seed. Cache starts empty so draw-through fires on
        // first /v1/models call and fetches real catalogs from live APIs.
        let cache = self.cache.read().await;
        let count = cache.get_all_models().len();
        if count > 0 {
            info!("Loaded {} models from disk cache", count);
        } else {
            info!("Cache empty — draw-through will fetch from providers on first request");
        }
    }

    async fn update_rule_engine_from_env(&self) {
        // Delegate to dsel::discover_providers() — the single source of truth
        // for key detection, placeholder filtering, and priority ordering.
        let providers = crate::keymux::dsel::discover_providers();
        let mut engine = self.rule_engine.write().await;
        for p in &providers {
            // Seed quota tracking via a zero-token usage call (auto-creates entry)
            let _ = engine.track_token_usage(&p.name, 0);
            info!("Rule engine: provider {} active (key: {})", p.name, p.key_env);
        }
    }

    /// Draw-through cache: return cached models, or fetch from providers on miss.
    /// API keys are the asset; base URLs are const mappings.
    /// context_window values are capped by MODELMUX_MAX_CONTEXT_WINDOW if set.
    pub async fn get_models(&self) -> Value {
        // Use shared helper so other modules leverage same value
        let max_ctx = crate::models::utils::max_context_window();

        // Check cache first — only return if we have non-expired entries
        {
            let cache = self.cache.read().await;
            let cached = cache.get_all_models();
            let live: Vec<&crate::models::cache::CachedModel> = cached.iter()
                .filter(|m| !m.is_expired())
                .collect();
            if !live.is_empty() {
                // Populate model cards from cached data
                let live_owned: Vec<CachedModel> = live.iter().map(|m| (*m).clone()).collect();
                self.card_store.populate_from_cached(&live_owned);

                let mut models: Vec<Value> = live.iter().map(|m| json!({
                    "id": &m.id,
                    "object": "model",
                    "created": m.cached_at,
                    "owned_by": &m.provider,
                    "permission": [],
                    "root": &m.id,
                    "parent": null,
                })).collect();
                // Always inject passthru model when flag+key are set
                let passthru = std::env::var("MODELMUX_ENABLE_OLLAMA_OPENROUTER")
                    .map(|v| { let v = v.to_ascii_lowercase(); v == "1" || v == "true" || v == "yes" || v == "on" })
                    .unwrap_or(false);
                if passthru && std::env::var("OPENROUTER_API_KEY").is_ok() {
                    if !models.iter().any(|m| m.get("id").and_then(|i| i.as_str()) == Some("ollama/openrouter-free")) {
                        models.push(json!({
                            "id": "ollama/openrouter-free",
                            "object": "model",
                            "created": chrono::Utc::now().timestamp(),
                            "owned_by": "ollama",
                            "permission": [],
                            "root": "ollama/openrouter-free",
                            "parent": null,
                        }));
                    }
                }
                return json!({ "object": "list", "data": models });
            }
        }

        // Cache miss — draw through from upstream providers using API keys
        let providers = crate::keymux::dsel::discover_providers();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();

        for p in &providers {
            // Ollama-first default: do not include OpenRouter models unless explicitly requested.
            if p.name == "openrouter" {
                let include_openrouter = std::env::var("MODELMUX_INCLUDE_OPENROUTER_MODELS")
                    .map(|v| {
                        let v = v.to_ascii_lowercase();
                        v == "1" || v == "true" || v == "yes" || v == "on"
                    })
                    .unwrap_or(false);
                if !include_openrouter {
                    continue;
                }
            }
            let api_key = match std::env::var(&p.key_env) {
                Ok(k) if crate::keymux::dsel::is_real_key_pub(&k) => k,
                _ => continue,
            };
            // Provider-specific auth and URL handling
            let url = if p.name == "gemini" {
                format!("{}/models?key={}", p.base_url, api_key)
            } else if p.name == "perplexity" {
                // Perplexity has no /models endpoint; skip fetch, seed known models
                let mut cache = self.cache.write().await;
                for model_name in &["sonar", "sonar-pro", "sonar-deep-research", "sonar-reasoning", "sonar-reasoning-pro"] {
                    cache.cache(crate::models::cache::CachedModel {
                        id: format!("perplexity/{}", model_name),
                        provider: "perplexity".to_string(),
                        name: model_name.to_string(),
                        context_window: crate::models::utils::max_context_window(),
                        max_tokens: 32768,
                        input_cost_per_million: 0.0,
                        output_cost_per_million: 0.0,
                        is_free: false,
                        supports_streaming: true,
                        supports_tools: true,
                        cached_at: now,
                        expires_at: Some(now + 86400), // 24hr for static
                    });
                }
                info!("Draw-through: seeded 5 known perplexity models");
                continue;
            } else {
                format!("{}/models", p.base_url)
            };
            let mut req = self.http_client
                .get(&url)
                .timeout(std::time::Duration::from_secs(10));
            // Gemini uses query param auth; everyone else uses Bearer
            if p.name != "gemini" {
                req = req.header("Authorization", format!("Bearer {}", api_key));
            }
            let resp = req.send().await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    if let Ok(body) = r.json::<serde_json::Value>().await {
                        let empty = vec![];
                        // OpenAI uses "data", Gemini uses "models"
                        let data = body.get("data")
                            .or_else(|| body.get("models"))
                            .and_then(|d| d.as_array())
                            .unwrap_or(&empty);
                        let mut cache = self.cache.write().await;
                        let max_ctx = crate::models::utils::max_context_window();
                        for m in data {
                            // OpenAI: "id", Gemini: "name" (format: "models/gemini-2.0-flash")
                            let raw_id = m.get("id")
                                .or_else(|| m.get("name"))
                                .and_then(|i| i.as_str())
                                .unwrap_or("unknown");
                            // Strip "models/" prefix from Gemini
                            let clean_id = raw_id.strip_prefix("models/").unwrap_or(raw_id);
                            let model_id = format!("{}/{}", p.name, clean_id);

                            // Extract real metadata from provider response
                            // OpenRouter/kilo: context_length, top_provider.max_completion_tokens
                            // Gemini: inputTokenLimit, outputTokenLimit
                            // NVIDIA/others: may use context_length or max_tokens
                            let ctx = m.get("context_length").and_then(|v| v.as_u64())
                                .or_else(|| m.get("inputTokenLimit").and_then(|v| v.as_u64()))
                                .or_else(|| m.get("max_context_length").and_then(|v| v.as_u64()))
                                .unwrap_or(max_ctx)
                                .min(max_ctx);

                            let max_out = m.get("top_provider")
                                .and_then(|tp| tp.get("max_completion_tokens"))
                                .and_then(|v| v.as_u64())
                                .or_else(|| m.get("outputTokenLimit").and_then(|v| v.as_u64()))
                                .or_else(|| m.get("max_completion_tokens").and_then(|v| v.as_u64()))
                                .unwrap_or(32768);

                            let pricing = m.get("pricing");
                            let input_cost = pricing
                                .and_then(|p| p.get("prompt").and_then(|v| v.as_str()))
                                .and_then(|s| s.parse::<f64>().ok())
                                .map(|c| c * 1_000_000.0) // per-token → per-million
                                .unwrap_or(0.0);
                            let output_cost = pricing
                                .and_then(|p| p.get("completion").and_then(|v| v.as_str()))
                                .and_then(|s| s.parse::<f64>().ok())
                                .map(|c| c * 1_000_000.0)
                                .unwrap_or(0.0);

                            cache.cache(crate::models::cache::CachedModel {
                                id: model_id,
                                provider: p.name.clone(),
                                name: raw_id.to_string(),
                                context_window: ctx,
                                max_tokens: max_out,
                                input_cost_per_million: input_cost,
                                output_cost_per_million: output_cost,
                                is_free: input_cost == 0.0 && output_cost == 0.0,
                                supports_streaming: true,
                                supports_tools: true,
                                cached_at: now,
                                expires_at: Some(now + 3600), // 1hr TTL
                            });
                        }
                        info!("Draw-through: fetched {} models from {}", data.len(), p.name);
                    }
                }
                _ => {
                    // Log what actually happened
                    match &resp {
                        Ok(r) => warn!("Draw-through: {} returned HTTP {} from {}", p.name, r.status(), url),
                        Err(e) => warn!("Draw-through: {} request failed: {} (url: {})", p.name, e, url),
                    }
                    // Timeout/error — seed a default so the provider still appears
                    let mut cache = self.cache.write().await;
                    cache.cache(crate::models::cache::CachedModel {
                        id: format!("{}/default", p.name),
                        provider: p.name.clone(),
                        name: format!("{} (via {})", p.name, p.key_env),
                        context_window: crate::models::utils::max_context_window(),
                        max_tokens: 32768,
                        input_cost_per_million: 0.0,
                        output_cost_per_million: 0.0,
                        is_free: false,
                        supports_streaming: true,
                        supports_tools: true,
                        cached_at: now,
                        expires_at: Some(now + 300), // 5min TTL on fallback
                    });
                    warn!("Draw-through: {} unreachable, seeded default", p.name);
                }
            }
        }

        // Populate model cards from freshly-fetched cache
        {
            let cache = self.cache.read().await;
            let all = cache.get_all_models();
            self.card_store.populate_from_cached(&all);
        }

        // Now read from freshly-populated cache
        let cache = self.cache.read().await;
        let mut models: Vec<Value> = cache.get_all_models().iter().map(|m| json!({
            "id": &m.id,
            "object": "model",
            "created": m.cached_at,
            "owned_by": &m.provider,
            "permission": [],
            "root": &m.id,
            "parent": null,
        })).collect();

        // If passthru is active we may need to inject the fake model here too
        let passthru = std::env::var("MODELMUX_ENABLE_OLLAMA_OPENROUTER")
            .map(|v| {
                let v = v.to_ascii_lowercase();
                v == "1" || v == "true" || v == "yes" || v == "on"
            })
            .unwrap_or(false);
        if passthru && std::env::var("OPENROUTER_API_KEY").is_ok() {
            // avoid duplicate if already cached
            if !models.iter().any(|m| m.get("id").and_then(|i| i.as_str()) == Some("ollama/openrouter-free")) {
                models.push(json!({
                    "id": "ollama/openrouter-free",
                    "object": "model",
                    "created": chrono::Utc::now().timestamp(),
                    "owned_by": "ollama",
                    "permission": [],
                    "root": "ollama/openrouter-free",
                    "parent": null,
                }));
            }
        }

        json!({ "object": "list", "data": models })
    }

    /// Try OpenRouter free-tier fallback models until one succeeds.
    async fn try_openrouter_free_fallback(
        &self,
        request_template: &Value,
        context: &str,
    ) -> Result<Value, ProxyError> {
        // Opt-in only. Keep this OFF by default so Ollama path stays stable.
        // However, explicit passthru requests should still work even if the general
        // fallback flag is disabled. We'll allow either the old fallback env var
        // or the new passthru flag to enable functionality.
        let fallback_enabled = std::env::var("MODELMUX_ENABLE_OPENROUTER_FALLBACK")
            .map(|v| {
                let v = v.to_ascii_lowercase();
                v == "1" || v == "true" || v == "yes" || v == "on"
            })
            .unwrap_or(false);
        let passthru_flag = std::env::var("MODELMUX_ENABLE_OLLAMA_OPENROUTER")
            .map(|v| {
                let v = v.to_ascii_lowercase();
                v == "1" || v == "true" || v == "yes" || v == "on"
            })
            .unwrap_or(false);

        if !fallback_enabled && !passthru_flag {
            return Err(ProxyError::UpstreamError(context.to_string()));
        }

        let or_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| ProxyError::UpstreamError(format!("{} ; fallback unavailable: OPENROUTER_API_KEY missing", context)))?;

        if !crate::keymux::dsel::is_real_key_pub(&or_key) {
            return Err(ProxyError::UpstreamError(format!(
                "{} ; fallback unavailable: OPENROUTER_API_KEY is placeholder/invalid",
                context
            )));
        }

        let mut candidates: Vec<String> = Vec::new();
        if let Ok(model) = std::env::var("OPENROUTER_FREE_MODEL") {
            if !model.trim().is_empty() {
                candidates.push(model);
            }
        }
        for m in [
            "qwen/qwen3-4b:free",
            "meta-llama/llama-3.2-3b-instruct:free",
            "google/gemma-3-4b-it:free",
            "z-ai/glm-4.5-air:free",
        ] {
            if !candidates.iter().any(|c| c == m) {
                candidates.push(m.to_string());
            }
        }

        let mut last_error = String::new();
        for candidate in candidates {
            let mut req_body = request_template.clone();
            req_body["stream"] = json!(false);
            req_body["model"] = json!(candidate.clone());

            let response = self.http_client
                .post("https://openrouter.ai/api/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", or_key))
                .header("Content-Type", "application/json")
                .json(&req_body)
                .timeout(std::time::Duration::from_secs(120))
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    info!("Fallback succeeded with OpenRouter free model {}", candidate);
                    let json: Value = resp.json().await
                        .map_err(|e| ProxyError::UpstreamError(format!(
                            "{} ; fallback parse error on {}: {}",
                            context, candidate, e
                        )))?;
                    return Ok(json);
                }
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    last_error = format!("{} => HTTP {}: {}", candidate, status, &text[..text.len().min(300)]);
                }
                Err(e) => {
                    last_error = format!("{} => request error: {}", candidate, e);
                }
            }
        }

        Err(ProxyError::UpstreamError(format!(
            "{} ; all OpenRouter free fallbacks failed: {}",
            context,
            last_error
        )))
    }

    /// Extract parameter size from model name — works across thousands of models
    /// by regex-matching common naming conventions like "70b", "8x22b", "nano-4b", etc.
    fn estimate_param_size(name: &str) -> String {
        let lower = name.to_ascii_lowercase();
        // Match patterns like "70b", "8b", "671b", "8x22b", "4b-v1.1"
        // Look for the largest number followed by 'b' (for billions)
        let mut best: Option<(usize, u64)> = None; // (position, size)
        let bytes = lower.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i].is_ascii_digit() {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    i += 1;
                }
                if i < bytes.len() && bytes[i] == b'b'
                    && (i + 1 >= bytes.len() || !bytes[i + 1].is_ascii_alphabetic())
                {
                    if let Ok(n) = lower[start..i].parse::<f64>() {
                        let size = n as u64;
                        if size > 0 {
                            match &best {
                                Some((_, prev)) if size <= *prev => {}
                                _ => best = Some((start, size)),
                            }
                        }
                    }
                }
            }
            i += 1;
        }
        best.map(|(_, s)| format!("{}B", s)).unwrap_or_else(|| "unknown".into())
    }

    /// Convert a chat.completion response to a chat.completion.chunk for SSE streaming
    fn completion_to_stream_chunk(response: &Value) -> Value {
        let choices = response.get("choices").and_then(|c| c.as_array()).cloned().unwrap_or_default();
        let stream_choices: Vec<Value> = choices.into_iter().map(|choice| {
            let msg = choice.get("message").cloned().unwrap_or(json!({}));
            json!({
                "index": choice.get("index").cloned().unwrap_or(json!(0)),
                "delta": {
                    "role": msg.get("role").cloned().unwrap_or(json!("assistant")),
                    "content": msg.get("content").cloned().unwrap_or(json!("")),
                },
                "finish_reason": choice.get("finish_reason").cloned().unwrap_or(json!(null)),
            })
        }).collect();

        json!({
            "id": response.get("id").cloned().unwrap_or(json!("chatcmpl-proxy")),
            "object": "chat.completion.chunk",
            "created": response.get("created").cloned().unwrap_or(json!(0)),
            "model": response.get("model").cloned().unwrap_or(json!("")),
            "choices": stream_choices,
        })
    }

    /// Handle chat completions (OpenAI-compatible /v1/chat/completions endpoint)
    /// Routes via DSEL provider discovery — not the static registry.
    pub async fn chat_completions(&self, mut request: Value) -> Result<Value, ProxyError> {
        let primary_model = self.prepare_request_model(&mut request).await?;

        match self.send_chat_completion_request(request.clone()).await {
            Ok((response, _provider_name)) => Ok(response),
            Err(primary_error) => {
                if let Some(fallback_model) = self.current_fallback_model().await {
                    if fallback_model.trim_end_matches(":latest")
                        != primary_model.trim_end_matches(":latest")
                    {
                        warn!(
                            "{}; attempting configured fallback model {}",
                            primary_error,
                            fallback_model
                        );
                        let mut fallback_request = request.clone();
                        fallback_request["model"] = json!(fallback_model);
                        let _ = self.prepare_request_model(&mut fallback_request).await?;
                        match self.send_chat_completion_request(fallback_request.clone()).await {
                            Ok((response, _provider_name)) => return Ok(response),
                            Err(fallback_error) => {
                                warn!("{}; attempting OpenRouter free fallback", fallback_error);
                                return self
                                    .try_openrouter_free_fallback(
                                        &fallback_request,
                                        &fallback_error.to_string(),
                                    )
                                    .await;
                            }
                        }
                    }
                }

                warn!("{}; attempting OpenRouter free fallback", primary_error);
                self.try_openrouter_free_fallback(&request, &primary_error.to_string())
                    .await
            }
        }
    }

    /// Handle Ollama native /api/chat endpoint
    /// Translates Ollama format ↔ OpenAI format
    pub async fn ollama_chat(&self, request: Value) -> Result<Value, ProxyError> {
        let model_raw = request
            .get("model")
            .and_then(|m| m.as_str())
            .ok_or_else(|| ProxyError::BadRequest("Missing model parameter".to_string()))?
            .to_string();

        // Strip ":latest" tag
        let model = model_raw.trim_end_matches(":latest");

        // Build OpenAI-format request
        let mut openai_request = json!({
            "model": model,
            "messages": request.get("messages").cloned().unwrap_or(json!([])),
            "stream": false,
            "temperature": request.get("options").and_then(|o| o.get("temperature")).cloned(),
        });
        // Forward tools if present in Ollama request
        if let Some(tools) = request.get("tools") {
            openai_request["tools"] = tools.clone();
        }

        // Route through standard chat_completions
        let openai_resp = self.chat_completions(openai_request).await?;

        // Convert OpenAI response → Ollama native format
        let content = openai_resp
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("");

        let resp_model = openai_resp.get("model")
            .and_then(|m| m.as_str())
            .unwrap_or(model);

        // Check if the OpenAI response includes tool_calls
        let message_obj = openai_resp
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"));

        let tool_calls = message_obj
            .and_then(|m| m.get("tool_calls"))
            .cloned();

        let mut msg = serde_json::json!({
            "role": "assistant",
            "content": content
        });
        if let Some(tc) = tool_calls {
            msg["tool_calls"] = tc;
        }

        Ok(json!({
            "model": resp_model,
            "created_at": chrono::Utc::now().to_rfc3339(),
            "message": msg,
            "done": true,
            "total_duration": 0,
            "load_duration": 0,
            "prompt_eval_count": openai_resp.pointer("/usage/prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            "prompt_eval_duration": 0,
            "eval_count": openai_resp.pointer("/usage/completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            "eval_duration": 0
        }))
    }

    /// Handle Ollama streaming mode (NDJSON over a single HTTP response body)
    /// Note: transport is not chunked; we still emit valid Ollama NDJSON frames.
    pub async fn ollama_chat_stream_body(&self, request: Value) -> Result<String, ProxyError> {
        let single = self.ollama_chat(request).await?;

        let model = single.get("model").cloned().unwrap_or(json!("unknown"));
        let created_at = single.get("created_at").cloned().unwrap_or(json!(chrono::Utc::now().to_rfc3339()));
        let message = single.get("message").cloned().unwrap_or(json!({"role":"assistant","content":""}));

        // Frame 1: assistant message payload
        let frame1 = json!({
            "model": model,
            "created_at": created_at,
            "message": message,
            "done": false
        });

        // Frame 2: completion metadata
        let frame2 = json!({
            "model": single.get("model").cloned().unwrap_or(json!("unknown")),
            "created_at": single.get("created_at").cloned().unwrap_or(json!(chrono::Utc::now().to_rfc3339())),
            "done": true,
            "total_duration": single.get("total_duration").cloned().unwrap_or(json!(0)),
            "load_duration": single.get("load_duration").cloned().unwrap_or(json!(0)),
            "prompt_eval_count": single.get("prompt_eval_count").cloned().unwrap_or(json!(0)),
            "prompt_eval_duration": single.get("prompt_eval_duration").cloned().unwrap_or(json!(0)),
            "eval_count": single.get("eval_count").cloned().unwrap_or(json!(0)),
            "eval_duration": single.get("eval_duration").cloned().unwrap_or(json!(0))
        });

        Ok(format!("{}\n{}\n", frame1, frame2))
    }

    /// Select provider using DSEL quota management
    async fn select_provider(&self, model: &str) -> Result<Arc<ProviderEntry>, ProxyError> {
        let mut current_model = model.to_string();
        let mut attempts = 0;
        let max_attempts = 3;

        loop {
            attempts += 1;
            
            // Parse provider from model ID (e.g., "kilo_code/model-name" -> "kilo_code")
            let provider_name = current_model.split('/').next().unwrap_or("kilo_code");

            // Check quota availability
            let engine = self.rule_engine.read().await;
            if !engine.has_sufficient_quota(provider_name, 100) {
                warn!("Provider {} out of quota, trying fallback", provider_name);
                drop(engine);
                
                // Try fallback provider
                if let Some(fallback) = &self.config.fallback_model {
                    current_model = fallback.clone();
                    if attempts >= max_attempts {
                        return Err(ProxyError::NotFound("All providers out of quota".to_string()));
                    }
                    continue;
                } else {
                    return Err(ProxyError::NotFound(format!("Provider {} out of quota", provider_name)));
                }
            }
            drop(engine);

            // Get provider from registry
            return self.registry
                .get_provider(provider_name)
                .map(|p| Arc::new(p.clone()))
                .ok_or_else(|| ProxyError::NotFound(format!("Provider not found: {}", provider_name)));
        }
    }

    /// Forward request to upstream provider
    async fn forward_to_provider(
        &self,
        provider: &ProviderEntry,
        model: &str,
        request: Value,
        api_key: Option<String>,
    ) -> Result<Value, ProxyError> {
        let url = if provider.is_openai_compatible {
            format!("{}/chat/completions", provider.base_url)
        } else {
            // Handle Anthropic and other non-compatible APIs
            self.transform_and_route(provider, model, request.clone(), api_key.clone()).await?
        };

        let mut req_builder = self.http_client.post(&url)
            .header("Content-Type", "application/json");

        if let Some(key) = &api_key {
            if let Some(prefix) = &provider.auth_prefix {
                req_builder = req_builder.header(&provider.auth_header, format!("{} {}", prefix, key));
            } else {
                req_builder = req_builder.header(&provider.auth_header, key);
            }
        }

        // Transform request if needed (e.g., for Anthropic)
        let final_request = if provider.name == "anthropic" {
            self.transform_for_anthropic(model, &request)
        } else {
            request.clone()
        };

        let response = req_builder
            .json(&final_request)
            .send()
            .await
            .map_err(|e| ProxyError::UpstreamError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProxyError::UpstreamError(format!("Provider error {}: {}", status, error_text)));
        }

        let response_json: Value = response
            .json()
            .await
            .map_err(|e| ProxyError::UpstreamError(format!("Parse error: {}", e)))?;

        // Transform response if needed
        if provider.name == "anthropic" {
            Ok(self.transform_from_anthropic(&response_json))
        } else {
            Ok(response_json)
        }
    }

    /// Transform OpenAI request to Anthropic format
    fn transform_for_anthropic(&self, model: &str, request: &Value) -> Value {
        let empty_msgs = vec![];
        let messages = request.get("messages").and_then(|m| m.as_array()).unwrap_or(&empty_msgs);
        let mut system_prompt = None;
        let mut anthropic_messages = Vec::new();

        for msg in messages {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content = msg.get("content").cloned().unwrap_or(json!(""));

            if role == "system" {
                system_prompt = Some(content);
            } else {
                anthropic_messages.push(json!({
                    "role": if role == "assistant" { "assistant" } else { "user" },
                    "content": content
                }));
            }
        }

        let mut body = json!({
            "model": model.replace("anthropic/", ""),
            "messages": anthropic_messages,
            "max_tokens": request.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(4096),
        });

        if let Some(system) = system_prompt {
            body["system"] = system;
        }

        if let Some(temp) = request.get("temperature") {
            body["temperature"] = temp.clone();
        }

        if let Some(stream) = request.get("stream") {
            body["stream"] = stream.clone();
        }

        body
    }

    /// Transform Anthropic response to OpenAI format
    fn transform_from_anthropic(&self, response: &Value) -> Value {
        let empty_content = vec![];
        let content = response.get("content").and_then(|c| c.as_array()).unwrap_or(&empty_content);
        let text = content
            .iter()
            .find(|c| c.get("type").and_then(|t| t.as_str()) == Some("text"))
            .and_then(|c| c.get("text").and_then(|t| t.as_str()))
            .unwrap_or("");

        let empty_usage = json!({});
        let usage = response.get("usage").unwrap_or(&empty_usage);

        json!({
            "id": response.get("id").and_then(|i| i.as_str()).unwrap_or(""),
            "object": "chat.completion",
            "created": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            "model": response.get("model").and_then(|m| m.as_str()).unwrap_or(""),
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": text
                },
                "finish_reason": response.get("stop_reason").and_then(|s| s.as_str()).unwrap_or("stop")
            }],
            "usage": {
                "prompt_tokens": usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                "completion_tokens": usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                "total_tokens": usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
                    + usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
            }
        })
    }

    /// Transform and route for non-OpenAI-compatible providers
    async fn transform_and_route(
        &self,
        provider: &ProviderEntry,
        model: &str,
        request: Value,
        api_key: Option<String>,
    ) -> Result<String, ProxyError> {
        if provider.name == "anthropic" {
            Ok(format!("{}/v1/messages", provider.base_url))
        } else {
            Ok(format!("{}/chat/completions", provider.base_url))
        }
    }

    /// Get proxy health status
    pub async fn health(&self) -> Value {
        let cache = self.cache.read().await;
        let control = self.control.read().await;
        let state = control.snapshot(&self.config);
        
        json!({
            "status": "healthy",
            "models_cached": cache.get_all_models().len(),
            "providers_available": self.registry.get_enabled_providers().len(),
            "quota_status": "ok",
            "preferred_provider": state.routing.preferred_provider,
            "claude_model_rewrite": state.claude_model_rewrite.enabled,
            "streaming_enabled": state.streaming.enabled,
        })
    }

    /// Get proxy statistics
    pub async fn stats(&self) -> Value {
        let cache = self.cache.read().await;
        let control = self.control.read().await;
        let state = control.snapshot(&self.config);
        
        json!({
            "uptime_secs": 0,
            "models_cached": cache.get_all_models().len(),
            "requests_total": 0,
            "requests_success": 0,
            "requests_error": 0,
            "providers_active": state.providers.len(),
            "streaming_mode": state.streaming.ollama_chat,
        })
    }

    pub async fn control_state(&self) -> GatewayControlState {
        let control = self.control.read().await;
        control.snapshot(&self.config)
    }

    pub async fn apply_control_action(
        &self,
        action: GatewayControlAction,
    ) -> Result<GatewayControlState, ProxyError> {
        let mut control = self.control.write().await;
        if matches!(action, GatewayControlAction::Reset) {
            *control = GatewayRuntimeControl::from_config(&self.config);
            return Ok(control.snapshot(&self.config));
        }
        control.apply_action(action).map_err(ProxyError::BadRequest)?;
        Ok(control.snapshot(&self.config))
    }

    pub async fn toolbar_state(&self) -> ToolbarState {
        let gateway = self.control_state().await;
        derive_toolbar_state(&gateway)
    }

    pub async fn apply_toolbar_action(
        &self,
        action: ToolbarAction,
    ) -> Result<ToolbarState, ProxyError> {
        match action {
            ToolbarAction::RescanEnv => {
                self.update_rule_engine_from_env().await;
            }
            ToolbarAction::ResetRuntime => {
                let mut control = self.control.write().await;
                *control = GatewayRuntimeControl::from_config(&self.config);
            }
            ToolbarAction::SetStreamingEnabled { enabled } => {
                let _ = self
                    .apply_control_action(GatewayControlAction::SetStreamingEnabled { enabled })
                    .await?;
            }
            ToolbarAction::SetPreferredProvider { provider } => {
                let _ = self
                    .apply_control_action(GatewayControlAction::SetPreferredProvider { provider })
                    .await?;
            }
            ToolbarAction::ClearPreferredProvider => {
                let _ = self
                    .apply_control_action(GatewayControlAction::ClearPreferredProvider)
                    .await?;
            }
            ToolbarAction::SetDefaultModel { model } => {
                let _ = self
                    .apply_control_action(GatewayControlAction::SetDefaultModel { model })
                    .await?;
            }
            ToolbarAction::ClearDefaultModel => {
                let _ = self
                    .apply_control_action(GatewayControlAction::ClearDefaultModel)
                    .await?;
            }
            ToolbarAction::SetFallbackModel { model } => {
                let _ = self
                    .apply_control_action(GatewayControlAction::SetFallbackModel { model })
                    .await?;
            }
            ToolbarAction::ClearFallbackModel => {
                let _ = self
                    .apply_control_action(GatewayControlAction::ClearFallbackModel)
                    .await?;
            }
            ToolbarAction::SetClaudeRewriteEnabled { enabled } => {
                let mut control = self.control.write().await;
                control.claude_model_rewrite.enabled = enabled;
            }
            ToolbarAction::SetClaudeRewritePolicy {
                enabled,
                default_model,
                haiku_model,
                sonnet_model,
                opus_model,
                reasoning_model,
            } => {
                let _ = self
                    .apply_control_action(GatewayControlAction::SetClaudeRewritePolicy {
                        enabled,
                        default_model,
                        haiku_model,
                        sonnet_model,
                        opus_model,
                        reasoning_model,
                    })
                    .await?;
            }
            ToolbarAction::ClearClaudeRewritePolicy => {
                let _ = self
                    .apply_control_action(GatewayControlAction::ClearClaudeRewritePolicy)
                    .await?;
            }
            ToolbarAction::SetProviderKeyPolicy {
                provider,
                env_key,
                override_env_key,
                precedence,
            } => {
                let _ = self
                    .apply_control_action(GatewayControlAction::SetProviderKeyPolicy {
                        provider,
                        env_key,
                        override_env_key,
                        precedence,
                    })
                    .await?;
            }
            ToolbarAction::ClearProviderKeyPolicy { provider } => {
                let _ = self
                    .apply_control_action(GatewayControlAction::ClearProviderKeyPolicy { provider })
                    .await?;
            }
            ToolbarAction::ImportCcSwitchKeysAdditive { path } => {
                let _ = self
                    .apply_control_action(GatewayControlAction::ImportCcSwitchKeysAdditive {
                        path,
                    })
                    .await?;
            }
        }

        Ok(self.toolbar_state().await)
    }

    /// Start the HTTP server
    pub async fn start_server(&self) -> Result<(), ProxyError> {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
        use tokio::net::TcpListener;

        let addr = format!("{}:{}", self.config.bind_address, self.config.port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| ProxyError::BindFailed(format!("Failed to bind {}: {}", addr, e)))?;

        info!("🚀 ModelMux listening on {}", addr);
        info!("   OpenAI-compatible endpoint: http://{}/v1", addr);
        info!("   Models endpoint: http://{}/v1/models", addr);
        info!("   Health check: http://{}/health", addr);

        // Clone necessary state for the server loop
        let proxy_config = self.config.clone();
        let registry = Arc::clone(&self.registry);
        let cache = Arc::clone(&self.cache);
        let rule_engine = Arc::clone(&self.rule_engine);
        let control = Arc::clone(&self.control);
        let http_client = self.http_client.clone();
        let metacache = Arc::clone(&self.metacache);

        loop {
            let (stream, _) = listener.accept().await.map_err(|e| {
                ProxyError::AcceptFailed(format!("Failed to accept: {}", e))
            })?;

            // Create a minimal proxy instance for this connection
            let connection_proxy = ModelProxy {
                config: proxy_config.clone(),
                registry: Arc::clone(&registry),
                cache: Arc::clone(&cache),
                rule_engine: Arc::clone(&rule_engine),
                control: Arc::clone(&control),
                http_client: http_client.clone(),
                metacache: Arc::clone(&metacache),
                card_store: Arc::clone(&self.card_store),
            };
            let proxy = Arc::new(connection_proxy);

            tokio::spawn(async move {
                let (read_half, mut write_half) = tokio::io::split(stream);
                let mut reader = BufReader::new(read_half);
                let mut line = String::new();

                // Read request line
                if reader.read_line(&mut line).await.is_err() {
                    return;
                }

                let parts: Vec<&str> = line.trim().split_whitespace().collect();
                if parts.len() < 2 {
                    return;
                }

                let method = parts[0].to_string();
                let path = parts[1].to_string();

                // Read headers
                let mut headers = HashMap::new();
                let mut content_length = 0usize;
                loop {
                    line.clear();
                    if reader.read_line(&mut line).await.is_err() || line.trim().is_empty() {
                        break;
                    }
                    let header_line = line.trim();
                    if let Some(colon) = header_line.find(':') {
                        let key = header_line[..colon].trim().to_lowercase();
                        let value = header_line[colon + 1..].trim().to_string();
                        if key == "content-length" {
                            content_length = value.parse().unwrap_or(0);
                        }
                        headers.insert(key, value);
                    }
                }

                // Read body if present
                let mut body = vec![0u8; content_length];
                if content_length > 0 {
                    if reader.read_exact(&mut body).await.is_err() {
                        return;
                    }
                }

                // Route request
                let response = proxy.handle_request(&method, &path, &headers, &body).await;
                info!("<<< {} {} → {}", method, path, response.status);

                // Write response
                let mut response_bytes = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    response.status,
                    response.status_text,
                    response.content_type,
                    response.body.len()
                )
                .into_bytes();
                response_bytes.extend_from_slice(&response.body);

                let _ = write_half.write_all(&response_bytes).await;
                let _ = write_half.flush().await;
            });
        }
    }

    fn clone_proxy(&self) -> ModelProxy {
        ModelProxy {
            config: self.config.clone(),
            registry: Arc::clone(&self.registry),
            cache: Arc::clone(&self.cache),
            rule_engine: Arc::clone(&self.rule_engine),
            control: Arc::clone(&self.control),
            http_client: self.http_client.clone(),
            metacache: Arc::clone(&self.metacache),
            card_store: Arc::clone(&self.card_store),
        }
    }

    async fn prepare_request_model(&self, request: &mut Value) -> Result<String, ProxyError> {
        let control = self.control.read().await.clone();

        let mut model = request
            .get("model")
            .and_then(|m| m.as_str())
            .map(|m| m.to_string())
            .or_else(|| control.effective_default_model().map(|m| m.to_string()))
            .or_else(|| self.config.default_model.clone())
            .ok_or_else(|| ProxyError::BadRequest("Missing model parameter".to_string()))?;

        if let Some(rewritten) = control.rewrite_model(&model, request) {
            model = rewritten;
        }

        if let Some(provider) = control.preferred_provider_for_model(&model) {
            model = format!("{}/{}", provider, model);
        }

        if !control.streaming_enabled {
            request["stream"] = json!(false);
        }

        request["model"] = json!(model.clone());
        Ok(model)
    }

    async fn current_fallback_model(&self) -> Option<String> {
        let control = self.control.read().await;
        control
            .effective_fallback_model()
            .map(|m| m.to_string())
            .or_else(|| self.config.fallback_model.clone())
    }

    async fn send_chat_completion_request(
        &self,
        mut request: Value,
    ) -> Result<(Value, String), ProxyError> {
        let model_raw = request
            .get("model")
            .and_then(|m| m.as_str())
            .ok_or_else(|| ProxyError::BadRequest("Missing model parameter".to_string()))?
            .to_string();

        let model = model_raw.trim_end_matches(":latest");

        if model == "ollama/openrouter-free" {
            let response = self.try_openrouter_free_fallback(&request, "ollama passthru").await?;
            return Ok((response, "openrouter".to_string()));
        }

        let route_opt = crate::dsel::route(model);
        let (provider_name, base_url, key_env) = if let Some(r) = route_opt {
            r
        } else {
            match self.metacache.read().await.get(model) {
                Ok(Some(meta)) => {
                    let provider = meta.provider.clone();
                    let base = format!("https://api.{}.com/v1", provider);
                    let key_env = format!("{}_API_KEY", provider.to_uppercase());
                    (provider, base, key_env)
                }
                Ok(None) | Err(_) => {
                    return Err(ProxyError::NotFound(format!("No provider for model: {}", model)));
                }
            }
        };

        let upstream_model = model
            .strip_prefix(&format!("{}/", provider_name))
            .unwrap_or(model);

        info!(
            "Chat: '{}' → provider '{}', upstream model '{}', url '{}'",
            model, provider_name, upstream_model, base_url
        );

        let api_key = std::env::var(&key_env)
            .map_err(|_| ProxyError::Unauthorized(format!("Missing API key: {}", key_env)))?;

        request["stream"] = json!(false);
        request["model"] = json!(upstream_model);

        let url = format!("{}/chat/completions", base_url);
        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await;

        let resp_json: Value = match response {
            Ok(resp) if resp.status().is_success() => resp
                .json()
                .await
                .map_err(|e| ProxyError::UpstreamError(format!("Parse error: {}", e)))?,
            Ok(resp) => {
                let status = resp.status();
                let error_text = resp.text().await.unwrap_or_default();
                return Err(ProxyError::UpstreamError(format!(
                    "Provider {} error {}: {}",
                    provider_name,
                    status,
                    &error_text[..error_text.len().min(500)]
                )));
            }
            Err(e) => {
                return Err(ProxyError::UpstreamError(format!("Primary request failed: {}", e)));
            }
        };

        if let Some(usage) = resp_json.get("usage") {
            let total = usage.get("total_tokens").and_then(|t| t.as_u64()).unwrap_or(0);
            let _ = crate::dsel::track_tokens(&provider_name, total);
        }

        Ok((resp_json, provider_name))
    }

    async fn handle_request(
        &self,
        method: &str,
        path: &str,
        _headers: &HashMap<String, String>,
        body: &[u8],
    ) -> HttpResponse {
        info!(">>> {} {}", method, path);

        match (method, path) {
            ("GET", "/") => HttpResponse::ok("\"Ollama is running\"".to_string()),
            ("GET", "/api/version") => HttpResponse::ok(r#"{"version":"0.6.4"}"#.to_string()),
            ("GET", "/api/tags") => {
                // quick override: when OLLAMA_FORCE3 is set, return exactly three identical
                // tuples with a single name field, ignoring any real tags or models.
                if std::env::var("OLLAMA_FORCE3").is_ok() {
                    let tag = serde_json::json!({
                        "name": "forced-model",
                    });
                    let resp = serde_json::json!({ "models": [tag.clone(), tag.clone(), tag] });
                    return HttpResponse::ok(serde_json::to_string(&resp).unwrap());
                }
                // Tuple strategy: expose each cached model as a minimal
                // (name, model) tuple — the PROVIDER/MODEL id is the only
                // real data. No fake sizes, digests, or GGUF metadata.
                let models_val = self.get_models().await;
                let empty = vec![];
                let data = models_val.get("data").and_then(|d| d.as_array()).unwrap_or(&empty);
                let ollama_models: Vec<Value> = data.iter().filter_map(|m| {
                    let id = m.get("id").and_then(|i| i.as_str())?;
                    if id.starts_with("openrouter/") {
                        let include_openrouter = std::env::var("MODELMUX_INCLUDE_OPENROUTER_MODELS")
                            .map(|v| {
                                let v = v.to_ascii_lowercase();
                                v == "1" || v == "true" || v == "yes" || v == "on"
                            })
                            .unwrap_or(false);
                        if !include_openrouter {
                            return None;
                        }
                    }
                    Some(serde_json::json!({
                        "name": id,
                        "model": id,
                    }))
                }).collect();
                HttpResponse::ok(serde_json::to_string(&serde_json::json!({ "models": ollama_models })).unwrap())
            }
            ("GET", "/v1/models") | ("GET", "/models") => {
                let models = self.get_models().await;
                HttpResponse::ok(serde_json::to_string(&models).unwrap())
            }
            ("GET", "/health") => {
                let health = self.health().await;
                HttpResponse::ok(serde_json::to_string(&health).unwrap())
            }
            ("GET", "/stats") => {
                let stats = self.stats().await;
                HttpResponse::ok(serde_json::to_string(&stats).unwrap())
            }
            ("GET", "/control/state") | ("GET", "/v1/control/state") => {
                let state = self.control_state().await;
                HttpResponse::ok(serde_json::to_string(&state).unwrap())
            }
            ("GET", "/toolbar/state") | ("GET", "/v1/toolbar/state") => {
                let state = self.toolbar_state().await;
                HttpResponse::ok(serde_json::to_string(&state).unwrap())
            }
            ("POST", "/control/actions") | ("POST", "/v1/control/actions") => {
                let action: GatewayControlAction = match serde_json::from_slice(body) {
                    Ok(v) => v,
                    Err(e) => {
                        return HttpResponse::bad_request(format!(
                            "Invalid control action JSON: {}",
                            e
                        ))
                    }
                };

                match self.apply_control_action(action).await {
                    Ok(state) => HttpResponse::ok(serde_json::to_string(&state).unwrap()),
                    Err(e) => HttpResponse::from_error(e),
                }
            }
            ("POST", "/toolbar/actions") | ("POST", "/v1/toolbar/actions") => {
                let action: ToolbarAction = match serde_json::from_slice(body) {
                    Ok(v) => v,
                    Err(e) => {
                        return HttpResponse::bad_request(format!(
                            "Invalid toolbar action JSON: {}",
                            e
                        ))
                    }
                };

                match self.apply_toolbar_action(action).await {
                    Ok(state) => HttpResponse::ok(serde_json::to_string(&state).unwrap()),
                    Err(e) => HttpResponse::from_error(e),
                }
            }
            ("POST", "/v1/chat/completions") | ("POST", "/chat/completions") => {
                let request: Value = match serde_json::from_slice(body) {
                    Ok(v) => v,
                    Err(e) => return HttpResponse::bad_request(format!("Invalid JSON: {}", e)),
                };

                let wants_stream = request.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);

                match self.chat_completions(request).await {
                    Ok(response) => {
                        if wants_stream {
                            // Convert non-streaming response to SSE format for clients expecting stream
                            let chunk = Self::completion_to_stream_chunk(&response);
                            let mut sse_body = String::new();
                            sse_body.push_str(&format!("data: {}\n\n", serde_json::to_string(&chunk).unwrap()));
                            sse_body.push_str("data: [DONE]\n\n");
                            HttpResponse::sse(sse_body)
                        } else {
                            HttpResponse::ok(serde_json::to_string(&response).unwrap())
                        }
                    }
                    Err(e) => HttpResponse::from_error(e),
                }
            }
            ("POST", "/api/chat") => {
                let request: Value = match serde_json::from_slice(body) {
                    Ok(v) => v,
                    Err(e) => return HttpResponse::bad_request(format!("Invalid JSON: {}", e)),
                };

                let wants_stream = request.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);
                let streaming_enabled = self.control.read().await.streaming_enabled;

                if wants_stream && streaming_enabled {
                    match self.ollama_chat_stream_body(request).await {
                        Ok(ndjson_body) => HttpResponse::ndjson(ndjson_body),
                        Err(e) => HttpResponse::from_error(e),
                    }
                } else {
                    match self.ollama_chat(request).await {
                        Ok(response) => HttpResponse::ok(serde_json::to_string(&response).unwrap()),
                        Err(e) => HttpResponse::from_error(e),
                    }
                }
            }
            ("POST", "/api/show") => {
                let request: Value = match serde_json::from_slice(body) {
                    Ok(v) => v,
                    Err(_) => return HttpResponse::bad_request("Invalid JSON".to_string()),
                };
                let name = request.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
                let model_id = name.trim_end_matches(":latest");

                // Look up real model metadata from cache
                let cache = self.cache.read().await;
                let (ctx, max_out, param_size) = if let Some(cached) = cache.find(model_id) {
                    (cached.context_window as i64, cached.max_tokens as i64, Self::estimate_param_size(&cached.name))
                } else {
                    let default_ctx = crate::models::utils::max_context_window() as i64;
                    (default_ctx, 32768i64, "unknown".to_string())
                };
                drop(cache);

                // Derive capabilities from WebModelCard tags
                let mut capabilities: Vec<String> = vec!["completion".to_string()];
                if let Some(card) = self.card_store.get_card(model_id) {
                    for tag in &card.tags {
                        if !capabilities.contains(tag) {
                            capabilities.push(tag.clone());
                        }
                    }
                } else {
                    // Default: assume tools support
                    capabilities.push("tools".to_string());
                }

                HttpResponse::ok(serde_json::to_string(&serde_json::json!({
                    "modelfile": format!("FROM {}", model_id),
                    "parameters": format!("num_ctx {}\nstop \"<|im_end|>\"", ctx),
                    "template": "{{ .Prompt }}",
                    "details": {
                        "parent_model": "",
                        "format": "gguf",
                        "family": "modelmux",
                        "families": ["modelmux"],
                        "parameter_size": param_size,
                        "quantization_level": "FP16"
                    },
                    "model_info": {
                        "general.architecture": "modelmux",
                        "general.parameter_count": 671_000_000_000i64,
                        "llama.context_length": ctx,
                        "llama.max_output_length": max_out
                    },
                    "capabilities": capabilities
                })).unwrap())
            }
            ("GET", "/api/ps") => {
                HttpResponse::ok(serde_json::to_string(&serde_json::json!({
                    "models": []
                })).unwrap())
            }
            _ => HttpResponse::not_found(),
        }
    }
}

/// HTTP response helper
struct HttpResponse {
    status: u16,
    status_text: &'static str,
    content_type: &'static str,
    body: Vec<u8>,
}

impl HttpResponse {
    fn ok(body: String) -> Self {
        Self {
            status: 200,
            status_text: "OK",
            content_type: "application/json",
            body: body.into_bytes(),
        }
    }

    fn ndjson(body: String) -> Self {
        Self {
            status: 200,
            status_text: "OK",
            content_type: "application/x-ndjson",
            body: body.into_bytes(),
        }
    }

    fn sse(body: String) -> Self {
        Self {
            status: 200,
            status_text: "OK",
            content_type: "text/event-stream",
            body: body.into_bytes(),
        }
    }

    fn not_found() -> Self {
        Self {
            status: 404,
            status_text: "Not Found",
            content_type: "application/json",
            body: br#"{"error":"not_found"}"#.to_vec(),
        }
    }

    fn bad_request(msg: String) -> Self {
        Self {
            status: 400,
            status_text: "Bad Request",
            content_type: "application/json",
            body: serde_json::to_string(&json!({"error": msg})).unwrap().into_bytes(),
        }
    }

    fn from_error(e: ProxyError) -> Self {
        match e {
            ProxyError::BadRequest(msg) => HttpResponse::bad_request(msg),
            ProxyError::Unauthorized(msg) => Self {
                status: 401,
                status_text: "Unauthorized",
                content_type: "application/json",
                body: serde_json::to_string(&json!({"error": msg})).unwrap().into_bytes(),
            },
            ProxyError::NotFound(msg) => Self {
                status: 404,
                status_text: "Not Found",
                content_type: "application/json",
                body: serde_json::to_string(&json!({"error": msg})).unwrap().into_bytes(),
            },
            ProxyError::UpstreamError(msg) => Self {
                status: 502,
                status_text: "Bad Gateway",
                content_type: "application/json",
                body: serde_json::to_string(&json!({"error": msg})).unwrap().into_bytes(),
            },
            _ => Self {
                status: 500,
                status_text: "Internal Server Error",
                content_type: "application/json",
                body: serde_json::to_string(&json!({"error": "internal_error"})).unwrap().into_bytes(),
            },
        }
    }
}

/// Proxy errors
#[derive(Debug)]
pub enum ProxyError {
    BadRequest(String),
    Unauthorized(String),
    NotFound(String),
    UpstreamError(String),
    BindFailed(String),
    AcceptFailed(String),
    Other(String),
}

impl std::fmt::Display for ProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxyError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            ProxyError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            ProxyError::NotFound(msg) => write!(f, "Not found: {}", msg),
            ProxyError::UpstreamError(msg) => write!(f, "Upstream error: {}", msg),
            ProxyError::BindFailed(msg) => write!(f, "Bind failed: {}", msg),
            ProxyError::AcceptFailed(msg) => write!(f, "Accept failed: {}", msg),
            ProxyError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for ProxyError {}
