use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

use super::control::{GatewayControlState, GatewayFacadeFamily, ProviderKeyPrecedence};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolbarServiceStatus {
    Running,
    Degraded,
    Cold,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolbarServiceManager {
    ExternalLauncher,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolbarConfidenceBucket {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolbarPersistenceKind {
    Volatile,
    MarkdownTodo,
    Sqlite,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolbarSurfaceKind {
    ControlApi,
    OpenAiCompat,
    OllamaCompat,
    QuotaLedger,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolbarServiceState {
    pub status: ToolbarServiceStatus,
    pub manager: ToolbarServiceManager,
    pub bind_address: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolbarRouteState {
    pub family: GatewayFacadeFamily,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub unified_agent_port: bool,
    pub failover_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolbarEnvState {
    pub recognized_keys: usize,
    pub unknown_keys: usize,
    pub confidence: ToolbarConfidenceBucket,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolbarDebtState {
    pub open_items: usize,
    pub blocked_items: usize,
    pub persistence: ToolbarPersistenceKind,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolbarRuntimeState {
    pub streaming_enabled: bool,
    pub claude_rewrite_enabled: bool,
    pub keymux_strategy: String,
    pub provider_key_overrides: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolbarSurfaceState {
    pub kind: ToolbarSurfaceKind,
    pub available: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolbarState {
    pub service: ToolbarServiceState,
    pub route: ToolbarRouteState,
    pub env: ToolbarEnvState,
    pub debt: ToolbarDebtState,
    pub runtime: ToolbarRuntimeState,
    pub surfaces: Vec<ToolbarSurfaceState>,
    pub available_actions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ToolbarAction {
    RescanEnv,
    ResetRuntime,
    SetStreamingEnabled {
        enabled: bool,
    },
    SetPreferredProvider {
        provider: String,
    },
    ClearPreferredProvider,
    SetDefaultModel {
        model: String,
    },
    ClearDefaultModel,
    SetFallbackModel {
        model: String,
    },
    ClearFallbackModel,
    SetClaudeRewriteEnabled {
        enabled: bool,
    },
    SetClaudeRewritePolicy {
        enabled: bool,
        default_model: Option<String>,
        haiku_model: Option<String>,
        sonnet_model: Option<String>,
        opus_model: Option<String>,
        reasoning_model: Option<String>,
    },
    ClearClaudeRewritePolicy,
    SetProviderKeyPolicy {
        provider: String,
        env_key: Option<String>,
        override_env_key: Option<String>,
        precedence: ProviderKeyPrecedence,
    },
    ClearProviderKeyPolicy {
        provider: String,
    },
    ImportCcSwitchKeysAdditive {
        path: Option<String>,
    },
}

pub fn derive_toolbar_state(gateway: &GatewayControlState) -> ToolbarState {
    let env = scan_env_state();
    let debt = scan_debt_state();
    let route = derive_route_state(gateway);
    let service = ToolbarServiceState {
        status: service_status_for_metrics(
            !gateway.providers.is_empty(),
            route.provider.is_some() || route.model.is_some(),
            env.unknown_keys,
            debt.blocked_items,
        ),
        manager: ToolbarServiceManager::ExternalLauncher,
        bind_address: gateway.transport.bind_address.clone(),
        port: gateway.transport.port,
    };
    let runtime = ToolbarRuntimeState {
        streaming_enabled: gateway.streaming.enabled,
        claude_rewrite_enabled: gateway.claude_model_rewrite.enabled,
        keymux_strategy: gateway.keymux.strategy.clone(),
        provider_key_overrides: gateway
            .keymux
            .provider_keys
            .iter()
            .filter(|state| state.override_env_key.is_some())
            .count(),
    };
    let surfaces = vec![
        ToolbarSurfaceState {
            kind: ToolbarSurfaceKind::ControlApi,
            available: true,
            detail: "/control/state + /control/actions".to_string(),
        },
        ToolbarSurfaceState {
            kind: ToolbarSurfaceKind::OpenAiCompat,
            available: true,
            detail: "/v1/models + /v1/chat/completions".to_string(),
        },
        ToolbarSurfaceState {
            kind: ToolbarSurfaceKind::OllamaCompat,
            available: true,
            detail: "/api/tags + /api/chat".to_string(),
        },
        ToolbarSurfaceState {
            kind: ToolbarSurfaceKind::QuotaLedger,
            available: !gateway.providers.is_empty(),
            detail: if gateway.providers.is_empty() {
                "no active provider ledgers".to_string()
            } else {
                format!("{} provider ledgers active", gateway.providers.len())
            },
        },
    ];

    ToolbarState {
        service,
        route,
        env,
        debt,
        runtime,
        surfaces,
        available_actions: vec![
            "rescan_env".to_string(),
            "reset_runtime".to_string(),
            "set_streaming_enabled".to_string(),
            "set_preferred_provider".to_string(),
            "clear_preferred_provider".to_string(),
            "set_default_model".to_string(),
            "clear_default_model".to_string(),
            "set_fallback_model".to_string(),
            "clear_fallback_model".to_string(),
            "set_claude_rewrite_enabled".to_string(),
            "set_claude_rewrite_policy".to_string(),
            "clear_claude_rewrite_policy".to_string(),
            "set_provider_key_policy".to_string(),
            "clear_provider_key_policy".to_string(),
            "import_cc_switch_keys_additive".to_string(),
        ],
    }
}

fn derive_route_state(gateway: &GatewayControlState) -> ToolbarRouteState {
    let model = gateway
        .routing
        .default_model
        .clone()
        .or_else(|| gateway.claude_model_rewrite.default_model.clone())
        .or_else(|| gateway.routing.fallback_model.clone());
    let provider = gateway
        .routing
        .preferred_provider
        .clone()
        .or_else(|| model.as_deref().and_then(provider_from_model))
        .or_else(|| gateway.providers.first().map(|p| p.name.clone()));
    let family = provider
        .as_deref()
        .and_then(|name| {
            gateway
                .providers
                .iter()
                .find(|provider| provider.name == name)
                .map(|provider| provider.family)
        })
        .or_else(|| model.as_deref().map(family_from_model))
        .unwrap_or(GatewayFacadeFamily::Unknown);

    ToolbarRouteState {
        family,
        provider,
        model,
        unified_agent_port: gateway.transport.unified_agent_port,
        failover_enabled: gateway.routing.failover_enabled,
    }
}

fn scan_env_state() -> ToolbarEnvState {
    scan_env_state_from(env::vars())
}

fn scan_env_state_from<I>(vars: I) -> ToolbarEnvState
where
    I: IntoIterator<Item = (String, String)>,
{
    let mut recognized_keys = 0usize;
    let mut unknown_keys = 0usize;

    for (key, value) in vars {
        if value.trim().is_empty() {
            continue;
        }

        let normalized = key.trim().to_ascii_uppercase();
        if is_known_env_key(&normalized) {
            recognized_keys += 1;
            continue;
        }

        if normalized.ends_with("_API_KEY") || normalized.ends_with("_BASE_URL") {
            unknown_keys += 1;
        }
    }

    let confidence = if recognized_keys == 0 {
        ToolbarConfidenceBucket::Low
    } else if unknown_keys > 0 {
        ToolbarConfidenceBucket::Medium
    } else {
        ToolbarConfidenceBucket::High
    };

    ToolbarEnvState {
        recognized_keys,
        unknown_keys,
        confidence,
    }
}

fn scan_debt_state() -> ToolbarDebtState {
    let sqlite_path = env::var("MODELMUX_LEDGER_DB")
        .ok()
        .map(PathBuf::from)
        .or_else(default_sqlite_path);

    if let Some(path) = sqlite_path {
        if path.exists() {
            return ToolbarDebtState {
                open_items: 0,
                blocked_items: 0,
                persistence: ToolbarPersistenceKind::Sqlite,
                source_path: Some(path.display().to_string()),
            };
        }
    }

    if let Some(path) = todo_markdown_path() {
        if let Ok(content) = fs::read_to_string(&path) {
            let summary = parse_todo_markdown(&content);
            return ToolbarDebtState {
                open_items: summary.open_items,
                blocked_items: summary.blocked_items,
                persistence: ToolbarPersistenceKind::MarkdownTodo,
                source_path: Some(path.display().to_string()),
            };
        }
    }

    ToolbarDebtState {
        open_items: 0,
        blocked_items: 0,
        persistence: ToolbarPersistenceKind::Volatile,
        source_path: None,
    }
}

fn default_sqlite_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".modelmux").join("toolbar.sqlite"))
}

fn todo_markdown_path() -> Option<PathBuf> {
    if let Ok(path) = env::var("MODELMUX_TODO_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    let cwd = env::current_dir().ok()?;
    let path = cwd.join("TODO.md");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct TodoSummary {
    open_items: usize,
    blocked_items: usize,
}

fn parse_todo_markdown(content: &str) -> TodoSummary {
    let mut summary = TodoSummary::default();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("- [ ]") {
            summary.open_items += 1;
            if trimmed.to_ascii_uppercase().contains("BLOCKED") {
                summary.blocked_items += 1;
            }
        }
    }

    summary
}

fn service_status_for_metrics(
    has_providers: bool,
    has_route_hint: bool,
    unknown_env_keys: usize,
    blocked_items: usize,
) -> ToolbarServiceStatus {
    if !has_providers {
        return ToolbarServiceStatus::Cold;
    }

    if !has_route_hint || unknown_env_keys > 0 || blocked_items > 0 {
        ToolbarServiceStatus::Degraded
    } else {
        ToolbarServiceStatus::Running
    }
}

fn provider_from_model(model: &str) -> Option<String> {
    let prefix = model.split('/').next()?.trim();
    if prefix.is_empty() || prefix.eq_ignore_ascii_case(model) {
        None
    } else {
        Some(prefix.to_ascii_lowercase())
    }
}

fn family_from_model(model: &str) -> GatewayFacadeFamily {
    let lower = model.trim().to_ascii_lowercase();
    if lower.starts_with("anthropic/") || lower.starts_with("claude-") {
        GatewayFacadeFamily::AnthropicCompatible
    } else if lower.starts_with("gemini") || lower.starts_with("google/") {
        GatewayFacadeFamily::GeminiNative
    } else if lower.starts_with("ollama/") || lower.starts_with("lmstudio/") {
        GatewayFacadeFamily::OllamaCompatible
    } else if lower.contains('/') {
        GatewayFacadeFamily::OpenAiCompatible
    } else {
        GatewayFacadeFamily::Unknown
    }
}

fn is_known_env_key(key: &str) -> bool {
    const PROVIDER_KEY_ALIASES: &[&str] = &[
        "KILOCODE_API_KEY",
        "KILOAI_API_KEY",
        "KILO_CODE_API_KEY",
        "KILO_API_KEY",
        "MOONSHOTAI_API_KEY",
        "KIMI_API_KEY",
        "MOONSHOT_API_KEY",
        "DEEPSEEK_API_KEY",
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "OPENROUTER_API_KEY",
        "GROQ_API_KEY",
        "XAI_API_KEY",
        "GROK_API_KEY",
        "CEREBRAS_API_KEY",
        "NVIDIA_API_KEY",
        "OPENCODE_API_KEY",
        "ZENMUX_API_KEY",
        "PERPLEXITY_API_KEY",
        "GEMINI_API_KEY",
    ];
    const PROVIDER_BASE_URLS: &[&str] = &[
        "KILO_CODE_BASE_URL",
        "MOONSHOT_BASE_URL",
        "MOONSHOTAI_BASE_URL",
        "DEEPSEEK_BASE_URL",
        "OPENAI_BASE_URL",
        "ANTHROPIC_BASE_URL",
        "OPENROUTER_BASE_URL",
        "GROQ_BASE_URL",
        "XAI_BASE_URL",
        "CEREBRAS_BASE_URL",
        "NVIDIA_BASE_URL",
        "OPENCODE_BASE_URL",
        "ZENMUX_BASE_URL",
        "PERPLEXITY_BASE_URL",
        "GEMINI_BASE_URL",
    ];
    const RUNTIME_KEYS: &[&str] = &[
        "MODELMUX_CLAUDE_REWRITE",
        "MODELMUX_CLAUDE_DEFAULT_MODEL",
        "MODELMUX_CLAUDE_HAIKU_MODEL",
        "MODELMUX_CLAUDE_SONNET_MODEL",
        "MODELMUX_CLAUDE_OPUS_MODEL",
        "MODELMUX_CLAUDE_REASONING_MODEL",
        "MODELMUX_DEFAULT_MODEL",
        "MODELMUX_FALLBACK_MODEL",
        "MODELMUX_PORT",
        "MODELMUX_HOST",
        "MODELMUX_LOG_LEVEL",
        "MODELMUX_TODO_PATH",
        "MODELMUX_LEDGER_DB",
        "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
        "ANTHROPIC_REASONING_MODEL",
    ];

    PROVIDER_KEY_ALIASES.contains(&key)
        || PROVIDER_BASE_URLS.contains(&key)
        || RUNTIME_KEYS.contains(&key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modelmux::control::{
        ClaudeModelRewritePolicy, GatewayKeymuxState, GatewayProviderStatus, GatewayRoutingMode,
        GatewayRoutingState, GatewayStreamingState, GatewayTransportState,
    };

    #[test]
    fn parse_todo_markdown_counts_open_and_blocked_items() {
        let summary = parse_todo_markdown(
            r#"
            - [ ] First item
            - [ ] BLOCKED: Needs a branch
            - [x] Done
            "#,
        );

        assert_eq!(summary.open_items, 2);
        assert_eq!(summary.blocked_items, 1);
    }

    #[test]
    fn service_status_degrades_on_unknown_env_or_blocked_items() {
        assert_eq!(
            service_status_for_metrics(true, true, 0, 0),
            ToolbarServiceStatus::Running
        );
        assert_eq!(
            service_status_for_metrics(true, true, 1, 0),
            ToolbarServiceStatus::Degraded
        );
        assert_eq!(
            service_status_for_metrics(false, false, 0, 0),
            ToolbarServiceStatus::Cold
        );
    }

    #[test]
    fn derive_toolbar_route_prefers_configured_provider_and_model() {
        let gateway = GatewayControlState {
            transport: GatewayTransportState {
                bind_address: "127.0.0.1".to_string(),
                port: 8888,
                unified_agent_port: true,
                listener: "http1".to_string(),
            },
            routing: GatewayRoutingState {
                mode: GatewayRoutingMode::ModelPrefixThenPriority,
                preferred_provider: Some("anthropic".to_string()),
                default_model: Some("anthropic/claude-3-5-sonnet".to_string()),
                fallback_model: None,
                failover_enabled: true,
            },
            streaming: GatewayStreamingState {
                enabled: true,
                openai_chat_completions: "disabled".to_string(),
                ollama_chat: "ndjson".to_string(),
            },
            claude_model_rewrite: ClaudeModelRewritePolicy {
                enabled: false,
                default_model: None,
                haiku_model: None,
                sonnet_model: None,
                opus_model: None,
                reasoning_model: None,
            },
            keymux: GatewayKeymuxState {
                strategy: "default".to_string(),
                provider_keys: Vec::new(),
            },
            providers: vec![GatewayProviderStatus {
                name: "anthropic".to_string(),
                family: GatewayFacadeFamily::AnthropicCompatible,
                base_url: "https://api.anthropic.com/v1".to_string(),
                key_env: "ANTHROPIC_API_KEY".to_string(),
                priority: 1,
                active: true,
                tokens_used_today: 0,
                estimated_remaining_quota: 0,
                quota_confidence: 0.0,
            }],
        };

        let toolbar = derive_toolbar_state(&gateway);
        assert_eq!(toolbar.route.provider.as_deref(), Some("anthropic"));
        assert_eq!(
            toolbar.route.model.as_deref(),
            Some("anthropic/claude-3-5-sonnet")
        );
        assert_eq!(
            toolbar.route.family,
            GatewayFacadeFamily::AnthropicCompatible
        );
    }

    #[test]
    fn toolbar_action_accepts_canonical_rewrite_action() {
        let action: ToolbarAction = serde_json::from_value(serde_json::json!({
            "action": "set_claude_rewrite_policy",
            "enabled": true,
            "default_model": "ordinal/default",
            "sonnet_model": "ordinal/sonnet"
        }))
        .unwrap();

        assert_eq!(
            action,
            ToolbarAction::SetClaudeRewritePolicy {
                enabled: true,
                default_model: Some("ordinal/default".to_string()),
                haiku_model: None,
                sonnet_model: Some("ordinal/sonnet".to_string()),
                opus_model: None,
                reasoning_model: None,
            }
        );
    }

    #[test]
    fn scan_env_state_from_counts_recognized_runtime_keys() {
        let state = scan_env_state_from(vec![
            (
                "MODELMUX_DEFAULT_MODEL".to_string(),
                "anthropic/sonnet".to_string(),
            ),
            ("MODELMUX_PORT".to_string(), "8888".to_string()),
            ("OPENAI_API_KEY".to_string(), "sk-live-key".to_string()),
        ]);

        assert_eq!(state.recognized_keys, 3);
        assert_eq!(state.unknown_keys, 0);
        assert_eq!(state.confidence, ToolbarConfidenceBucket::High);
    }

    #[test]
    fn scan_env_state_from_counts_unknown_provider_keys() {
        let state = scan_env_state_from(vec![
            ("CUSTOMLAB_API_KEY".to_string(), "abc123".to_string()),
            (
                "MODELMUX_DEFAULT_MODEL".to_string(),
                "openai/gpt-4o-mini".to_string(),
            ),
        ]);

        assert_eq!(state.recognized_keys, 1);
        assert_eq!(state.unknown_keys, 1);
        assert_eq!(state.confidence, ToolbarConfidenceBucket::Medium);
    }
}
