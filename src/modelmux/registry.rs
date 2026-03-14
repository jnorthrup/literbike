//! Model Registry for ModelMux
//!
//! Registers and manages model providers and their configurations.
//! Similar to Kilo Gateway's provider registry.

use crate::modelmux::cache::CachedModel;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub name: String,
    pub display_name: String,
    pub base_url: String,
    pub api_key_env: String,
    pub auth_header: String,
    pub auth_prefix: Option<String>,
    pub is_openai_compatible: bool,
    pub supports_streaming: bool,
    pub default_timeout_secs: u64,
}

impl ProviderEntry {
    pub fn new(name: &str, base_url: &str, api_key_env: &str) -> Self {
        Self {
            name: name.to_string(),
            display_name: name.to_string(),
            base_url: base_url.to_string(),
            api_key_env: api_key_env.to_string(),
            auth_header: "Authorization".to_string(),
            auth_prefix: Some("Bearer".to_string()),
            is_openai_compatible: true,
            supports_streaming: true,
            default_timeout_secs: 120,
        }
    }

    pub fn with_auth(mut self, header: &str, prefix: Option<&str>) -> Self {
        self.auth_header = header.to_string();
        self.auth_prefix = prefix.map(|s| s.to_string());
        self
    }

    pub fn with_compatibility(mut self, openai: bool) -> Self {
        self.is_openai_compatible = openai;
        self
    }
}

/// Model entry in registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub provider: String,
    pub aliases: Vec<String>,
    pub enabled: bool,
    pub priority: u8,
}

impl ModelEntry {
    pub fn new(id: &str, provider: &str) -> Self {
        Self {
            id: id.to_string(),
            provider: provider.to_string(),
            aliases: Vec::new(),
            enabled: true,
            priority: 50,
        }
    }

    pub fn with_alias(mut self, alias: &str) -> Self {
        self.aliases.push(alias.to_string());
        self
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
}

/// Model registry managing providers and models
pub struct ModelRegistry {
    providers: HashMap<String, ProviderEntry>,
    models: HashMap<String, ModelEntry>,
    model_aliases: HashMap<String, String>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
            models: HashMap::new(),
            model_aliases: HashMap::new(),
        };
        registry.register_builtin_providers();
        registry
    }

    /// Register builtin providers
    fn register_builtin_providers(&mut self) {
        // Ollama (local)
        self.providers.insert(
            "ollama".to_string(),
            ProviderEntry::new("ollama", "http://localhost:11434", "").without_auth(),
        );

        // LMStudio (local)
        self.providers.insert(
            "lmstudio".to_string(),
            ProviderEntry::new("lmstudio", "http://localhost:1234/v1", "")
                .without_auth()
                .with_compatibility(true),
        );
    }

    /// Register a provider
    pub fn register_provider(&mut self, provider: ProviderEntry) {
        self.providers.insert(provider.name.clone(), provider);
    }

    /// Register a model
    pub fn register_model(&mut self, model: ModelEntry) {
        let id = model.id.clone();
        for alias in &model.aliases {
            self.model_aliases.insert(alias.clone(), id.clone());
        }
        self.models.insert(id, model);
    }

    /// Resolve model alias to canonical ID
    pub fn resolve_model<'a>(&'a self, model_id: &'a str) -> &'a str {
        self.model_aliases
            .get(model_id)
            .map(|s| s.as_str())
            .unwrap_or(model_id)
    }

    /// Get provider by name
    pub fn get_provider(&self, name: &str) -> Option<&ProviderEntry> {
        self.providers.get(name)
    }

    /// Get all providers
    pub fn get_all_providers(&self) -> Vec<&ProviderEntry> {
        self.providers.values().collect()
    }

    /// Get enabled providers from env
    pub fn get_enabled_providers(&self) -> Vec<&ProviderEntry> {
        self.providers
            .values()
            .filter(|p| {
                if p.api_key_env.is_empty() {
                    true // Local providers (ollama, lmstudio)
                } else {
                    std::env::var(&p.api_key_env).is_ok()
                }
            })
            .collect()
    }

    /// Get model by ID
    pub fn get_model(&self, model_id: &str) -> Option<&ModelEntry> {
        let resolved = self.resolve_model(model_id);
        self.models.get(resolved)
    }

    /// Get all models
    pub fn get_all_models(&self) -> Vec<&ModelEntry> {
        self.models.values().filter(|m| m.enabled).collect()
    }

    /// Get models by provider
    pub fn get_provider_models(&self, provider: &str) -> Vec<&ModelEntry> {
        self.models
            .values()
            .filter(|m| m.provider == provider && m.enabled)
            .collect()
    }

    /// Import models from cache
    pub fn import_from_cache(&mut self, models: &[CachedModel]) {
        for model in models {
            let entry = ModelEntry::new(&model.id, &model.provider);
            self.register_model(entry);
        }
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Extension for providers without auth
impl ProviderEntry {
    pub fn without_auth(mut self) -> Self {
        self.api_key_env = String::new();
        self.auth_header = String::new();
        self.auth_prefix = None;
        self
    }
}
