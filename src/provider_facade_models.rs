use crate::model_serving_taxonomy::{
    comprehensive_model_serving_taxonomy, AuthTemplate, CacheArtifact, ModelApiAction,
    MuxSurface, ProviderFamily, QuotaDimension, ServingTemplateId, ThinkingHint,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnvVarRole {
    ProviderId,
    BaseUrl,
    Model,
    ReasoningModel,
    ApiKey,
    AccessToken,
    RefreshToken,
    OAuthClientId,
    OAuthClientSecret,
    OAuthTokenUrl,
    OAuthAuthUrl,
    OAuthAudience,
    OAuthScopes,
    PubkeyFingerprint,
    PubkeyMaterial,
    PubkeyAllowedProviders,
    KeymuxUrl,
    KeyVaultUrl,
    MuxPolicy,
    QuotaProfile,
    TemplateOverride,
    WrapperPath,
    ControlToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVarBinding {
    pub key: String,
    pub aliases: Vec<String>,
    pub role: EnvVarRole,
    pub required: bool,
    pub secret: bool,
    pub pattern_hint: Option<String>,
    pub examples: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvRecognitionRule {
    pub id: String,
    pub family_hint: ProviderFamily,
    pub host_contains_any: Vec<String>,
    pub header_contains_any: Vec<String>,
    pub env_keys_all: Vec<String>,
    pub env_keys_any: Vec<String>,
    pub inferred_templates: Vec<ServingTemplateId>,
    pub inferred_muxes: Vec<MuxSurface>,
    pub inferred_auth_templates: Vec<AuthTemplate>,
    pub confidence: u8,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OAuthGrantType {
    AuthorizationCodePkce,
    ClientCredentials,
    DeviceCode,
    JwtBearer,
    TokenExchange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthGrantSpec {
    pub id: String,
    pub grant_type: OAuthGrantType,
    pub token_endpoint_envs: Vec<String>,
    pub auth_endpoint_envs: Vec<String>,
    pub client_id_envs: Vec<String>,
    pub client_secret_envs: Vec<String>,
    pub audience_envs: Vec<String>,
    pub scope_envs: Vec<String>,
    pub default_scopes: Vec<String>,
    pub pkce: bool,
    pub refresh_supported: bool,
    pub bind_pubkey_fingerprint: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PubkeyGrantType {
    SshPubkeySessionUnlock,
    Ed25519ChallengeResponse,
    DetachedJwsVerification,
    MtlsClientCertificateBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PubkeyGrantSpec {
    pub id: String,
    pub grant_type: PubkeyGrantType,
    pub fingerprint_envs: Vec<String>,
    pub pubkey_envs: Vec<String>,
    pub keymux_url_envs: Vec<String>,
    pub allowed_provider_envs: Vec<String>,
    pub session_ttl_secs: Option<u64>,
    pub requires_oauth_token_binding: bool,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderTemplateSurface {
    pub template: ServingTemplateId,
    pub default_mux: MuxSurface,
    pub optional_muxes: Vec<MuxSurface>,
    pub actions: Vec<ModelApiAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderAccessModel {
    pub auth_templates: Vec<AuthTemplate>,
    pub oauth_grants: Vec<OAuthGrantSpec>,
    pub pubkey_grants: Vec<PubkeyGrantSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderFacadeObjectModel {
    pub id: String,
    pub display_name: String,
    pub family: ProviderFamily,
    pub templates: Vec<ProviderTemplateSurface>,
    pub env_bindings: Vec<EnvVarBinding>,
    pub env_recognition_rules: Vec<String>,
    pub access: ProviderAccessModel,
    pub quota_dimensions: Vec<QuotaDimension>,
    pub cache_artifacts: Vec<CacheArtifact>,
    pub thinking_hints: Vec<ThinkingHint>,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WrapperStage {
    IngressPath,
    Auth,
    EnvRecognition,
    DselOverride,
    TemplateSelection,
    MuxSelection,
    QuotaJoin,
    QuotaMacro,
    Transform,
    Cache,
    Audit,
    Egress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FacadePathWrapperPattern {
    pub id: String,
    pub pattern: String,
    pub regex_like: bool,
    pub example: String,
    pub captures: Vec<String>,
    pub stage_order: Vec<WrapperStage>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuotaMacro {
    pub id: String,
    pub dimensions: Vec<QuotaDimension>,
    pub defaults: BTreeMap<String, String>,
    pub inherits: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FacadeMacro {
    pub id: String,
    pub description: String,
    pub transforms: Vec<String>,
    pub cache_artifacts: Vec<CacheArtifact>,
    pub thinking_hints: Vec<ThinkingHint>,
    pub quota_macro: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WrapperQuotaMacroBinding {
    pub id: String,
    pub wrapper_pattern_id: String,
    pub quota_macro_id: String,
    pub macro_ids: Vec<String>,
    pub priority: u16,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FacadeRouteMatrixRow {
    pub id: String,
    pub provider_selector: String,
    pub model_selector: String,
    pub action: ModelApiAction,
    pub template: ServingTemplateId,
    pub default_mux: MuxSurface,
    pub optional_muxes: Vec<MuxSurface>,
    pub wrapper_binding_ids: Vec<String>,
    pub env_recognition_rule_ids: Vec<String>,
    pub grant_refs: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FacadeV1Matrix {
    pub env_rules: Vec<EnvRecognitionRule>,
    pub providers: Vec<ProviderFacadeObjectModel>,
    pub wrapper_patterns: Vec<FacadePathWrapperPattern>,
    pub quota_macros: Vec<QuotaMacro>,
    pub macros: Vec<FacadeMacro>,
    pub wrapper_quota_macro_bindings: Vec<WrapperQuotaMacroBinding>,
    pub routes: Vec<FacadeRouteMatrixRow>,
}

fn s(items: &[&str]) -> Vec<String> {
    items.iter().map(|v| (*v).to_string()).collect()
}

fn env_binding(
    key: &str,
    aliases: &[&str],
    role: EnvVarRole,
    required: bool,
    secret: bool,
    pattern_hint: Option<&str>,
) -> EnvVarBinding {
    EnvVarBinding {
        key: key.to_string(),
        aliases: s(aliases),
        role,
        required,
        secret,
        pattern_hint: pattern_hint.map(str::to_string),
        examples: vec![],
        notes: None,
    }
}

fn common_provider_env_bindings() -> Vec<EnvVarBinding> {
    vec![
        env_binding("MODEL_PROVIDER", &[], EnvVarRole::ProviderId, false, false, None),
        env_binding("MODEL_NAME", &[], EnvVarRole::Model, false, false, None),
        env_binding("MODEL_MUX_POLICY", &[], EnvVarRole::MuxPolicy, false, false, None),
        env_binding("MODEL_QUOTA_PROFILE", &[], EnvVarRole::QuotaProfile, false, false, None),
        env_binding(
            "MODEL_API_TEMPLATE",
            &[],
            EnvVarRole::TemplateOverride,
            false,
            false,
            None,
        ),
        env_binding("FACADE_V1_WRAPPER", &[], EnvVarRole::WrapperPath, false, false, Some("xxxxx/.*(./*)?")),
        env_binding("CC_SWITCH_CONTROL_TOKEN", &[], EnvVarRole::ControlToken, false, true, None),
        env_binding("KEYMUX_URL", &[], EnvVarRole::KeymuxUrl, false, false, None),
        env_binding("KEYVAULT_URL", &[], EnvVarRole::KeyVaultUrl, false, false, None),
    ]
}

fn env_recognition_rules() -> Vec<EnvRecognitionRule> {
    vec![
        EnvRecognitionRule {
            id: "openai-compatible".to_string(),
            family_hint: ProviderFamily::OpenAiCompatible,
            host_contains_any: s(&["api.openai.com", "openrouter.ai", "localhost:11434"]),
            header_contains_any: s(&["authorization: bearer"]),
            env_keys_all: vec![],
            env_keys_any: s(&["OPENAI_API_KEY", "OPENAI_BASE_URL"]),
            inferred_templates: vec![ServingTemplateId::OpenAiV1, ServingTemplateId::OpenAiResponses],
            inferred_muxes: vec![MuxSurface::ChatCompletions, MuxSurface::Responses],
            inferred_auth_templates: vec![AuthTemplate::BearerAuthorization],
            confidence: 80,
            notes: "Generic OpenAI-compatible ENV recognition rule.".to_string(),
        },
        EnvRecognitionRule {
            id: "anthropic-compatible".to_string(),
            family_hint: ProviderFamily::AnthropicCompatible,
            host_contains_any: s(&["api.anthropic.com", "anthropic"]),
            header_contains_any: s(&["anthropic-version:", "x-api-key:"]),
            env_keys_all: vec![],
            env_keys_any: s(&["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL"]),
            inferred_templates: vec![ServingTemplateId::ClaudeMessages, ServingTemplateId::ClaudeCodeCompat],
            inferred_muxes: vec![MuxSurface::ClaudeMessages, MuxSurface::ChatCompletions],
            inferred_auth_templates: vec![AuthTemplate::AnthropicXApiKey, AuthTemplate::BearerAuthorization],
            confidence: 85,
            notes: "Anthropic/Claude compatible ENV recognition rule.".to_string(),
        },
        EnvRecognitionRule {
            id: "gemini-native".to_string(),
            family_hint: ProviderFamily::GeminiNative,
            host_contains_any: s(&["googleapis.com", "generativelanguage.googleapis.com"]),
            header_contains_any: s(&["x-goog-api-key"]),
            env_keys_all: vec![],
            env_keys_any: s(&["GEMINI_API_KEY", "GOOGLE_API_KEY"]),
            inferred_templates: vec![ServingTemplateId::GeminiNative, ServingTemplateId::OpenAiGeminiCompat],
            inferred_muxes: vec![MuxSurface::GeminiGenerateContent, MuxSurface::ChatCompletions],
            inferred_auth_templates: vec![AuthTemplate::GoogleApiKeyHeader, AuthTemplate::GoogleApiKeyQuery],
            confidence: 85,
            notes: "Gemini native and compatibility shim rule.".to_string(),
        },
        EnvRecognitionRule {
            id: "opencode-zen".to_string(),
            family_hint: ProviderFamily::OpenCodeZen,
            host_contains_any: s(&["opencode.ai/zen", "opencode.ai"]),
            header_contains_any: s(&["authorization: bearer", "x-api-key:"]),
            env_keys_all: vec![],
            env_keys_any: s(&["OPENAI_BASE_URL", "ANTHROPIC_BASE_URL"]),
            inferred_templates: vec![ServingTemplateId::OpenCodeZenCompat],
            inferred_muxes: vec![
                MuxSurface::ChatCompletions,
                MuxSurface::ClaudeMessages,
                MuxSurface::Responses,
            ],
            inferred_auth_templates: vec![AuthTemplate::BearerAuthorization, AuthTemplate::AnthropicXApiKey],
            confidence: 90,
            notes: "OpenCode Zen mixed endpoint compatibility rule.".to_string(),
        },
        EnvRecognitionRule {
            id: "control-plane".to_string(),
            family_hint: ProviderFamily::ControlPlane,
            host_contains_any: s(&["127.0.0.1", "localhost"]),
            header_contains_any: s(&["authorization: bearer"]),
            env_keys_all: vec![],
            env_keys_any: s(&["CC_SWITCH_CONTROL_TOKEN", "KEYMUX_URL", "KEYVAULT_URL"]),
            inferred_templates: vec![ServingTemplateId::OpenApi3Commands, ServingTemplateId::KeyVaultOps],
            inferred_muxes: vec![MuxSurface::OpenApi3Commands, MuxSurface::KeyVaultOps],
            inferred_auth_templates: vec![AuthTemplate::BearerAuthorization, AuthTemplate::NoneOrSession],
            confidence: 75,
            notes: "Local facade control-plane rule for OpenAPI and key vault ops.".to_string(),
        },
    ]
}

fn oauth_grants_catalog() -> Vec<OAuthGrantSpec> {
    vec![
        OAuthGrantSpec {
            id: "oauth-client-credentials".to_string(),
            grant_type: OAuthGrantType::ClientCredentials,
            token_endpoint_envs: s(&["OAUTH_TOKEN_URL", "AZURE_AUTH_TOKEN_URL"]),
            auth_endpoint_envs: vec![],
            client_id_envs: s(&["OAUTH_CLIENT_ID", "AZURE_CLIENT_ID"]),
            client_secret_envs: s(&["OAUTH_CLIENT_SECRET", "AZURE_CLIENT_SECRET"]),
            audience_envs: s(&["OAUTH_AUDIENCE", "AZURE_AUTH_AUDIENCE"]),
            scope_envs: s(&["OAUTH_SCOPES"]),
            default_scopes: vec![],
            pkce: false,
            refresh_supported: false,
            bind_pubkey_fingerprint: false,
            notes: "Server-side facade token minting for providers requiring OAuth2 client credentials.".to_string(),
        },
        OAuthGrantSpec {
            id: "oauth-device-code".to_string(),
            grant_type: OAuthGrantType::DeviceCode,
            token_endpoint_envs: s(&["OAUTH_TOKEN_URL"]),
            auth_endpoint_envs: s(&["OAUTH_DEVICE_AUTH_URL"]),
            client_id_envs: s(&["OAUTH_CLIENT_ID"]),
            client_secret_envs: vec![],
            audience_envs: s(&["OAUTH_AUDIENCE"]),
            scope_envs: s(&["OAUTH_SCOPES"]),
            default_scopes: s(&["openid", "profile"]),
            pkce: false,
            refresh_supported: true,
            bind_pubkey_fingerprint: true,
            notes: "CLI-friendly device flow with optional pubkey-bound session unlock.".to_string(),
        },
        OAuthGrantSpec {
            id: "oauth-pkce".to_string(),
            grant_type: OAuthGrantType::AuthorizationCodePkce,
            token_endpoint_envs: s(&["OAUTH_TOKEN_URL"]),
            auth_endpoint_envs: s(&["OAUTH_AUTH_URL"]),
            client_id_envs: s(&["OAUTH_CLIENT_ID"]),
            client_secret_envs: vec![],
            audience_envs: s(&["OAUTH_AUDIENCE"]),
            scope_envs: s(&["OAUTH_SCOPES"]),
            default_scopes: s(&["openid", "profile"]),
            pkce: true,
            refresh_supported: true,
            bind_pubkey_fingerprint: true,
            notes: "Browser-assisted PKCE flow for local facade v1 sessions.".to_string(),
        },
    ]
}

fn pubkey_grants_catalog() -> Vec<PubkeyGrantSpec> {
    vec![
        PubkeyGrantSpec {
            id: "ssh-pubkey-keymux-unlock".to_string(),
            grant_type: PubkeyGrantType::SshPubkeySessionUnlock,
            fingerprint_envs: s(&["SSH_PUBKEY_FINGERPRINT", "PUBKEY_FINGERPRINT"]),
            pubkey_envs: s(&["SSH_PUBLIC_KEY", "PUBKEY_MATERIAL"]),
            keymux_url_envs: s(&["KEYMUX_URL"]),
            allowed_provider_envs: s(&["PUBKEY_ALLOWED_PROVIDERS"]),
            session_ttl_secs: Some(900),
            requires_oauth_token_binding: false,
            notes: "Unlock provider keys via SSH gate/keymux session; maps to provider scopes.".to_string(),
        },
        PubkeyGrantSpec {
            id: "ed25519-challenge".to_string(),
            grant_type: PubkeyGrantType::Ed25519ChallengeResponse,
            fingerprint_envs: s(&["PUBKEY_FINGERPRINT"]),
            pubkey_envs: s(&["PUBKEY_MATERIAL"]),
            keymux_url_envs: s(&["KEYMUX_URL"]),
            allowed_provider_envs: s(&["PUBKEY_ALLOWED_PROVIDERS"]),
            session_ttl_secs: Some(300),
            requires_oauth_token_binding: true,
            notes: "Challenge/response proof of possession before facade issues scoped session token.".to_string(),
        },
    ]
}

fn wrapper_patterns() -> Vec<FacadePathWrapperPattern> {
    vec![
        FacadePathWrapperPattern {
            id: "facade-v1-provider-action".to_string(),
            pattern: "/v1/{provider}/{action}".to_string(),
            regex_like: false,
            example: "/v1/openai/chat/completions".to_string(),
            captures: s(&["provider", "action"]),
            stage_order: vec![
                WrapperStage::IngressPath,
                WrapperStage::EnvRecognition,
                WrapperStage::DselOverride,
                WrapperStage::TemplateSelection,
                WrapperStage::MuxSelection,
                WrapperStage::QuotaJoin,
                WrapperStage::Transform,
                WrapperStage::Cache,
                WrapperStage::Egress,
            ],
            notes: "Canonical facade path for provider/action selection.".to_string(),
        },
        FacadePathWrapperPattern {
            id: "facade-v1-provider-model-action".to_string(),
            pattern: "/v1/{provider}/{model}/{action}".to_string(),
            regex_like: false,
            example: "/v1/opencode-zen/glm-5/chat/completions".to_string(),
            captures: s(&["provider", "model", "action"]),
            stage_order: vec![
                WrapperStage::IngressPath,
                WrapperStage::EnvRecognition,
                WrapperStage::DselOverride,
                WrapperStage::TemplateSelection,
                WrapperStage::MuxSelection,
                WrapperStage::QuotaJoin,
                WrapperStage::Transform,
                WrapperStage::Cache,
                WrapperStage::Egress,
            ],
            notes: "Provider+model explicit routing for DSEL/manual override flows.".to_string(),
        },
        FacadePathWrapperPattern {
            id: "facade-v1-wrapper-catchall".to_string(),
            pattern: "xxxxx/.*(./*)?".to_string(),
            regex_like: true,
            example: "openai/chat/completions".to_string(),
            captures: s(&["wrapper", "path_rest"]),
            stage_order: vec![
                WrapperStage::IngressPath,
                WrapperStage::Auth,
                WrapperStage::EnvRecognition,
                WrapperStage::DselOverride,
                WrapperStage::QuotaMacro,
                WrapperStage::Transform,
                WrapperStage::Cache,
                WrapperStage::Audit,
                WrapperStage::Egress,
            ],
            notes: "Regex-like wrapper path requested for generic facade driving; use as late-bound catchall.".to_string(),
        },
        FacadePathWrapperPattern {
            id: "facade-openapi-control".to_string(),
            pattern: "/v1/control/openapi/{resource}".to_string(),
            regex_like: false,
            example: "/v1/control/openapi/commands".to_string(),
            captures: s(&["resource"]),
            stage_order: vec![
                WrapperStage::IngressPath,
                WrapperStage::Auth,
                WrapperStage::QuotaJoin,
                WrapperStage::QuotaMacro,
                WrapperStage::Egress,
            ],
            notes: "OpenAPI3 command surface wrapper.".to_string(),
        },
        FacadePathWrapperPattern {
            id: "facade-keyvault-control".to_string(),
            pattern: "/v1/control/keyvault/{op}".to_string(),
            regex_like: false,
            example: "/v1/control/keyvault/list".to_string(),
            captures: s(&["op"]),
            stage_order: vec![
                WrapperStage::IngressPath,
                WrapperStage::Auth,
                WrapperStage::QuotaJoin,
                WrapperStage::QuotaMacro,
                WrapperStage::Audit,
                WrapperStage::Egress,
            ],
            notes: "Key vault / keymux operations on same facade.".to_string(),
        },
    ]
}

fn quota_macros() -> Vec<QuotaMacro> {
    let mut balanced_defaults = BTreeMap::new();
    balanced_defaults.insert("rpm".to_string(), "60".to_string());
    balanced_defaults.insert("tpm".to_string(), "120000".to_string());
    balanced_defaults.insert("concurrency".to_string(), "8".to_string());

    let mut reasoning_defaults = BTreeMap::new();
    reasoning_defaults.insert("rpm".to_string(), "30".to_string());
    reasoning_defaults.insert("tpm".to_string(), "80000".to_string());
    reasoning_defaults.insert("reasoning_budget".to_string(), "20000".to_string());
    reasoning_defaults.insert("concurrency".to_string(), "4".to_string());

    let mut control_defaults = BTreeMap::new();
    control_defaults.insert("rpm".to_string(), "120".to_string());
    control_defaults.insert("concurrency".to_string(), "16".to_string());

    vec![
        QuotaMacro {
            id: "balanced-chat".to_string(),
            dimensions: vec![
                QuotaDimension::RequestsPerMinute,
                QuotaDimension::TokensPerMinute,
                QuotaDimension::ConcurrentRequests,
                QuotaDimension::ContextWindowTokens,
                QuotaDimension::MaxOutputTokens,
            ],
            defaults: balanced_defaults,
            inherits: vec![],
            notes: "General-purpose chat completions macro.".to_string(),
        },
        QuotaMacro {
            id: "reasoning-heavy".to_string(),
            dimensions: vec![
                QuotaDimension::RequestsPerMinute,
                QuotaDimension::TokensPerMinute,
                QuotaDimension::ReasoningBudgetTokens,
                QuotaDimension::ConcurrentRequests,
                QuotaDimension::ContextWindowTokens,
            ],
            defaults: reasoning_defaults,
            inherits: s(&["balanced-chat"]),
            notes: "Reasoning-heavy responses/Claude-thinking macro.".to_string(),
        },
        QuotaMacro {
            id: "control-plane".to_string(),
            dimensions: vec![QuotaDimension::RequestsPerMinute, QuotaDimension::ConcurrentRequests],
            defaults: control_defaults,
            inherits: vec![],
            notes: "OpenAPI command + key vault ops macro.".to_string(),
        },
    ]
}

fn facade_macros() -> Vec<FacadeMacro> {
    vec![
        FacadeMacro {
            id: "macro-chat-wrapper".to_string(),
            description: "OpenAI chat facade wrapper transform".to_string(),
            transforms: s(&["normalize-env", "resolve-provider", "chat-request-shape"]),
            cache_artifacts: vec![
                CacheArtifact::ModelSheets,
                CacheArtifact::QuotaHints,
                CacheArtifact::ContextHints,
            ],
            thinking_hints: vec![ThinkingHint::OpenAiReasoningEffort],
            quota_macro: Some("balanced-chat".to_string()),
            notes: "Default chat wrapper macro.".to_string(),
        },
        FacadeMacro {
            id: "macro-response-wrapper".to_string(),
            description: "Responses API facade wrapper transform".to_string(),
            transforms: s(&["normalize-env", "resolve-provider", "response-api-shape"]),
            cache_artifacts: vec![
                CacheArtifact::ModelSheets,
                CacheArtifact::QuotaHints,
                CacheArtifact::ThinkingHints,
                CacheArtifact::ContextHints,
            ],
            thinking_hints: vec![
                ThinkingHint::OpenAiResponsesReasoning,
                ThinkingHint::ClaudeThinkingBlocks,
            ],
            quota_macro: Some("reasoning-heavy".to_string()),
            notes: "Reasoning-oriented responses wrapper.".to_string(),
        },
        FacadeMacro {
            id: "macro-control-plane".to_string(),
            description: "OpenAPI/key-vault control plane wrapper".to_string(),
            transforms: s(&["auth-control-token", "control-router"]),
            cache_artifacts: vec![CacheArtifact::ApiTemplateHints],
            thinking_hints: vec![ThinkingHint::None],
            quota_macro: Some("control-plane".to_string()),
            notes: "Self-contained facade control plane macro.".to_string(),
        },
    ]
}

fn wrapper_quota_macro_bindings() -> Vec<WrapperQuotaMacroBinding> {
    vec![
        WrapperQuotaMacroBinding {
            id: "bind-provider-action-chat".to_string(),
            wrapper_pattern_id: "facade-v1-provider-action".to_string(),
            quota_macro_id: "balanced-chat".to_string(),
            macro_ids: s(&["macro-chat-wrapper"]),
            priority: 100,
            notes: "Default provider/action wrapper binding.".to_string(),
        },
        WrapperQuotaMacroBinding {
            id: "bind-provider-model-action-reasoning".to_string(),
            wrapper_pattern_id: "facade-v1-provider-model-action".to_string(),
            quota_macro_id: "reasoning-heavy".to_string(),
            macro_ids: s(&["macro-response-wrapper"]),
            priority: 110,
            notes: "Provider+model explicit wrapper favors reasoning quota and response mux.".to_string(),
        },
        WrapperQuotaMacroBinding {
            id: "bind-catchall-generic".to_string(),
            wrapper_pattern_id: "facade-v1-wrapper-catchall".to_string(),
            quota_macro_id: "balanced-chat".to_string(),
            macro_ids: s(&["macro-chat-wrapper", "macro-response-wrapper"]),
            priority: 10,
            notes: "Low-priority generic catchall for xxxxx/.*(./*)? wrapper hierarchy.".to_string(),
        },
        WrapperQuotaMacroBinding {
            id: "bind-openapi-control".to_string(),
            wrapper_pattern_id: "facade-openapi-control".to_string(),
            quota_macro_id: "control-plane".to_string(),
            macro_ids: s(&["macro-control-plane"]),
            priority: 200,
            notes: "OpenAPI control plane path binding.".to_string(),
        },
        WrapperQuotaMacroBinding {
            id: "bind-keyvault-control".to_string(),
            wrapper_pattern_id: "facade-keyvault-control".to_string(),
            quota_macro_id: "control-plane".to_string(),
            macro_ids: s(&["macro-control-plane"]),
            priority: 200,
            notes: "Key vault control plane path binding.".to_string(),
        },
    ]
}

pub fn provider_facade_object_models() -> Vec<ProviderFacadeObjectModel> {
    let taxonomy = comprehensive_model_serving_taxonomy();
    let oauth = oauth_grants_catalog();
    let pubkey = pubkey_grants_catalog();
    let common_env = common_provider_env_bindings();

    let by_template = |template: ServingTemplateId| {
        taxonomy
            .iter()
            .find(|e| e.template == template)
            .expect("template in taxonomy")
    };

    let make_surface = |template: ServingTemplateId| {
        let e = by_template(template);
        ProviderTemplateSurface {
            template,
            default_mux: e.default_mux,
            optional_muxes: e.optional_muxes.clone(),
            actions: e.supported_actions.clone(),
        }
    };

    vec![
        ProviderFacadeObjectModel {
            id: "openai-compat".to_string(),
            display_name: "OpenAI Compatible".to_string(),
            family: ProviderFamily::OpenAiCompatible,
            templates: vec![
                make_surface(ServingTemplateId::OpenAiV1),
                make_surface(ServingTemplateId::OpenAiResponses),
                make_surface(ServingTemplateId::OpenAiHarmony),
            ],
            env_bindings: {
                let mut v = common_env.clone();
                v.extend(vec![
                    env_binding("OPENAI_API_KEY", &[], EnvVarRole::ApiKey, false, true, None),
                    env_binding("OPENAI_BASE_URL", &[], EnvVarRole::BaseUrl, false, false, None),
                    env_binding("OPENAI_MODEL", &[], EnvVarRole::Model, false, false, None),
                ]);
                v
            },
            env_recognition_rules: s(&["openai-compatible"]),
            access: ProviderAccessModel {
                auth_templates: vec![AuthTemplate::BearerAuthorization, AuthTemplate::ApiKeyHeader],
                oauth_grants: vec![oauth[0].clone(), oauth[1].clone()],
                pubkey_grants: vec![pubkey[0].clone()],
            },
            quota_dimensions: by_template(ServingTemplateId::OpenAiV1).quota_dimensions.clone(),
            cache_artifacts: by_template(ServingTemplateId::OpenAiV1).cache_artifacts.clone(),
            thinking_hints: by_template(ServingTemplateId::OpenAiResponses).thinking_hints.clone(),
            notes: "Generic provider object model for OpenAI-compatible and derived response/harmony surfaces.".to_string(),
        },
        ProviderFacadeObjectModel {
            id: "anthropic-compat".to_string(),
            display_name: "Anthropic / Claude Compatible".to_string(),
            family: ProviderFamily::AnthropicCompatible,
            templates: vec![
                make_surface(ServingTemplateId::ClaudeMessages),
                make_surface(ServingTemplateId::ClaudeCodeCompat),
            ],
            env_bindings: {
                let mut v = common_env.clone();
                v.extend(vec![
                    env_binding("ANTHROPIC_AUTH_TOKEN", &["ANTHROPIC_API_KEY"], EnvVarRole::ApiKey, false, true, None),
                    env_binding("ANTHROPIC_BASE_URL", &[], EnvVarRole::BaseUrl, false, false, None),
                    env_binding("ANTHROPIC_MODEL", &[], EnvVarRole::Model, false, false, None),
                    env_binding("ANTHROPIC_REASONING_MODEL", &[], EnvVarRole::ReasoningModel, false, false, None),
                ]);
                v
            },
            env_recognition_rules: s(&["anthropic-compatible"]),
            access: ProviderAccessModel {
                auth_templates: vec![AuthTemplate::AnthropicXApiKey, AuthTemplate::BearerAuthorization],
                oauth_grants: vec![oauth[2].clone()],
                pubkey_grants: vec![pubkey[0].clone(), pubkey[1].clone()],
            },
            quota_dimensions: by_template(ServingTemplateId::ClaudeCodeCompat).quota_dimensions.clone(),
            cache_artifacts: by_template(ServingTemplateId::ClaudeMessages).cache_artifacts.clone(),
            thinking_hints: by_template(ServingTemplateId::ClaudeCodeCompat).thinking_hints.clone(),
            notes: "Claude Code-compatible provider object with count_tokens and thinking semantics.".to_string(),
        },
        ProviderFacadeObjectModel {
            id: "gemini".to_string(),
            display_name: "Gemini".to_string(),
            family: ProviderFamily::GeminiNative,
            templates: vec![
                make_surface(ServingTemplateId::GeminiNative),
                make_surface(ServingTemplateId::OpenAiGeminiCompat),
            ],
            env_bindings: {
                let mut v = common_env.clone();
                v.extend(vec![
                    env_binding("GEMINI_API_KEY", &["GOOGLE_API_KEY"], EnvVarRole::ApiKey, false, true, None),
                    env_binding("GOOGLE_GEMINI_BASE_URL", &[], EnvVarRole::BaseUrl, false, false, None),
                ]);
                v
            },
            env_recognition_rules: s(&["gemini-native"]),
            access: ProviderAccessModel {
                auth_templates: vec![AuthTemplate::GoogleApiKeyHeader, AuthTemplate::GoogleApiKeyQuery],
                oauth_grants: vec![oauth[1].clone()],
                pubkey_grants: vec![pubkey[1].clone()],
            },
            quota_dimensions: by_template(ServingTemplateId::GeminiNative).quota_dimensions.clone(),
            cache_artifacts: by_template(ServingTemplateId::GeminiNative).cache_artifacts.clone(),
            thinking_hints: by_template(ServingTemplateId::GeminiNative).thinking_hints.clone(),
            notes: "Gemini native plus optional OpenAI-compatible facade surface.".to_string(),
        },
        ProviderFacadeObjectModel {
            id: "opencode-zen".to_string(),
            display_name: "OpenCode Zen".to_string(),
            family: ProviderFamily::OpenCodeZen,
            templates: vec![make_surface(ServingTemplateId::OpenCodeZenCompat)],
            env_bindings: {
                let mut v = common_env.clone();
                v.extend(vec![
                    env_binding("OPENAI_API_KEY", &[], EnvVarRole::ApiKey, false, true, None),
                    env_binding("OPENAI_BASE_URL", &[], EnvVarRole::BaseUrl, false, false, None),
                    env_binding("ANTHROPIC_AUTH_TOKEN", &[], EnvVarRole::ApiKey, false, true, None),
                    env_binding("ANTHROPIC_BASE_URL", &[], EnvVarRole::BaseUrl, false, false, None),
                    env_binding("ANTHROPIC_MODEL", &[], EnvVarRole::Model, false, false, None),
                ]);
                v
            },
            env_recognition_rules: s(&["opencode-zen"]),
            access: ProviderAccessModel {
                auth_templates: vec![AuthTemplate::BearerAuthorization, AuthTemplate::AnthropicXApiKey],
                oauth_grants: vec![oauth[1].clone(), oauth[2].clone()],
                pubkey_grants: vec![pubkey[0].clone()],
            },
            quota_dimensions: by_template(ServingTemplateId::OpenCodeZenCompat).quota_dimensions.clone(),
            cache_artifacts: by_template(ServingTemplateId::OpenCodeZenCompat).cache_artifacts.clone(),
            thinking_hints: by_template(ServingTemplateId::OpenCodeZenCompat).thinking_hints.clone(),
            notes: "Mixed endpoint/provider surface with orthogonal mux selection (chat/responses/claude).".to_string(),
        },
        ProviderFacadeObjectModel {
            id: "facade-control-plane".to_string(),
            display_name: "Facade Control Plane".to_string(),
            family: ProviderFamily::ControlPlane,
            templates: vec![
                make_surface(ServingTemplateId::OpenApi3Commands),
                make_surface(ServingTemplateId::KeyVaultOps),
            ],
            env_bindings: common_env.clone(),
            env_recognition_rules: s(&["control-plane"]),
            access: ProviderAccessModel {
                auth_templates: vec![AuthTemplate::BearerAuthorization, AuthTemplate::NoneOrSession],
                oauth_grants: vec![oauth[0].clone()],
                pubkey_grants: vec![pubkey[0].clone(), pubkey[1].clone()],
            },
            quota_dimensions: by_template(ServingTemplateId::OpenApi3Commands).quota_dimensions.clone(),
            cache_artifacts: vec![CacheArtifact::ApiTemplateHints],
            thinking_hints: vec![ThinkingHint::None],
            notes: "OpenAPI commands + key vault/keymux ops replacing CLI dependencies on the same v1 facade.".to_string(),
        },
    ]
}

pub fn facade_v1_route_matrix() -> FacadeV1Matrix {
    let providers = provider_facade_object_models();
    let env_rules = env_recognition_rules();
    let wrapper_patterns = wrapper_patterns();
    let quota_macros = quota_macros();
    let macros = facade_macros();
    let wrapper_quota_macro_bindings = wrapper_quota_macro_bindings();

    let routes = vec![
        FacadeRouteMatrixRow {
            id: "route-openai-chat".to_string(),
            provider_selector: "openai-compat".to_string(),
            model_selector: "*".to_string(),
            action: ModelApiAction::ChatCompletions,
            template: ServingTemplateId::OpenAiV1,
            default_mux: MuxSurface::ChatCompletions,
            optional_muxes: vec![MuxSurface::Responses, MuxSurface::HarmonyResponses],
            wrapper_binding_ids: s(&["bind-provider-action-chat", "bind-catchall-generic"]),
            env_recognition_rule_ids: s(&["openai-compatible"]),
            grant_refs: s(&["oauth-client-credentials", "ssh-pubkey-keymux-unlock"]),
            notes: "Generic OpenAI-compatible chat facade route.".to_string(),
        },
        FacadeRouteMatrixRow {
            id: "route-openai-responses".to_string(),
            provider_selector: "openai-compat".to_string(),
            model_selector: "*".to_string(),
            action: ModelApiAction::Responses,
            template: ServingTemplateId::OpenAiResponses,
            default_mux: MuxSurface::Responses,
            optional_muxes: vec![MuxSurface::ChatCompletions, MuxSurface::HarmonyResponses],
            wrapper_binding_ids: s(&["bind-provider-model-action-reasoning", "bind-catchall-generic"]),
            env_recognition_rule_ids: s(&["openai-compatible"]),
            grant_refs: s(&["oauth-client-credentials", "ed25519-challenge"]),
            notes: "Responses facade route with reasoning quota macro.".to_string(),
        },
        FacadeRouteMatrixRow {
            id: "route-claude-messages".to_string(),
            provider_selector: "anthropic-compat".to_string(),
            model_selector: "*".to_string(),
            action: ModelApiAction::AnthropicMessages,
            template: ServingTemplateId::ClaudeMessages,
            default_mux: MuxSurface::ClaudeMessages,
            optional_muxes: vec![MuxSurface::ChatCompletions, MuxSurface::Responses],
            wrapper_binding_ids: s(&["bind-provider-action-chat", "bind-catchall-generic"]),
            env_recognition_rule_ids: s(&["anthropic-compatible", "opencode-zen"]),
            grant_refs: s(&["oauth-pkce", "ssh-pubkey-keymux-unlock"]),
            notes: "Anthropic messages route with optional mux conversion.".to_string(),
        },
        FacadeRouteMatrixRow {
            id: "route-gemini-generate".to_string(),
            provider_selector: "gemini".to_string(),
            model_selector: "*".to_string(),
            action: ModelApiAction::GeminiGenerateContent,
            template: ServingTemplateId::GeminiNative,
            default_mux: MuxSurface::GeminiGenerateContent,
            optional_muxes: vec![MuxSurface::ChatCompletions, MuxSurface::Responses],
            wrapper_binding_ids: s(&["bind-provider-model-action-reasoning", "bind-catchall-generic"]),
            env_recognition_rule_ids: s(&["gemini-native"]),
            grant_refs: s(&["oauth-device-code", "ed25519-challenge"]),
            notes: "Gemini native route with optional compat mux surfaces.".to_string(),
        },
        FacadeRouteMatrixRow {
            id: "route-opencode-zen-chat".to_string(),
            provider_selector: "opencode-zen".to_string(),
            model_selector: "*".to_string(),
            action: ModelApiAction::ChatCompletions,
            template: ServingTemplateId::OpenCodeZenCompat,
            default_mux: MuxSurface::ChatCompletions,
            optional_muxes: vec![MuxSurface::ClaudeMessages, MuxSurface::Responses],
            wrapper_binding_ids: s(&["bind-provider-model-action-reasoning", "bind-catchall-generic"]),
            env_recognition_rule_ids: s(&["opencode-zen"]),
            grant_refs: s(&["oauth-device-code", "ssh-pubkey-keymux-unlock"]),
            notes: "OpenCode Zen mixed endpoint route with orthogonal mux policy.".to_string(),
        },
        FacadeRouteMatrixRow {
            id: "route-control-openapi".to_string(),
            provider_selector: "facade-control-plane".to_string(),
            model_selector: "_".to_string(),
            action: ModelApiAction::OpenApiDiscovery,
            template: ServingTemplateId::OpenApi3Commands,
            default_mux: MuxSurface::OpenApi3Commands,
            optional_muxes: vec![],
            wrapper_binding_ids: s(&["bind-openapi-control"]),
            env_recognition_rule_ids: s(&["control-plane"]),
            grant_refs: s(&["oauth-client-credentials", "ssh-pubkey-keymux-unlock"]),
            notes: "Self-contained OpenAPI command surface.".to_string(),
        },
        FacadeRouteMatrixRow {
            id: "route-control-keyvault".to_string(),
            provider_selector: "facade-control-plane".to_string(),
            model_selector: "_".to_string(),
            action: ModelApiAction::KeyVaultRead,
            template: ServingTemplateId::KeyVaultOps,
            default_mux: MuxSurface::KeyVaultOps,
            optional_muxes: vec![],
            wrapper_binding_ids: s(&["bind-keyvault-control"]),
            env_recognition_rule_ids: s(&["control-plane"]),
            grant_refs: s(&["oauth-client-credentials", "ed25519-challenge"]),
            notes: "Key vault/keymux read route under unified facade.".to_string(),
        },
    ];

    FacadeV1Matrix {
        env_rules,
        providers,
        wrapper_patterns,
        quota_macros,
        macros,
        wrapper_quota_macro_bindings,
        routes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_contains_oauth_and_pubkey_grants() {
        let matrix = facade_v1_route_matrix();
        let providers = &matrix.providers;
        assert!(
            providers.iter().any(|p| !p.access.oauth_grants.is_empty()),
            "expected oauth grants in at least one provider model"
        );
        assert!(
            providers.iter().any(|p| !p.access.pubkey_grants.is_empty()),
            "expected pubkey grants in at least one provider model"
        );
    }

    #[test]
    fn wrapper_hierarchy_contains_regex_like_catchall() {
        let matrix = facade_v1_route_matrix();
        let catchall = matrix
            .wrapper_patterns
            .iter()
            .find(|w| w.id == "facade-v1-wrapper-catchall")
            .expect("catchall wrapper pattern");
        assert!(catchall.regex_like);
        assert_eq!(catchall.pattern, "xxxxx/.*(./*)?");
    }

    #[test]
    fn control_plane_routes_are_present() {
        let matrix = facade_v1_route_matrix();
        assert!(matrix.routes.iter().any(|r| {
            r.template == ServingTemplateId::OpenApi3Commands
                && r.action == ModelApiAction::OpenApiDiscovery
        }));
        assert!(matrix.routes.iter().any(|r| {
            r.template == ServingTemplateId::KeyVaultOps
                && matches!(
                    r.action,
                    ModelApiAction::KeyVaultRead
                        | ModelApiAction::KeyVaultWrite
                        | ModelApiAction::KeyVaultList
                )
        }));
    }
}
