use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderFamily {
    OpenAiCompatible,
    AnthropicCompatible,
    GeminiNative,
    AzureOpenAi,
    OpenRouter,
    OpenCodeZen,
    Ollama,
    ControlPlane,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServingTemplateId {
    OpenAiV1,
    OpenAiResponses,
    OpenAiHarmony,
    OpenAiGeminiCompat,
    ClaudeMessages,
    ClaudeCodeCompat,
    GeminiNative,
    AzureOpenAiCompat,
    OpenRouterCompat,
    OpenCodeZenCompat,
    Trait4,
    OpenApi3Commands,
    KeyVaultOps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelApiAction {
    ModelsList,
    ModelsGet,
    ChatCompletions,
    Responses,
    Embeddings,
    Images,
    AudioTranscriptions,
    AudioSpeech,
    Moderations,
    AnthropicMessages,
    AnthropicCountTokens,
    GeminiGenerateContent,
    GeminiStreamGenerateContent,
    GeminiCountTokens,
    OpenApiDiscovery,
    CommandInvoke,
    KeyVaultRead,
    KeyVaultWrite,
    KeyVaultList,
    Health,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MuxSurface {
    Native,
    ChatCompletions,
    Responses,
    HarmonyResponses,
    ClaudeMessages,
    GeminiGenerateContent,
    OpenApi3Commands,
    KeyVaultOps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthTemplate {
    BearerAuthorization,
    ApiKeyHeader,
    AnthropicXApiKey,
    GoogleApiKeyHeader,
    GoogleApiKeyQuery,
    AzureApiKeyHeader,
    NoneOrSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaDimension {
    RequestsPerMinute,
    TokensPerMinute,
    TokensPerDay,
    ConcurrentRequests,
    ContextWindowTokens,
    MaxOutputTokens,
    ReasoningBudgetTokens,
    ToolCallBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThinkingHint {
    None,
    ClaudeThinkingBlocks,
    OpenAiReasoningEffort,
    OpenAiResponsesReasoning,
    GeminiThinkingBudget,
    ProviderSpecific,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheArtifact {
    ModelSheets,
    ContextHints,
    QuotaHints,
    ThinkingHints,
    ApiTemplateHints,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelServingTaxonomyEntry {
    pub template: ServingTemplateId,
    pub family: ProviderFamily,
    pub default_mux: MuxSurface,
    pub optional_muxes: Vec<MuxSurface>,
    pub supported_actions: Vec<ModelApiAction>,
    pub env_keys: Vec<String>,
    pub auth_templates: Vec<AuthTemplate>,
    pub quota_dimensions: Vec<QuotaDimension>,
    pub cache_artifacts: Vec<CacheArtifact>,
    pub thinking_hints: Vec<ThinkingHint>,
    pub dsel_axes: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProtocolDecode {
    pub method: String,
    pub path: String,
    pub host: Option<String>,
    pub family: ProviderFamily,
    pub template: ServingTemplateId,
    pub action: ModelApiAction,
    pub default_mux: MuxSurface,
    pub optional_muxes: Vec<MuxSurface>,
    pub auth_templates: Vec<AuthTemplate>,
    pub env_keys: Vec<String>,
    pub quota_dimensions: Vec<QuotaDimension>,
    pub cache_artifacts: Vec<CacheArtifact>,
    pub thinking_hints: Vec<ThinkingHint>,
    pub confidence: u8,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequestPrefix {
    pub method: String,
    pub path: String,
    pub version: String,
    pub headers: BTreeMap<String, String>,
    pub body_snippet: String,
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_string()).collect()
}

fn all_cache_artifacts() -> Vec<CacheArtifact> {
    vec![
        CacheArtifact::ModelSheets,
        CacheArtifact::ContextHints,
        CacheArtifact::QuotaHints,
        CacheArtifact::ThinkingHints,
        CacheArtifact::ApiTemplateHints,
    ]
}

fn common_dsel_axes() -> Vec<String> {
    strings(&[
        "provider",
        "model",
        "action",
        "mux-surface",
        "auth-template",
        "quota-policy",
        "thinking-policy",
        "cache-policy",
    ])
}

pub fn comprehensive_model_serving_taxonomy() -> Vec<ModelServingTaxonomyEntry> {
    vec![
        ModelServingTaxonomyEntry {
            template: ServingTemplateId::OpenAiV1,
            family: ProviderFamily::OpenAiCompatible,
            default_mux: MuxSurface::ChatCompletions,
            optional_muxes: vec![MuxSurface::Responses, MuxSurface::HarmonyResponses],
            supported_actions: vec![
                ModelApiAction::ModelsList,
                ModelApiAction::ChatCompletions,
                ModelApiAction::Embeddings,
                ModelApiAction::Images,
                ModelApiAction::AudioTranscriptions,
                ModelApiAction::AudioSpeech,
                ModelApiAction::Moderations,
            ],
            env_keys: strings(&["OPENAI_API_KEY", "OPENAI_BASE_URL", "OPENAI_MODEL"]),
            auth_templates: vec![AuthTemplate::BearerAuthorization],
            quota_dimensions: vec![
                QuotaDimension::RequestsPerMinute,
                QuotaDimension::TokensPerMinute,
                QuotaDimension::ContextWindowTokens,
                QuotaDimension::MaxOutputTokens,
                QuotaDimension::ConcurrentRequests,
            ],
            cache_artifacts: all_cache_artifacts(),
            thinking_hints: vec![ThinkingHint::OpenAiReasoningEffort],
            dsel_axes: common_dsel_axes(),
            notes: "Baseline OpenAI-compatible /v1 surface; use DSEL or ENV recognition to select provider-specific templates.".to_string(),
        },
        ModelServingTaxonomyEntry {
            template: ServingTemplateId::OpenAiResponses,
            family: ProviderFamily::OpenAiCompatible,
            default_mux: MuxSurface::Responses,
            optional_muxes: vec![MuxSurface::ChatCompletions, MuxSurface::HarmonyResponses],
            supported_actions: vec![
                ModelApiAction::Responses,
                ModelApiAction::ModelsList,
                ModelApiAction::Health,
            ],
            env_keys: strings(&["OPENAI_API_KEY", "OPENAI_BASE_URL", "OPENAI_MODEL"]),
            auth_templates: vec![AuthTemplate::BearerAuthorization],
            quota_dimensions: vec![
                QuotaDimension::RequestsPerMinute,
                QuotaDimension::TokensPerMinute,
                QuotaDimension::ReasoningBudgetTokens,
                QuotaDimension::ContextWindowTokens,
                QuotaDimension::MaxOutputTokens,
            ],
            cache_artifacts: all_cache_artifacts(),
            thinking_hints: vec![ThinkingHint::OpenAiResponsesReasoning, ThinkingHint::OpenAiReasoningEffort],
            dsel_axes: common_dsel_axes(),
            notes: "Responses API is orthogonal to provider selection and should be mux-selectable from compatible templates.".to_string(),
        },
        ModelServingTaxonomyEntry {
            template: ServingTemplateId::OpenAiHarmony,
            family: ProviderFamily::OpenAiCompatible,
            default_mux: MuxSurface::HarmonyResponses,
            optional_muxes: vec![MuxSurface::Responses, MuxSurface::ChatCompletions],
            supported_actions: vec![ModelApiAction::Responses, ModelApiAction::ModelsList],
            env_keys: strings(&["OPENAI_API_KEY", "OPENAI_BASE_URL", "OPENAI_MODEL"]),
            auth_templates: vec![AuthTemplate::BearerAuthorization],
            quota_dimensions: vec![
                QuotaDimension::TokensPerMinute,
                QuotaDimension::ReasoningBudgetTokens,
                QuotaDimension::ContextWindowTokens,
            ],
            cache_artifacts: all_cache_artifacts(),
            thinking_hints: vec![ThinkingHint::OpenAiResponsesReasoning, ThinkingHint::ProviderSpecific],
            dsel_axes: common_dsel_axes(),
            notes: "Harmony-style responses template (e.g. NVIDIA-compatible responses semantics) with response-first mux default.".to_string(),
        },
        ModelServingTaxonomyEntry {
            template: ServingTemplateId::OpenAiGeminiCompat,
            family: ProviderFamily::OpenAiCompatible,
            default_mux: MuxSurface::ChatCompletions,
            optional_muxes: vec![MuxSurface::GeminiGenerateContent, MuxSurface::Responses],
            supported_actions: vec![
                ModelApiAction::ChatCompletions,
                ModelApiAction::ModelsList,
                ModelApiAction::Responses,
            ],
            env_keys: strings(&["GEMINI_API_KEY", "OPENAI_API_KEY", "OPENAI_BASE_URL", "OPENAI_MODEL"]),
            auth_templates: vec![AuthTemplate::GoogleApiKeyHeader, AuthTemplate::BearerAuthorization],
            quota_dimensions: vec![
                QuotaDimension::RequestsPerMinute,
                QuotaDimension::TokensPerMinute,
                QuotaDimension::ContextWindowTokens,
                QuotaDimension::MaxOutputTokens,
            ],
            cache_artifacts: all_cache_artifacts(),
            thinking_hints: vec![ThinkingHint::GeminiThinkingBudget],
            dsel_axes: common_dsel_axes(),
            notes: "OpenAI-compatible shim over Gemini-like providers; keep mux selection orthogonal so native Gemini can remain optional.".to_string(),
        },
        ModelServingTaxonomyEntry {
            template: ServingTemplateId::ClaudeMessages,
            family: ProviderFamily::AnthropicCompatible,
            default_mux: MuxSurface::ClaudeMessages,
            optional_muxes: vec![MuxSurface::ChatCompletions, MuxSurface::Responses],
            supported_actions: vec![
                ModelApiAction::AnthropicMessages,
                ModelApiAction::AnthropicCountTokens,
                ModelApiAction::ModelsList,
            ],
            env_keys: strings(&["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL", "ANTHROPIC_MODEL"]),
            auth_templates: vec![AuthTemplate::AnthropicXApiKey, AuthTemplate::BearerAuthorization],
            quota_dimensions: vec![
                QuotaDimension::RequestsPerMinute,
                QuotaDimension::TokensPerMinute,
                QuotaDimension::ContextWindowTokens,
                QuotaDimension::MaxOutputTokens,
                QuotaDimension::ToolCallBudget,
            ],
            cache_artifacts: all_cache_artifacts(),
            thinking_hints: vec![ThinkingHint::ClaudeThinkingBlocks],
            dsel_axes: common_dsel_axes(),
            notes: "Anthropic Messages-compatible surface with optional transforms to OpenAI chat/responses for compatible backends.".to_string(),
        },
        ModelServingTaxonomyEntry {
            template: ServingTemplateId::ClaudeCodeCompat,
            family: ProviderFamily::AnthropicCompatible,
            default_mux: MuxSurface::ClaudeMessages,
            optional_muxes: vec![MuxSurface::ChatCompletions, MuxSurface::Responses],
            supported_actions: vec![
                ModelApiAction::AnthropicMessages,
                ModelApiAction::AnthropicCountTokens,
            ],
            env_keys: strings(&[
                "ANTHROPIC_AUTH_TOKEN",
                "ANTHROPIC_BASE_URL",
                "ANTHROPIC_MODEL",
                "ANTHROPIC_REASONING_MODEL",
            ]),
            auth_templates: vec![AuthTemplate::AnthropicXApiKey, AuthTemplate::BearerAuthorization],
            quota_dimensions: vec![
                QuotaDimension::RequestsPerMinute,
                QuotaDimension::TokensPerMinute,
                QuotaDimension::ReasoningBudgetTokens,
                QuotaDimension::ContextWindowTokens,
                QuotaDimension::MaxOutputTokens,
            ],
            cache_artifacts: all_cache_artifacts(),
            thinking_hints: vec![ThinkingHint::ClaudeThinkingBlocks, ThinkingHint::ProviderSpecific],
            dsel_axes: common_dsel_axes(),
            notes: "Claude Code-specific semantics layered on Anthropic-compatible paths, including count_tokens and beta headers.".to_string(),
        },
        ModelServingTaxonomyEntry {
            template: ServingTemplateId::GeminiNative,
            family: ProviderFamily::GeminiNative,
            default_mux: MuxSurface::GeminiGenerateContent,
            optional_muxes: vec![MuxSurface::ChatCompletions, MuxSurface::Responses],
            supported_actions: vec![
                ModelApiAction::GeminiGenerateContent,
                ModelApiAction::GeminiStreamGenerateContent,
                ModelApiAction::GeminiCountTokens,
                ModelApiAction::ModelsList,
            ],
            env_keys: strings(&["GEMINI_API_KEY", "GOOGLE_API_KEY", "GOOGLE_GEMINI_BASE_URL"]),
            auth_templates: vec![AuthTemplate::GoogleApiKeyHeader, AuthTemplate::GoogleApiKeyQuery],
            quota_dimensions: vec![
                QuotaDimension::RequestsPerMinute,
                QuotaDimension::TokensPerMinute,
                QuotaDimension::ContextWindowTokens,
                QuotaDimension::MaxOutputTokens,
                QuotaDimension::ConcurrentRequests,
            ],
            cache_artifacts: all_cache_artifacts(),
            thinking_hints: vec![ThinkingHint::GeminiThinkingBudget],
            dsel_axes: common_dsel_axes(),
            notes: "Native Gemini generateContent/countTokens surface; may be muxed into OpenAI-compatible surfaces by policy.".to_string(),
        },
        ModelServingTaxonomyEntry {
            template: ServingTemplateId::AzureOpenAiCompat,
            family: ProviderFamily::AzureOpenAi,
            default_mux: MuxSurface::ChatCompletions,
            optional_muxes: vec![MuxSurface::Responses],
            supported_actions: vec![
                ModelApiAction::ChatCompletions,
                ModelApiAction::Responses,
                ModelApiAction::Embeddings,
                ModelApiAction::Images,
            ],
            env_keys: strings(&[
                "AZURE_OPENAI_API_KEY",
                "AZURE_OPENAI_ENDPOINT",
                "AZURE_OPENAI_DEPLOYMENT",
            ]),
            auth_templates: vec![AuthTemplate::AzureApiKeyHeader, AuthTemplate::BearerAuthorization],
            quota_dimensions: vec![
                QuotaDimension::RequestsPerMinute,
                QuotaDimension::TokensPerMinute,
                QuotaDimension::ConcurrentRequests,
                QuotaDimension::ContextWindowTokens,
            ],
            cache_artifacts: all_cache_artifacts(),
            thinking_hints: vec![ThinkingHint::OpenAiReasoningEffort],
            dsel_axes: common_dsel_axes(),
            notes: "Azure-hosted OpenAI-compatible deployments; deployment path semantics should be part of DSEL mapping.".to_string(),
        },
        ModelServingTaxonomyEntry {
            template: ServingTemplateId::OpenRouterCompat,
            family: ProviderFamily::OpenRouter,
            default_mux: MuxSurface::ChatCompletions,
            optional_muxes: vec![MuxSurface::Responses, MuxSurface::ClaudeMessages],
            supported_actions: vec![
                ModelApiAction::ChatCompletions,
                ModelApiAction::Responses,
                ModelApiAction::ModelsList,
            ],
            env_keys: strings(&["OPENROUTER_API_KEY", "OPENAI_API_KEY", "OPENAI_BASE_URL", "OPENAI_MODEL"]),
            auth_templates: vec![AuthTemplate::BearerAuthorization],
            quota_dimensions: vec![
                QuotaDimension::RequestsPerMinute,
                QuotaDimension::TokensPerMinute,
                QuotaDimension::ContextWindowTokens,
                QuotaDimension::MaxOutputTokens,
            ],
            cache_artifacts: all_cache_artifacts(),
            thinking_hints: vec![ThinkingHint::OpenAiReasoningEffort, ThinkingHint::ProviderSpecific],
            dsel_axes: common_dsel_axes(),
            notes: "Aggregator surface with broad model namespace; template should preserve provider/model routing metadata.".to_string(),
        },
        ModelServingTaxonomyEntry {
            template: ServingTemplateId::OpenCodeZenCompat,
            family: ProviderFamily::OpenCodeZen,
            default_mux: MuxSurface::ChatCompletions,
            optional_muxes: vec![MuxSurface::ClaudeMessages, MuxSurface::Responses],
            supported_actions: vec![
                ModelApiAction::ChatCompletions,
                ModelApiAction::Responses,
                ModelApiAction::AnthropicMessages,
                ModelApiAction::ModelsList,
            ],
            env_keys: strings(&[
                "OPENAI_API_KEY",
                "OPENAI_BASE_URL",
                "ANTHROPIC_AUTH_TOKEN",
                "ANTHROPIC_BASE_URL",
                "ANTHROPIC_MODEL",
            ]),
            auth_templates: vec![AuthTemplate::BearerAuthorization, AuthTemplate::AnthropicXApiKey],
            quota_dimensions: vec![
                QuotaDimension::RequestsPerMinute,
                QuotaDimension::TokensPerMinute,
                QuotaDimension::ContextWindowTokens,
                QuotaDimension::MaxOutputTokens,
                QuotaDimension::ReasoningBudgetTokens,
            ],
            cache_artifacts: all_cache_artifacts(),
            thinking_hints: vec![
                ThinkingHint::ClaudeThinkingBlocks,
                ThinkingHint::OpenAiReasoningEffort,
                ThinkingHint::ProviderSpecific,
            ],
            dsel_axes: common_dsel_axes(),
            notes: "OpenCode Zen mixed-endpoint compatibility template; mux is selected orthogonally per provider/model capability.".to_string(),
        },
        ModelServingTaxonomyEntry {
            template: ServingTemplateId::Trait4,
            family: ProviderFamily::Unknown,
            default_mux: MuxSurface::Native,
            optional_muxes: vec![
                MuxSurface::ChatCompletions,
                MuxSurface::Responses,
                MuxSurface::ClaudeMessages,
                MuxSurface::GeminiGenerateContent,
            ],
            supported_actions: vec![
                ModelApiAction::CommandInvoke,
                ModelApiAction::Unknown,
            ],
            env_keys: strings(&[
                "MODEL_PROVIDER",
                "MODEL_NAME",
                "MODEL_API_TEMPLATE",
                "MODEL_MUX_POLICY",
                "MODEL_QUOTA_PROFILE",
            ]),
            auth_templates: vec![
                AuthTemplate::BearerAuthorization,
                AuthTemplate::ApiKeyHeader,
                AuthTemplate::AnthropicXApiKey,
                AuthTemplate::GoogleApiKeyHeader,
            ],
            quota_dimensions: vec![
                QuotaDimension::RequestsPerMinute,
                QuotaDimension::TokensPerMinute,
                QuotaDimension::ContextWindowTokens,
                QuotaDimension::MaxOutputTokens,
                QuotaDimension::ReasoningBudgetTokens,
                QuotaDimension::ConcurrentRequests,
            ],
            cache_artifacts: all_cache_artifacts(),
            thinking_hints: vec![
                ThinkingHint::ClaudeThinkingBlocks,
                ThinkingHint::OpenAiReasoningEffort,
                ThinkingHint::OpenAiResponsesReasoning,
                ThinkingHint::GeminiThinkingBudget,
                ThinkingHint::ProviderSpecific,
            ],
            dsel_axes: strings(&[
                "provider",
                "model",
                "action",
                "template-id",
                "mux-surface",
                "quota-profile",
                "thinking-profile",
                "context-profile",
                "cache-policy",
            ]),
            notes: "Manual DSEL override template for orthogonal trait mapping across provider/model/action/mux surfaces.".to_string(),
        },
        ModelServingTaxonomyEntry {
            template: ServingTemplateId::OpenApi3Commands,
            family: ProviderFamily::ControlPlane,
            default_mux: MuxSurface::OpenApi3Commands,
            optional_muxes: vec![],
            supported_actions: vec![ModelApiAction::OpenApiDiscovery, ModelApiAction::CommandInvoke],
            env_keys: strings(&["CC_SWITCH_OPENAPI_BIND", "CC_SWITCH_CONTROL_TOKEN"]),
            auth_templates: vec![AuthTemplate::BearerAuthorization, AuthTemplate::NoneOrSession],
            quota_dimensions: vec![QuotaDimension::ConcurrentRequests],
            cache_artifacts: vec![CacheArtifact::ApiTemplateHints],
            thinking_hints: vec![ThinkingHint::None],
            dsel_axes: common_dsel_axes(),
            notes: "Self-contained OpenAPI3 command surface replacing client-specific CLI dependencies.".to_string(),
        },
        ModelServingTaxonomyEntry {
            template: ServingTemplateId::KeyVaultOps,
            family: ProviderFamily::ControlPlane,
            default_mux: MuxSurface::KeyVaultOps,
            optional_muxes: vec![],
            supported_actions: vec![
                ModelApiAction::KeyVaultList,
                ModelApiAction::KeyVaultRead,
                ModelApiAction::KeyVaultWrite,
            ],
            env_keys: strings(&["KEYMUX_URL", "KEYVAULT_URL", "CC_SWITCH_CONTROL_TOKEN"]),
            auth_templates: vec![AuthTemplate::BearerAuthorization, AuthTemplate::NoneOrSession],
            quota_dimensions: vec![QuotaDimension::ConcurrentRequests],
            cache_artifacts: vec![CacheArtifact::ApiTemplateHints],
            thinking_hints: vec![ThinkingHint::None],
            dsel_axes: common_dsel_axes(),
            notes: "Control-plane key vault / keymux operations exposed on the same single-provider proxy surface.".to_string(),
        },
    ]
}

fn find_header_end(data: &[u8]) -> Option<(usize, usize)> {
    for (i, win) in data.windows(4).enumerate() {
        if win == b"\r\n\r\n" {
            return Some((i, 4));
        }
    }
    for (i, win) in data.windows(2).enumerate() {
        if win == b"\n\n" {
            return Some((i, 2));
        }
    }
    None
}

fn is_http_method(method: &str) -> bool {
    matches!(
        method,
        "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS" | "CONNECT"
    )
}

pub fn parse_http_request_prefix(data: &[u8]) -> Option<HttpRequestPrefix> {
    let max_len = data.len().min(8192);
    let data = &data[..max_len];
    let (header_end_idx, terminator_len) = find_header_end(data)?;
    let head = std::str::from_utf8(&data[..header_end_idx]).ok()?;
    let body_start = header_end_idx.saturating_add(terminator_len);
    let body_snippet = String::from_utf8_lossy(&data[body_start..data.len().min(body_start + 512)])
        .to_string();

    let mut lines = head.lines();
    let request_line = lines.next()?.trim_end_matches('\r');
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    if !is_http_method(&method) {
        return None;
    }
    let path = parts.next()?.to_string();
    let version = parts.next().unwrap_or("HTTP/1.1").to_string();

    let mut headers = BTreeMap::new();
    for line in lines {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    Some(HttpRequestPrefix {
        method,
        path,
        version,
        headers,
        body_snippet,
    })
}

fn family_from_host(host: Option<&str>) -> ProviderFamily {
    let host = host.unwrap_or_default().to_ascii_lowercase();
    if host.contains("api.anthropic.com") {
        ProviderFamily::AnthropicCompatible
    } else if host.contains("openrouter.ai") {
        ProviderFamily::OpenRouter
    } else if host.contains("opencode.ai") {
        ProviderFamily::OpenCodeZen
    } else if host.contains("generativelanguage.googleapis.com") || host.contains("googleapis.com") {
        ProviderFamily::GeminiNative
    } else if host.contains("openai.azure.com") || host.contains(".cognitiveservices.azure.com") {
        ProviderFamily::AzureOpenAi
    } else if host.contains("localhost:11434") || host.contains("127.0.0.1:11434") {
        ProviderFamily::Ollama
    } else if !host.is_empty() {
        ProviderFamily::OpenAiCompatible
    } else {
        ProviderFamily::Unknown
    }
}

fn best_template_for_request(req: &HttpRequestPrefix) -> (ServingTemplateId, ProviderFamily, ModelApiAction, u8, Vec<String>) {
    let path_lc = req.path.to_ascii_lowercase();
    let host = req.headers.get("host").cloned();
    let host_lc = host.clone().unwrap_or_default().to_ascii_lowercase();
    let mut notes = Vec::new();

    if path_lc.contains("/openapi.json") || path_lc.contains("/api-docs/openapi.json") {
        return (
            ServingTemplateId::OpenApi3Commands,
            ProviderFamily::ControlPlane,
            ModelApiAction::OpenApiDiscovery,
            95,
            vec!["openapi discovery path".to_string()],
        );
    }

    if path_lc.starts_with("/v1/keys")
        || path_lc.starts_with("/v1/secrets")
        || path_lc.starts_with("/vault/")
        || path_lc.contains("/keyvault/")
        || path_lc.contains("/keymux/")
    {
        let action = if req.method == "GET" {
            if path_lc.ends_with("/keys") || path_lc.ends_with("/secrets") {
                ModelApiAction::KeyVaultList
            } else {
                ModelApiAction::KeyVaultRead
            }
        } else {
            ModelApiAction::KeyVaultWrite
        };
        return (
            ServingTemplateId::KeyVaultOps,
            ProviderFamily::ControlPlane,
            action,
            90,
            vec!["key vault/keymux control path".to_string()],
        );
    }

    if path_lc.contains(":streamgeneratecontent") {
        return (
            ServingTemplateId::GeminiNative,
            ProviderFamily::GeminiNative,
            ModelApiAction::GeminiStreamGenerateContent,
            96,
            vec!["Gemini streamGenerateContent path".to_string()],
        );
    }
    if path_lc.contains(":generatecontent") {
        return (
            ServingTemplateId::GeminiNative,
            ProviderFamily::GeminiNative,
            ModelApiAction::GeminiGenerateContent,
            96,
            vec!["Gemini generateContent path".to_string()],
        );
    }
    if path_lc.contains(":counttokens") {
        return (
            ServingTemplateId::GeminiNative,
            ProviderFamily::GeminiNative,
            ModelApiAction::GeminiCountTokens,
            96,
            vec!["Gemini countTokens path".to_string()],
        );
    }

    if path_lc.contains("/v1/messages/count_tokens") {
        let is_claude_code = req
            .headers
            .get("anthropic-beta")
            .map(|v| v.to_ascii_lowercase().contains("claude-code"))
            .unwrap_or(false)
            || req
                .headers
                .get("user-agent")
                .map(|v| v.to_ascii_lowercase().contains("claude"))
                .unwrap_or(false);
        if is_claude_code {
            notes.push("anthropic-beta/user-agent suggests claude-code".to_string());
            return (
                ServingTemplateId::ClaudeCodeCompat,
                ProviderFamily::AnthropicCompatible,
                ModelApiAction::AnthropicCountTokens,
                95,
                notes,
            );
        }
        return (
            ServingTemplateId::ClaudeMessages,
            ProviderFamily::AnthropicCompatible,
            ModelApiAction::AnthropicCountTokens,
            90,
            vec!["Anthropic count_tokens path".to_string()],
        );
    }

    if path_lc.starts_with("/v1/messages") || path_lc.contains("/claude/v1/messages") {
        let is_claude_code = req
            .headers
            .get("anthropic-beta")
            .map(|v| v.to_ascii_lowercase().contains("claude-code"))
            .unwrap_or(false)
            || req
                .headers
                .get("user-agent")
                .map(|v| v.to_ascii_lowercase().contains("claude"))
                .unwrap_or(false);
        let template = if is_claude_code {
            notes.push("claude-code header/user-agent hint".to_string());
            ServingTemplateId::ClaudeCodeCompat
        } else {
            ServingTemplateId::ClaudeMessages
        };
        return (
            template,
            ProviderFamily::AnthropicCompatible,
            ModelApiAction::AnthropicMessages,
            if is_claude_code { 94 } else { 90 },
            notes,
        );
    }

    if path_lc == "/v1/models" || path_lc.ends_with("/v1/models") {
        let family = family_from_host(host.as_deref());
        let template = match family {
            ProviderFamily::OpenCodeZen => ServingTemplateId::OpenCodeZenCompat,
            ProviderFamily::OpenRouter => ServingTemplateId::OpenRouterCompat,
            ProviderFamily::AzureOpenAi => ServingTemplateId::AzureOpenAiCompat,
            ProviderFamily::GeminiNative => ServingTemplateId::GeminiNative,
            _ => ServingTemplateId::OpenAiV1,
        };
        return (
            template,
            family,
            ModelApiAction::ModelsList,
            88,
            vec!["v1/models path".to_string()],
        );
    }

    if path_lc.contains("/v1/chat/completions") || path_lc.ends_with("/chat/completions") {
        if host_lc.contains("opencode.ai") {
            return (
                ServingTemplateId::OpenCodeZenCompat,
                ProviderFamily::OpenCodeZen,
                ModelApiAction::ChatCompletions,
                93,
                vec!["OpenCode Zen host + chat/completions".to_string()],
            );
        }
        if host_lc.contains("openrouter.ai") {
            return (
                ServingTemplateId::OpenRouterCompat,
                ProviderFamily::OpenRouter,
                ModelApiAction::ChatCompletions,
                93,
                vec!["OpenRouter host + chat/completions".to_string()],
            );
        }
        if host_lc.contains("googleapis.com")
            || req.headers.contains_key("x-goog-api-key")
            || req.body_snippet.to_ascii_lowercase().contains("\"google\"")
        {
            return (
                ServingTemplateId::OpenAiGeminiCompat,
                ProviderFamily::OpenAiCompatible,
                ModelApiAction::ChatCompletions,
                86,
                vec!["OpenAI chat path with Gemini/google hints".to_string()],
            );
        }
        if host_lc.contains("openai.azure.com") || host_lc.contains(".cognitiveservices.azure.com") {
            return (
                ServingTemplateId::AzureOpenAiCompat,
                ProviderFamily::AzureOpenAi,
                ModelApiAction::ChatCompletions,
                94,
                vec!["Azure OpenAI host + chat/completions".to_string()],
            );
        }
        return (
            ServingTemplateId::OpenAiV1,
            ProviderFamily::OpenAiCompatible,
            ModelApiAction::ChatCompletions,
            90,
            vec!["OpenAI-compatible chat/completions path".to_string()],
        );
    }

    if path_lc.contains("/v1/responses") || path_lc.ends_with("/responses") {
        if host_lc.contains("integrate.api.nvidia.com")
            || host_lc.contains("nvidia")
            || path_lc.contains("harmony")
            || req
                .headers
                .get("x-openai-harmony")
                .is_some()
        {
            return (
                ServingTemplateId::OpenAiHarmony,
                ProviderFamily::OpenAiCompatible,
                ModelApiAction::Responses,
                92,
                vec!["Responses path with harmony/NVIDIA hint".to_string()],
            );
        }
        if host_lc.contains("opencode.ai") {
            return (
                ServingTemplateId::OpenCodeZenCompat,
                ProviderFamily::OpenCodeZen,
                ModelApiAction::Responses,
                90,
                vec!["OpenCode Zen host + responses".to_string()],
            );
        }
        return (
            ServingTemplateId::OpenAiResponses,
            ProviderFamily::OpenAiCompatible,
            ModelApiAction::Responses,
            92,
            vec!["OpenAI-compatible responses path".to_string()],
        );
    }

    if path_lc.contains("/v1/embeddings") {
        return (
            ServingTemplateId::OpenAiV1,
            ProviderFamily::OpenAiCompatible,
            ModelApiAction::Embeddings,
            89,
            vec!["embeddings path".to_string()],
        );
    }

    if path_lc.contains("/v1/images") {
        return (
            ServingTemplateId::OpenAiV1,
            ProviderFamily::OpenAiCompatible,
            ModelApiAction::Images,
            89,
            vec!["images path".to_string()],
        );
    }

    if path_lc.contains("/v1/audio/transcriptions") {
        return (
            ServingTemplateId::OpenAiV1,
            ProviderFamily::OpenAiCompatible,
            ModelApiAction::AudioTranscriptions,
            89,
            vec!["audio transcriptions path".to_string()],
        );
    }

    if path_lc.contains("/v1/audio/speech") {
        return (
            ServingTemplateId::OpenAiV1,
            ProviderFamily::OpenAiCompatible,
            ModelApiAction::AudioSpeech,
            89,
            vec!["audio speech path".to_string()],
        );
    }

    if path_lc.contains("/v1/moderations") {
        return (
            ServingTemplateId::OpenAiV1,
            ProviderFamily::OpenAiCompatible,
            ModelApiAction::Moderations,
            89,
            vec!["moderations path".to_string()],
        );
    }

    if req.headers.contains_key("x-dsel-traits") || path_lc.contains("/v1/trait4") {
        return (
            ServingTemplateId::Trait4,
            ProviderFamily::Unknown,
            ModelApiAction::CommandInvoke,
            80,
            vec!["manual DSEL/Trait4 hint".to_string()],
        );
    }

    let family = if req.headers.contains_key("anthropic-version") {
        ProviderFamily::AnthropicCompatible
    } else if req.headers.contains_key("x-goog-api-key") {
        ProviderFamily::GeminiNative
    } else if req.headers.contains_key("api-key")
        && (host_lc.contains("openai.azure.com") || host_lc.contains(".cognitiveservices.azure.com"))
    {
        ProviderFamily::AzureOpenAi
    } else {
        family_from_host(host.as_deref())
    };

    let template = match family {
        ProviderFamily::AnthropicCompatible => ServingTemplateId::ClaudeMessages,
        ProviderFamily::GeminiNative => ServingTemplateId::GeminiNative,
        ProviderFamily::AzureOpenAi => ServingTemplateId::AzureOpenAiCompat,
        ProviderFamily::OpenRouter => ServingTemplateId::OpenRouterCompat,
        ProviderFamily::OpenCodeZen => ServingTemplateId::OpenCodeZenCompat,
        ProviderFamily::ControlPlane => ServingTemplateId::OpenApi3Commands,
        _ => ServingTemplateId::Trait4,
    };

    (
        template,
        family,
        ModelApiAction::Unknown,
        45,
        vec!["fallback heuristic based on host/headers".to_string()],
    )
}

fn entry_by_template(template: ServingTemplateId) -> ModelServingTaxonomyEntry {
    comprehensive_model_serving_taxonomy()
        .into_iter()
        .find(|e| e.template == template)
        .unwrap_or_else(|| ModelServingTaxonomyEntry {
            template: ServingTemplateId::Trait4,
            family: ProviderFamily::Unknown,
            default_mux: MuxSurface::Native,
            optional_muxes: vec![],
            supported_actions: vec![ModelApiAction::Unknown],
            env_keys: vec![],
            auth_templates: vec![],
            quota_dimensions: vec![],
            cache_artifacts: vec![],
            thinking_hints: vec![],
            dsel_axes: common_dsel_axes(),
            notes: "Fallback taxonomy entry".to_string(),
        })
}

pub fn classify_http_request_prefix(data: &[u8]) -> Option<ModelProtocolDecode> {
    let req = parse_http_request_prefix(data)?;
    let (template, family, action, confidence, mut notes) = best_template_for_request(&req);
    let entry = entry_by_template(template);

    if family != entry.family && family != ProviderFamily::Unknown {
        notes.push(format!(
            "runtime family heuristic {:?} overrides taxonomy default {:?}",
            family, entry.family
        ));
    }

    let host = req.headers.get("host").cloned();

    Some(ModelProtocolDecode {
        method: req.method,
        path: req.path,
        host,
        family: if family == ProviderFamily::Unknown {
            entry.family
        } else {
            family
        },
        template,
        action,
        default_mux: entry.default_mux,
        optional_muxes: entry.optional_muxes,
        auth_templates: entry.auth_templates,
        env_keys: entry.env_keys,
        quota_dimensions: entry.quota_dimensions,
        cache_artifacts: entry.cache_artifacts,
        thinking_hints: entry.thinking_hints,
        confidence,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openai_chat_completions() {
        let req = b"POST /v1/chat/completions HTTP/1.1\r\nHost: api.openai.com\r\nAuthorization: Bearer sk-test\r\nContent-Type: application/json\r\n\r\n{\"model\":\"gpt-4.1\"}";
        let decoded = classify_http_request_prefix(req).expect("decode");
        assert_eq!(decoded.template, ServingTemplateId::OpenAiV1);
        assert_eq!(decoded.action, ModelApiAction::ChatCompletions);
        assert_eq!(decoded.default_mux, MuxSurface::ChatCompletions);
        assert!(decoded.env_keys.iter().any(|k| k == "OPENAI_API_KEY"));
    }

    #[test]
    fn parses_anthropic_claude_code_count_tokens() {
        let req = b"POST /v1/messages/count_tokens HTTP/1.1\r\nHost: api.anthropic.com\r\nx-api-key: sk-ant\r\nanthropic-version: 2023-06-01\r\nanthropic-beta: claude-code-20250219\r\n\r\n{}";
        let decoded = classify_http_request_prefix(req).expect("decode");
        assert_eq!(decoded.template, ServingTemplateId::ClaudeCodeCompat);
        assert_eq!(decoded.action, ModelApiAction::AnthropicCountTokens);
        assert_eq!(decoded.default_mux, MuxSurface::ClaudeMessages);
    }

    #[test]
    fn parses_gemini_native_generate_content() {
        let req = b"POST /v1beta/models/gemini-2.5-pro:generateContent HTTP/1.1\r\nHost: generativelanguage.googleapis.com\r\nx-goog-api-key: test\r\n\r\n{}";
        let decoded = classify_http_request_prefix(req).expect("decode");
        assert_eq!(decoded.template, ServingTemplateId::GeminiNative);
        assert_eq!(decoded.action, ModelApiAction::GeminiGenerateContent);
        assert_eq!(decoded.default_mux, MuxSurface::GeminiGenerateContent);
    }

    #[test]
    fn parses_control_plane_openapi_and_keyvault() {
        let openapi = b"GET /api-docs/openapi.json HTTP/1.1\r\nHost: 127.0.0.1:7777\r\n\r\n";
        let d1 = classify_http_request_prefix(openapi).expect("openapi decode");
        assert_eq!(d1.template, ServingTemplateId::OpenApi3Commands);
        assert_eq!(d1.action, ModelApiAction::OpenApiDiscovery);

        let kv = b"GET /v1/keys HTTP/1.1\r\nHost: 127.0.0.1:7777\r\nAuthorization: Bearer local\r\n\r\n";
        let d2 = classify_http_request_prefix(kv).expect("kv decode");
        assert_eq!(d2.template, ServingTemplateId::KeyVaultOps);
        assert_eq!(d2.action, ModelApiAction::KeyVaultList);
    }

    #[test]
    fn exposes_trait4_entry() {
        let taxonomy = comprehensive_model_serving_taxonomy();
        assert!(taxonomy.iter().any(|e| e.template == ServingTemplateId::Trait4));
    }
}
