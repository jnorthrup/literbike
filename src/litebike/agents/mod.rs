use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod model_hierarchy;
pub mod web_tools;

/// Agent configuration system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub workspace: String,
    pub model: AgentModelConfig,
    pub capabilities: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentModelConfig {
    pub primary: String,
    pub fallbacks: Vec<String>,
    pub custom_name: Option<String>,
    pub reasoning: Option<String>, // "high", "medium", "low"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntime {
    pub agents: Vec<AgentConfig>,
    pub gateways: GatewayConfig,
    pub providers: ProviderConfigs,
    pub web_tools: WebToolConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub token: String,
    pub port: u16,
    pub mode: String,
    pub safety: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub disable_device_auth: bool,
    pub allow_host_header_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfigs {
    pub openrouter: ProviderAuth,
    pub groq: ProviderAuth,
    pub nvidia: ProviderAuth,
    pub google: ProviderAuth,
    pub openai: ProviderAuth,
    pub deepseek: ProviderAuth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAuth {
    pub base_url: String,
    pub api_key: String,
    pub api_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebToolConfig {
    pub enabled: bool,
    pub providers: Vec<WebSearchProvider>,
    pub json_tools: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchProvider {
    pub name: String,
    pub api_key: String,
    pub endpoint: String,
}

impl AgentRuntime {
    /// Create default configuration
    pub fn new_with_defaults(access_token: &str) -> Self {
        Self {
            agents: vec![
                AgentConfig {
                    id: "claude".to_string(),
                    name: "Claude".to_string(),
                    workspace: "/home/admin/projects/claude-main".to_string(),
                    model: AgentModelConfig {
                        primary: "anthropic/claude-3.5-haiku".to_string(),
                        fallbacks: vec![
                            "openrouter/free".to_string(),
                            "nvidia/meta/llama-3.1-8b-instruct".to_string(),
                        ],
                        custom_name: Some("Claude".to_string()),
                        reasoning: Some("high".to_string()),
                    },
                    capabilities: vec![
                        "web_search".to_string(),
                        "json_processing".to_string(),
                        "file_operations".to_string(),
                        "code_analysis".to_string(),
                    ],
                    enabled: true,
                },
                AgentConfig {
                    id: "coder".to_string(),
                    name: "Coding Agent".to_string(),
                    workspace: "/home/admin/projects/coder-workspace".to_string(),
                    model: AgentModelConfig {
                        primary: "groq/llama-3.1-8b-instant".to_string(),
                        fallbacks: vec![
                            "nvidia/meta/llama-3.1-8b-instruct".to_string(),
                            "openrouter/free".to_string(),
                        ],
                        custom_name: None,
                        reasoning: Some("medium".to_string()),
                    },
                    capabilities: vec![
                        "code_generation".to_string(),
                        "code_review".to_string(),
                        "testing".to_string(),
                    ],
                    enabled: true,
                },
                AgentConfig {
                    id: "litebike".to_string(),
                    name: "Litebike Bot".to_string(),
                    workspace: "/home/admin/projects/litebike".to_string(),
                    model: AgentModelConfig {
                        primary: "groq/llama-3.1-8b-instant".to_string(),
                        fallbacks: vec!["openrouter/free".to_string()],
                        custom_name: None,
                        reasoning: Some("low".to_string()),
                    },
                    capabilities: vec!["p2p_routing".to_string()],
                    enabled: true,
                },
            ],
            gateways: GatewayConfig {
                token: access_token.to_string(),
                port: 18789,
                mode: "local".to_string(),
                safety: SecurityConfig {
                    disable_device_auth: true,
                    allow_host_header_fallback: true,
                },
            },
            providers: ProviderConfigs {
                openrouter: ProviderAuth {
                    base_url: "https://openrouter.ai/api/v1".to_string(),
                    api_key: std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
                    api_type: "openai-completions".to_string(),
                },
                groq: ProviderAuth {
                    base_url: "https://api.groq.com/openai/v1".to_string(),
                    api_key: std::env::var("GROQ_API_KEY").unwrap_or_default(),
                    api_type: "openai-completions".to_string(),
                },
                nvidia: ProviderAuth {
                    base_url: "https://api.nvidia.com/v1".to_string(),
                    api_key: std::env::var("NVIDIA_API_KEY").unwrap_or_default(),
                    api_type: "openai-completions".to_string(),
                },
                google: ProviderAuth {
                    base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
                    api_key: std::env::var("GEMINI_API_KEY").unwrap_or_default(),
                    api_type: "google-generative-ai".to_string(),
                },
                openai: ProviderAuth {
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
                    api_type: "openai-completions".to_string(),
                },
                deepseek: ProviderAuth {
                    base_url: "https://api.deepseek.com".to_string(),
                    api_key: std::env::var("DEEPSEEK_API_KEY").unwrap_or_default(),
                    api_type: "openai-completions".to_string(),
                },
            },
            web_tools: WebToolConfig {
                enabled: true,
                providers: vec![
                    WebSearchProvider {
                        name: "brave".to_string(),
                        api_key: std::env::var("BRAVE_SEARCH_API_KEY").unwrap_or_default(),
                        endpoint: "https://api.search.brave.com/res/v1/web/search".to_string(),
                    },
                    WebSearchProvider {
                        name: "duckduckgo".to_string(),
                        api_key: std::env::var("DUCKDUCK_SEARCH_API_KEY").unwrap_or_default(),
                        endpoint: "https://duckduckgo.com/".to_string(),
                    },
                ],
                json_tools: true,
            },
        }
    }

    /// Export as JSON for OpenClaw config
    pub fn to_openclaw_config(&self) -> Value {
        serde_json::json!({
            "meta": {
                "lastTouchedVersion": "2026.2.25",
                "lastTouchedAt": chrono::Utc::now().to_rfc3339(),
            },
            "agents": {
                "defaults": {
                    "model": self.agents[0].model.primary.clone(),
                    "maxConcurrent": 8,
                },
                "list": self.agents.iter().map(|a| serde_json::json!({
                    "id": a.id,
                    "name": a.name,
                    "workspace": a.workspace,
                    "model": {
                        "primary": a.model.primary,
                        "fallbacks": a.model.fallbacks,
                    },
                    "capabilities": a.capabilities,
                    "enabled": a.enabled,
                })).collect::<Vec<_>>(),
            },
            "gateway": {
                "mode": self.gateways.mode,
                "port": self.gateways.port,
                "controlUi": {
                    "dangerouslyAllowHostHeaderOriginFallback": self.gateways.safety.allow_host_header_fallback,
                    "dangerouslyDisableDeviceAuth": self.gateways.safety.disable_device_auth,
                },
                "auth": {
                    "mode": "token",
                    "token": self.gateways.token,
                }
            },
            "models": {
                "mode": "merge",
                "providers": self.providers_to_json(),
            },
            "webTools": {
                "enabled": self.web_tools.enabled,
                "providers": self.web_tools.providers.iter().map(|p| serde_json::json!({
                    "name": p.name,
                    "endpoint": p.endpoint,
                })).collect::<Vec<_>>(),
                "jsonTools": self.web_tools.json_tools,
            }
        })
    }

    fn providers_to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "openrouter": {
                "baseUrl": self.providers.openrouter.base_url,
                "apiKey": self.providers.openrouter.api_key,
                "api": self.providers.openrouter.api_type,
                "models": [
                    {"id": "openrouter/free", "name": "OpenRouter Free"},
                    {"id": "anthropic/claude-3.5-haiku", "name": "Claude 3.5 Haiku"},
                ],
            },
            "groq": {
                "baseUrl": self.providers.groq.base_url,
                "apiKey": self.providers.groq.api_key,
                "api": self.providers.groq.api_type,
                "models": [
                    {"id": "llama-3.1-8b-instant", "name": "Llama 3.1 8B (Groq)"},
                    {"id": "llama3-8b-8192", "name": "Llama 3 8B (Groq)"},
                ],
            },
            "nvidia": {
                "baseUrl": self.providers.nvidia.base_url,
                "apiKey": self.providers.nvidia.api_key,
                "api": self.providers.nvidia.api_type,
                "models": [
                    {"id": "meta/llama-3.1-8b-instruct", "name": "Llama 3.1 8B (NVIDIA) - FREE"},
                ],
            },
            "google": {
                "baseUrl": self.providers.google.base_url,
                "apiKey": self.providers.google.api_key,
                "api": self.providers.google.api_type,
                "models": [
                    {"id": "gemini-2.0-flash", "name": "Gemini 2.0 Flash"},
                    {"id": "gemini-1.5-flash", "name": "Gemini 1.5 Flash"},
                ],
            },
            "openai": {
                "baseUrl": self.providers.openai.base_url,
                "apiKey": self.providers.openai.api_key,
                "api": self.providers.openai.api_type,
                "models": [
                    {"id": "gpt-4o-mini", "name": "GPT-4o Mini"},
                    {"id": "gpt-4o", "name": "GPT-4o"},
                ],
            },
            "deepseek": {
                "baseUrl": self.providers.deepseek.base_url,
                "apiKey": self.providers.deepseek.api_key,
                "api": self.providers.deepseek.api_type,
                "models": [
                    {"id": "deepseek-chat", "name": "DeepSeek Chat"},
                ],
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_creation() {
        let runtime = AgentRuntime::new_with_defaults("test-token");
        assert_eq!(runtime.agents.len(), 3);
        assert_eq!(runtime.gateways.token, "test-token");
        assert!(!runtime.web_tools.providers.is_empty());
    }

    #[test]
    fn test_to_openclaw_config() {
        let runtime = AgentRuntime::new_with_defaults("test-token-123");
        let config = runtime.to_openclaw_config();

        assert!(config["agents"].is_object());
        assert!(config["gateway"].is_object());
        assert!(config["models"].is_object());
        assert!(config["webTools"].is_object());

        let gateway = &config["gateway"];
        assert_eq!(gateway["auth"]["token"], "test-token-123");
        assert_eq!(gateway["port"], 18789);
    }

    #[test]
    fn test_capabilities() {
        let runtime = AgentRuntime::new_with_defaults("test-token");
        let claude = &runtime.agents[0];
        assert!(claude.capabilities.contains(&"web_search".to_string()));
        assert!(claude.capabilities.contains(&"json_processing".to_string()));
    }
}
