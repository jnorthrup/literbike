//! Model Cache for ModelMux
//!
//! Caches model selections and provider configurations.
//! Models loaded from env/API only - no predefined models.

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Cached model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedModel {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub context_window: u64,
    pub max_tokens: u64,
    pub input_cost_per_million: f64,
    pub output_cost_per_million: f64,
    pub is_free: bool,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub cached_at: u64,
    pub expires_at: Option<u64>,
}

impl CachedModel {
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            now > expires
        } else {
            false
        }
    }
}

/// Model cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub cache_dir: PathBuf,
    pub max_age_secs: u64,
    pub enable_disk_cache: bool,
    pub enable_memory_cache: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        let cache_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".modelmux/cache");
        Self {
            cache_dir,
            max_age_secs: 3600,
            enable_disk_cache: true,
            enable_memory_cache: true,
        }
    }
}

/// Model cache with memory and disk backing
pub struct ModelCache {
    config: CacheConfig,
    memory_cache: HashMap<String, CachedModel>,
    models_by_provider: HashMap<String, Vec<String>>,
    cas: Option<crate::modelmux::metamodel::BlobStore>,
}

impl ModelCache {
    pub fn new(config: CacheConfig) -> Self {
        let cas_dir = config.cache_dir.join("cas");
        let mut cache = Self {
            config,
            memory_cache: HashMap::new(),
            models_by_provider: HashMap::new(),
            cas: Some(crate::modelmux::metamodel::BlobStore::new(cas_dir)),
        };
        cache.init();
        cache
    }

    pub fn with_defaults() -> Self {
        Self::new(CacheConfig::default())
    }

    pub fn empty() -> Self {
        Self {
            config: CacheConfig::default(),
            memory_cache: HashMap::new(),
            models_by_provider: HashMap::new(),
            cas: None,
        }
    }

    fn init(&mut self) {
        if self.config.enable_disk_cache {
            if let Err(e) = fs::create_dir_all(&self.config.cache_dir) {
                warn!("Failed to create cache dir: {}", e);
            }
            self.load_from_disk();
        }
    }

    pub fn get(&self, model_id: &str) -> Option<CachedModel> {
        if self.config.enable_memory_cache {
            self.memory_cache.get(model_id).cloned()
        } else {
            self.load_from_disk_single(model_id)
        }
    }

    /// Find a model by exact id, or by suffix match (e.g. "deepseek/deepseek-chat"
    /// matches "kilo_code/deepseek/deepseek-chat")
    pub fn find(&self, query: &str) -> Option<CachedModel> {
        // Exact match first
        if let Some(m) = self.get(query) {
            return Some(m);
        }
        // Suffix match: query might omit the provider prefix
        let suffix = format!("/{}", query);
        self.memory_cache
            .values()
            .find(|m| m.id.ends_with(&suffix) || m.id == query)
            .cloned()
    }

    pub fn get_provider_models(&self, provider: &str) -> Vec<CachedModel> {
        let mut models = Vec::new();
        if let Some(model_ids) = self.models_by_provider.get(provider) {
            for id in model_ids {
                if let Some(model) = self.get(id) {
                    models.push(model);
                }
            }
        }
        models
    }

    pub fn get_all_models(&self) -> Vec<CachedModel> {
        self.memory_cache.values().cloned().collect()
    }

    pub fn cache(&mut self, model: CachedModel) {
        let provider = model.provider.clone();
        let id = model.id.clone();

        if self.config.enable_memory_cache {
            // insert into in-memory map
            self.memory_cache.insert(id.clone(), model.clone());
            self.models_by_provider
                .entry(provider.clone())
                .or_insert_with(Vec::new)
                .push(id.clone());
        }

        if self.config.enable_disk_cache {
            self.save_to_disk(&id);
        }

        // also record CAS blob if available
        if let Some(bs) = &self.cas {
            if let Ok(bytes) = serde_json::to_vec(&model) {
                let _ = bs.put_cas(&bytes);
            }
        }

        debug!("Cached model: {}", id);
    }

    pub fn cache_many(&mut self, models: Vec<CachedModel>) {
        for model in models {
            self.cache(model);
        }
        info!("Cached {} models", self.memory_cache.len());
    }

    pub fn clear(&mut self) {
        self.memory_cache.clear();
        self.models_by_provider.clear();
        if self.config.enable_disk_cache {
            let _ = fs::remove_dir_all(&self.config.cache_dir);
            let _ = fs::create_dir_all(&self.config.cache_dir);
        }
        info!("Cleared model cache");
    }

    fn cache_file_path(&self, model_id: &str) -> PathBuf {
        let safe_id = model_id.replace('/', "_").replace(':', "_");
        self.config.cache_dir.join(format!("{}.json", safe_id))
    }

    fn save_to_disk(&self, model_id: &str) {
        if let Some(model) = self.memory_cache.get(model_id) {
            let path = self.cache_file_path(model_id);
            let _ = fs::write(&path, serde_json::to_string_pretty(model).unwrap());
        }
    }

    fn load_from_disk(&mut self) {
        if !self.config.cache_dir.exists() {
            return;
        }

        if let Ok(entries) = fs::read_dir(&self.config.cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(model) = serde_json::from_str::<CachedModel>(&content) {
                            if !model.is_expired() {
                                let provider = model.provider.clone();
                                let id = model.id.clone();
                                self.memory_cache.insert(id.clone(), model);
                                self.models_by_provider
                                    .entry(provider)
                                    .or_insert_with(Vec::new)
                                    .push(id);
                            }
                        }
                    }
                }
            }
        }

        info!("Loaded {} models from disk cache", self.memory_cache.len());
    }

    fn load_from_disk_single(&self, model_id: &str) -> Option<CachedModel> {
        let path = self.cache_file_path(model_id);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(model) = serde_json::from_str::<CachedModel>(&content) {
                    if !model.is_expired() {
                        return Some(model);
                    }
                }
            }
        }
        None
    }
}

/// Predefined model definitions for common providers (~100 models)
pub mod predefined {
    use super::CachedModel;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn m(id: &str, provider: &str, name: &str, ctx: u64, max_tok: u64, free: bool) -> CachedModel {
        CachedModel {
            id: id.to_string(),
            provider: provider.to_string(),
            name: name.to_string(),
            context_window: ctx,
            max_tokens: max_tok,
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
            is_free: free,
            supports_streaming: true,
            supports_tools: true,
            cached_at: now(),
            expires_at: Some(now() + 86400),
        }
    }

    // ── NVIDIA free-tier models (17) ────────────────────────────────
    pub fn nvidia_free_models() -> Vec<CachedModel> {
        vec![
            m(
                "nvidia/meta/llama-3.3-70b-instruct",
                "nvidia",
                "Llama 3.3 70B Instruct (NVIDIA)",
                128000,
                4096,
                true,
            ),
            m(
                "nvidia/meta/llama-3.1-405b-instruct",
                "nvidia",
                "Llama 3.1 405B Instruct (NVIDIA)",
                128000,
                4096,
                true,
            ),
            m(
                "nvidia/meta/llama-3.1-70b-instruct",
                "nvidia",
                "Llama 3.1 70B Instruct (NVIDIA)",
                128000,
                4096,
                true,
            ),
            m(
                "nvidia/meta/llama-3.1-8b-instruct",
                "nvidia",
                "Llama 3.1 8B Instruct (NVIDIA)",
                128000,
                4096,
                true,
            ),
            m(
                "nvidia/google/gemma-2-27b-it",
                "nvidia",
                "Gemma 2 27B IT (NVIDIA)",
                8192,
                4096,
                true,
            ),
            m(
                "nvidia/google/gemma-2-9b-it",
                "nvidia",
                "Gemma 2 9B IT (NVIDIA)",
                8192,
                4096,
                true,
            ),
            m(
                "nvidia/microsoft/phi-3-medium-128k-instruct",
                "nvidia",
                "Phi-3 Medium 128K (NVIDIA)",
                128000,
                4096,
                true,
            ),
            m(
                "nvidia/microsoft/phi-3-mini-128k-instruct",
                "nvidia",
                "Phi-3 Mini 128K (NVIDIA)",
                128000,
                4096,
                true,
            ),
            m(
                "nvidia/mistralai/mistral-large-2-instruct",
                "nvidia",
                "Mistral Large 2 (NVIDIA)",
                128000,
                4096,
                true,
            ),
            m(
                "nvidia/mistralai/mixtral-8x22b-instruct-v0.1",
                "nvidia",
                "Mixtral 8x22B (NVIDIA)",
                65536,
                4096,
                true,
            ),
            m(
                "nvidia/moonshotai/kimi-2-instruct",
                "nvidia",
                "Kimi 2 Instruct (NVIDIA)",
                256000,
                64000,
                true,
            ),
            m(
                "nvidia/deepseek-ai/deepseek-r1",
                "nvidia",
                "DeepSeek R1 (NVIDIA)",
                128000,
                8192,
                true,
            ),
            m(
                "nvidia/qwen/qwen2.5-72b-instruct",
                "nvidia",
                "Qwen 2.5 72B (NVIDIA)",
                128000,
                4096,
                true,
            ),
            m(
                "nvidia/qwen/qwen2.5-coder-32b-instruct",
                "nvidia",
                "Qwen 2.5 Coder 32B (NVIDIA)",
                128000,
                4096,
                true,
            ),
            m(
                "nvidia/nvidia/nemotron-mini-4b-instruct",
                "nvidia",
                "Nemotron Mini 4B (NVIDIA)",
                4096,
                4096,
                true,
            ),
            m(
                "nvidia/nvidia/llama-3.1-nemotron-70b-instruct",
                "nvidia",
                "Llama 3.1 Nemotron 70B (NVIDIA)",
                128000,
                4096,
                true,
            ),
            m(
                "nvidia/z-ai/glm4.7",
                "nvidia",
                "GLM 4.7 (NVIDIA)",
                128000,
                4096,
                true,
            ),
        ]
    }

    // ── Kilo.ai Gateway free models (10) ────────────────────────────
    pub fn kilo_free_models() -> Vec<CachedModel> {
        vec![
            m(
                "kilo_code/minimax-minimax-m2.5:free",
                "kilo_code",
                "MiniMax M2.5 (Free)",
                256000,
                64000,
                true,
            ),
            m(
                "kilo_code/meta-llama/llama-3.3-70b:free",
                "kilo_code",
                "Llama 3.3 70B (Kilo Free)",
                128000,
                4096,
                true,
            ),
            m(
                "kilo_code/google/gemma-2-27b-it:free",
                "kilo_code",
                "Gemma 2 27B (Kilo Free)",
                8192,
                4096,
                true,
            ),
            m(
                "kilo_code/deepseek/deepseek-r1:free",
                "kilo_code",
                "DeepSeek R1 (Kilo Free)",
                128000,
                8192,
                true,
            ),
            m(
                "kilo_code/qwen/qwen2.5-72b-instruct:free",
                "kilo_code",
                "Qwen 2.5 72B (Kilo Free)",
                128000,
                4096,
                true,
            ),
            m(
                "kilo_code/mistralai/mistral-large:free",
                "kilo_code",
                "Mistral Large (Kilo Free)",
                128000,
                4096,
                true,
            ),
            m(
                "kilo_code/microsoft/phi-3-medium:free",
                "kilo_code",
                "Phi-3 Medium (Kilo Free)",
                128000,
                4096,
                true,
            ),
            m(
                "kilo_code/nvidia/llama-3.1-nemotron-70b:free",
                "kilo_code",
                "Nemotron 70B (Kilo Free)",
                128000,
                4096,
                true,
            ),
            m(
                "kilo_code/anthropic/claude-3-haiku:free",
                "kilo_code",
                "Claude 3 Haiku (Kilo Free)",
                200000,
                4096,
                true,
            ),
            m(
                "kilo_code/openai/gpt-4o-mini:free",
                "kilo_code",
                "GPT-4o Mini (Kilo Free)",
                128000,
                16384,
                true,
            ),
        ]
    }

    // ── ZenMux models (10) ──────────────────────────────────────────
    pub fn zenmux_models() -> Vec<CachedModel> {
        vec![
            m(
                "zenmux/claude-3.5-sonnet",
                "zenmux",
                "Claude 3.5 Sonnet (ZenMux)",
                200000,
                8192,
                false,
            ),
            m(
                "zenmux/claude-3-haiku",
                "zenmux",
                "Claude 3 Haiku (ZenMux)",
                200000,
                4096,
                false,
            ),
            m(
                "zenmux/gpt-4o",
                "zenmux",
                "GPT-4o (ZenMux)",
                128000,
                16384,
                false,
            ),
            m(
                "zenmux/gpt-4o-mini",
                "zenmux",
                "GPT-4o Mini (ZenMux)",
                128000,
                16384,
                false,
            ),
            m(
                "zenmux/llama-3.3-70b",
                "zenmux",
                "Llama 3.3 70B (ZenMux)",
                128000,
                4096,
                false,
            ),
            m(
                "zenmux/deepseek-r1",
                "zenmux",
                "DeepSeek R1 (ZenMux)",
                128000,
                8192,
                false,
            ),
            m(
                "zenmux/gemma-2-27b",
                "zenmux",
                "Gemma 2 27B (ZenMux)",
                8192,
                4096,
                false,
            ),
            m(
                "zenmux/qwen2.5-72b",
                "zenmux",
                "Qwen 2.5 72B (ZenMux)",
                128000,
                4096,
                false,
            ),
            m(
                "zenmux/mistral-large",
                "zenmux",
                "Mistral Large (ZenMux)",
                128000,
                4096,
                false,
            ),
            m(
                "zenmux/phi-3-medium",
                "zenmux",
                "Phi-3 Medium (ZenMux)",
                128000,
                4096,
                false,
            ),
        ]
    }

    // ── OpenRouter models (20) — dupes of models on other providers ─
    pub fn openrouter_models() -> Vec<CachedModel> {
        vec![
            m(
                "openrouter/meta-llama/llama-3.3-70b-instruct",
                "openrouter",
                "Llama 3.3 70B Instruct",
                128000,
                4096,
                false,
            ),
            m(
                "openrouter/meta-llama/llama-3.1-405b-instruct",
                "openrouter",
                "Llama 3.1 405B Instruct",
                128000,
                4096,
                false,
            ),
            m(
                "openrouter/meta-llama/llama-3.1-70b-instruct",
                "openrouter",
                "Llama 3.1 70B Instruct",
                128000,
                4096,
                false,
            ),
            m(
                "openrouter/meta-llama/llama-3.1-8b-instruct",
                "openrouter",
                "Llama 3.1 8B Instruct",
                128000,
                4096,
                false,
            ),
            m(
                "openrouter/google/gemma-2-27b-it",
                "openrouter",
                "Gemma 2 27B IT",
                8192,
                4096,
                false,
            ),
            m(
                "openrouter/google/gemma-2-9b-it",
                "openrouter",
                "Gemma 2 9B IT",
                8192,
                4096,
                false,
            ),
            m(
                "openrouter/mistralai/mistral-large-2407",
                "openrouter",
                "Mistral Large 2407",
                128000,
                4096,
                false,
            ),
            m(
                "openrouter/mistralai/mixtral-8x22b-instruct",
                "openrouter",
                "Mixtral 8x22B Instruct",
                65536,
                4096,
                false,
            ),
            m(
                "openrouter/microsoft/phi-3-medium-128k-instruct",
                "openrouter",
                "Phi-3 Medium 128K",
                128000,
                4096,
                false,
            ),
            m(
                "openrouter/qwen/qwen-2.5-72b-instruct",
                "openrouter",
                "Qwen 2.5 72B Instruct",
                128000,
                4096,
                false,
            ),
            m(
                "openrouter/qwen/qwen-2.5-coder-32b-instruct",
                "openrouter",
                "Qwen 2.5 Coder 32B",
                128000,
                4096,
                false,
            ),
            m(
                "openrouter/deepseek/deepseek-r1",
                "openrouter",
                "DeepSeek R1",
                128000,
                8192,
                false,
            ),
            m(
                "openrouter/deepseek/deepseek-chat",
                "openrouter",
                "DeepSeek Chat",
                128000,
                4096,
                false,
            ),
            m(
                "openrouter/nvidia/llama-3.1-nemotron-70b-instruct",
                "openrouter",
                "Nemotron 70B Instruct",
                128000,
                4096,
                false,
            ),
            m(
                "openrouter/anthropic/claude-3.5-sonnet",
                "openrouter",
                "Claude 3.5 Sonnet",
                200000,
                8192,
                false,
            ),
            m(
                "openrouter/anthropic/claude-3-haiku",
                "openrouter",
                "Claude 3 Haiku",
                200000,
                4096,
                false,
            ),
            m(
                "openrouter/openai/gpt-4o",
                "openrouter",
                "GPT-4o",
                128000,
                16384,
                false,
            ),
            m(
                "openrouter/openai/gpt-4o-mini",
                "openrouter",
                "GPT-4o Mini",
                128000,
                16384,
                false,
            ),
            m(
                "openrouter/minimax/minimax-m2.5",
                "openrouter",
                "MiniMax M2.5",
                256000,
                64000,
                false,
            ),
            m(
                "openrouter/moonshotai/kimi-2-instruct",
                "openrouter",
                "Kimi 2 Instruct",
                256000,
                64000,
                false,
            ),
        ]
    }

    // ── Moonshot models (4) ─────────────────────────────────────────
    pub fn moonshot_models() -> Vec<CachedModel> {
        vec![
            m(
                "moonshotai/kimi-k2",
                "moonshot",
                "Kimi K2 (Moonshot)",
                256000,
                64000,
                true,
            ),
            m(
                "moonshotai/kimi-k2.5",
                "moonshot",
                "Kimi K2.5 (Moonshot)",
                256000,
                64000,
                true,
            ),
            m(
                "moonshotai/kimi-2-instruct",
                "moonshot",
                "Kimi 2 Instruct (Moonshot)",
                256000,
                64000,
                true,
            ),
            m(
                "moonshotai/moonlight-16b",
                "moonshot",
                "Moonlight 16B (Moonshot)",
                32768,
                4096,
                true,
            ),
        ]
    }

    // ── DeepSeek models (4) ─────────────────────────────────────────
    pub fn deepseek_models() -> Vec<CachedModel> {
        vec![
            m(
                "deepseek/dk-lm",
                "deepseek",
                "DK-LM (DeepSeek)",
                256000,
                64000,
                true,
            ),
            m(
                "deepseek/deepseek-r1",
                "deepseek",
                "DeepSeek R1",
                128000,
                8192,
                true,
            ),
            m(
                "deepseek/deepseek-chat",
                "deepseek",
                "DeepSeek Chat",
                128000,
                4096,
                true,
            ),
            m(
                "deepseek/deepseek-coder",
                "deepseek",
                "DeepSeek Coder",
                128000,
                4096,
                true,
            ),
        ]
    }

    // ── OpenCode models (3) ─────────────────────────────────────────
    pub fn opencode_free_models() -> Vec<CachedModel> {
        vec![
            m(
                "opencode/kimi-2-instruct",
                "opencode",
                "Kimi 2 Instruct (OpenCode)",
                256000,
                64000,
                true,
            ),
            m(
                "opencode/deepseek-r1",
                "opencode",
                "DeepSeek R1 (OpenCode)",
                128000,
                8192,
                true,
            ),
            m(
                "opencode/llama-3.3-70b",
                "opencode",
                "Llama 3.3 70B (OpenCode)",
                128000,
                4096,
                true,
            ),
        ]
    }

    // ── Groq models (4) ─────────────────────────────────────────────
    pub fn groq_models() -> Vec<CachedModel> {
        vec![
            m(
                "groq/llama-3.3-70b-versatile",
                "groq",
                "Llama 3.3 70B Versatile (Groq)",
                128000,
                32768,
                true,
            ),
            m(
                "groq/llama-3.1-8b-instant",
                "groq",
                "Llama 3.1 8B Instant (Groq)",
                128000,
                8192,
                true,
            ),
            m(
                "groq/mixtral-8x7b-32768",
                "groq",
                "Mixtral 8x7B (Groq)",
                32768,
                32768,
                true,
            ),
            m(
                "groq/gemma2-9b-it",
                "groq",
                "Gemma 2 9B IT (Groq)",
                8192,
                8192,
                true,
            ),
        ]
    }

    // ── Cerebras models (2) ─────────────────────────────────────────
    pub fn cerebras_models() -> Vec<CachedModel> {
        vec![
            m(
                "cerebras/llama-3.3-70b",
                "cerebras",
                "Llama 3.3 70B (Cerebras)",
                128000,
                8192,
                true,
            ),
            m(
                "cerebras/llama-3.1-8b",
                "cerebras",
                "Llama 3.1 8B (Cerebras)",
                128000,
                8192,
                true,
            ),
        ]
    }

    // ── xAI models (2) ──────────────────────────────────────────────
    pub fn xai_models() -> Vec<CachedModel> {
        vec![
            m("xai/grok-2", "xai", "Grok 2 (xAI)", 128000, 4096, false),
            m(
                "xai/grok-beta",
                "xai",
                "Grok Beta (xAI)",
                128000,
                4096,
                false,
            ),
        ]
    }

    // ── Gemini models (5) ───────────────────────────────────────────
    pub fn gemini_models() -> Vec<CachedModel> {
        vec![
            m(
                "gemini/gemini-2.0-flash",
                "gemini",
                "Gemini 2.0 Flash",
                1048576,
                8192,
                true,
            ),
            m(
                "gemini/gemini-2.0-flash-lite",
                "gemini",
                "Gemini 2.0 Flash Lite",
                1048576,
                8192,
                true,
            ),
            m(
                "gemini/gemini-1.5-pro",
                "gemini",
                "Gemini 1.5 Pro",
                2097152,
                8192,
                false,
            ),
            m(
                "gemini/gemini-1.5-flash",
                "gemini",
                "Gemini 1.5 Flash",
                1048576,
                8192,
                true,
            ),
            m(
                "gemini/gemini-1.5-flash-8b",
                "gemini",
                "Gemini 1.5 Flash 8B",
                1048576,
                8192,
                true,
            ),
        ]
    }

    // ── OpenAI models (5) ───────────────────────────────────────────
    pub fn openai_models() -> Vec<CachedModel> {
        vec![
            m("openai/gpt-4o", "openai", "GPT-4o", 128000, 16384, false),
            m(
                "openai/gpt-4o-mini",
                "openai",
                "GPT-4o Mini",
                128000,
                16384,
                false,
            ),
            m(
                "openai/gpt-4-turbo",
                "openai",
                "GPT-4 Turbo",
                128000,
                4096,
                false,
            ),
            m(
                "openai/o1-preview",
                "openai",
                "o1 Preview",
                128000,
                32768,
                false,
            ),
            m("openai/o1-mini", "openai", "o1 Mini", 128000, 65536, false),
        ]
    }

    // ── Anthropic models (5) ────────────────────────────────────────
    pub fn anthropic_models() -> Vec<CachedModel> {
        vec![
            m(
                "anthropic/claude-3.5-sonnet",
                "anthropic",
                "Claude 3.5 Sonnet",
                200000,
                8192,
                false,
            ),
            m(
                "anthropic/claude-3.5-haiku",
                "anthropic",
                "Claude 3.5 Haiku",
                200000,
                8192,
                false,
            ),
            m(
                "anthropic/claude-3-opus",
                "anthropic",
                "Claude 3 Opus",
                200000,
                4096,
                false,
            ),
            m(
                "anthropic/claude-3-sonnet",
                "anthropic",
                "Claude 3 Sonnet",
                200000,
                4096,
                false,
            ),
            m(
                "anthropic/claude-3-haiku",
                "anthropic",
                "Claude 3 Haiku",
                200000,
                4096,
                false,
            ),
        ]
    }

    /// All predefined models from every provider (~100 total)
    pub fn all_predefined_models() -> Vec<CachedModel> {
        let mut all = Vec::with_capacity(100);
        all.extend(nvidia_free_models());
        all.extend(kilo_free_models());
        all.extend(zenmux_models());
        all.extend(openrouter_models());
        all.extend(moonshot_models());
        all.extend(deepseek_models());
        all.extend(opencode_free_models());
        all.extend(groq_models());
        all.extend(cerebras_models());
        all.extend(xai_models());
        all.extend(gemini_models());
        all.extend(openai_models());
        all.extend(anthropic_models());
        all
    }
}
