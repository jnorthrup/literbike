use crate::keymux::dsel;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::proxy::ProxyConfig;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GatewayFacadeFamily {
    OpenAiCompatible,
    AnthropicCompatible,
    GeminiNative,
    OllamaCompatible,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GatewayRoutingMode {
    ModelPrefixThenPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayProviderStatus {
    pub name: String,
    pub family: GatewayFacadeFamily,
    pub base_url: String,
    pub key_env: String,
    pub priority: u8,
    pub active: bool,
    pub tokens_used_today: u64,
    pub estimated_remaining_quota: u64,
    pub quota_confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayTransportState {
    pub bind_address: String,
    pub port: u16,
    pub unified_agent_port: bool,
    pub listener: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayRoutingState {
    pub mode: GatewayRoutingMode,
    pub preferred_provider: Option<String>,
    pub default_model: Option<String>,
    pub fallback_model: Option<String>,
    pub failover_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayStreamingState {
    pub enabled: bool,
    pub openai_chat_completions: String,
    pub ollama_chat: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaudeModelRewritePolicy {
    pub enabled: bool,
    pub default_model: Option<String>,
    pub haiku_model: Option<String>,
    pub sonnet_model: Option<String>,
    pub opus_model: Option<String>,
    pub reasoning_model: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKeyPrecedence {
    EnvironmentFirst,
    OverrideFirst,
    EnvironmentOnly,
    OverrideOnly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKeySource {
    Environment,
    Override,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderKeyPolicy {
    pub provider: String,
    pub env_key: Option<String>,
    pub override_env_key: Option<String>,
    pub precedence: ProviderKeyPrecedence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderKeyResolutionState {
    pub provider: String,
    pub env_key: Option<String>,
    pub override_env_key: Option<String>,
    pub precedence: ProviderKeyPrecedence,
    pub selected_source: ProviderKeySource,
    pub selected_env_key: Option<String>,
    pub key_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayKeymuxState {
    pub strategy: String,
    pub provider_keys: Vec<ProviderKeyResolutionState>,
}

pub trait ProviderKeyPolicyResolver: Send + Sync {
    fn resolve_policy(&self, policy: &ProviderKeyPolicy) -> ProviderKeyResolutionState;

    fn strategy_name(&self) -> &'static str {
        "default"
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultProviderKeyPolicyResolver;

impl ProviderKeyPolicyResolver for DefaultProviderKeyPolicyResolver {
    fn resolve_policy(&self, policy: &ProviderKeyPolicy) -> ProviderKeyResolutionState {
        let env_present = policy
            .env_key
            .as_deref()
            .is_some_and(env_key_present_and_nonempty);
        let override_present = policy
            .override_env_key
            .as_deref()
            .is_some_and(env_key_present_and_nonempty);

        let (selected_source, selected_env_key, key_present) = match policy.precedence {
            ProviderKeyPrecedence::EnvironmentFirst => {
                if env_present {
                    (ProviderKeySource::Environment, policy.env_key.clone(), true)
                } else if override_present {
                    (
                        ProviderKeySource::Override,
                        policy.override_env_key.clone(),
                        true,
                    )
                } else {
                    (ProviderKeySource::Missing, None, false)
                }
            }
            ProviderKeyPrecedence::OverrideFirst => {
                if override_present {
                    (
                        ProviderKeySource::Override,
                        policy.override_env_key.clone(),
                        true,
                    )
                } else if env_present {
                    (ProviderKeySource::Environment, policy.env_key.clone(), true)
                } else {
                    (ProviderKeySource::Missing, None, false)
                }
            }
            ProviderKeyPrecedence::EnvironmentOnly => {
                if env_present {
                    (ProviderKeySource::Environment, policy.env_key.clone(), true)
                } else {
                    (ProviderKeySource::Missing, policy.env_key.clone(), false)
                }
            }
            ProviderKeyPrecedence::OverrideOnly => {
                if override_present {
                    (
                        ProviderKeySource::Override,
                        policy.override_env_key.clone(),
                        true,
                    )
                } else {
                    (
                        ProviderKeySource::Missing,
                        policy.override_env_key.clone(),
                        false,
                    )
                }
            }
        };

        ProviderKeyResolutionState {
            provider: policy.provider.clone(),
            env_key: policy.env_key.clone(),
            override_env_key: policy.override_env_key.clone(),
            precedence: policy.precedence,
            selected_source,
            selected_env_key,
            key_present,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayControlState {
    pub transport: GatewayTransportState,
    pub routing: GatewayRoutingState,
    pub streaming: GatewayStreamingState,
    pub claude_model_rewrite: ClaudeModelRewritePolicy,
    pub keymux: GatewayKeymuxState,
    pub providers: Vec<GatewayProviderStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayRuntimeControl {
    pub preferred_provider: Option<String>,
    pub default_model: Option<String>,
    pub fallback_model: Option<String>,
    pub streaming_enabled: bool,
    pub claude_model_rewrite: ClaudeModelRewritePolicy,
    pub provider_key_policies: BTreeMap<String, ProviderKeyPolicy>,
}

impl GatewayRuntimeControl {
    pub fn from_config(config: &ProxyConfig) -> Self {
        let default_model = config.default_model.clone();
        let fallback_model = config.fallback_model.clone();
        let rewrite_configured = [
            "MODELMUX_CLAUDE_DEFAULT_MODEL",
            "MODELMUX_CLAUDE_SONNET_MODEL",
            "MODELMUX_CLAUDE_OPUS_MODEL",
            "MODELMUX_CLAUDE_HAIKU_MODEL",
            "MODELMUX_CLAUDE_REASONING_MODEL",
            "ANTHROPIC_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_REASONING_MODEL",
        ]
        .into_iter()
        .any(|key| env_string(key).is_some());
        let rewrite = ClaudeModelRewritePolicy {
            enabled: bool_env("MODELMUX_CLAUDE_REWRITE").unwrap_or(rewrite_configured),
            default_model: env_string_any(&["MODELMUX_CLAUDE_DEFAULT_MODEL", "ANTHROPIC_MODEL"]),
            haiku_model: env_string_any(&[
                "MODELMUX_CLAUDE_HAIKU_MODEL",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            ]),
            sonnet_model: env_string_any(&[
                "MODELMUX_CLAUDE_SONNET_MODEL",
                "ANTHROPIC_DEFAULT_SONNET_MODEL",
            ]),
            opus_model: env_string_any(&[
                "MODELMUX_CLAUDE_OPUS_MODEL",
                "ANTHROPIC_DEFAULT_OPUS_MODEL",
            ]),
            reasoning_model: env_string_any(&[
                "MODELMUX_CLAUDE_REASONING_MODEL",
                "ANTHROPIC_REASONING_MODEL",
            ]),
        };

        Self {
            preferred_provider: None,
            default_model,
            fallback_model,
            streaming_enabled: config.enable_streaming,
            claude_model_rewrite: rewrite,
            provider_key_policies: BTreeMap::new(),
        }
    }

    pub fn snapshot(&self, config: &ProxyConfig) -> GatewayControlState {
        let mut quota_map = BTreeMap::new();
        for (provider, used, remaining, confidence) in dsel::all_provider_quotas() {
            quota_map.insert(provider, (used, remaining, confidence));
        }

        let providers: Vec<GatewayProviderStatus> = dsel::discover_providers()
            .into_iter()
            .map(|provider| {
                let (used, remaining, confidence) = quota_map
                    .get(&provider.name)
                    .copied()
                    .unwrap_or((0, 0, 0.0));

                GatewayProviderStatus {
                    family: infer_provider_family(&provider.name, &provider.base_url),
                    name: provider.name,
                    base_url: provider.base_url,
                    key_env: provider.key_env,
                    priority: provider.priority,
                    active: true,
                    tokens_used_today: used,
                    estimated_remaining_quota: remaining,
                    quota_confidence: confidence,
                }
            })
            .collect();
        let keymux = self.snapshot_keymux_state(&providers);

        GatewayControlState {
            transport: GatewayTransportState {
                bind_address: config.bind_address.clone(),
                port: config.port,
                unified_agent_port: config.port == 8888,
                listener: "http1".to_string(),
            },
            routing: GatewayRoutingState {
                mode: GatewayRoutingMode::ModelPrefixThenPriority,
                preferred_provider: self.preferred_provider.clone(),
                default_model: self.default_model.clone(),
                fallback_model: self.fallback_model.clone(),
                failover_enabled: self.fallback_model.is_some()
                    || std::env::var("OPENROUTER_API_KEY").is_ok(),
            },
            streaming: GatewayStreamingState {
                enabled: self.streaming_enabled,
                openai_chat_completions: "disabled".to_string(),
                ollama_chat: if self.streaming_enabled {
                    "ndjson".to_string()
                } else {
                    "disabled".to_string()
                },
            },
            claude_model_rewrite: self.claude_model_rewrite.clone(),
            keymux,
            providers,
        }
    }

    pub fn apply_action(&mut self, action: GatewayControlAction) -> Result<(), String> {
        match action {
            GatewayControlAction::SetPreferredProvider { provider } => {
                if dsel::get_provider(&provider).is_none() {
                    return Err(format!("Unknown provider: {}", provider));
                }
                self.preferred_provider = Some(provider);
            }
            GatewayControlAction::ClearPreferredProvider => {
                self.preferred_provider = None;
            }
            GatewayControlAction::SetDefaultModel { model } => {
                self.default_model = normalize_string(model);
            }
            GatewayControlAction::ClearDefaultModel => {
                self.default_model = None;
            }
            GatewayControlAction::SetFallbackModel { model } => {
                self.fallback_model = normalize_string(model);
            }
            GatewayControlAction::ClearFallbackModel => {
                self.fallback_model = None;
            }
            GatewayControlAction::SetStreamingEnabled { enabled } => {
                self.streaming_enabled = enabled;
            }
            GatewayControlAction::SetClaudeRewritePolicy {
                enabled,
                default_model,
                haiku_model,
                sonnet_model,
                opus_model,
                reasoning_model,
            } => {
                self.claude_model_rewrite.enabled = enabled;
                self.claude_model_rewrite.default_model = normalize_optional(default_model);
                self.claude_model_rewrite.haiku_model = normalize_optional(haiku_model);
                self.claude_model_rewrite.sonnet_model = normalize_optional(sonnet_model);
                self.claude_model_rewrite.opus_model = normalize_optional(opus_model);
                self.claude_model_rewrite.reasoning_model = normalize_optional(reasoning_model);
            }
            GatewayControlAction::ClearClaudeRewritePolicy => {
                self.claude_model_rewrite = ClaudeModelRewritePolicy {
                    enabled: false,
                    default_model: None,
                    haiku_model: None,
                    sonnet_model: None,
                    opus_model: None,
                    reasoning_model: None,
                };
            }
            GatewayControlAction::SetProviderKeyPolicy {
                provider,
                env_key,
                override_env_key,
                precedence,
            } => {
                let provider = normalize_provider_id(provider)?;
                let env_key = normalize_env_key_name(env_key)?;
                let override_env_key = normalize_env_key_name(override_env_key)?;
                if env_key.is_none() && override_env_key.is_none() {
                    return Err(
                        "Provider key policy requires env_key or override_env_key".to_string()
                    );
                }

                self.provider_key_policies.insert(
                    provider.clone(),
                    ProviderKeyPolicy {
                        provider,
                        env_key,
                        override_env_key,
                        precedence,
                    },
                );
            }
            GatewayControlAction::ClearProviderKeyPolicy { provider } => {
                let provider = normalize_provider_id(provider)?;
                self.provider_key_policies.remove(&provider);
            }
            GatewayControlAction::ImportCcSwitchKeysAdditive { path } => {
                self.import_cc_switch_keys_additive(path)?;
            }
            GatewayControlAction::Reset => {
                *self = Self::from_config(config_defaults());
            }
        }

        Ok(())
    }

    pub fn effective_default_model(&self) -> Option<&str> {
        self.default_model.as_deref()
    }

    pub fn effective_fallback_model(&self) -> Option<&str> {
        self.fallback_model.as_deref()
    }

    pub fn preferred_provider_for_model(&self, model: &str) -> Option<String> {
        if model.contains('/') {
            None
        } else {
            self.preferred_provider.clone()
        }
    }

    pub fn rewrite_model(&self, model: &str, request: &Value) -> Option<String> {
        let policy = &self.claude_model_rewrite;
        if !policy.enabled || !is_claude_like_model(model) {
            return None;
        }

        if has_thinking_enabled(request) {
            if let Some(reasoning) = policy.reasoning_model.as_ref() {
                return Some(reasoning.clone());
            }
        }

        let lower = model.to_ascii_lowercase();
        if lower.contains("haiku") {
            if let Some(model) = policy.haiku_model.as_ref() {
                return Some(model.clone());
            }
        }
        if lower.contains("opus") {
            if let Some(model) = policy.opus_model.as_ref() {
                return Some(model.clone());
            }
        }
        if lower.contains("sonnet") {
            if let Some(model) = policy.sonnet_model.as_ref() {
                return Some(model.clone());
            }
        }

        let mut mapped = policy.default_model.clone()?;
        if has_tools(request) && should_fallback_to_default_for_tool_use(&mapped) {
            if let Some(default_model) = policy.default_model.as_ref() {
                mapped = default_model.clone();
            }
        }
        Some(mapped)
    }

    fn snapshot_keymux_state(&self, providers: &[GatewayProviderStatus]) -> GatewayKeymuxState {
        let mut merged = BTreeMap::new();
        for provider in providers {
            merged.insert(
                provider.name.to_ascii_lowercase(),
                ProviderKeyPolicy {
                    provider: provider.name.to_ascii_lowercase(),
                    env_key: normalize_env_key_name(Some(provider.key_env.clone())).unwrap_or(None),
                    override_env_key: None,
                    precedence: ProviderKeyPrecedence::EnvironmentFirst,
                },
            );
        }

        for (provider, policy) in &self.provider_key_policies {
            merged.insert(provider.to_ascii_lowercase(), policy.clone());
        }

        let resolver = DefaultProviderKeyPolicyResolver;
        let provider_keys = merged
            .into_values()
            .map(|policy| resolver.resolve_policy(&policy))
            .collect();

        GatewayKeymuxState {
            strategy: resolver.strategy_name().to_string(),
            provider_keys,
        }
    }

    fn import_cc_switch_keys_additive(&mut self, path: Option<String>) -> Result<usize, String> {
        let source_path = resolve_cc_switch_config_path(path)?;
        let source = std::fs::read_to_string(&source_path)
            .map_err(|e| format!("failed to read {}: {}", source_path.display(), e))?;
        let parsed: Value =
            serde_json::from_str(&source).map_err(|e| format!("invalid cc-switch JSON: {}", e))?;
        let records = extract_cc_switch_provider_env_records(&parsed);

        let mut imported_count = 0usize;
        for record in records {
            let mut first_key: Option<String> = None;
            for (env_key, env_value) in &record.env_pairs {
                if env_value.trim().is_empty() {
                    continue;
                }
                // Additive import: inject cc-switch key material into runtime env.
                unsafe {
                    std::env::set_var(env_key, env_value);
                }
                if first_key.is_none() {
                    first_key = Some(env_key.clone());
                }
                imported_count += 1;
            }

            let Some(env_key) = first_key else {
                continue;
            };

            self.provider_key_policies
                .entry(record.provider.clone())
                .and_modify(|policy| {
                    if policy.env_key.is_none() {
                        policy.env_key = Some(env_key.clone());
                    }
                })
                .or_insert(ProviderKeyPolicy {
                    provider: record.provider,
                    env_key: Some(env_key),
                    override_env_key: None,
                    precedence: ProviderKeyPrecedence::EnvironmentFirst,
                });
        }

        Ok(imported_count)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum GatewayControlAction {
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
    SetStreamingEnabled {
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
    Reset,
}

fn config_defaults() -> &'static ProxyConfig {
    static DEFAULTS: std::sync::OnceLock<ProxyConfig> = std::sync::OnceLock::new();
    DEFAULTS.get_or_init(ProxyConfig::default)
}

fn normalize_string(input: String) -> Option<String> {
    normalize_optional(Some(input))
}

fn normalize_optional(input: Option<String>) -> Option<String> {
    input.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_provider_id(provider: String) -> Result<String, String> {
    let provider = provider.trim().to_ascii_lowercase();
    if provider.is_empty() {
        Err("provider must not be empty".to_string())
    } else {
        Ok(provider)
    }
}

fn normalize_env_key_name(input: Option<String>) -> Result<Option<String>, String> {
    let Some(value) = normalize_optional(input) else {
        return Ok(None);
    };

    if !value
        .chars()
        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
    {
        return Err(format!("invalid env key '{}': expected [A-Z0-9_]+", value));
    }

    Ok(Some(value))
}

fn env_string(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .and_then(|v| normalize_optional(Some(v)))
}

fn env_string_any(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| env_string(key))
}

fn bool_env(key: &str) -> Option<bool> {
    std::env::var(key).ok().and_then(|v| {
        let lower = v.trim().to_ascii_lowercase();
        match lower.as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    })
}

fn env_key_present_and_nonempty(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn infer_provider_family(provider: &str, base_url: &str) -> GatewayFacadeFamily {
    let provider_lower = provider.to_ascii_lowercase();
    let base_lower = base_url.to_ascii_lowercase();

    if provider_lower.contains("anthropic") || provider_lower.contains("claude") {
        return GatewayFacadeFamily::AnthropicCompatible;
    }
    if provider_lower.contains("gemini")
        || provider_lower.contains("google")
        || base_lower.contains("generativelanguage.googleapis.com")
    {
        return GatewayFacadeFamily::GeminiNative;
    }
    if provider_lower.contains("ollama")
        || provider_lower.contains("lmstudio")
        || base_lower.contains("localhost:11434")
    {
        return GatewayFacadeFamily::OllamaCompatible;
    }
    if !base_lower.is_empty() {
        return GatewayFacadeFamily::OpenAiCompatible;
    }

    GatewayFacadeFamily::Unknown
}

fn is_claude_like_model(model: &str) -> bool {
    let normalized = model.trim().to_ascii_lowercase();
    normalized.starts_with("claude-") || normalized.starts_with("anthropic/claude-")
}

fn has_tools(request: &Value) -> bool {
    request
        .get("tools")
        .and_then(|v| v.as_array())
        .map(|arr| !arr.is_empty())
        .unwrap_or(false)
}

fn should_fallback_to_default_for_tool_use(mapped_model: &str) -> bool {
    let normalized = mapped_model.trim().to_ascii_lowercase();
    !normalized.is_empty() && !is_claude_like_model(&normalized) && normalized.ends_with(":free")
}

fn has_thinking_enabled(request: &Value) -> bool {
    matches!(
        request
            .get("thinking")
            .and_then(|v| v.as_object())
            .and_then(|o| o.get("type"))
            .and_then(|t| t.as_str()),
        Some("enabled") | Some("adaptive")
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CcSwitchProviderEnvRecord {
    provider: String,
    env_pairs: Vec<(String, String)>,
}

fn resolve_cc_switch_config_path(path: Option<String>) -> Result<PathBuf, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = normalize_optional(path) {
        candidates.push(PathBuf::from(path));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".cc-switch/config.json"));
        candidates.push(home.join(".cc-switch/config.json.migrated"));
        candidates.push(home.join(".cc-switch/config.json.bak"));
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| "no cc-switch config file found".to_string())
}

fn extract_cc_switch_provider_env_records(root: &Value) -> Vec<CcSwitchProviderEnvRecord> {
    let mut records = Vec::new();
    let Some(apps) = root.as_object() else {
        return records;
    };

    for app_value in apps.values() {
        let Some(providers) = app_value.get("providers").and_then(Value::as_object) else {
            continue;
        };

        for (provider_id, provider_value) in providers {
            let name = provider_value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(provider_id);
            let provider = slugify_provider_name(name);
            if provider.is_empty() {
                continue;
            }

            let Some(env) = provider_value
                .get("settingsConfig")
                .and_then(|v| v.get("env"))
                .and_then(Value::as_object)
            else {
                continue;
            };

            let mut env_pairs = Vec::new();
            for (key, value) in env {
                if !looks_like_key_material_name(key) {
                    continue;
                }
                if let Some(secret) = value.as_str() {
                    let key = key.trim().to_ascii_uppercase();
                    env_pairs.push((key, secret.to_string()));
                }
            }

            if !env_pairs.is_empty() {
                records.push(CcSwitchProviderEnvRecord {
                    provider,
                    env_pairs,
                });
            }
        }
    }

    records
}

fn slugify_provider_name(name: &str) -> String {
    let mut out = String::new();
    let mut prev_is_sep = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_is_sep = false;
        } else if !prev_is_sep {
            out.push('_');
            prev_is_sep = true;
        }
    }
    out.trim_matches('_').to_string()
}

fn looks_like_key_material_name(key: &str) -> bool {
    let key = key.trim().to_ascii_uppercase();
    key.ends_with("_API_KEY") || key.ends_with("_AUTH_TOKEN")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn config() -> ProxyConfig {
        ProxyConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8888,
            enable_streaming: true,
            enable_caching: true,
            default_model: Some("openai/gpt-4o-mini".to_string()),
            fallback_model: None,
            request_timeout_secs: 30,
            max_retries: 2,
        }
    }

    #[test]
    fn claude_rewrite_policy_maps_sonnet() {
        let mut control = GatewayRuntimeControl::from_config(&config());
        control
            .apply_action(GatewayControlAction::SetClaudeRewritePolicy {
                enabled: true,
                default_model: Some("anthropic/claude-sonnet-4.5".to_string()),
                haiku_model: None,
                sonnet_model: Some("ordinal/sonnet".to_string()),
                opus_model: None,
                reasoning_model: Some("ordinal/reasoning".to_string()),
            })
            .unwrap();

        let mapped = control.rewrite_model("claude-sonnet-4-5", &json!({}));
        assert_eq!(mapped.as_deref(), Some("ordinal/sonnet"));
    }

    #[test]
    fn claude_rewrite_policy_prefers_reasoning_model() {
        let mut control = GatewayRuntimeControl::from_config(&config());
        control
            .apply_action(GatewayControlAction::SetClaudeRewritePolicy {
                enabled: true,
                default_model: Some("ordinal/default".to_string()),
                haiku_model: None,
                sonnet_model: None,
                opus_model: None,
                reasoning_model: Some("ordinal/reasoning".to_string()),
            })
            .unwrap();

        let mapped =
            control.rewrite_model("claude-sonnet-4-5", &json!({"thinking":{"type":"enabled"}}));
        assert_eq!(mapped.as_deref(), Some("ordinal/reasoning"));
    }

    #[test]
    fn control_action_accepts_canonical_rewrite_action() {
        let action: GatewayControlAction = serde_json::from_value(json!({
            "action": "set_claude_rewrite_policy",
            "enabled": true,
            "default_model": "ordinal/default",
            "sonnet_model": "ordinal/sonnet"
        }))
        .unwrap();

        assert_eq!(
            action,
            GatewayControlAction::SetClaudeRewritePolicy {
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
    fn control_action_accepts_canonical_clear_rewrite_action() {
        let action: GatewayControlAction =
            serde_json::from_value(json!({ "action": "clear_claude_rewrite_policy" })).unwrap();

        assert_eq!(action, GatewayControlAction::ClearClaudeRewritePolicy);
    }

    #[test]
    fn provider_key_policy_action_sets_precedence_state() {
        let mut control = GatewayRuntimeControl::from_config(&config());
        control
            .apply_action(GatewayControlAction::SetProviderKeyPolicy {
                provider: "openai".to_string(),
                env_key: Some("OPENAI_API_KEY".to_string()),
                override_env_key: Some("OPENAI_API_KEY_OVERRIDE".to_string()),
                precedence: ProviderKeyPrecedence::OverrideFirst,
            })
            .unwrap();

        let state = control.snapshot(&config());
        let openai = state
            .keymux
            .provider_keys
            .iter()
            .find(|entry| entry.provider == "openai")
            .expect("openai key policy present");
        assert_eq!(openai.precedence, ProviderKeyPrecedence::OverrideFirst);
        assert_eq!(
            openai.override_env_key.as_deref(),
            Some("OPENAI_API_KEY_OVERRIDE")
        );
    }

    #[test]
    fn provider_key_policy_rejects_invalid_env_name() {
        let mut control = GatewayRuntimeControl::from_config(&config());
        let result = control.apply_action(GatewayControlAction::SetProviderKeyPolicy {
            provider: "openai".to_string(),
            env_key: Some("openai_api_key".to_string()),
            override_env_key: None,
            precedence: ProviderKeyPrecedence::EnvironmentFirst,
        });
        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap_or_default()
            .contains("expected [A-Z0-9_]+"));
    }

    #[test]
    fn extract_cc_switch_provider_env_records_reads_provider_env_keys() {
        let value = json!({
            "version": 2,
            "claude": {
                "providers": {
                    "p1": {
                        "name": "Kimi Code US",
                        "settingsConfig": {
                            "env": {
                                "ANTHROPIC_AUTH_TOKEN": "sk-1",
                                "ANTHROPIC_BASE_URL": "https://api.kimi.com/coding"
                            }
                        }
                    }
                }
            }
        });

        let records = extract_cc_switch_provider_env_records(&value);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].provider, "kimi_code_us");
        assert_eq!(records[0].env_pairs.len(), 1);
        assert_eq!(records[0].env_pairs[0].0, "ANTHROPIC_AUTH_TOKEN");
    }

    #[test]
    fn import_cc_switch_keys_additive_populates_policy_map() {
        let temp = tempfile::NamedTempFile::new().expect("tmp file");
        std::fs::write(
            temp.path(),
            r#"{
                "version": 2,
                "codex": {
                    "providers": {
                        "p2": {
                            "name": "OpenAI Main",
                            "settingsConfig": {
                                "env": {
                                    "OPENAI_API_KEY": "sk-test-imported"
                                }
                            }
                        }
                    }
                }
            }"#,
        )
        .expect("write cc-switch sample");

        let mut control = GatewayRuntimeControl::from_config(&config());
        let imported = control
            .import_cc_switch_keys_additive(Some(temp.path().display().to_string()))
            .expect("import keys");
        assert_eq!(imported, 1);
        assert!(control.provider_key_policies.contains_key("openai_main"));
    }
}
