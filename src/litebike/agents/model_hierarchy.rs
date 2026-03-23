use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Model hierarchy and selection system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelHierarchy {
    /// Root models with highest capability
    pub roots: Vec<ModelNode>,
    /// Flat map of all providers
    pub providers: HashMap<String, ProviderConfig>,
    /// Active model preferences by task type
    pub preferences: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelNode {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub tier: u8,  // 0 = highest tier
    pub reasoning: bool,
    pub capabilities: Vec<String>,
    pub cost_per_million_tokens: f64,
    pub context_window: usize,
    pub children: Vec<ModelNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub api_type: String,
    pub models: Vec<ModelMeta>,
    pub is_free: bool,
    pub free_quota_tokens: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMeta {
    pub id: String,
    pub display_name: String,
    pub max_tokens: usize,
    pub context_window: usize,
}

impl ModelHierarchy {
    pub fn new() -> Self {
        let mut h = Self {
            roots: Vec::new(),
            providers: HashMap::new(),
            preferences: HashMap::new(),
        };

        // Build default hierarchy with NVIDIA free quota models
        h.load_nvidia_models();
        h.load_openrouter_models();
        h.load_groq_models();
        h.load_openai_models();
        h.load_gemini_models();

        // Set task-specific preferences
        h.preferences.insert("coding".to_string(), vec![
            "groq/llama-3.1-8b-instant".to_string(),
            "nvidia/meta/llama-3.1-8b-instruct".to_string(),
            "openrouter/free".to_string(),
        ]);

        h.preferences.insert("reasoning".to_string(), vec![
            "anthropic/claude-3.5-haiku".to_string(),
            "openrouter/anthropic/claude-3.5-haiku".to_string(),
        ]);

        h.preferences.insert("analysis".to_string(), vec![
            "nvidia/meta/llama-3.1-8b-instruct".to_string(),
            "openrouter/free".to_string(),
        ]);

        h
    }

    fn max_ctx() -> usize {
        literbike::modelmux::utils::max_context_window() as usize
    }

    fn load_nvidia_models(&mut self) {
        let max_ctx = Self::max_ctx();
        let nvidia = ProviderConfig {
            name: "NVIDIA".to_string(),
            base_url: "https://api.nvidia.com/v1".to_string(),
            api_key: std::env::var("NVIDIA_API_KEY").unwrap_or_default(),
            api_type: "openai-completions".to_string(),
            models: vec![ModelMeta {
                id: "meta/llama-3.1-8b-instruct".to_string(),
                display_name: "Llama 3.1 8B (NVIDIA) - FREE QUOTA".to_string(),
                max_tokens: 8000,
                context_window: max_ctx,
            }],
            is_free: true,
            free_quota_tokens: Some(1000000000), // 1B tokens
        };
        self.providers.insert("nvidia".to_string(), nvidia);

        let node = ModelNode {
            id: "meta/llama-3.1-8b-instruct".to_string(),
            name: "Llama 3.1 8B (NVIDIA)".to_string(),
            provider: "nvidia".to_string(),
            tier: 2,
            reasoning: false,
            capabilities: vec!["text".to_string(), "code".to_string()],
            cost_per_million_tokens: 0.0,  // FREE
            context_window: max_ctx,
            children: Vec::new(),
        };
        self.roots.push(node);
    }

    fn load_openrouter_models(&mut self) {
        let max_ctx = Self::max_ctx();
        let openrouter = ProviderConfig {
            name: "OpenRouter".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            api_key: std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
            api_type: "openai-completions".to_string(),
            models: vec![
                ModelMeta {
                    id: "openrouter/free".to_string(),
                    display_name: "OpenRouter Free".to_string(),
                    max_tokens: 4096,
                    context_window: max_ctx,
                },
                ModelMeta {
                    id: "anthropic/claude-3.5-haiku".to_string(),
                    display_name: "Claude 3.5 Haiku".to_string(),
                    max_tokens: 8192,
                    context_window: max_ctx,
                },
            ],
            is_free: false,
            free_quota_tokens: None,
        };
        self.providers.insert("openrouter".to_string(), openrouter);

        // Add to hierarchy
        let haiku_node = ModelNode {
            id: "anthropic/claude-3.5-haiku".to_string(),
            name: "Claude 3.5 Haiku".to_string(),
            provider: "openrouter".to_string(),
            tier: 1,
            reasoning: true,
            capabilities: vec!["text".to_string(), "code".to_string(), "analysis".to_string()],
            cost_per_million_tokens: 2.0,
            context_window: max_ctx,
            children: Vec::new(),
        };
        self.roots.push(haiku_node);
    }

    fn load_groq_models(&mut self) {
        let max_ctx = Self::max_ctx();
        let groq = ProviderConfig {
            name: "Groq".to_string(),
            base_url: "https://api.groq.com/openai/v1".to_string(),
            api_key: std::env::var("GROQ_API_KEY").unwrap_or_default(),
            api_type: "openai-completions".to_string(),
            models: vec![
                ModelMeta {
                    id: "llama-3.1-8b-instant".to_string(),
                    display_name: "Llama 3.1 8B (Groq)".to_string(),
                    max_tokens: 8000,
                    context_window: max_ctx,
                },
                ModelMeta {
                    id: "llama3-8b-8192".to_string(),
                    display_name: "Llama 3 8B (Groq)".to_string(),
                    max_tokens: 8000,
                    context_window: max_ctx,
                },
            ],
            is_free: true,
            free_quota_tokens: Some(1000000000), // 1B tokens free on Groq
        };
        self.providers.insert("groq".to_string(), groq);

        let node = ModelNode {
            id: "llama-3.1-8b-instant".to_string(),
            name: "Llama 3.1 8B (Groq)".to_string(),
            provider: "groq".to_string(),
            tier: 2,
            reasoning: false,
            capabilities: vec!["text".to_string(), "code".to_string()],
            cost_per_million_tokens: 0.0,
            context_window: max_ctx,
            children: Vec::new(),
        };
        self.roots.push(node);
    }

    fn load_openai_models(&mut self) {
        let max_ctx = Self::max_ctx();
        let openai = ProviderConfig {
            name: "OpenAI".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            api_type: "openai-completions".to_string(),
            models: vec![
                ModelMeta {
                    id: "gpt-4o-mini".to_string(),
                    display_name: "GPT-4o Mini".to_string(),
                    max_tokens: 16384,
                    context_window: max_ctx,
                },
                ModelMeta {
                    id: "gpt-4o".to_string(),
                    display_name: "GPT-4o".to_string(),
                    max_tokens: 16384,
                    context_window: max_ctx,
                },
            ],
            is_free: false,
            free_quota_tokens: None,
        };
        self.providers.insert("openai".to_string(), openai);
    }

    fn load_gemini_models(&mut self) {
        let max_ctx = Self::max_ctx();
        let google = ProviderConfig {
            name: "Google".to_string(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            api_key: std::env::var("GEMINI_API_KEY").unwrap_or_default(),
            api_type: "google-generative-ai".to_string(),
            models: vec![
                ModelMeta {
                    id: "gemini-2.0-flash".to_string(),
                    display_name: "Gemini 2.0 Flash".to_string(),
                    max_tokens: 8192,
                    context_window: max_ctx,
                },
                ModelMeta {
                    id: "gemini-1.5-flash".to_string(),
                    display_name: "Gemini 1.5 Flash".to_string(),
                    max_tokens: 8192,
                    context_window: max_ctx,
                },
            ],
            is_free: true,
            free_quota_tokens: Some(1500000000), // 1.5B tokens
        };
        self.providers.insert("google".to_string(), google);
    }

    /// Select best model for task type
    pub fn select_model(&self, task_type: &str) -> Option<String> {
        self.preferences.get(task_type)
            .and_then(|models| models.first())
            .map(|s| s.clone())
    }

    /// Get model by ID
    pub fn get_model(&self, model_id: &str) -> Option<&ModelNode> {
        self.roots.iter()
            .find(|n| n.id == model_id)
    }

    /// List all models with free status
    pub fn list_models(&self) -> Vec<(String, String, bool, String)> {
        self.providers.iter()
            .flat_map(|(provider_name, config)| {
                config.models.iter().map(|m| {
                    let is_free = config.is_free;
                    let status = if is_free {
                        "FREE".to_string()
                    } else {
                        "PAID".to_string()
                    };
                    (m.id.clone(), m.display_name.clone(), is_free, status)
                })
            })
            .collect()
    }

    /// Export model selection for agent config
    pub fn export_agent_config(&self) -> Value {
        let coding_model = self.select_model("coding").unwrap_or("groq/llama-3.1-8b-instant".to_string());
        let reasoning_model = self.select_model("reasoning").unwrap_or("anthropic/claude-3.5-haiku".to_string());

        serde_json::json!({
            "defaults": {
                "model": coding_model,
                "maxConcurrent": 8,
            },
            "coding_agent": {
                "primary": "groq/llama-3.1-8b-instant",
                "fallbacks": [
                    "nvidia/meta/llama-3.1-8b-instruct",
                    "openrouter/free",
                    "openai/gpt-4o-mini"
                ]
            },
            "reasoning_agent": {
                "primary": reasoning_model,
                "fallbacks": [
                    "openrouter/free",
                    "nvidia/meta/llama-3.1-8b-instruct"
                ]
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_hierarchy_creation() {
        let hierarchy = ModelHierarchy::new();
        assert!(!hierarchy.roots.is_empty());
        assert!(!hierarchy.providers.is_empty());
        assert!(!hierarchy.preferences.is_empty());
    }

    #[test]
    fn test_nvidia_free_quota() {
        let hierarchy = ModelHierarchy::new();
        let nvidia = hierarchy.providers.get("nvidia");
        assert!(nvidia.is_some());
        assert!(nvidia.unwrap().is_free);
        assert!(nvidia.unwrap().free_quota_tokens.is_some());
    }

    #[test]
    fn test_model_selection() {
        let hierarchy = ModelHierarchy::new();
        let coding_model = hierarchy.select_model("coding");
        assert!(coding_model.is_some());
        let reasoning_model = hierarchy.select_model("reasoning");
        assert!(reasoning_model.is_some());
    }

    #[test]
    fn test_model_listing() {
        let hierarchy = ModelHierarchy::new();
        let models = hierarchy.list_models();
        // Should have at least 4 providers with models
        assert!(models.len() >= 4);
        // At least some should be free
        let free_models: Vec<_> = models.iter().filter(|(_, _, free, _)| *free).collect();
        assert!(!free_models.is_empty());
    }

    #[test]
    fn test_export_config() {
        let hierarchy = ModelHierarchy::new();
        let config = hierarchy.export_agent_config();
        assert!(config["defaults"].is_object());
        assert!(config["coding_agent"].is_object());
        assert!(config["reasoning_agent"].is_object());
    }
}
