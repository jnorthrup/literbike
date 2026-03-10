use crate::model_serving_taxonomy::ProviderFamily;
use crate::provider_facade_models::{facade_v1_route_matrix, EnvVarRole};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKind {
    ModelProvider,
    Exchange,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchAuthHint {
    Bearer,
    HeaderXSubscriptionToken,
    QueryApiKey,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuxPragmaticFormat {
    OpenAiCompatible,
    AnthropicCompatible,
    GeminiNative,
    ControlPlane,
    ExchangeRest,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuxAuthOption {
    BearerAuthorization,
    ApiKeyHeader,
    AnthropicXApiKey,
    GoogleApiKeyHeader,
    GoogleApiKeyQuery,
    NoneOrSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuxRouteKind {
    Health,
    ModelsList,
    ChatCompletions,
    Responses,
    Embeddings,
    AnthropicMessages,
    AnthropicCountTokens,
    GeminiGenerateContent,
    GeminiStreamGenerateContent,
    GeminiCountTokens,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuxUrlMapping {
    pub route: MuxRouteKind,
    pub method: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostnameAccessOptions {
    pub base_url: String,
    pub hostname: Option<String>,
    pub api_kind: ApiKind,
    pub family_hint: Option<ProviderFamily>,
    pub format: MuxPragmaticFormat,
    pub auth_options: Vec<MuxAuthOption>,
    pub supports_models_probe: bool,
    pub model_probe_urls: Vec<String>,
    pub url_mappings: Vec<MuxUrlMapping>,
    pub matched_rule_ids: Vec<String>,
    pub confidence: u8,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaticOptionSpec {
    pub raw: String,
    pub key: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaticModelRef {
    pub raw: String,
    pub host_token: String,
    pub host: String,
    pub port: Option<u16>,
    pub base_url: String,
    pub option_tokens: Vec<String>,
    pub option_specs: Vec<PragmaticOptionSpec>,
    pub selector_prefixes: Vec<String>,
    pub modality: Option<String>,
    pub metadata: BTreeMap<String, String>,
    pub notes: Vec<String>,
    pub upstream_model_id: String,
    pub access: HostnameAccessOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PragmaticModelRefError {
    InvalidPrefix,
    MissingHostBlockClose,
    MissingModelId,
    EmptyHostToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaticWidenedModelCandidate {
    pub boundary: String,
    pub model: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaticModelFragments {
    pub provider_fragment: Option<String>,
    pub namespace_fragments: Vec<String>,
    pub leaf_fragment: String,
    pub all_fragments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaticUnifiedPortRoute {
    pub agent_name: String,
    pub unified_port: u16,
    pub route_key: String,
    pub host_scope: Option<String>,
    pub selectors: Vec<String>,
    pub modality: Option<String>,
    pub fragments: PragmaticModelFragments,
    pub upstream_model_id: String,
    pub metadata: BTreeMap<String, String>,
    pub notes: Vec<String>,
    pub dsel_tags: Vec<String>,
    pub pipeline_hints: Vec<String>,
    pub widened_models: Vec<PragmaticWidenedModelCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaticUnifiedPortConfig {
    pub agent_name: String,
    pub unified_port: u16,
}

pub const UNIFIED_PORT_AGENT_NAME: &str = "unified-port";
pub const DEFAULT_UNIFIED_PORT: u16 = 8888;

impl Default for PragmaticUnifiedPortConfig {
    fn default() -> Self {
        Self {
            agent_name: UNIFIED_PORT_AGENT_NAME.to_string(),
            unified_port: DEFAULT_UNIFIED_PORT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvBindingSource {
    KnownExact { canonical_key: String },
    KnownAlias { canonical_key: String },
    SearchApiKey { group: String, index: Option<u32> },
    GenericApiKey { prefix: String },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericApiKeyClassification {
    pub prefix: String,
    pub api_kind: ApiKind,
    pub family_hint: Option<ProviderFamily>,
    pub base_url_key: Option<String>,
    pub base_url: Option<String>,
    pub matched_rule_ids: Vec<String>,
    pub confidence: u8,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericApiModelsProbeClassification {
    pub api_kind: ApiKind,
    pub family_hint: Option<ProviderFamily>,
    pub confidence: u8,
    pub reason: String,
    pub matched_probe_url: Option<String>,
}

pub trait GenericApiModelsProbe {
    fn probe_models_capability(
        &self,
        base_url: &str,
        candidate_urls: &[String],
    ) -> Option<GenericApiModelsProbeClassification>;
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GenericApiModelsProbeCache {
    pub by_base_url: BTreeMap<String, GenericApiModelsProbeClassification>,
}

impl GenericApiModelsProbeCache {
    pub fn get(&self, base_url: &str) -> Option<&GenericApiModelsProbeClassification> {
        let key = normalize_probe_cache_key(base_url)?;
        self.by_base_url.get(&key)
    }

    pub fn insert(&mut self, base_url: &str, value: GenericApiModelsProbeClassification) {
        if let Some(key) = normalize_probe_cache_key(base_url) {
            self.by_base_url.insert(key, value);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchApiKeyEntry {
    pub key: String,
    pub group: String,
    pub index: Option<u32>,
    pub order: usize,
    pub auth_hint: SearchAuthHint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchApiKeyGroup {
    pub group: String,
    pub entries: Vec<SearchApiKeyEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedEnvEntry {
    pub key: String,
    pub value: String,
    pub role: Option<EnvVarRole>,
    pub source: EnvBindingSource,
    pub generic_api_key: Option<GenericApiKeyClassification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedEnvProfile {
    pub entries: Vec<NormalizedEnvEntry>,
    pub search_key_groups: Vec<SearchApiKeyGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelmuxMvpApiKeyBinding {
    pub env_key: String,
    pub prefix: Option<String>,
    pub api_kind: ApiKind,
    pub family_hint: Option<ProviderFamily>,
    pub base_url: Option<String>,
    pub confidence: u8,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelmuxMvpReadiness {
    pub ready: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelmuxMvpLifecycle {
    pub route: PragmaticUnifiedPortRoute,
    pub env_profile: NormalizedEnvProfile,
    pub search_enabled: bool,
    pub provider_api_keys: Vec<ModelmuxMvpApiKeyBinding>,
    pub exchange_api_keys: Vec<ModelmuxMvpApiKeyBinding>,
    pub unknown_api_keys: Vec<ModelmuxMvpApiKeyBinding>,
    pub selected_provider_api_key: Option<ModelmuxMvpApiKeyBinding>,
    pub readiness: ModelmuxMvpReadiness,
    pub lifecycle_tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelmuxMvpQuotaSelection {
    pub selected: Option<QuotaInventoryRouteCandidate>,
    pub candidates: Vec<QuotaInventoryRouteCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaDrainerSelectionPolicy {
    FreeFirstThenPaidFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaDrainerDryRunOptions {
    pub policy: QuotaDrainerSelectionPolicy,
    pub min_remaining_requests: u64,
    pub min_remaining_tokens: u64,
}

impl Default for QuotaDrainerDryRunOptions {
    fn default() -> Self {
        Self {
            policy: QuotaDrainerSelectionPolicy::FreeFirstThenPaidFallback,
            min_remaining_requests: 1,
            min_remaining_tokens: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaDrainerDryRunStepKind {
    Discover,
    Score,
    Select,
    Fallback,
    Review,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaDrainerDryRunStep {
    pub kind: QuotaDrainerDryRunStepKind,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaDrainerDryRunResult {
    pub policy: QuotaDrainerSelectionPolicy,
    pub route_key: String,
    pub selection: ModelmuxMvpQuotaSelection,
    pub selected: Option<QuotaInventoryRouteCandidate>,
    pub fallback_used: bool,
    pub free_candidates: usize,
    pub paid_candidates: usize,
    pub steps: Vec<QuotaDrainerDryRunStep>,
    pub ready: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaInventorySourceKind {
    LitebikeNative,
    LiteLlmCompatibleAdmin,
    CcSwitchSqlite,
    StaticMock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaInventorySlot {
    pub source_kind: QuotaInventorySourceKind,
    pub slot_id: String,
    pub model_id: String,
    pub fragments: PragmaticModelFragments,
    pub family_hint: Option<ProviderFamily>,
    pub base_url: Option<String>,
    pub selectors: Vec<String>,
    pub free: bool,
    pub enabled: bool,
    pub healthy: bool,
    pub remaining_requests: Option<u64>,
    pub remaining_tokens: Option<u64>,
    pub metadata: BTreeMap<String, String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteLlmCompatibleQuotaInventoryRecord {
    pub slot_id: String,
    pub model_id: String,
    pub api_base: Option<String>,
    pub enabled: bool,
    pub healthy: bool,
    pub remaining_requests: Option<u64>,
    pub remaining_tokens: Option<u64>,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CcSwitchSqliteQuotaInventoryRow {
    pub slot_id: String,
    pub provider_hint: Option<String>,
    pub model_id: String,
    pub base_url: Option<String>,
    pub state: Option<String>,
    pub enabled: bool,
    pub remaining_requests: Option<i64>,
    pub remaining_tokens: Option<i64>,
    pub selectors: Vec<String>,
    pub metadata: BTreeMap<String, String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockQuotaInventoryRecord {
    pub slot_id: String,
    pub model_ref_or_id: String,
    pub base_url: Option<String>,
    pub enabled: bool,
    pub healthy: bool,
    pub free: bool,
    pub selectors: Vec<String>,
    pub remaining_requests: Option<u64>,
    pub remaining_tokens: Option<u64>,
    pub metadata: BTreeMap<String, String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaInventoryAdapterError {
    pub source_kind: QuotaInventorySourceKind,
    pub message: String,
}

pub trait QuotaInventoryAdapter {
    fn source_kind(&self) -> QuotaInventorySourceKind;
    fn load_quota_inventory(&self) -> Result<Vec<QuotaInventorySlot>, QuotaInventoryAdapterError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StaticMockQuotaInventoryAdapter {
    pub records: Vec<MockQuotaInventoryRecord>,
}

impl QuotaInventoryAdapter for StaticMockQuotaInventoryAdapter {
    fn source_kind(&self) -> QuotaInventorySourceKind {
        QuotaInventorySourceKind::StaticMock
    }

    fn load_quota_inventory(&self) -> Result<Vec<QuotaInventorySlot>, QuotaInventoryAdapterError> {
        Ok(normalize_mock_quota_inventory(&self.records))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaInventoryRouteCandidate {
    pub slot: QuotaInventorySlot,
    pub score: i64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
struct KnownBinding {
    canonical_key: String,
    role: EnvVarRole,
    alias: bool,
}

#[derive(Debug, Clone)]
struct EnvRuleHint {
    id: String,
    family_hint: ProviderFamily,
    host_contains_any: Vec<String>,
    confidence: u8,
}

fn normalize_key(key: &str) -> String {
    key.trim().to_ascii_uppercase()
}

fn lower_host_from_url(base_url: &str) -> Option<String> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let without_scheme = match trimmed.split_once("://") {
        Some((_, rest)) => rest,
        None => trimmed,
    };
    let host = without_scheme.split('/').next().unwrap_or("").trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

fn split_host_port_token(host_token: &str) -> (String, Option<u16>) {
    let trimmed = host_token.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }

    let authority = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))
        .unwrap_or(trimmed);

    let authority_no_path = authority.split('/').next().unwrap_or(authority);
    if let Some((host, port_str)) = authority_no_path.rsplit_once(':') {
        if !host.is_empty() && port_str.chars().all(|c| c.is_ascii_digit()) {
            return (host.to_ascii_lowercase(), port_str.parse::<u16>().ok());
        }
    }
    (authority_no_path.to_ascii_lowercase(), None)
}

fn default_scheme_for_host(host: &str, port: Option<u16>, options: &[String]) -> &'static str {
    let lower_opts: Vec<String> = options.iter().map(|s| s.to_ascii_lowercase()).collect();
    if lower_opts.iter().any(|o| o == "http" || o == "insecure") {
        return "http";
    }
    if lower_opts.iter().any(|o| o == "https" || o == "tls") {
        return "https";
    }

    let is_local =
        host == "localhost" || host == "127.0.0.1" || host == "::1" || host.ends_with(".local");

    if is_local {
        return "http";
    }

    match port {
        Some(80) | Some(8080) | Some(8888) | Some(11434) => "http",
        _ => "https",
    }
}

fn base_url_from_host_token_and_options(host_token: &str, options: &[String]) -> String {
    let trimmed = host_token.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }
    let (host, port) = split_host_port_token(trimmed);
    let scheme = default_scheme_for_host(&host, port, options);
    format!("{scheme}://{trimmed}")
}

fn known_bindings_index() -> HashMap<String, KnownBinding> {
    let matrix = facade_v1_route_matrix();
    let mut out = HashMap::<String, KnownBinding>::new();

    for provider in matrix.providers {
        for binding in provider.env_bindings {
            let canonical = normalize_key(&binding.key);
            out.entry(canonical.clone()).or_insert(KnownBinding {
                canonical_key: binding.key.clone(),
                role: binding.role,
                alias: false,
            });

            for alias in binding.aliases {
                let alias_norm = normalize_key(&alias);
                out.entry(alias_norm).or_insert(KnownBinding {
                    canonical_key: binding.key.clone(),
                    role: binding.role,
                    alias: true,
                });
            }
        }
    }

    out
}

fn env_rule_hints() -> Vec<EnvRuleHint> {
    let matrix = facade_v1_route_matrix();
    matrix
        .env_rules
        .into_iter()
        .map(|r| EnvRuleHint {
            id: r.id,
            family_hint: r.family_hint,
            host_contains_any: r
                .host_contains_any
                .into_iter()
                .map(|s| s.to_ascii_lowercase())
                .collect(),
            confidence: r.confidence,
        })
        .collect()
}

fn parse_search_api_key_suffix(key_norm: &str) -> Option<(String, Option<u32>)> {
    if let Some(prefix) = key_norm.strip_suffix("_SEARCH_API_KEY") {
        if !prefix.is_empty() {
            return Some((prefix.to_string(), None));
        }
    }

    let marker = "_SEARCH_API_KEY_";
    let idx = key_norm.rfind(marker)?;
    let (prefix, rest) = key_norm.split_at(idx);
    if prefix.is_empty() {
        return None;
    }
    let num_str = &rest[marker.len()..];
    if num_str.is_empty() || !num_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let parsed = num_str.parse::<u32>().ok()?;
    Some((prefix.to_string(), Some(parsed)))
}

fn infer_search_auth_hint(group: &str) -> SearchAuthHint {
    match group {
        "BRAVE" | "BRAVESEARCH" => SearchAuthHint::HeaderXSubscriptionToken,
        "SERPAPI" => SearchAuthHint::QueryApiKey,
        "TAVILY" | "EXA" | "SEARXNG" | "SERPER" => SearchAuthHint::Bearer,
        _ => SearchAuthHint::Unknown,
    }
}

fn is_generic_api_key(key_norm: &str) -> Option<String> {
    let prefix = key_norm.strip_suffix("_API_KEY")?;
    if prefix.is_empty() {
        return None;
    }
    Some(prefix.to_string())
}

fn candidate_base_url_keys(prefix: &str) -> Vec<String> {
    let mut out = vec![format!("{prefix}_BASE_URL")];
    match prefix {
        "GEMINI" => out.push("GOOGLE_GEMINI_BASE_URL".to_string()),
        "GOOGLE" => out.push("GOOGLE_GEMINI_BASE_URL".to_string()),
        "ANTHROPIC" => out.push("ANTHROPIC_BASE_URL".to_string()),
        "OPENAI" => out.push("OPENAI_BASE_URL".to_string()),
        _ => {}
    }
    out
}

fn first_present_base_url(
    prefix: &str,
    env_map: &BTreeMap<String, String>,
) -> (Option<String>, Option<String>) {
    for key in candidate_base_url_keys(prefix) {
        if let Some(v) = env_map.get(&key) {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                return (Some(key), Some(trimmed.to_string()));
            }
        }
    }
    (None, None)
}

fn classify_by_prefix(prefix: &str) -> Option<(ApiKind, Option<ProviderFamily>, u8, &'static str)> {
    match prefix {
        // Explicit model providers / common OpenAI-compatible providers
        "OPENAI" => Some((
            ApiKind::ModelProvider,
            Some(ProviderFamily::OpenAiCompatible),
            95,
            "known provider prefix",
        )),
        "ANTHROPIC" => Some((
            ApiKind::ModelProvider,
            Some(ProviderFamily::AnthropicCompatible),
            95,
            "known provider prefix",
        )),
        "GEMINI" | "GOOGLE" => Some((
            ApiKind::ModelProvider,
            Some(ProviderFamily::GeminiNative),
            90,
            "known provider prefix",
        )),
        "OPENROUTER" | "DEEPSEEK" | "MOONSHOT" | "MINIMAX" | "MISTRAL" | "GROQ" | "TOGETHER"
        | "PERPLEXITY" | "CEREBRAS" | "COHERE" | "XAI" | "NVIDIA" | "ZAI" | "GLM" | "KILO"
        | "KILOAI" | "KIMI" => Some((
            ApiKind::ModelProvider,
            Some(ProviderFamily::OpenAiCompatible),
            80,
            "known model-provider prefix",
        )),

        // Common exchange/API trading surfaces
        "BINANCE" | "BYBIT" | "KRAKEN" | "OKX" | "HUOBI" | "HTX" | "COINBASE" | "KUCOIN"
        | "BITGET" | "MEXC" | "GATEIO" | "GATE" | "HYPERLIQUID" => {
            Some((ApiKind::Exchange, None, 90, "known exchange prefix"))
        }
        _ => None,
    }
}

fn classify_by_host(
    base_url: Option<&str>,
    rules: &[EnvRuleHint],
) -> Option<(
    ApiKind,
    Option<ProviderFamily>,
    Vec<String>,
    u8,
    &'static str,
)> {
    let Some(url) = base_url else {
        return None;
    };
    let hay = url.to_ascii_lowercase();

    let mut matched_rules: Vec<&EnvRuleHint> = rules
        .iter()
        .filter(|rule| {
            rule.host_contains_any
                .iter()
                .any(|needle| !needle.is_empty() && hay.contains(needle))
        })
        .collect();
    matched_rules.sort_by(|a, b| b.confidence.cmp(&a.confidence));

    if let Some(best) = matched_rules.first() {
        let ids = matched_rules.iter().map(|r| r.id.clone()).collect();
        return Some((
            ApiKind::ModelProvider,
            Some(best.family_hint),
            ids,
            best.confidence,
            "matched literbike env recognition host rule",
        ));
    }

    let exchange_host_hints = [
        "binance",
        "bybit",
        "kraken",
        "okx",
        "coinbase",
        "kucoin",
        "bitget",
        "mexc",
        "gate.io",
        "hyperliquid",
    ];

    if exchange_host_hints
        .iter()
        .any(|needle| hay.contains(needle))
    {
        return Some((
            ApiKind::Exchange,
            None,
            vec![],
            80,
            "matched exchange host heuristic",
        ));
    }

    None
}

fn classify_generic_api_key_no_network(
    prefix: &str,
    env_map: &BTreeMap<String, String>,
    rules: &[EnvRuleHint],
) -> GenericApiKeyClassification {
    let (base_url_key, base_url) = first_present_base_url(prefix, env_map);

    if let Some((kind, family_hint, matched_rule_ids, confidence, reason)) =
        classify_by_host(base_url.as_deref(), rules)
    {
        return GenericApiKeyClassification {
            prefix: prefix.to_string(),
            api_kind: kind,
            family_hint,
            base_url_key,
            base_url,
            matched_rule_ids,
            confidence,
            reason: reason.to_string(),
        };
    }

    if let Some((kind, family_hint, confidence, reason)) = classify_by_prefix(prefix) {
        return GenericApiKeyClassification {
            prefix: prefix.to_string(),
            api_kind: kind,
            family_hint,
            base_url_key,
            base_url,
            matched_rule_ids: vec![],
            confidence,
            reason: reason.to_string(),
        };
    }

    GenericApiKeyClassification {
        prefix: prefix.to_string(),
        api_kind: ApiKind::Unknown,
        family_hint: None,
        base_url_key,
        base_url,
        matched_rule_ids: vec![],
        confidence: 20,
        reason: "no-network classification unresolved".to_string(),
    }
}

fn normalize_probe_cache_key(base_url: &str) -> Option<String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn generic_api_models_probe_urls(base_url: &str) -> Vec<String> {
    let inferred = infer_hostname_access_options(base_url);
    if !inferred.model_probe_urls.is_empty() {
        return inferred.model_probe_urls;
    }
    build_model_probe_urls(base_url)
}

fn apply_generic_api_probe_classification(
    mut classification: GenericApiKeyClassification,
    probe: &GenericApiModelsProbeClassification,
    source: &str,
) -> GenericApiKeyClassification {
    classification.api_kind = probe.api_kind;
    classification.family_hint = probe.family_hint;
    classification.confidence = probe.confidence;
    classification.matched_rule_ids.clear();
    classification.reason = format!("{source}: {}", probe.reason);
    classification
}

fn classify_generic_api_key_with_optional_probe(
    prefix: &str,
    env_map: &BTreeMap<String, String>,
    rules: &[EnvRuleHint],
    probe: Option<&dyn GenericApiModelsProbe>,
    probe_cache: Option<&mut GenericApiModelsProbeCache>,
) -> GenericApiKeyClassification {
    let classification = classify_generic_api_key_no_network(prefix, env_map, rules);
    if classification.api_kind != ApiKind::Unknown {
        return classification;
    }

    let Some(base_url) = classification.base_url.as_deref() else {
        return classification;
    };

    if let Some(cached) = probe_cache
        .as_ref()
        .and_then(|cache| cache.get(base_url))
        .cloned()
    {
        return apply_generic_api_probe_classification(
            classification,
            &cached,
            "models probe cache",
        );
    }

    let Some(prober) = probe else {
        return classification;
    };

    let candidate_urls = generic_api_models_probe_urls(base_url);
    if candidate_urls.is_empty() {
        return classification;
    }

    let Some(probe_result) = prober.probe_models_capability(base_url, &candidate_urls) else {
        return classification;
    };

    if let Some(cache) = probe_cache {
        cache.insert(base_url, probe_result.clone());
    }

    apply_generic_api_probe_classification(classification, &probe_result, "models probe")
}

pub fn build_model_probe_urls(base_url: &str) -> Vec<String> {
    // cc-switch-compatible probing strategy: try both /v1/models and /models.
    // This intentionally deduplicates while preserving order.
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return vec![];
    }

    let primary = if trimmed.ends_with("/v1") {
        format!("{trimmed}/models")
    } else {
        format!("{trimmed}/v1/models")
    };

    let fallback = if trimmed.ends_with("/v1") {
        format!("{trimmed}/v1/models")
    } else {
        format!("{trimmed}/models")
    };

    let mut seen = HashSet::new();
    vec![primary, fallback]
        .into_iter()
        .filter(|url| seen.insert(url.clone()))
        .collect()
}

fn ensure_api_version_base(base_url: &str, version_path: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return version_path.to_string();
    }

    if trimmed.ends_with(version_path) {
        return trimmed.to_string();
    }

    // If the caller already passed a deeper versioned path, trust it.
    if trimmed.contains(&format!("{}/", version_path)) {
        return trimmed.to_string();
    }

    format!("{trimmed}{version_path}")
}

fn build_gemini_model_probe_urls(base_url: &str) -> Vec<String> {
    let base = ensure_api_version_base(base_url, "/v1beta");
    vec![format!("{base}/models")]
}

fn push_mapping(mappings: &mut Vec<MuxUrlMapping>, route: MuxRouteKind, method: &str, url: String) {
    mappings.push(MuxUrlMapping {
        route,
        method: method.to_string(),
        url,
    });
}

fn infer_format_from_family_and_kind(
    api_kind: ApiKind,
    family_hint: Option<ProviderFamily>,
) -> MuxPragmaticFormat {
    if api_kind == ApiKind::Exchange {
        return MuxPragmaticFormat::ExchangeRest;
    }

    match family_hint {
        Some(ProviderFamily::AnthropicCompatible) => MuxPragmaticFormat::AnthropicCompatible,
        Some(ProviderFamily::GeminiNative) => MuxPragmaticFormat::GeminiNative,
        Some(ProviderFamily::ControlPlane) => MuxPragmaticFormat::ControlPlane,
        Some(ProviderFamily::OpenAiCompatible)
        | Some(ProviderFamily::OpenRouter)
        | Some(ProviderFamily::AzureOpenAi)
        | Some(ProviderFamily::OpenCodeZen)
        | Some(ProviderFamily::Ollama) => MuxPragmaticFormat::OpenAiCompatible,
        Some(ProviderFamily::Unknown) => MuxPragmaticFormat::Unknown,
        None => match api_kind {
            ApiKind::ModelProvider => MuxPragmaticFormat::OpenAiCompatible,
            ApiKind::Exchange => MuxPragmaticFormat::ExchangeRest,
            ApiKind::Unknown => MuxPragmaticFormat::Unknown,
        },
    }
}

fn auth_options_for_format(format: MuxPragmaticFormat) -> Vec<MuxAuthOption> {
    match format {
        MuxPragmaticFormat::OpenAiCompatible => vec![
            MuxAuthOption::BearerAuthorization,
            MuxAuthOption::ApiKeyHeader,
        ],
        MuxPragmaticFormat::AnthropicCompatible => vec![
            MuxAuthOption::AnthropicXApiKey,
            MuxAuthOption::BearerAuthorization,
        ],
        MuxPragmaticFormat::GeminiNative => vec![
            MuxAuthOption::GoogleApiKeyHeader,
            MuxAuthOption::GoogleApiKeyQuery,
        ],
        MuxPragmaticFormat::ControlPlane => vec![
            MuxAuthOption::BearerAuthorization,
            MuxAuthOption::NoneOrSession,
        ],
        MuxPragmaticFormat::ExchangeRest => vec![
            MuxAuthOption::ApiKeyHeader,
            MuxAuthOption::BearerAuthorization,
        ],
        MuxPragmaticFormat::Unknown => vec![],
    }
}

fn build_pragmatic_url_mappings(base_url: &str, format: MuxPragmaticFormat) -> Vec<MuxUrlMapping> {
    let mut mappings = Vec::<MuxUrlMapping>::new();
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return mappings;
    }

    push_mapping(
        &mut mappings,
        MuxRouteKind::Health,
        "GET",
        format!("{trimmed}/health"),
    );

    match format {
        MuxPragmaticFormat::OpenAiCompatible => {
            let base = ensure_api_version_base(trimmed, "/v1");
            push_mapping(
                &mut mappings,
                MuxRouteKind::ModelsList,
                "GET",
                format!("{base}/models"),
            );
            push_mapping(
                &mut mappings,
                MuxRouteKind::ChatCompletions,
                "POST",
                format!("{base}/chat/completions"),
            );
            push_mapping(
                &mut mappings,
                MuxRouteKind::Responses,
                "POST",
                format!("{base}/responses"),
            );
            push_mapping(
                &mut mappings,
                MuxRouteKind::Embeddings,
                "POST",
                format!("{base}/embeddings"),
            );
        }
        MuxPragmaticFormat::AnthropicCompatible => {
            let base = ensure_api_version_base(trimmed, "/v1");
            // Some Anthropic-compatible gateways expose /v1/models; keep as pragmatic probe.
            push_mapping(
                &mut mappings,
                MuxRouteKind::ModelsList,
                "GET",
                format!("{base}/models"),
            );
            push_mapping(
                &mut mappings,
                MuxRouteKind::AnthropicMessages,
                "POST",
                format!("{base}/messages"),
            );
            push_mapping(
                &mut mappings,
                MuxRouteKind::AnthropicCountTokens,
                "POST",
                format!("{base}/messages/count_tokens"),
            );
        }
        MuxPragmaticFormat::GeminiNative => {
            let base = ensure_api_version_base(trimmed, "/v1beta");
            push_mapping(
                &mut mappings,
                MuxRouteKind::ModelsList,
                "GET",
                format!("{base}/models"),
            );
            push_mapping(
                &mut mappings,
                MuxRouteKind::GeminiGenerateContent,
                "POST",
                format!("{base}/models/{{model}}:generateContent"),
            );
            push_mapping(
                &mut mappings,
                MuxRouteKind::GeminiStreamGenerateContent,
                "POST",
                format!("{base}/models/{{model}}:streamGenerateContent?alt=sse"),
            );
            push_mapping(
                &mut mappings,
                MuxRouteKind::GeminiCountTokens,
                "POST",
                format!("{base}/models/{{model}}:countTokens"),
            );
        }
        MuxPragmaticFormat::ControlPlane => {
            push_mapping(
                &mut mappings,
                MuxRouteKind::ModelsList,
                "GET",
                format!("{trimmed}/v1/models"),
            );
        }
        MuxPragmaticFormat::ExchangeRest | MuxPragmaticFormat::Unknown => {}
    }

    mappings
}

pub fn infer_hostname_access_options(base_url: &str) -> HostnameAccessOptions {
    let rules = env_rule_hints();
    let host = lower_host_from_url(base_url);

    let (api_kind, family_hint, matched_rule_ids, confidence, reason) = if let Some((
        kind,
        family,
        rule_ids,
        conf,
        why,
    )) =
        classify_by_host(host.as_deref(), &rules)
    {
        (kind, family, rule_ids, conf, why.to_string())
    } else {
        let host_str = host.as_deref().unwrap_or("");
        let (kind, family, conf, why) = if host_str.contains("binance")
            || host_str.contains("bybit")
            || host_str.contains("kraken")
            || host_str.contains("okx")
            || host_str.contains("coinbase")
            || host_str.contains("hyperliquid")
        {
            (
                ApiKind::Exchange,
                None,
                80,
                "matched exchange host heuristic",
            )
        } else {
            (
                ApiKind::Unknown,
                None,
                20,
                "hostname classification unresolved",
            )
        };
        (kind, family, vec![], conf, why.to_string())
    };

    let format = infer_format_from_family_and_kind(api_kind, family_hint);
    let supports_models_probe = matches!(
        format,
        MuxPragmaticFormat::OpenAiCompatible
            | MuxPragmaticFormat::AnthropicCompatible
            | MuxPragmaticFormat::GeminiNative
    );
    let model_probe_urls = match format {
        MuxPragmaticFormat::GeminiNative => build_gemini_model_probe_urls(base_url),
        MuxPragmaticFormat::OpenAiCompatible | MuxPragmaticFormat::AnthropicCompatible => {
            build_model_probe_urls(base_url)
        }
        _ => vec![],
    };
    let url_mappings = build_pragmatic_url_mappings(base_url, format);

    HostnameAccessOptions {
        base_url: base_url.trim().to_string(),
        hostname: host,
        api_kind,
        family_hint,
        format,
        auth_options: auth_options_for_format(format),
        supports_models_probe,
        model_probe_urls,
        url_mappings,
        matched_rule_ids,
        confidence,
        reason,
    }
}

pub fn parse_pragmatic_model_ref(input: &str) -> Result<PragmaticModelRef, PragmaticModelRefError> {
    let raw = input.trim();
    if raw.starts_with("/{") {
        return parse_pragmatic_model_ref_host_block(raw);
    }
    if raw.starts_with('/') {
        return parse_pragmatic_model_ref_selector_shorthand(raw);
    }
    Err(PragmaticModelRefError::InvalidPrefix)
}

fn parse_pragmatic_model_ref_host_block(
    raw: &str,
) -> Result<PragmaticModelRef, PragmaticModelRefError> {
    if !raw.starts_with("/{") {
        return Err(PragmaticModelRefError::InvalidPrefix);
    }

    let close_idx = raw
        .find("}/")
        .ok_or(PragmaticModelRefError::MissingHostBlockClose)?;

    let inside = &raw[2..close_idx];
    let mut tokens: Vec<String> = inside
        .split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect();

    if tokens.is_empty() {
        return Err(PragmaticModelRefError::EmptyHostToken);
    }

    let host_token = tokens.remove(0);
    if host_token.trim().is_empty() {
        return Err(PragmaticModelRefError::EmptyHostToken);
    }

    let upstream_model_id = raw[(close_idx + 2)..].trim().to_string();
    if upstream_model_id.is_empty() {
        return Err(PragmaticModelRefError::MissingModelId);
    }

    let (option_specs, modality, metadata, notes) = parse_pragmatic_option_tokens(&tokens);
    let base_url = base_url_from_host_token_and_options(&host_token, &tokens);
    let access = infer_hostname_access_options(&base_url);
    let (host, port) = split_host_port_token(&host_token);

    Ok(PragmaticModelRef {
        raw: raw.to_string(),
        host_token,
        host,
        port,
        base_url,
        option_tokens: tokens,
        option_specs,
        selector_prefixes: vec![],
        modality,
        metadata,
        notes,
        upstream_model_id,
        access,
    })
}

fn parse_pragmatic_model_ref_selector_shorthand(
    raw: &str,
) -> Result<PragmaticModelRef, PragmaticModelRefError> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('/') || trimmed.starts_with("/{") {
        return Err(PragmaticModelRefError::InvalidPrefix);
    }

    let without_leading = trimmed.trim_start_matches('/');
    let (selector, model_rest) = without_leading
        .split_once('/')
        .ok_or(PragmaticModelRefError::MissingModelId)?;

    let selector = selector.trim();
    let model_rest = model_rest.trim();
    if selector.is_empty() {
        return Err(PragmaticModelRefError::InvalidPrefix);
    }
    if model_rest.is_empty() {
        return Err(PragmaticModelRefError::MissingModelId);
    }

    let selector_norm = selector.to_ascii_lowercase();
    let option_tokens = vec![format!("modality/{selector_norm}")];
    let (option_specs, modality, metadata, notes) = parse_pragmatic_option_tokens(&option_tokens);
    let access = infer_hostname_access_options("");

    Ok(PragmaticModelRef {
        raw: trimmed.to_string(),
        host_token: String::new(),
        host: String::new(),
        port: None,
        base_url: String::new(),
        option_tokens,
        option_specs,
        selector_prefixes: vec![selector_norm],
        modality,
        metadata,
        notes,
        upstream_model_id: model_rest.to_string(),
        access,
    })
}

fn parse_pragmatic_option_tokens(
    tokens: &[String],
) -> (
    Vec<PragmaticOptionSpec>,
    Option<String>,
    BTreeMap<String, String>,
    Vec<String>,
) {
    let mut specs = Vec::with_capacity(tokens.len());
    let mut modality: Option<String> = None;
    let mut metadata = BTreeMap::<String, String>::new();
    let mut notes = Vec::<String>::new();

    for token in tokens {
        let t = token.trim();
        if t.is_empty() {
            continue;
        }

        if let Some(note) = t
            .strip_prefix("note=")
            .or_else(|| t.strip_prefix("note:"))
            .map(str::trim)
        {
            if !note.is_empty() {
                notes.push(note.to_string());
            }
            continue;
        }

        if let Some(rest) = t.strip_prefix("meta:") {
            if let Some((k, v)) = rest.split_once('=') {
                let key = k.trim();
                let value = v.trim();
                if !key.is_empty() {
                    metadata.insert(key.to_string(), value.to_string());
                    continue;
                }
            }
        }

        if let Some((k, v)) = t.split_once('=') {
            let key = k.trim();
            let value = v.trim();
            if !key.is_empty() {
                if key.eq_ignore_ascii_case("note") {
                    if !value.is_empty() {
                        notes.push(value.to_string());
                    }
                } else {
                    metadata.insert(key.to_string(), value.to_string());
                }
                continue;
            }
        }

        if let Some((k, v)) = t.split_once('/') {
            let key = k.trim();
            let value = v.trim();
            if !key.is_empty() && !value.is_empty() {
                if key.eq_ignore_ascii_case("modality") && modality.is_none() {
                    modality = Some(value.to_ascii_lowercase());
                }
                specs.push(PragmaticOptionSpec {
                    raw: t.to_string(),
                    key: key.to_ascii_lowercase(),
                    value: Some(value.to_string()),
                });
                continue;
            }
        }

        specs.push(PragmaticOptionSpec {
            raw: t.to_string(),
            key: t.to_ascii_lowercase(),
            value: None,
        });
    }

    (specs, modality, metadata, notes)
}

pub fn format_pragmatic_model_ref_access_line(
    input: &str,
) -> Result<String, PragmaticModelRefError> {
    let parsed = parse_pragmatic_model_ref(input)?;
    let opts_line = format_hostname_access_options_line(&parsed.access);
    let options = parsed.option_tokens.join("|");
    let selectors = parsed.selector_prefixes.join("|");
    let modality = parsed.modality.clone().unwrap_or_default();
    let metadata = parsed
        .metadata
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("|");
    let notes = parsed.notes.join("|");
    Ok(format!(
        "host_token={};selectors={selectors};modality={modality};options={options};metadata={metadata};notes={notes};model={};{}",
        parsed.host_token, parsed.upstream_model_id, opts_line
    ))
}

pub fn resolve_pragmatic_unified_port_route(
    input: &str,
) -> Result<PragmaticUnifiedPortRoute, PragmaticModelRefError> {
    let parsed = parse_pragmatic_model_ref(input)?;
    Ok(resolve_pragmatic_unified_port_route_from_parsed(&parsed))
}

pub fn resolve_pragmatic_unified_port_route_with_config(
    input: &str,
    config: &PragmaticUnifiedPortConfig,
) -> Result<PragmaticUnifiedPortRoute, PragmaticModelRefError> {
    let parsed = parse_pragmatic_model_ref(input)?;
    Ok(resolve_pragmatic_unified_port_route_from_parsed_with_config(&parsed, config))
}

pub fn resolve_pragmatic_unified_port_route_from_parsed(
    parsed: &PragmaticModelRef,
) -> PragmaticUnifiedPortRoute {
    resolve_pragmatic_unified_port_route_from_parsed_with_config(
        parsed,
        &PragmaticUnifiedPortConfig::default(),
    )
}

pub fn resolve_pragmatic_unified_port_route_from_parsed_with_config(
    parsed: &PragmaticModelRef,
    config: &PragmaticUnifiedPortConfig,
) -> PragmaticUnifiedPortRoute {
    let fragments = split_pragmatic_model_fragments(&parsed.upstream_model_id);
    let family_hint =
        infer_provider_family_from_fragments(parsed, &fragments).or(parsed.access.family_hint);
    let pipeline_hints = infer_unified_port_pipeline_hints(parsed, &fragments, family_hint);
    let widened_models = infer_widened_model_candidates(parsed, &fragments, family_hint);
    let host_scope = if parsed.host_token.trim().is_empty() {
        None
    } else {
        Some(parsed.host_token.clone())
    };
    let selectors = parsed.selector_prefixes.clone();
    let modality = parsed.modality.clone();

    let mut dsel_tags = vec![
        format!("agent/{}", config.agent_name),
        format!("port/{}", config.unified_port),
    ];
    for selector in &selectors {
        dsel_tags.push(format!("selector/{selector}"));
    }
    if let Some(m) = modality.as_deref() {
        dsel_tags.push(format!("modality/{m}"));
    }
    if let Some(provider) = fragments.provider_fragment.as_deref() {
        dsel_tags.push(format!("provider/{provider}"));
    }
    if let Some(host) = parsed.access.hostname.as_deref() {
        dsel_tags.push(format!("host/{host}"));
    }
    if parsed.access.format != MuxPragmaticFormat::Unknown {
        dsel_tags.push(format!("pipeline/{}", fmt_format(parsed.access.format)));
    }
    for hint in &pipeline_hints {
        dsel_tags.push(format!("pipeline-hint/{hint}"));
    }
    for spec in &parsed.option_specs {
        if spec.key == "modality" {
            continue;
        }
        match spec.value.as_deref() {
            Some(v) => dsel_tags.push(format!("opt/{}/{}", spec.key, v)),
            None => dsel_tags.push(format!("opt/{}", spec.key)),
        }
    }
    dsel_tags.sort();
    dsel_tags.dedup();

    let host_part = host_scope.clone().unwrap_or_else(|| "default".to_string());
    let selector_part = if selectors.is_empty() {
        "default".to_string()
    } else {
        selectors.join("+")
    };
    let route_key = format!(
        "{}:{host_part}:{selector_part}:{}",
        config.agent_name, parsed.upstream_model_id
    );

    PragmaticUnifiedPortRoute {
        agent_name: config.agent_name.clone(),
        unified_port: config.unified_port,
        route_key,
        host_scope,
        selectors,
        modality,
        fragments,
        upstream_model_id: parsed.upstream_model_id.clone(),
        metadata: parsed.metadata.clone(),
        notes: parsed.notes.clone(),
        dsel_tags,
        pipeline_hints,
        widened_models,
    }
}

pub fn format_pragmatic_unified_port_route_line(
    input: &str,
) -> Result<String, PragmaticModelRefError> {
    let route = resolve_pragmatic_unified_port_route(input)?;
    format_pragmatic_unified_port_route_line_from_route(&route)
}

pub fn format_pragmatic_unified_port_route_line_with_config(
    input: &str,
    config: &PragmaticUnifiedPortConfig,
) -> Result<String, PragmaticModelRefError> {
    let route = resolve_pragmatic_unified_port_route_with_config(input, config)?;
    format_pragmatic_unified_port_route_line_from_route(&route)
}

fn format_pragmatic_unified_port_route_line_from_route(
    route: &PragmaticUnifiedPortRoute,
) -> Result<String, PragmaticModelRefError> {
    let selectors = route.selectors.join("|");
    let modality = route.modality.clone().unwrap_or_default();
    let pipes = route.pipeline_hints.join("|");
    let widen = route
        .widened_models
        .iter()
        .map(|w| format!("{}:{}", w.boundary, w.model))
        .collect::<Vec<_>>()
        .join("|");
    Ok(format!(
        "agent={};port={};host_scope={};selectors={selectors};modality={modality};route_key={};pipelines={pipes};model={};widened={widen}",
        route.agent_name,
        route.unified_port,
        route.host_scope.as_deref().unwrap_or(""),
        route.route_key,
        route.upstream_model_id
    ))
}

pub fn run_modelmux_mvp_lifecycle<I>(
    env_pairs: I,
    model_ref: &str,
) -> Result<ModelmuxMvpLifecycle, PragmaticModelRefError>
where
    I: IntoIterator<Item = (String, String)>,
{
    run_modelmux_mvp_lifecycle_with_options(env_pairs, model_ref, None, None, None)
}

pub fn run_modelmux_mvp_lifecycle_with_options<I>(
    env_pairs: I,
    model_ref: &str,
    unified_port_config: Option<&PragmaticUnifiedPortConfig>,
    generic_api_probe: Option<&dyn GenericApiModelsProbe>,
    generic_api_probe_cache: Option<&mut GenericApiModelsProbeCache>,
) -> Result<ModelmuxMvpLifecycle, PragmaticModelRefError>
where
    I: IntoIterator<Item = (String, String)>,
{
    let env_profile = normalize_env_pairs_with_generic_api_probe(
        env_pairs,
        generic_api_probe,
        generic_api_probe_cache,
    );
    let route = match unified_port_config {
        Some(cfg) => resolve_pragmatic_unified_port_route_with_config(model_ref, cfg)?,
        None => resolve_pragmatic_unified_port_route(model_ref)?,
    };

    let (provider_api_keys, exchange_api_keys, unknown_api_keys) =
        collect_modelmux_mvp_api_key_bindings(&env_profile);
    let selected_provider_api_key = select_modelmux_mvp_provider_key(&route, &provider_api_keys);
    let readiness = infer_modelmux_mvp_readiness(
        &route,
        selected_provider_api_key.as_ref(),
        &exchange_api_keys,
        &env_profile,
    );
    let search_enabled = !env_profile.search_key_groups.is_empty();
    let lifecycle_tags = build_modelmux_mvp_lifecycle_tags(
        &route,
        search_enabled,
        &provider_api_keys,
        &exchange_api_keys,
        &unknown_api_keys,
        selected_provider_api_key.as_ref(),
        &readiness,
    );

    Ok(ModelmuxMvpLifecycle {
        route,
        env_profile,
        search_enabled,
        provider_api_keys,
        exchange_api_keys,
        unknown_api_keys,
        selected_provider_api_key,
        readiness,
        lifecycle_tags,
    })
}

pub fn format_modelmux_mvp_lifecycle_line(lifecycle: &ModelmuxMvpLifecycle) -> String {
    let selected_key = lifecycle
        .selected_provider_api_key
        .as_ref()
        .map(|b| b.env_key.as_str())
        .unwrap_or("");
    let provider_count = lifecycle.provider_api_keys.len();
    let exchange_count = lifecycle.exchange_api_keys.len();
    let unknown_count = lifecycle.unknown_api_keys.len();
    let boundaries = lifecycle
        .route
        .widened_models
        .iter()
        .map(|w| w.boundary.clone())
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "ready={};reason={};route_key={};search={};provider_keys={provider_count};exchange_keys={exchange_count};unknown_keys={unknown_count};selected_key={selected_key};model={};widened_boundaries={boundaries}",
        lifecycle.readiness.ready,
        lifecycle.readiness.reason,
        lifecycle.route.route_key,
        lifecycle.search_enabled,
        lifecycle.route.upstream_model_id
    )
}
fn infer_provider_family_from_provider_fragment(provider_fragment: &str) -> Option<ProviderFamily> {
    // Rule: Any provider with _API_KEY is valid, default to OpenAiCompatible
    match provider_fragment.to_ascii_lowercase().as_str() {
        "anthropic" | "claude" => Some(ProviderFamily::AnthropicCompatible),
        "google" | "gemini" => Some(ProviderFamily::GeminiNative),
        "ollama" => Some(ProviderFamily::Ollama),
        _ => Some(ProviderFamily::OpenAiCompatible),  // Default for any XXXXX_API_KEY provider
    }
}

fn normalize_quota_selector_tokens(
    selector_tokens: &[String],
    free_hint: bool,
) -> (Vec<String>, bool) {
    let mut selectors = Vec::new();
    let mut free = free_hint;

    for token in selector_tokens {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower == "free" {
            free = true;
            selectors.push("free".to_string());
            continue;
        }
        if let Some(value) = lower
            .strip_prefix("modality/")
            .or_else(|| lower.strip_prefix("modality="))
        {
            let value = value.trim();
            if !value.is_empty() {
                if value == "free" {
                    free = true;
                }
                selectors.push(value.to_string());
            }
            continue;
        }
        if let Some(value) = lower.strip_prefix("selector/") {
            let value = value.trim();
            if !value.is_empty() {
                selectors.push(value.to_string());
            }
            continue;
        }
        // Bare tags are treated as selectors only when they look like a routing axis token.
        if !lower.contains(':') && !lower.contains('=') && !lower.contains(' ') {
            selectors.push(lower);
        }
    }

    selectors.sort();
    selectors.dedup();
    (selectors, free)
}

fn build_quota_inventory_slot(
    source_kind: QuotaInventorySourceKind,
    slot_id: String,
    model_id: String,
    base_url: Option<String>,
    enabled: bool,
    healthy: bool,
    selector_tokens: &[String],
    free_hint: bool,
    remaining_requests: Option<u64>,
    remaining_tokens: Option<u64>,
    metadata: BTreeMap<String, String>,
    mut notes: Vec<String>,
) -> QuotaInventorySlot {
    let fragments = split_pragmatic_model_fragments(&model_id);
    let family_hint = fragments
        .provider_fragment
        .as_deref()
        .and_then(infer_provider_family_from_provider_fragment);
    let (selectors, free) = normalize_quota_selector_tokens(selector_tokens, free_hint);

    notes.retain(|n| !n.trim().is_empty());
    notes.sort();
    notes.dedup();

    QuotaInventorySlot {
        source_kind,
        slot_id,
        model_id,
        fragments,
        family_hint,
        base_url,
        selectors,
        free,
        enabled,
        healthy,
        remaining_requests,
        remaining_tokens,
        metadata,
        notes,
    }
}

fn sanitize_quota_count_i64(value: Option<i64>) -> Option<u64> {
    match value {
        Some(v) if v >= 0 => Some(v as u64),
        _ => None,
    }
}

pub fn normalize_litellm_compatible_quota_inventory(
    records: &[LiteLlmCompatibleQuotaInventoryRecord],
) -> Vec<QuotaInventorySlot> {
    let mut out = records
        .iter()
        .map(|record| {
            build_quota_inventory_slot(
                QuotaInventorySourceKind::LiteLlmCompatibleAdmin,
                record.slot_id.clone(),
                record.model_id.trim().to_string(),
                record.api_base.clone(),
                record.enabled,
                record.healthy,
                &record.tags,
                false,
                record.remaining_requests,
                record.remaining_tokens,
                record.metadata.clone(),
                record.notes.clone(),
            )
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.slot_id.cmp(&b.slot_id));
    out
}

fn compose_cc_switch_model_id(provider_hint: Option<&str>, model_id: &str) -> String {
    let trimmed = model_id.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.contains('/') {
        return trimmed.to_string();
    }
    let provider = provider_hint.unwrap_or("").trim();
    if provider.is_empty() {
        trimmed.to_string()
    } else {
        format!("{provider}/{trimmed}")
    }
}

fn infer_cc_switch_health_from_state(enabled: bool, state: Option<&str>) -> bool {
    if !enabled {
        return false;
    }
    let Some(state) = state.map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    let lower = state.to_ascii_lowercase();
    !matches!(
        lower.as_str(),
        "cooldown" | "disabled" | "error" | "depleted" | "blocked"
    )
}

pub fn normalize_cc_switch_sqlite_quota_inventory(
    rows: &[CcSwitchSqliteQuotaInventoryRow],
) -> Vec<QuotaInventorySlot> {
    let mut out = rows
        .iter()
        .map(|row| {
            let model_id = compose_cc_switch_model_id(row.provider_hint.as_deref(), &row.model_id);
            let mut metadata = row.metadata.clone();
            if let Some(state) = row
                .state
                .as_ref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            {
                metadata
                    .entry("cc_switch_state".to_string())
                    .or_insert_with(|| state.to_string());
            }
            build_quota_inventory_slot(
                QuotaInventorySourceKind::CcSwitchSqlite,
                row.slot_id.clone(),
                model_id,
                row.base_url.clone(),
                row.enabled,
                infer_cc_switch_health_from_state(row.enabled, row.state.as_deref()),
                &row.selectors,
                false,
                sanitize_quota_count_i64(row.remaining_requests),
                sanitize_quota_count_i64(row.remaining_tokens),
                metadata,
                row.notes.clone(),
            )
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.slot_id.cmp(&b.slot_id));
    out
}

fn normalize_mock_quota_record_model_fields(
    record: &MockQuotaInventoryRecord,
) -> (String, Option<String>, Vec<String>, bool, Vec<String>) {
    if record.model_ref_or_id.trim_start().starts_with('/') {
        if let Ok(route) = resolve_pragmatic_unified_port_route(&record.model_ref_or_id) {
            let mut selector_tokens = record.selectors.clone();
            selector_tokens.extend(route.selectors.clone());
            if let Some(modality) = route.modality.as_ref() {
                selector_tokens.push(format!("modality/{modality}"));
            }
            let notes = if route.notes.is_empty() {
                record.notes.clone()
            } else {
                let mut merged = record.notes.clone();
                merged.extend(route.notes.clone());
                merged
            };
            return (
                route.upstream_model_id,
                record.base_url.clone(),
                selector_tokens,
                record.free,
                notes,
            );
        }
    }

    (
        record.model_ref_or_id.trim().to_string(),
        record.base_url.clone(),
        record.selectors.clone(),
        record.free,
        record.notes.clone(),
    )
}

pub fn normalize_mock_quota_inventory(
    records: &[MockQuotaInventoryRecord],
) -> Vec<QuotaInventorySlot> {
    let mut out = records
        .iter()
        .map(|record| {
            let (model_id, base_url, selector_tokens, free_hint, notes) =
                normalize_mock_quota_record_model_fields(record);
            build_quota_inventory_slot(
                QuotaInventorySourceKind::StaticMock,
                record.slot_id.clone(),
                model_id,
                base_url,
                record.enabled,
                record.healthy,
                &selector_tokens,
                free_hint,
                record.remaining_requests,
                record.remaining_tokens,
                record.metadata.clone(),
                notes,
            )
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.slot_id.cmp(&b.slot_id));
    out
}

fn route_expected_family(route: &PragmaticUnifiedPortRoute) -> Option<ProviderFamily> {
    route
        .fragments
        .provider_fragment
        .as_deref()
        .and_then(infer_provider_family_from_provider_fragment)
        .or_else(|| {
            if route
                .pipeline_hints
                .iter()
                .any(|h| h == "anthropic-compatible")
            {
                Some(ProviderFamily::AnthropicCompatible)
            } else if route.pipeline_hints.iter().any(|h| h == "gemini-native") {
                Some(ProviderFamily::GeminiNative)
            } else if route.pipeline_hints.iter().any(|h| h == "openrouter") {
                Some(ProviderFamily::OpenRouter)
            } else if route
                .pipeline_hints
                .iter()
                .any(|h| h == "openai-compatible")
            {
                Some(ProviderFamily::OpenAiCompatible)
            } else {
                None
            }
        })
}

pub fn score_quota_inventory_slots_for_route(
    route: &PragmaticUnifiedPortRoute,
    slots: &[QuotaInventorySlot],
) -> Vec<QuotaInventoryRouteCandidate> {
    let route_host = route.host_scope.as_ref().map(|s| s.to_ascii_lowercase());
    let route_modality = route.modality.as_ref().map(|s| s.to_ascii_lowercase());
    let route_selectors: HashSet<String> = route
        .selectors
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let expected_family = route_expected_family(route);

    let mut out = Vec::new();

    for slot in slots {
        let mut score: i64 = 0;
        let mut reasons = Vec::new();

        if slot.enabled {
            score += 40;
            reasons.push("enabled".to_string());
        } else {
            score -= 300;
            reasons.push("disabled".to_string());
        }

        if slot.healthy {
            score += 30;
            reasons.push("healthy".to_string());
        } else {
            score -= 120;
            reasons.push("unhealthy".to_string());
        }

        if slot.model_id == route.upstream_model_id {
            score += 150;
            reasons.push("exact-model".to_string());
        } else if !route.fragments.leaf_fragment.is_empty()
            && slot.fragments.leaf_fragment == route.fragments.leaf_fragment
        {
            score += 60;
            reasons.push("leaf-match".to_string());
        }

        if let (Some(route_provider), Some(slot_provider)) = (
            route.fragments.provider_fragment.as_ref(),
            slot.fragments.provider_fragment.as_ref(),
        ) {
            if route_provider.eq_ignore_ascii_case(slot_provider) {
                score += 45;
                reasons.push("provider-fragment-match".to_string());
            }
        }

        if let Some(expected_family) = expected_family {
            if slot.family_hint == Some(expected_family) {
                score += 35;
                reasons.push("family-match".to_string());
            }
        }

        if let Some(modality) = route_modality.as_deref() {
            if modality == "free" && slot.free {
                score += 70;
                reasons.push("free-modality".to_string());
            } else if modality == "free" && !slot.free {
                score -= 20;
                reasons.push("missing-free-modality".to_string());
            }
        }

        let selector_overlap = slot
            .selectors
            .iter()
            .filter(|s| route_selectors.contains(&s.to_ascii_lowercase()))
            .count() as i64;
        if selector_overlap > 0 {
            score += selector_overlap * 12;
            reasons.push(format!("selector-overlap={selector_overlap}"));
        }

        if let (Some(route_host), Some(slot_host)) = (
            route_host.as_deref(),
            slot.base_url.as_deref().and_then(lower_host_from_url),
        ) {
            if route_host == slot_host {
                score += 25;
                reasons.push("host-scope-match".to_string());
            }
        }

        if let Some(remaining) = slot.remaining_requests {
            let bump = remaining.min(200) as i64;
            score += bump;
            reasons.push(format!("remaining-requests={remaining}"));
        }
        if let Some(remaining) = slot.remaining_tokens {
            let bump = (remaining / 1000).min(200) as i64;
            score += bump;
            reasons.push(format!("remaining-tokens={remaining}"));
        }

        reasons.sort();
        reasons.dedup();
        out.push(QuotaInventoryRouteCandidate {
            slot: slot.clone(),
            score,
            reasons,
        });
    }

    out.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.slot.slot_id.cmp(&b.slot.slot_id))
    });
    out
}

pub fn select_best_quota_inventory_slot_for_route(
    route: &PragmaticUnifiedPortRoute,
    slots: &[QuotaInventorySlot],
) -> Option<QuotaInventoryRouteCandidate> {
    let scored = score_quota_inventory_slots_for_route(route, slots);
    scored
        .iter()
        .find(|c| c.slot.enabled && c.slot.healthy)
        .cloned()
        .or_else(|| scored.into_iter().next())
}

pub fn evaluate_modelmux_mvp_quota_inventory(
    lifecycle: &ModelmuxMvpLifecycle,
    slots: &[QuotaInventorySlot],
) -> ModelmuxMvpQuotaSelection {
    let candidates = score_quota_inventory_slots_for_route(&lifecycle.route, slots);
    let selected = candidates
        .iter()
        .find(|c| c.slot.enabled && c.slot.healthy)
        .cloned()
        .or_else(|| candidates.first().cloned());
    ModelmuxMvpQuotaSelection {
        selected,
        candidates,
    }
}

pub fn format_modelmux_mvp_quota_selection_line(selection: &ModelmuxMvpQuotaSelection) -> String {
    let selected_slot = selection
        .selected
        .as_ref()
        .map(|c| c.slot.slot_id.as_str())
        .unwrap_or("");
    let selected_model = selection
        .selected
        .as_ref()
        .map(|c| c.slot.model_id.as_str())
        .unwrap_or("");
    let selected_score = selection
        .selected
        .as_ref()
        .map(|c| c.score.to_string())
        .unwrap_or_default();
    let selected_source = selection
        .selected
        .as_ref()
        .map(|c| fmt_quota_inventory_source_kind(c.slot.source_kind))
        .unwrap_or("");
    let selected_usable = selection
        .selected
        .as_ref()
        .map(|c| c.slot.enabled && c.slot.healthy)
        .unwrap_or(false);

    let top_reasons = selection
        .selected
        .as_ref()
        .map(|c| c.reasons.join("|"))
        .unwrap_or_default();

    format!(
        "quota_candidates={};quota_selected_slot={selected_slot};quota_selected_model={selected_model};quota_selected_source={selected_source};quota_selected_score={selected_score};quota_selected_usable={selected_usable};quota_reasons={top_reasons}",
        selection.candidates.len(),
    )
}

fn quota_candidate_meets_minima(
    candidate: &QuotaInventoryRouteCandidate,
    options: &QuotaDrainerDryRunOptions,
) -> bool {
    let req_ok = candidate
        .slot
        .remaining_requests
        .map(|v| v >= options.min_remaining_requests)
        .unwrap_or(true);
    let tok_ok = candidate
        .slot
        .remaining_tokens
        .map(|v| v >= options.min_remaining_tokens)
        .unwrap_or(true);
    req_ok && tok_ok
}

fn quota_candidate_is_usable_for_drain(
    candidate: &QuotaInventoryRouteCandidate,
    options: &QuotaDrainerDryRunOptions,
) -> bool {
    candidate.slot.enabled
        && candidate.slot.healthy
        && quota_candidate_meets_minima(candidate, options)
}

pub fn run_modelmux_quota_drainer_dry_run(
    lifecycle: &ModelmuxMvpLifecycle,
    slots: &[QuotaInventorySlot],
) -> QuotaDrainerDryRunResult {
    run_modelmux_quota_drainer_dry_run_with_options(
        lifecycle,
        slots,
        &QuotaDrainerDryRunOptions::default(),
    )
}

pub fn run_modelmux_quota_drainer_dry_run_with_options(
    lifecycle: &ModelmuxMvpLifecycle,
    slots: &[QuotaInventorySlot],
    options: &QuotaDrainerDryRunOptions,
) -> QuotaDrainerDryRunResult {
    let selection = evaluate_modelmux_mvp_quota_inventory(lifecycle, slots);
    let free_candidates = selection.candidates.iter().filter(|c| c.slot.free).count();
    let paid_candidates = selection.candidates.iter().filter(|c| !c.slot.free).count();

    let mut steps = Vec::new();
    steps.push(QuotaDrainerDryRunStep {
        kind: QuotaDrainerDryRunStepKind::Discover,
        summary: format!(
            "discovered {} quota slot candidates",
            selection.candidates.len()
        ),
    });
    steps.push(QuotaDrainerDryRunStep {
        kind: QuotaDrainerDryRunStepKind::Score,
        summary: format!(
            "scored route {} (free_candidates={free_candidates}, paid_candidates={paid_candidates})",
            lifecycle.route.route_key
        ),
    });

    let mut fallback_used = false;
    let mut selected: Option<QuotaInventoryRouteCandidate> = None;

    match options.policy {
        QuotaDrainerSelectionPolicy::FreeFirstThenPaidFallback => {
            if let Some(free_pick) = selection
                .candidates
                .iter()
                .find(|c| c.slot.free && quota_candidate_is_usable_for_drain(c, options))
                .cloned()
            {
                steps.push(QuotaDrainerDryRunStep {
                    kind: QuotaDrainerDryRunStepKind::Select,
                    summary: format!("selected free-tier slot {}", free_pick.slot.slot_id),
                });
                selected = Some(free_pick);
            } else {
                steps.push(QuotaDrainerDryRunStep {
                    kind: QuotaDrainerDryRunStepKind::Fallback,
                    summary: "no usable free-tier slot; evaluating paid fallback".to_string(),
                });
                fallback_used = true;
                if let Some(paid_pick) = selection
                    .candidates
                    .iter()
                    .find(|c| !c.slot.free && quota_candidate_is_usable_for_drain(c, options))
                    .cloned()
                {
                    steps.push(QuotaDrainerDryRunStep {
                        kind: QuotaDrainerDryRunStepKind::Select,
                        summary: format!("selected paid fallback slot {}", paid_pick.slot.slot_id),
                    });
                    selected = Some(paid_pick);
                }
            }
        }
    }

    let (ready, reason) = if let Some(sel) = selected.as_ref() {
        (
            true,
            if fallback_used {
                format!("paid fallback selected: {}", sel.slot.slot_id)
            } else {
                format!("free-tier selected: {}", sel.slot.slot_id)
            },
        )
    } else if selection.candidates.is_empty() {
        (false, "no quota inventory candidates".to_string())
    } else if free_candidates > 0 {
        (
            false,
            "free candidates exist but none passed usability minima; paid fallback unavailable"
                .to_string(),
        )
    } else {
        (false, "no usable quota candidate selected".to_string())
    };

    steps.push(QuotaDrainerDryRunStep {
        kind: QuotaDrainerDryRunStepKind::Review,
        summary: format!("ready={ready};reason={reason}"),
    });

    QuotaDrainerDryRunResult {
        policy: options.policy.clone(),
        route_key: lifecycle.route.route_key.clone(),
        selection,
        selected,
        fallback_used,
        free_candidates,
        paid_candidates,
        steps,
        ready,
        reason,
    }
}

pub fn format_quota_drainer_dry_run_line(result: &QuotaDrainerDryRunResult) -> String {
    let selected_slot = result
        .selected
        .as_ref()
        .map(|c| c.slot.slot_id.as_str())
        .unwrap_or("");
    let selected_free = result
        .selected
        .as_ref()
        .map(|c| c.slot.free)
        .unwrap_or(false);
    let selected_score = result
        .selected
        .as_ref()
        .map(|c| c.score.to_string())
        .unwrap_or_default();

    format!(
        "quota_drainer_ready={};reason={};route_key={};policy=free-first;free_candidates={};paid_candidates={};fallback_used={};selected_slot={selected_slot};selected_free={selected_free};selected_score={selected_score}",
        result.ready,
        result.reason,
        result.route_key,
        result.free_candidates,
        result.paid_candidates,
        result.fallback_used
    )
}

fn collect_modelmux_mvp_api_key_bindings(
    profile: &NormalizedEnvProfile,
) -> (
    Vec<ModelmuxMvpApiKeyBinding>,
    Vec<ModelmuxMvpApiKeyBinding>,
    Vec<ModelmuxMvpApiKeyBinding>,
) {
    let mut provider = Vec::new();
    let mut exchange = Vec::new();
    let mut unknown = Vec::new();

    for entry in &profile.entries {
        let Some(binding) = modelmux_mvp_api_key_binding_from_entry(entry) else {
            continue;
        };
        match binding.api_kind {
            ApiKind::ModelProvider => provider.push(binding),
            ApiKind::Exchange => exchange.push(binding),
            ApiKind::Unknown => unknown.push(binding),
        }
    }

    (provider, exchange, unknown)
}

fn modelmux_mvp_api_key_binding_from_entry(
    entry: &NormalizedEnvEntry,
) -> Option<ModelmuxMvpApiKeyBinding> {
    if entry.role != Some(EnvVarRole::ApiKey) {
        return None;
    }
    if matches!(entry.source, EnvBindingSource::SearchApiKey { .. }) {
        return None;
    }

    if let Some(cls) = &entry.generic_api_key {
        return Some(ModelmuxMvpApiKeyBinding {
            env_key: entry.key.clone(),
            prefix: Some(cls.prefix.clone()),
            api_kind: cls.api_kind,
            family_hint: cls.family_hint,
            base_url: cls.base_url.clone(),
            confidence: cls.confidence,
            reason: cls.reason.clone(),
        });
    }

    infer_modelmux_mvp_known_api_key_binding(entry)
}

fn infer_modelmux_mvp_known_api_key_binding(
    entry: &NormalizedEnvEntry,
) -> Option<ModelmuxMvpApiKeyBinding> {
    let key_norm = normalize_key(&entry.key);
    if let Some(prefix) = is_generic_api_key(&key_norm) {
        if let Some((api_kind, family_hint, confidence, reason)) = classify_by_prefix(&prefix) {
            return Some(ModelmuxMvpApiKeyBinding {
                env_key: entry.key.clone(),
                prefix: Some(prefix),
                api_kind,
                family_hint,
                base_url: None,
                confidence,
                reason: reason.to_string(),
            });
        }
    }

    let canonical_candidate = match &entry.source {
        EnvBindingSource::KnownExact { canonical_key }
        | EnvBindingSource::KnownAlias { canonical_key } => Some(normalize_key(canonical_key)),
        _ => None,
    };

    let canonical = canonical_candidate.as_deref().unwrap_or(&key_norm);
    let derived = if canonical.contains("ANTHROPIC") {
        Some((
            "ANTHROPIC".to_string(),
            ApiKind::ModelProvider,
            Some(ProviderFamily::AnthropicCompatible),
            85,
            "known anthropic api-key alias",
        ))
    } else if canonical.contains("GEMINI") || canonical.contains("GOOGLE") {
        Some((
            "GOOGLE".to_string(),
            ApiKind::ModelProvider,
            Some(ProviderFamily::GeminiNative),
            80,
            "known gemini/google api-key alias",
        ))
    } else if canonical.contains("OPENAI") {
        Some((
            "OPENAI".to_string(),
            ApiKind::ModelProvider,
            Some(ProviderFamily::OpenAiCompatible),
            85,
            "known openai api-key alias",
        ))
    } else {
        None
    }?;

    Some(ModelmuxMvpApiKeyBinding {
        env_key: entry.key.clone(),
        prefix: Some(derived.0),
        api_kind: derived.1,
        family_hint: derived.2,
        base_url: None,
        confidence: derived.3,
        reason: derived.4.to_string(),
    })
}

fn normalize_provider_selector_token(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>()
}

fn provider_tokens_match(route_provider: &str, binding_prefix: &str) -> bool {
    let route_norm = normalize_provider_selector_token(route_provider);
    let binding_norm = normalize_provider_selector_token(binding_prefix);
    if route_norm.is_empty() || binding_norm.is_empty() {
        return false;
    }
    if route_norm == binding_norm
        || route_norm.contains(&binding_norm)
        || binding_norm.contains(&route_norm)
    {
        return true;
    }

    matches!(
        (route_norm.as_str(), binding_norm.as_str()),
        ("moonshotai", "moonshot")
            | ("moonshot", "moonshotai")
            | ("moonshotai", "kimi")
            | ("moonshot", "kimi")
            | ("kimi", "moonshotai")
            | ("kimi", "moonshot")
            | ("zai", "glm")
            | ("glm", "zai")
            | ("kiloai", "kilo")
            | ("kilo", "kiloai")
    )
}

fn is_gateway_preferred_prefix(prefix: &str) -> bool {
    matches!(normalize_provider_selector_token(prefix).as_str(), "kilo" | "kiloai")
}

fn select_modelmux_mvp_provider_key(
    route: &PragmaticUnifiedPortRoute,
    provider_keys: &[ModelmuxMvpApiKeyBinding],
) -> Option<ModelmuxMvpApiKeyBinding> {
    let provider_hint = route
        .fragments
        .provider_fragment
        .as_deref()
        .map(|s| s.to_ascii_lowercase());
    let host_hint = route.host_scope.as_deref().map(|h| h.to_ascii_lowercase());

    let mut best: Option<(i32, usize)> = None;
    for (idx, binding) in provider_keys.iter().enumerate() {
        let mut score = binding.confidence as i32;
        if let (Some(route_provider), Some(prefix)) =
            (provider_hint.as_deref(), binding.prefix.as_deref())
        {
            if provider_tokens_match(route_provider, prefix) {
                score += 100;
            }
        }
        if let Some(family) = binding.family_hint {
            if route
                .pipeline_hints
                .iter()
                .any(|h| h == family_to_pipeline_hint(family))
            {
                score += 40;
            }
        }
        if route.modality.as_deref() == Some("free")
            && binding
                .prefix
                .as_deref()
                .is_some_and(is_gateway_preferred_prefix)
        {
            score += 160;
        }
        if let (Some(host), Some(base)) = (host_hint.as_deref(), binding.base_url.as_deref()) {
            if base.to_ascii_lowercase().contains(host) {
                score += 20;
            }
        }

        match best {
            Some((best_score, best_idx))
                if score < best_score || (score == best_score && idx > best_idx) => {}
            _ => best = Some((score, idx)),
        }
    }

    best.map(|(_, idx)| provider_keys[idx].clone())
}

fn family_to_pipeline_hint(family: ProviderFamily) -> &'static str {
    match family {
        ProviderFamily::OpenAiCompatible => "openai-compatible",
        ProviderFamily::AnthropicCompatible => "anthropic-compatible",
        ProviderFamily::GeminiNative => "gemini-native",
        ProviderFamily::AzureOpenAi => "azure-openai",
        ProviderFamily::OpenRouter => "openrouter",
        ProviderFamily::OpenCodeZen => "opencodezen",
        ProviderFamily::Ollama => "ollama",
        ProviderFamily::ControlPlane => "control-plane",
        ProviderFamily::Unknown => "unknown",
    }
}

fn infer_modelmux_mvp_readiness(
    route: &PragmaticUnifiedPortRoute,
    selected_provider_key: Option<&ModelmuxMvpApiKeyBinding>,
    exchange_keys: &[ModelmuxMvpApiKeyBinding],
    env_profile: &NormalizedEnvProfile,
) -> ModelmuxMvpReadiness {
    if selected_provider_key.is_some() {
        return ModelmuxMvpReadiness {
            ready: true,
            reason: "provider api key selected".to_string(),
        };
    }

    if route_targets_exchange_surface(route) && !exchange_keys.is_empty() {
        return ModelmuxMvpReadiness {
            ready: true,
            reason: "exchange api key selected for trading path".to_string(),
        };
    }

    if route_targets_control_surface(route) {
        return ModelmuxMvpReadiness {
            ready: true,
            reason: "explicit host health/control path".to_string(),
        };
    }

    if !env_profile.search_key_groups.is_empty() && route.modality.as_deref() == Some("free") {
        return ModelmuxMvpReadiness {
            ready: false,
            reason: "search keys present but no provider api key selected".to_string(),
        };
    }

    ModelmuxMvpReadiness {
        ready: false,
        reason: "no provider api key selected".to_string(),
    }
}

fn build_modelmux_mvp_lifecycle_tags(
    route: &PragmaticUnifiedPortRoute,
    search_enabled: bool,
    provider_keys: &[ModelmuxMvpApiKeyBinding],
    exchange_keys: &[ModelmuxMvpApiKeyBinding],
    unknown_keys: &[ModelmuxMvpApiKeyBinding],
    selected_provider_key: Option<&ModelmuxMvpApiKeyBinding>,
    readiness: &ModelmuxMvpReadiness,
) -> Vec<String> {
    let mut tags = route.dsel_tags.clone();
    if search_enabled {
        tags.push("capability/search-enabled".to_string());
    }
    tags.push(format!("lifecycle/ready={}", readiness.ready));
    tags.push(format!("api-keys/provider={}", provider_keys.len()));
    tags.push(format!("api-keys/exchange={}", exchange_keys.len()));
    tags.push(format!("api-keys/unknown={}", unknown_keys.len()));
    if let Some(sel) = selected_provider_key {
        tags.push(format!("selected-env-key/{}", sel.env_key));
        tags.push(format!("selected-api-kind/{}", fmt_api_kind(sel.api_kind)));
        if let Some(prefix) = sel.prefix.as_deref() {
            tags.push(format!("selected-prefix/{}", prefix.to_ascii_lowercase()));
        }
    }
    for w in &route.widened_models {
        tags.push(format!("widened/{}", w.boundary));
    }
    tags.sort();
    tags.dedup();
    tags
}

fn split_pragmatic_model_fragments(model_id: &str) -> PragmaticModelFragments {
    let all_fragments: Vec<String> = model_id
        .split('/')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();

    if all_fragments.is_empty() {
        return PragmaticModelFragments {
            provider_fragment: None,
            namespace_fragments: vec![],
            leaf_fragment: String::new(),
            all_fragments,
        };
    }

    let leaf_fragment = all_fragments.last().cloned().unwrap_or_else(String::new);
    let provider_fragment = if all_fragments.len() >= 2 {
        Some(all_fragments[0].clone())
    } else {
        None
    };
    let namespace_fragments = if all_fragments.len() > 2 {
        all_fragments[1..all_fragments.len() - 1].to_vec()
    } else {
        vec![]
    };

    PragmaticModelFragments {
        provider_fragment,
        namespace_fragments,
        leaf_fragment,
        all_fragments,
    }
}

fn infer_provider_family_from_fragments(
    parsed: &PragmaticModelRef,
    fragments: &PragmaticModelFragments,
) -> Option<ProviderFamily> {
    if let Some(f) = parsed.access.family_hint {
        return Some(f);
    }

    let provider = fragments
        .provider_fragment
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    if provider.is_empty() {
        return None;
    }

    match provider.as_str() {
        "anthropic" | "claude" => Some(ProviderFamily::AnthropicCompatible),
        "google" | "gemini" => Some(ProviderFamily::GeminiNative),
        "ollama" => Some(ProviderFamily::Ollama),
        _ => Some(ProviderFamily::OpenAiCompatible),  // Default for any XXXXX_API_KEY provider
    }
}

fn infer_unified_port_pipeline_hints(
    parsed: &PragmaticModelRef,
    fragments: &PragmaticModelFragments,
    family_hint: Option<ProviderFamily>,
) -> Vec<String> {
    let mut hints = Vec::<String>::new();

    if parsed.access.format != MuxPragmaticFormat::Unknown {
        hints.push(fmt_format(parsed.access.format).to_string());
    }

    if let Some(family) = family_hint {
        hints.push(
            match family {
                ProviderFamily::OpenAiCompatible => "openai-compatible",
                ProviderFamily::AnthropicCompatible => "anthropic-compatible",
                ProviderFamily::GeminiNative => "gemini-native",
                ProviderFamily::AzureOpenAi => "azure-openai",
                ProviderFamily::OpenRouter => "openrouter",
                ProviderFamily::OpenCodeZen => "opencodezen",
                ProviderFamily::Ollama => "ollama",
                ProviderFamily::ControlPlane => "control-plane",
                ProviderFamily::Unknown => "unknown",
            }
            .to_string(),
        );
    }

    if parsed.selector_prefixes.iter().any(|s| s == "free")
        || parsed.modality.as_deref() == Some("free")
    {
        hints.push("quota-dsel-free".to_string());
    }
    if parsed_targets_exchange_surface(parsed) {
        hints.push("exchange-rest".to_string());
        hints.push("trading-path".to_string());
    }

    if parsed.option_specs.iter().any(|o| o.key == "vision") {
        hints.push("vision".to_string());
    }
    if parsed.option_specs.iter().any(|o| o.key == "tools") {
        hints.push("tools".to_string());
    }
    if parsed.option_specs.iter().any(|o| o.key == "reasoning") {
        hints.push("reasoning".to_string());
    }

    if let Some(provider) = fragments.provider_fragment.as_deref() {
        hints.push(format!("provider-{provider}"));
    }

    hints.sort();
    hints.dedup();
    hints
}

fn parsed_targets_exchange_surface(parsed: &PragmaticModelRef) -> bool {
    parsed
        .selector_prefixes
        .iter()
        .any(|s| matches!(s.as_str(), "trade" | "trading" | "exchange"))
        || matches!(
            parsed.modality.as_deref(),
            Some("trade") | Some("trading") | Some("exchange")
        )
}

fn route_targets_exchange_surface(route: &PragmaticUnifiedPortRoute) -> bool {
    route
        .selectors
        .iter()
        .any(|s| matches!(s.as_str(), "trade" | "trading" | "exchange"))
        || matches!(
            route.modality.as_deref(),
            Some("trade") | Some("trading") | Some("exchange")
        )
        || route
            .pipeline_hints
            .iter()
            .any(|h| matches!(h.as_str(), "exchange-rest" | "trading-path"))
}

fn route_targets_control_surface(route: &PragmaticUnifiedPortRoute) -> bool {
    route.host_scope.is_some()
        && (route.route_key.contains("/health")
            || route.route_key.contains("/control/")
            || route.route_key.contains("control/state")
            || route.route_key.ends_with("/control")
            || route.selectors.iter().any(|s| s == "control")
            || route
                .pipeline_hints
                .iter()
                .any(|h| matches!(h.as_str(), "control-plane" | "provider-control")))
}

fn infer_widened_model_candidates(
    parsed: &PragmaticModelRef,
    fragments: &PragmaticModelFragments,
    family_hint: Option<ProviderFamily>,
) -> Vec<PragmaticWidenedModelCandidate> {
    let mut out = Vec::<PragmaticWidenedModelCandidate>::new();
    let provider_fragment = fragments.provider_fragment.as_deref();

    if provider_fragment.is_some() {
        out.push(PragmaticWidenedModelCandidate {
            boundary: "litellm".to_string(),
            model: parsed.upstream_model_id.clone(),
            reason: "provider/model fragments already explicit".to_string(),
        });
    }

    match family_hint {
        Some(ProviderFamily::AnthropicCompatible) => {
            let anthropic_litellm = match provider_fragment {
                Some(_) => parsed.upstream_model_id.clone(),
                None => format!("anthropic/{}", parsed.upstream_model_id),
            };
            out.push(PragmaticWidenedModelCandidate {
                boundary: "litellm".to_string(),
                model: anthropic_litellm,
                reason: "Anthropic-compatible path can be widened to LiteLLM provider/model form"
                    .to_string(),
            });
            out.push(PragmaticWidenedModelCandidate {
                boundary: "anthropic-native".to_string(),
                model: fragments.leaf_fragment.clone(),
                reason: "Anthropic native messages APIs typically accept bare Claude model IDs"
                    .to_string(),
            });
        }
        Some(ProviderFamily::GeminiNative) => {
            out.push(PragmaticWidenedModelCandidate {
                boundary: "gemini-native".to_string(),
                model: parsed.upstream_model_id.clone(),
                reason: "Gemini native routes preserve explicit model fragments".to_string(),
            });
        }
        Some(ProviderFamily::OpenAiCompatible)
        | Some(ProviderFamily::AzureOpenAi)
        | Some(ProviderFamily::OpenRouter)
        | Some(ProviderFamily::Ollama)
        | Some(ProviderFamily::OpenCodeZen) => {
            out.push(PragmaticWidenedModelCandidate {
                boundary: "openai-compatible".to_string(),
                model: parsed.upstream_model_id.clone(),
                reason: "OpenAI-compatible boundaries can consume the same fragment model string"
                    .to_string(),
            });
        }
        Some(ProviderFamily::ControlPlane) | Some(ProviderFamily::Unknown) | None => {}
    }

    out.sort_by(|a, b| {
        a.boundary
            .cmp(&b.boundary)
            .then_with(|| a.model.cmp(&b.model))
    });
    out.dedup_by(|a, b| a.boundary == b.boundary && a.model == b.model);
    out
}

fn fmt_api_kind(kind: ApiKind) -> &'static str {
    match kind {
        ApiKind::ModelProvider => "model-provider",
        ApiKind::Exchange => "exchange",
        ApiKind::Unknown => "unknown",
    }
}

fn fmt_quota_inventory_source_kind(kind: QuotaInventorySourceKind) -> &'static str {
    match kind {
        QuotaInventorySourceKind::LitebikeNative => "litebike-native",
        QuotaInventorySourceKind::LiteLlmCompatibleAdmin => "litellm-compatible-admin",
        QuotaInventorySourceKind::CcSwitchSqlite => "cc-switch-sqlite",
        QuotaInventorySourceKind::StaticMock => "static-mock",
    }
}

fn fmt_format(format: MuxPragmaticFormat) -> &'static str {
    match format {
        MuxPragmaticFormat::OpenAiCompatible => "openai-compatible",
        MuxPragmaticFormat::AnthropicCompatible => "anthropic-compatible",
        MuxPragmaticFormat::GeminiNative => "gemini-native",
        MuxPragmaticFormat::ControlPlane => "control-plane",
        MuxPragmaticFormat::ExchangeRest => "exchange-rest",
        MuxPragmaticFormat::Unknown => "unknown",
    }
}

fn fmt_auth_option(auth: MuxAuthOption) -> &'static str {
    match auth {
        MuxAuthOption::BearerAuthorization => "bearer",
        MuxAuthOption::ApiKeyHeader => "api-key-header",
        MuxAuthOption::AnthropicXApiKey => "anthropic-x-api-key",
        MuxAuthOption::GoogleApiKeyHeader => "google-api-key-header",
        MuxAuthOption::GoogleApiKeyQuery => "google-api-key-query",
        MuxAuthOption::NoneOrSession => "none-or-session",
    }
}

pub fn format_hostname_access_options_line(opts: &HostnameAccessOptions) -> String {
    let host = opts.hostname.as_deref().unwrap_or("");
    let auth = opts
        .auth_options
        .iter()
        .map(|a| fmt_auth_option(*a))
        .collect::<Vec<_>>()
        .join("|");
    let probes = opts.model_probe_urls.join("|");
    let rules = opts.matched_rule_ids.join("|");
    format!(
        "host={host};kind={};format={};auth={auth};models_probe={probes};confidence={};rules={rules}",
        fmt_api_kind(opts.api_kind),
        fmt_format(opts.format),
        opts.confidence
    )
}

pub fn normalize_env_pairs<I>(pairs: I) -> NormalizedEnvProfile
where
    I: IntoIterator<Item = (String, String)>,
{
    normalize_env_pairs_with_generic_api_probe(pairs, None, None)
}

pub fn normalize_env_pairs_with_generic_api_probe<I>(
    pairs: I,
    generic_api_probe: Option<&dyn GenericApiModelsProbe>,
    mut generic_api_probe_cache: Option<&mut GenericApiModelsProbeCache>,
) -> NormalizedEnvProfile
where
    I: IntoIterator<Item = (String, String)>,
{
    let pairs_vec: Vec<(String, String)> = pairs.into_iter().collect();
    let mut env_map = BTreeMap::<String, String>::new();
    for (k, v) in &pairs_vec {
        env_map.insert(normalize_key(k), v.clone());
    }

    let known_index = known_bindings_index();
    let rules = env_rule_hints();

    let mut entries = Vec::with_capacity(pairs_vec.len());
    let mut search_groups_by_name: BTreeMap<String, Vec<SearchApiKeyEntry>> = BTreeMap::new();
    let mut search_group_first_seen: HashMap<String, usize> = HashMap::new();

    for (order, (raw_key, raw_value)) in pairs_vec.into_iter().enumerate() {
        let key_norm = normalize_key(&raw_key);

        if let Some((group, index)) = parse_search_api_key_suffix(&key_norm) {
            let search_entry = SearchApiKeyEntry {
                key: raw_key.clone(),
                group: group.clone(),
                index,
                order,
                auth_hint: infer_search_auth_hint(&group),
            };
            search_group_first_seen
                .entry(group.clone())
                .or_insert(order);
            search_groups_by_name
                .entry(group.clone())
                .or_default()
                .push(search_entry.clone());

            entries.push(NormalizedEnvEntry {
                key: raw_key,
                value: raw_value,
                role: Some(EnvVarRole::ApiKey),
                source: EnvBindingSource::SearchApiKey { group, index },
                generic_api_key: None,
            });
            continue;
        }

        if let Some(binding) = known_index.get(&key_norm) {
            entries.push(NormalizedEnvEntry {
                key: raw_key,
                value: raw_value,
                role: Some(binding.role),
                source: if binding.alias {
                    EnvBindingSource::KnownAlias {
                        canonical_key: binding.canonical_key.clone(),
                    }
                } else {
                    EnvBindingSource::KnownExact {
                        canonical_key: binding.canonical_key.clone(),
                    }
                },
                generic_api_key: None,
            });
            continue;
        }

        if let Some(prefix) = is_generic_api_key(&key_norm) {
            let classification = classify_generic_api_key_with_optional_probe(
                &prefix,
                &env_map,
                &rules,
                generic_api_probe,
                generic_api_probe_cache.as_mut().map(|cache| &mut **cache),
            );
            entries.push(NormalizedEnvEntry {
                key: raw_key,
                value: raw_value,
                role: Some(EnvVarRole::ApiKey),
                source: EnvBindingSource::GenericApiKey { prefix },
                generic_api_key: Some(classification),
            });
            continue;
        }

        entries.push(NormalizedEnvEntry {
            key: raw_key,
            value: raw_value,
            role: None,
            source: EnvBindingSource::Unknown,
            generic_api_key: None,
        });
    }

    let mut groups_with_order: Vec<(usize, SearchApiKeyGroup)> = search_groups_by_name
        .into_iter()
        .map(|(group, mut entries)| {
            entries.sort_by(|a, b| {
                let a_key = a.index.unwrap_or(0);
                let b_key = b.index.unwrap_or(0);
                a_key.cmp(&b_key).then(a.order.cmp(&b.order))
            });
            let first_seen = *search_group_first_seen.get(&group).unwrap_or(&usize::MAX);
            (first_seen, SearchApiKeyGroup { group, entries })
        })
        .collect();
    groups_with_order.sort_by_key(|(first_seen, _)| *first_seen);

    NormalizedEnvProfile {
        entries,
        search_key_groups: groups_with_order.into_iter().map(|(_, g)| g).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn kv(k: &str, v: &str) -> (String, String) {
        (k.to_string(), v.to_string())
    }

    struct MockGenericApiProbe {
        calls: Cell<usize>,
        result: Option<GenericApiModelsProbeClassification>,
    }

    impl GenericApiModelsProbe for MockGenericApiProbe {
        fn probe_models_capability(
            &self,
            _base_url: &str,
            _candidate_urls: &[String],
        ) -> Option<GenericApiModelsProbeClassification> {
            self.calls.set(self.calls.get() + 1);
            self.result.clone()
        }
    }

    #[test]
    fn known_alias_maps_to_canonical_role() {
        let profile = normalize_env_pairs(vec![kv("ANTHROPIC_API_KEY", "sk-ant-test")]);
        assert_eq!(profile.entries.len(), 1);
        let entry = &profile.entries[0];
        assert_eq!(entry.role, Some(EnvVarRole::ApiKey));
        match &entry.source {
            EnvBindingSource::KnownAlias { canonical_key } => {
                assert_eq!(canonical_key, "ANTHROPIC_AUTH_TOKEN");
            }
            other => panic!("expected KnownAlias, got {other:?}"),
        }
    }

    #[test]
    fn search_api_keys_are_grouped_and_ordered() {
        let profile = normalize_env_pairs(vec![
            kv("BRAVE_SEARCH_API_KEY_2", "k2"),
            kv("BRAVE_SEARCH_API_KEY", "k1"),
            kv("TAVILY_SEARCH_API_KEY_1", "t1"),
        ]);

        assert_eq!(profile.search_key_groups.len(), 2);
        assert_eq!(profile.search_key_groups[0].group, "BRAVE");
        assert_eq!(profile.search_key_groups[0].entries.len(), 2);
        assert_eq!(
            profile.search_key_groups[0].entries[0].key,
            "BRAVE_SEARCH_API_KEY"
        );
        assert_eq!(
            profile.search_key_groups[0].entries[0].auth_hint,
            SearchAuthHint::HeaderXSubscriptionToken
        );
        assert_eq!(
            profile.search_key_groups[0].entries[1].key,
            "BRAVE_SEARCH_API_KEY_2"
        );
        assert_eq!(profile.search_key_groups[1].group, "TAVILY");
    }

    #[test]
    fn generic_api_key_classifies_exchange_by_host() {
        let profile = normalize_env_pairs(vec![
            kv("BINANCE_API_KEY", "abc"),
            kv("BINANCE_BASE_URL", "https://api.binance.com"),
        ]);

        let entry = profile
            .entries
            .iter()
            .find(|e| e.key == "BINANCE_API_KEY")
            .expect("binance key present");

        let cls = entry.generic_api_key.as_ref().expect("classification");
        assert_eq!(cls.api_kind, ApiKind::Exchange);
        assert_eq!(cls.base_url_key.as_deref(), Some("BINANCE_BASE_URL"));
    }

    #[test]
    fn generic_api_key_classifies_model_provider_by_host_rule() {
        let profile = normalize_env_pairs(vec![
            kv("MYROUTER_API_KEY", "sk-test"),
            kv("MYROUTER_BASE_URL", "https://openrouter.ai/api"),
        ]);

        let entry = profile
            .entries
            .iter()
            .find(|e| e.key == "MYROUTER_API_KEY")
            .expect("key present");
        let cls = entry.generic_api_key.as_ref().expect("classification");
        assert_eq!(cls.api_kind, ApiKind::ModelProvider);
        assert!(cls
            .matched_rule_ids
            .iter()
            .any(|id| id == "openai-compatible"));
    }

    #[test]
    fn generic_api_key_classifies_unknown_when_no_hints() {
        let profile = normalize_env_pairs(vec![kv("FOO_API_KEY", "x")]);
        let cls = profile.entries[0]
            .generic_api_key
            .as_ref()
            .expect("classification");
        assert_eq!(cls.api_kind, ApiKind::Unknown);
    }

    #[test]
    fn generic_api_key_probe_upgrades_unknown_to_model_provider() {
        let probe = MockGenericApiProbe {
            calls: Cell::new(0),
            result: Some(GenericApiModelsProbeClassification {
                api_kind: ApiKind::ModelProvider,
                family_hint: Some(ProviderFamily::OpenAiCompatible),
                confidence: 91,
                reason: "mock /models probe matched".to_string(),
                matched_probe_url: Some("https://gateway.example/v1/models".to_string()),
            }),
        };
        let mut cache = GenericApiModelsProbeCache::default();

        let profile = normalize_env_pairs_with_generic_api_probe(
            vec![
                kv("FOO_API_KEY", "abc"),
                kv("FOO_BASE_URL", "https://gateway.example"),
            ],
            Some(&probe),
            Some(&mut cache),
        );

        let cls = profile
            .entries
            .iter()
            .find(|e| e.key == "FOO_API_KEY")
            .and_then(|e| e.generic_api_key.as_ref())
            .expect("classification");
        assert_eq!(cls.api_kind, ApiKind::ModelProvider);
        assert_eq!(cls.family_hint, Some(ProviderFamily::OpenAiCompatible));
        assert!(cls.reason.contains("models probe"));
        assert_eq!(probe.calls.get(), 1);
        assert!(cache.get("https://gateway.example").is_some());
    }

    #[test]
    fn generic_api_key_probe_failure_falls_back_to_no_network_unknown() {
        let probe = MockGenericApiProbe {
            calls: Cell::new(0),
            result: None,
        };

        let profile = normalize_env_pairs_with_generic_api_probe(
            vec![
                kv("FOO_API_KEY", "abc"),
                kv("FOO_BASE_URL", "https://gateway.example"),
            ],
            Some(&probe),
            None,
        );

        let cls = profile
            .entries
            .iter()
            .find(|e| e.key == "FOO_API_KEY")
            .and_then(|e| e.generic_api_key.as_ref())
            .expect("classification");
        assert_eq!(cls.api_kind, ApiKind::Unknown);
        assert_eq!(cls.reason, "no-network classification unresolved");
        assert_eq!(probe.calls.get(), 1);
    }

    #[test]
    fn generic_api_key_uses_cached_probe_result_when_probe_is_absent() {
        let mut cache = GenericApiModelsProbeCache::default();
        cache.insert(
            "https://gateway.example",
            GenericApiModelsProbeClassification {
                api_kind: ApiKind::ModelProvider,
                family_hint: Some(ProviderFamily::AnthropicCompatible),
                confidence: 88,
                reason: "cached anthropic-compatible probe".to_string(),
                matched_probe_url: Some("https://gateway.example/v1/models".to_string()),
            },
        );

        let profile = normalize_env_pairs_with_generic_api_probe(
            vec![
                kv("FOO_API_KEY", "abc"),
                kv("FOO_BASE_URL", "https://gateway.example"),
            ],
            None,
            Some(&mut cache),
        );

        let cls = profile
            .entries
            .iter()
            .find(|e| e.key == "FOO_API_KEY")
            .and_then(|e| e.generic_api_key.as_ref())
            .expect("classification");
        assert_eq!(cls.api_kind, ApiKind::ModelProvider);
        assert_eq!(cls.family_hint, Some(ProviderFamily::AnthropicCompatible));
        assert!(cls.reason.contains("models probe cache"));
    }

    #[test]
    fn model_probe_urls_follow_cc_switch_style() {
        let urls = build_model_probe_urls("https://api.example.com");
        assert_eq!(
            urls,
            vec![
                "https://api.example.com/v1/models".to_string(),
                "https://api.example.com/models".to_string()
            ]
        );
    }

    #[test]
    fn hostname_access_options_openai_compat_has_pragmatic_mappings() {
        let opts = infer_hostname_access_options("https://openrouter.ai/api");
        assert_eq!(opts.api_kind, ApiKind::ModelProvider);
        assert_eq!(opts.format, MuxPragmaticFormat::OpenAiCompatible);
        assert!(opts.supports_models_probe);
        assert!(opts
            .model_probe_urls
            .iter()
            .any(|u| u.ends_with("/v1/models")));
        assert!(opts.url_mappings.iter().any(
            |m| m.route == MuxRouteKind::ChatCompletions && m.url.contains("/chat/completions")
        ));
    }

    #[test]
    fn hostname_access_options_gemini_uses_v1beta_paths() {
        let opts = infer_hostname_access_options("https://generativelanguage.googleapis.com");
        assert_eq!(opts.format, MuxPragmaticFormat::GeminiNative);
        assert!(opts
            .model_probe_urls
            .iter()
            .any(|u| u.contains("/v1beta/models")));
        assert!(opts.url_mappings.iter().any(|m| {
            m.route == MuxRouteKind::GeminiGenerateContent && m.url.contains(":generateContent")
        }));
    }

    #[test]
    fn hostname_access_options_exchange_has_no_model_probe() {
        let opts = infer_hostname_access_options("https://api.binance.com");
        assert_eq!(opts.api_kind, ApiKind::Exchange);
        assert_eq!(opts.format, MuxPragmaticFormat::ExchangeRest);
        assert!(!opts.supports_models_probe);
        assert!(opts.model_probe_urls.is_empty());
    }

    #[test]
    fn pragmatic_hostname_access_line_is_stable() {
        let opts = infer_hostname_access_options("https://api.anthropic.com");
        let line = format_hostname_access_options_line(&opts);
        assert!(line.contains("host=api.anthropic.com"));
        assert!(line.contains("format=anthropic-compatible"));
        assert!(line.contains("auth=anthropic-x-api-key|bearer"));
    }

    #[test]
    fn parses_user_style_pragmatic_model_ref() {
        let parsed =
            parse_pragmatic_model_ref("/{localhost:8888,chat,vision,XXXXX}/HF-modelname-or-other")
                .expect("parse");

        assert_eq!(parsed.host_token, "localhost:8888");
        assert_eq!(parsed.host, "localhost");
        assert_eq!(parsed.port, Some(8888));
        assert_eq!(parsed.base_url, "http://localhost:8888");
        assert_eq!(
            parsed.option_tokens,
            vec![
                "chat".to_string(),
                "vision".to_string(),
                "XXXXX".to_string()
            ]
        );
        assert_eq!(parsed.upstream_model_id, "HF-modelname-or-other");
    }

    #[test]
    fn parses_ref_with_explicit_scheme_and_nested_model_id() {
        let parsed = parse_pragmatic_model_ref(
            "/{https://openrouter.ai:443,reasoning,tools}/anthropic/claude-3-5-sonnet",
        )
        .expect("parse");

        assert_eq!(parsed.base_url, "https://openrouter.ai:443");
        assert_eq!(parsed.host, "openrouter.ai");
        assert_eq!(parsed.port, Some(443));
        assert_eq!(parsed.upstream_model_id, "anthropic/claude-3-5-sonnet");
        assert_eq!(parsed.access.api_kind, ApiKind::ModelProvider);
    }

    #[test]
    fn pragmatic_model_ref_access_line_includes_host_options_and_mux_format() {
        let line = format_pragmatic_model_ref_access_line(
            "/{api.anthropic.com:443,chat,tools,https}/claude-opus-4-6",
        )
        .expect("format");
        assert!(line.contains("host_token=api.anthropic.com:443"));
        assert!(line.contains("options=chat|tools|https"));
        assert!(line.contains("model=claude-opus-4-6"));
        assert!(line.contains("format=anthropic-compatible"));
    }

    #[test]
    fn parses_free_modality_shorthand_ref_for_quota_bombing() {
        let parsed = parse_pragmatic_model_ref("/free/moonshotai/kimi-k2").expect("parse");

        assert_eq!(parsed.selector_prefixes, vec!["free".to_string()]);
        assert_eq!(parsed.modality.as_deref(), Some("free"));
        assert_eq!(parsed.upstream_model_id, "moonshotai/kimi-k2");
        assert!(parsed.host_token.is_empty());
        assert!(parsed.base_url.is_empty());
        assert_eq!(parsed.access.api_kind, ApiKind::Unknown);
        assert!(parsed
            .option_specs
            .iter()
            .any(|o| o.key == "modality" && o.value.as_deref() == Some("free")));
    }

    #[test]
    fn parses_host_block_option_metadata_and_notes() {
        let parsed = parse_pragmatic_model_ref(
            "/{localhost:8888,chat,modality/free,meta:quota=bombing,note=quota-bombing}/moonshotai/kimi-k2",
        )
        .expect("parse");

        assert_eq!(parsed.modality.as_deref(), Some("free"));
        assert_eq!(
            parsed.metadata.get("quota").map(String::as_str),
            Some("bombing")
        );
        assert_eq!(parsed.notes, vec!["quota-bombing".to_string()]);
        assert!(parsed
            .option_specs
            .iter()
            .any(|o| o.key == "chat" && o.value.is_none()));
    }

    #[test]
    fn free_modality_access_line_surfaces_selector_and_modality() {
        let line =
            format_pragmatic_model_ref_access_line("/free/moonshotai/kimi-k2").expect("format");
        assert!(line.contains("selectors=free"));
        assert!(line.contains("modality=free"));
        assert!(line.contains("model=moonshotai/kimi-k2"));
    }

    #[test]
    fn resolves_unified_port_route_for_free_moonshotai_fragment() {
        let route =
            resolve_pragmatic_unified_port_route("/free/moonshotai/kimi-k2").expect("route");

        assert_eq!(route.agent_name, UNIFIED_PORT_AGENT_NAME);
        assert_eq!(route.unified_port, 8888);
        assert_eq!(route.selectors, vec!["free".to_string()]);
        assert_eq!(route.modality.as_deref(), Some("free"));
        assert_eq!(
            route.fragments.provider_fragment.as_deref(),
            Some("moonshotai")
        );
        assert_eq!(route.fragments.leaf_fragment, "kimi-k2");
        assert!(route.route_key.starts_with("unified-port:default:free:"));
        assert!(route.dsel_tags.iter().any(|t| t == "modality/free"));
        assert!(route
            .pipeline_hints
            .iter()
            .any(|h| h == "openai-compatible"));
        assert!(route.pipeline_hints.iter().any(|h| h == "quota-dsel-free"));
        assert!(route
            .widened_models
            .iter()
            .any(|w| w.boundary == "litellm" && w.model == "moonshotai/kimi-k2"));
    }

    #[test]
    fn resolves_unified_port_route_for_anthropic_host_and_widening() {
        let route = resolve_pragmatic_unified_port_route(
            "/{api.anthropic.com:443,tools,https}/claude-opus-4-6",
        )
        .expect("route");

        assert_eq!(route.agent_name, "unified-port");
        assert_eq!(route.unified_port, DEFAULT_UNIFIED_PORT);
        assert_eq!(route.host_scope.as_deref(), Some("api.anthropic.com:443"));
        assert!(route
            .pipeline_hints
            .iter()
            .any(|h| h == "anthropic-compatible"));
        assert!(route.pipeline_hints.iter().any(|h| h == "tools"));
        assert!(route
            .dsel_tags
            .iter()
            .any(|t| t == "pipeline/anthropic-compatible"));
        assert!(route
            .widened_models
            .iter()
            .any(|w| w.boundary == "anthropic-native" && w.model == "claude-opus-4-6"));
        assert!(route
            .widened_models
            .iter()
            .any(|w| w.boundary == "litellm" && w.model == "anthropic/claude-opus-4-6"));
    }

    #[test]
    fn unified_port_route_line_surfaces_unified_port_and_pipelines() {
        let line =
            format_pragmatic_unified_port_route_line("/free/moonshotai/kimi-k2").expect("line");
        assert!(line.contains("agent=unified-port"));
        assert!(line.contains("port=8888"));
        assert!(line.contains("pipelines="));
        assert!(line.contains("model=moonshotai/kimi-k2"));
    }

    #[test]
    fn unified_port_route_accepts_caller_configured_agent_name_and_port() {
        let cfg = PragmaticUnifiedPortConfig {
            agent_name: "agent8888".to_string(),
            unified_port: 9999,
        };

        let route =
            resolve_pragmatic_unified_port_route_with_config("/free/moonshotai/kimi-k2", &cfg)
                .expect("route");
        assert_eq!(route.agent_name, "agent8888");
        assert_eq!(route.unified_port, 9999);
        assert!(route.route_key.starts_with("agent8888:default:free:"));
        assert!(route.dsel_tags.iter().any(|t| t == "agent/agent8888"));
        assert!(route.dsel_tags.iter().any(|t| t == "port/9999"));

        let line =
            format_pragmatic_unified_port_route_line_with_config("/free/moonshotai/kimi-k2", &cfg)
                .expect("line");
        assert!(line.contains("agent=agent8888"));
        assert!(line.contains("port=9999"));
    }

    #[test]
    fn quota_inventory_normalizes_litellm_compatible_records() {
        let mut metadata = BTreeMap::new();
        metadata.insert("tenant".to_string(), "alpha".to_string());
        let slots = normalize_litellm_compatible_quota_inventory(&[
            LiteLlmCompatibleQuotaInventoryRecord {
                slot_id: "litellm:moonshot/free".to_string(),
                model_id: "moonshotai/kimi-k2".to_string(),
                api_base: Some("http://127.0.0.1:4000".to_string()),
                enabled: true,
                healthy: true,
                remaining_requests: Some(120),
                remaining_tokens: Some(90_000),
                tags: vec!["modality/free".to_string(), "chat".to_string()],
                metadata,
                notes: vec!["quota-bombing".to_string()],
            },
        ]);

        assert_eq!(slots.len(), 1);
        let slot = &slots[0];
        assert_eq!(
            slot.source_kind,
            QuotaInventorySourceKind::LiteLlmCompatibleAdmin
        );
        assert_eq!(slot.slot_id, "litellm:moonshot/free");
        assert_eq!(slot.model_id, "moonshotai/kimi-k2");
        assert_eq!(slot.family_hint, Some(ProviderFamily::OpenAiCompatible));
        assert!(slot.free);
        assert!(slot.selectors.iter().any(|s| s == "free"));
        assert!(slot.selectors.iter().any(|s| s == "chat"));
        assert_eq!(slot.remaining_requests, Some(120));
        assert_eq!(slot.remaining_tokens, Some(90_000));
        assert_eq!(
            slot.metadata.get("tenant").map(String::as_str),
            Some("alpha")
        );
    }

    #[test]
    fn quota_inventory_normalizes_cc_switch_sqlite_rows() {
        let slots =
            normalize_cc_switch_sqlite_quota_inventory(&[CcSwitchSqliteQuotaInventoryRow {
                slot_id: "sqlite:row-1".to_string(),
                provider_hint: Some("anthropic".to_string()),
                model_id: "claude-opus-4-6".to_string(),
                base_url: Some("https://api.anthropic.com".to_string()),
                state: Some("cooldown".to_string()),
                enabled: true,
                remaining_requests: Some(-1),
                remaining_tokens: Some(5_000),
                selectors: vec!["modality/free".to_string()],
                metadata: BTreeMap::new(),
                notes: vec!["sqlite-local".to_string()],
            }]);

        assert_eq!(slots.len(), 1);
        let slot = &slots[0];
        assert_eq!(slot.source_kind, QuotaInventorySourceKind::CcSwitchSqlite);
        assert_eq!(slot.model_id, "anthropic/claude-opus-4-6");
        assert_eq!(slot.family_hint, Some(ProviderFamily::AnthropicCompatible));
        assert_eq!(slot.remaining_requests, None);
        assert_eq!(slot.remaining_tokens, Some(5_000));
        assert!(slot.free);
        assert!(slot.selectors.iter().any(|s| s == "free"));
        assert!(!slot.healthy);
        assert_eq!(
            slot.metadata.get("cc_switch_state").map(String::as_str),
            Some("cooldown")
        );
    }

    #[test]
    fn quota_inventory_static_mock_adapter_supports_local_scoring() {
        let adapter = StaticMockQuotaInventoryAdapter {
            records: vec![
                MockQuotaInventoryRecord {
                    slot_id: "mock-free-kimi".to_string(),
                    model_ref_or_id: "/free/moonshotai/kimi-k2".to_string(),
                    base_url: None,
                    enabled: true,
                    healthy: true,
                    free: true,
                    selectors: vec!["quota-dsel-free".to_string()],
                    remaining_requests: Some(25),
                    remaining_tokens: Some(20_000),
                    metadata: BTreeMap::new(),
                    notes: vec!["local-vm".to_string()],
                },
                MockQuotaInventoryRecord {
                    slot_id: "mock-paid-kimi".to_string(),
                    model_ref_or_id: "moonshotai/kimi-k2".to_string(),
                    base_url: None,
                    enabled: true,
                    healthy: true,
                    free: false,
                    selectors: vec!["chat".to_string()],
                    remaining_requests: Some(5),
                    remaining_tokens: Some(10_000),
                    metadata: BTreeMap::new(),
                    notes: vec![],
                },
                MockQuotaInventoryRecord {
                    slot_id: "mock-disabled".to_string(),
                    model_ref_or_id: "/free/openrouter/moonshotai/kimi-k2".to_string(),
                    base_url: None,
                    enabled: false,
                    healthy: false,
                    free: true,
                    selectors: vec![],
                    remaining_requests: Some(999),
                    remaining_tokens: Some(999_000),
                    metadata: BTreeMap::new(),
                    notes: vec![],
                },
            ],
        };

        let inventory = adapter.load_quota_inventory().expect("inventory");
        assert_eq!(inventory.len(), 3);
        assert!(inventory
            .iter()
            .any(|s| s.slot_id == "mock-free-kimi" && s.free));

        let route =
            resolve_pragmatic_unified_port_route("/free/moonshotai/kimi-k2").expect("route");
        let best = select_best_quota_inventory_slot_for_route(&route, &inventory).expect("best");

        assert_eq!(best.slot.slot_id, "mock-free-kimi");
        assert!(best.score > 0);
        assert!(best.reasons.iter().any(|r| r == "exact-model"));
        assert!(best.reasons.iter().any(|r| r == "free-modality"));
    }

    #[test]
    fn modelmux_mvp_quota_selection_companion_surfaces_top_candidate() {
        let lifecycle = run_modelmux_mvp_lifecycle(
            vec![kv("OPENAI_API_KEY", "sk-test")],
            "/free/moonshotai/kimi-k2",
        )
        .expect("lifecycle");

        let adapter = StaticMockQuotaInventoryAdapter {
            records: vec![
                MockQuotaInventoryRecord {
                    slot_id: "paid".to_string(),
                    model_ref_or_id: "moonshotai/kimi-k2".to_string(),
                    base_url: None,
                    enabled: true,
                    healthy: true,
                    free: false,
                    selectors: vec!["chat".to_string()],
                    remaining_requests: Some(9),
                    remaining_tokens: Some(9_000),
                    metadata: BTreeMap::new(),
                    notes: vec![],
                },
                MockQuotaInventoryRecord {
                    slot_id: "free".to_string(),
                    model_ref_or_id: "/free/moonshotai/kimi-k2".to_string(),
                    base_url: None,
                    enabled: true,
                    healthy: true,
                    free: true,
                    selectors: vec!["quota-dsel-free".to_string()],
                    remaining_requests: Some(4),
                    remaining_tokens: Some(4_000),
                    metadata: BTreeMap::new(),
                    notes: vec![],
                },
            ],
        };
        let slots = adapter.load_quota_inventory().expect("slots");
        let selection = evaluate_modelmux_mvp_quota_inventory(&lifecycle, &slots);

        assert_eq!(selection.candidates.len(), 2);
        assert_eq!(
            selection.selected.as_ref().map(|c| c.slot.slot_id.as_str()),
            Some("free")
        );

        let line = format_modelmux_mvp_quota_selection_line(&selection);
        assert!(line.contains("quota_candidates=2"));
        assert!(line.contains("quota_selected_slot=free"));
        assert!(line.contains("quota_selected_source=static-mock"));
        assert!(line.contains("quota_selected_usable=true"));
    }

    #[test]
    fn quota_drainer_dry_run_prefers_free_tier_first() {
        let lifecycle = run_modelmux_mvp_lifecycle(
            vec![kv("OPENAI_API_KEY", "sk-test")],
            "/free/moonshotai/kimi-k2",
        )
        .expect("lifecycle");

        let slots = normalize_mock_quota_inventory(&[
            MockQuotaInventoryRecord {
                slot_id: "free-slot".to_string(),
                model_ref_or_id: "/free/moonshotai/kimi-k2".to_string(),
                base_url: None,
                enabled: true,
                healthy: true,
                free: true,
                selectors: vec!["quota-dsel-free".to_string()],
                remaining_requests: Some(10),
                remaining_tokens: Some(10_000),
                metadata: BTreeMap::new(),
                notes: vec![],
            },
            MockQuotaInventoryRecord {
                slot_id: "paid-slot".to_string(),
                model_ref_or_id: "moonshotai/kimi-k2".to_string(),
                base_url: None,
                enabled: true,
                healthy: true,
                free: false,
                selectors: vec![],
                remaining_requests: Some(50),
                remaining_tokens: Some(50_000),
                metadata: BTreeMap::new(),
                notes: vec![],
            },
        ]);

        let result = run_modelmux_quota_drainer_dry_run(&lifecycle, &slots);
        assert!(result.ready);
        assert!(!result.fallback_used);
        assert_eq!(result.free_candidates, 1);
        assert_eq!(result.paid_candidates, 1);
        assert_eq!(
            result.selected.as_ref().map(|c| c.slot.slot_id.as_str()),
            Some("free-slot")
        );
        assert!(result
            .steps
            .iter()
            .any(|s| matches!(s.kind, QuotaDrainerDryRunStepKind::Select)
                && s.summary.contains("free-tier")));

        let line = format_quota_drainer_dry_run_line(&result);
        assert!(line.contains("quota_drainer_ready=true"));
        assert!(line.contains("fallback_used=false"));
        assert!(line.contains("selected_slot=free-slot"));
        assert!(line.contains("selected_free=true"));
    }

    #[test]
    fn quota_drainer_dry_run_falls_back_to_paid_when_free_depleted() {
        let lifecycle = run_modelmux_mvp_lifecycle(
            vec![kv("OPENAI_API_KEY", "sk-test")],
            "/free/moonshotai/kimi-k2",
        )
        .expect("lifecycle");

        let slots = normalize_mock_quota_inventory(&[
            MockQuotaInventoryRecord {
                slot_id: "free-depleted".to_string(),
                model_ref_or_id: "/free/moonshotai/kimi-k2".to_string(),
                base_url: None,
                enabled: true,
                healthy: true,
                free: true,
                selectors: vec![],
                remaining_requests: Some(0),
                remaining_tokens: Some(0),
                metadata: BTreeMap::new(),
                notes: vec![],
            },
            MockQuotaInventoryRecord {
                slot_id: "paid-fallback".to_string(),
                model_ref_or_id: "moonshotai/kimi-k2".to_string(),
                base_url: None,
                enabled: true,
                healthy: true,
                free: false,
                selectors: vec!["chat".to_string()],
                remaining_requests: Some(8),
                remaining_tokens: Some(9_000),
                metadata: BTreeMap::new(),
                notes: vec![],
            },
        ]);

        let result = run_modelmux_quota_drainer_dry_run(&lifecycle, &slots);
        assert!(result.ready);
        assert!(result.fallback_used);
        assert_eq!(
            result.selected.as_ref().map(|c| c.slot.slot_id.as_str()),
            Some("paid-fallback")
        );
        assert!(result.reason.contains("paid fallback selected"));
        assert!(result
            .steps
            .iter()
            .any(|s| matches!(s.kind, QuotaDrainerDryRunStepKind::Fallback)));

        let line = format_quota_drainer_dry_run_line(&result);
        assert!(line.contains("fallback_used=true"));
        assert!(line.contains("selected_slot=paid-fallback"));
        assert!(line.contains("selected_free=false"));
    }

    #[test]
    fn modelmux_mvp_lifecycle_tdd_free_route_search_and_key_partitioning() {
        let lifecycle = run_modelmux_mvp_lifecycle(
            vec![
                kv("BRAVE_SEARCH_API_KEY", "brv"),
                kv("OPENROUTER_API_KEY", "or-key"),
                kv("OPENROUTER_BASE_URL", "https://openrouter.ai/api"),
                kv("BINANCE_API_KEY", "bin-key"),
                kv("BINANCE_BASE_URL", "https://api.binance.com"),
            ],
            "/free/openrouter/moonshotai/kimi-k2",
        )
        .expect("lifecycle");

        assert!(lifecycle.search_enabled);
        assert_eq!(lifecycle.exchange_api_keys.len(), 1);
        assert!(!lifecycle.provider_api_keys.is_empty());
        assert_eq!(
            lifecycle
                .selected_provider_api_key
                .as_ref()
                .map(|b| b.env_key.as_str()),
            Some("OPENROUTER_API_KEY")
        );
        assert!(lifecycle.readiness.ready);
        assert!(lifecycle
            .lifecycle_tags
            .iter()
            .any(|t| t == "capability/search-enabled"));

        let line = format_modelmux_mvp_lifecycle_line(&lifecycle);
        assert!(line.contains("ready=true"));
        assert!(line.contains("search=true"));
        assert!(line.contains("provider_keys="));
        assert!(line.contains("exchange_keys=1"));
    }

    #[test]
    fn modelmux_mvp_lifecycle_tdd_probe_and_cache_reuse_on_live_vm_path() {
        let probe = MockGenericApiProbe {
            calls: Cell::new(0),
            result: Some(GenericApiModelsProbeClassification {
                api_kind: ApiKind::ModelProvider,
                family_hint: Some(ProviderFamily::OpenAiCompatible),
                confidence: 93,
                reason: "vm probe discovered openai-compatible /models".to_string(),
                matched_probe_url: Some("https://vm-gateway.internal/v1/models".to_string()),
            }),
        };
        let mut cache = GenericApiModelsProbeCache::default();
        let route_cfg = PragmaticUnifiedPortConfig {
            agent_name: "agent8888".to_string(),
            unified_port: 8888,
        };

        let first = run_modelmux_mvp_lifecycle_with_options(
            vec![
                kv("FOO_API_KEY", "foo"),
                kv("FOO_BASE_URL", "https://vm-gateway.internal"),
            ],
            "/free/moonshotai/kimi-k2",
            Some(&route_cfg),
            Some(&probe),
            Some(&mut cache),
        )
        .expect("first");
        assert!(first.readiness.ready);
        assert_eq!(probe.calls.get(), 1);
        assert_eq!(first.route.agent_name, "agent8888");
        assert!(first.route.route_key.starts_with("agent8888:default:free:"));
        assert_eq!(
            first
                .selected_provider_api_key
                .as_ref()
                .map(|b| b.env_key.as_str()),
            Some("FOO_API_KEY")
        );

        let second = run_modelmux_mvp_lifecycle_with_options(
            vec![
                kv("FOO_API_KEY", "foo"),
                kv("FOO_BASE_URL", "https://vm-gateway.internal"),
            ],
            "/free/moonshotai/kimi-k2",
            Some(&route_cfg),
            None,
            Some(&mut cache),
        )
        .expect("second");
        assert!(second.readiness.ready);
        assert_eq!(probe.calls.get(), 1);
        assert!(second
            .selected_provider_api_key
            .as_ref()
            .map(|b| b.reason.as_str())
            .unwrap_or("")
            .contains("models probe cache"));
    }

    #[test]
    fn modelmux_mvp_lifecycle_tdd_host_block_anthropic_alias_key() {
        let lifecycle = run_modelmux_mvp_lifecycle(
            vec![kv("ANTHROPIC_AUTH_TOKEN", "sk-ant")],
            "/{api.anthropic.com:443,tools,https}/claude-opus-4-6",
        )
        .expect("lifecycle");

        assert!(lifecycle.readiness.ready);
        assert_eq!(lifecycle.provider_api_keys.len(), 1);
        assert_eq!(
            lifecycle
                .selected_provider_api_key
                .as_ref()
                .map(|b| b.env_key.as_str()),
            Some("ANTHROPIC_AUTH_TOKEN")
        );
        assert!(lifecycle
            .route
            .widened_models
            .iter()
            .any(|w| w.boundary == "anthropic-native"));
        assert!(lifecycle
            .route
            .widened_models
            .iter()
            .any(|w| w.boundary == "litellm"));
    }

    #[test]
    fn modelmux_mvp_lifecycle_tdd_exchange_only_env_is_not_ready() {
        let lifecycle = run_modelmux_mvp_lifecycle(
            vec![
                kv("BINANCE_API_KEY", "bin"),
                kv("BINANCE_BASE_URL", "https://api.binance.com"),
                kv("BRAVE_SEARCH_API_KEY", "brv"),
            ],
            "/free/moonshotai/kimi-k2",
        )
        .expect("lifecycle");

        assert_eq!(lifecycle.provider_api_keys.len(), 0);
        assert_eq!(lifecycle.exchange_api_keys.len(), 1);
        assert!(lifecycle.selected_provider_api_key.is_none());
        assert!(!lifecycle.readiness.ready);
        assert!(lifecycle
            .readiness
            .reason
            .contains("no provider api key selected"));
    }

    #[test]
    fn trading_selector_route_is_ready_with_exchange_key() {
        let lifecycle = run_modelmux_mvp_lifecycle(
            vec![
                kv("BINANCE_API_KEY", "bin"),
                kv("BINANCE_BASE_URL", "https://api.binance.com"),
            ],
            "/trading/binance/btcusdt",
        )
        .expect("lifecycle");

        assert_eq!(lifecycle.provider_api_keys.len(), 0);
        assert_eq!(lifecycle.exchange_api_keys.len(), 1);
        assert!(lifecycle.readiness.ready);
        assert!(lifecycle
            .readiness
            .reason
            .contains("exchange api key selected for trading path"));
        assert!(lifecycle
            .route
            .pipeline_hints
            .iter()
            .any(|h| h == "exchange-rest"));
        assert!(lifecycle
            .route
            .pipeline_hints
            .iter()
            .any(|h| h == "trading-path"));
    }

    #[test]
    fn control_state_route_is_ready_without_provider_key() {
        let lifecycle = run_modelmux_mvp_lifecycle(
            vec![],
            "/{localhost:11434,control,https}/control/state",
        )
        .expect("lifecycle");

        assert!(lifecycle.readiness.ready);
        assert!(lifecycle
            .readiness
            .reason
            .contains("explicit host health/control path"));
    }

    #[test]
    fn modelmux_mvp_lifecycle_prefers_kilo_key_for_free_nvidia_route() {
        let lifecycle = run_modelmux_mvp_lifecycle(
            vec![
                kv("KILO_API_KEY", "kilo-key"),
                kv("NVIDIA_API_KEY", "nv-key"),
                kv("OPENAI_API_KEY", "oa-key"),
            ],
            "/free/nvidia/nemotron-3-nano-30b-a3b",
        )
        .expect("lifecycle");

        assert!(lifecycle.readiness.ready);
        assert_eq!(
            lifecycle
                .selected_provider_api_key
                .as_ref()
                .map(|b| b.env_key.as_str()),
            Some("KILO_API_KEY")
        );
    }

    #[test]
    fn modelmux_mvp_lifecycle_matches_zai_key_for_glm_route_without_gateway_key() {
        let lifecycle = run_modelmux_mvp_lifecycle(
            vec![
                kv("ZAI_API_KEY", "zai-key"),
                kv("OPENAI_API_KEY", "oa-key"),
            ],
            "/free/z-ai/glm-5",
        )
        .expect("lifecycle");

        assert!(lifecycle.readiness.ready);
        assert_eq!(
            lifecycle
                .selected_provider_api_key
                .as_ref()
                .map(|b| b.env_key.as_str()),
            Some("ZAI_API_KEY")
        );
    }

    #[test]
    fn modelmux_mvp_lifecycle_prefers_kilo_alias_key_for_free_glm_route() {
        let lifecycle = run_modelmux_mvp_lifecycle(
            vec![
                kv("KILOAI_API_KEY", "kiloai-key"),
                kv("ZAI_API_KEY", "zai-key"),
            ],
            "/free/z-ai/glm-5",
        )
        .expect("lifecycle");

        assert!(lifecycle.readiness.ready);
        assert_eq!(
            lifecycle
                .selected_provider_api_key
                .as_ref()
                .map(|b| b.env_key.as_str()),
            Some("KILOAI_API_KEY")
        );
    }

    #[test]
    fn modelmux_mvp_lifecycle_matches_kimi_alias_for_moonshot_route() {
        let lifecycle = run_modelmux_mvp_lifecycle(
            vec![
                kv("KIMI_API_KEY", "kimi-key"),
                kv("OPENAI_API_KEY", "oa-key"),
            ],
            "/free/moonshotai/kimi-k2.5",
        )
        .expect("lifecycle");

        assert!(lifecycle.readiness.ready);
        assert_eq!(
            lifecycle
                .selected_provider_api_key
                .as_ref()
                .map(|b| b.env_key.as_str()),
            Some("KIMI_API_KEY")
        );
    }

    #[test]
    fn parses_localhost_glm5_host_block_for_agent_host_quick_pick() {
        let parsed = parse_pragmatic_model_ref(
            "/{localhost:8888,chat,modality/free,meta:key=KILOAI_API_KEY,meta:quota=burst-free,note=glm5-free}/z-ai/glm-5",
        )
        .expect("parse");

        assert_eq!(parsed.host_token, "localhost:8888");
        assert_eq!(parsed.modality.as_deref(), Some("free"));
        assert_eq!(parsed.upstream_model_id, "z-ai/glm-5");
        assert_eq!(
            parsed.metadata.get("key").map(String::as_str),
            Some("KILOAI_API_KEY")
        );
        assert_eq!(
            parsed.metadata.get("quota").map(String::as_str),
            Some("burst-free")
        );
        assert_eq!(parsed.notes, vec!["glm5-free".to_string()]);
    }

    #[test]
    fn kimi_k25_host_pick_formats_local_route_line() {
        let line = format_pragmatic_unified_port_route_line(
            "/{localhost:8888,chat,modality/free,meta:key=KIMI_API_KEY,meta:quota=deep-context,note=kimi-k25-free}/moonshotai/kimi-k2.5",
        )
        .expect("line");

        assert!(line.contains("host_scope=localhost:8888"));
        assert!(line.contains("modality=free"));
        assert!(line.contains("model=moonshotai/kimi-k2.5"));
    }

    #[test]
    fn nvidia_host_pick_preserves_gateway_key_metadata() {
        let route = resolve_pragmatic_unified_port_route(
            "/{localhost:8888,chat,modality/hosted,meta:key=NVIDIA_API_KEY,meta:quota=nvidia-hosted,note=nvidia-glm5}/nvidia/z-ai/glm5",
        )
        .expect("route");

        assert_eq!(route.host_scope.as_deref(), Some("localhost:8888"));
        assert_eq!(
            route.fragments.provider_fragment.as_deref(),
            Some("nvidia")
        );
        assert_eq!(
            route.metadata.get("key").map(String::as_str),
            Some("NVIDIA_API_KEY")
        );
        assert_eq!(
            route.metadata.get("quota").map(String::as_str),
            Some("nvidia-hosted")
        );
        assert!(route.dsel_tags.iter().any(|t| t == "modality/hosted"));
    }
}
