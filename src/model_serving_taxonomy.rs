// Model-serving API taxonomy — overlay classifier for ML provider protocols
//
// Classifies HTTP request prefixes into model API categories and actions.
// Used by universal_listener to enrich Protocol::Http handling.

/// Which model provider is being targeted
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelProvider {
    OpenAi,
    Anthropic,
    Gemini,
    AzureOpenAi,
    OpenApi3,
    Unknown,
}

/// Which API action is being invoked
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelApiAction {
    ChatCompletions,
    Completions,
    Embeddings,
    Messages,       // Anthropic
    GenerateContent, // Gemini
    Models,
    Files,
    FineTune,
    Unknown,
}

/// Recommended multiplexing strategy for this API
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MuxStrategy {
    /// Route directly to one upstream
    Direct,
    /// Round-robin across upstreams
    RoundRobin,
    /// Least-connections
    LeastConn,
}

/// Full classification of an HTTP request as a model-serving API call
#[derive(Debug, Clone)]
pub struct ModelProtocolDecode {
    /// Provider family (same as provider, kept separate for logging)
    pub family: ModelProvider,
    pub provider: ModelProvider,
    pub action: ModelApiAction,
    pub path: String,
    /// Suggested request template (e.g. "openai-chat-v1")
    pub template: Option<String>,
    /// Recommended mux strategy for this endpoint
    pub default_mux: MuxStrategy,
    /// Classification confidence 0.0–1.0
    pub confidence: f32,
}

/// Classify the first bytes of an HTTP request as a model-serving API call.
/// Returns `None` if the request does not match any known model API pattern.
pub fn classify_http_request_prefix(prefix: &[u8]) -> Option<ModelProtocolDecode> {
    let text = std::str::from_utf8(prefix).ok()?;

    // Must start with an HTTP method
    let path_start = text.find(' ')? + 1;
    let path_end = text[path_start..].find(' ')
        .map(|i| path_start + i)
        .unwrap_or(text.len().min(path_start + 256));
    let path = text[path_start..path_end].to_string();

    // Detect provider from Host header or path
    let host = text.lines()
        .find(|l| l.to_ascii_lowercase().starts_with("host:"))
        .map(|l| l[5..].trim().to_ascii_lowercase())
        .unwrap_or_default();

    let provider = if host.contains("api.openai.com") || path.starts_with("/v1/") && !host.contains("anthropic") {
        ModelProvider::OpenAi
    } else if host.contains("anthropic.com") || path.starts_with("/v1/messages") {
        ModelProvider::Anthropic
    } else if host.contains("generativelanguage.googleapis.com") || path.contains("gemini") || path.contains("generateContent") {
        ModelProvider::Gemini
    } else if host.contains("openai.azure.com") {
        ModelProvider::AzureOpenAi
    } else if path.contains("/openapi") || path.contains("/swagger") {
        ModelProvider::OpenApi3
    } else {
        return None; // Not a recognized model API
    };

    let action = classify_action(&path, &provider);

    let template = template_for(&provider, &action);
    let default_mux = mux_for(&action);
    let confidence = if matches!(provider, ModelProvider::Unknown) { 0.5 } else { 0.95 };

    Some(ModelProtocolDecode {
        family: provider.clone(),
        provider,
        action,
        path,
        template,
        default_mux,
        confidence,
    })
}

fn template_for(provider: &ModelProvider, action: &ModelApiAction) -> Option<String> {
    let p = match provider {
        ModelProvider::OpenAi      => "openai",
        ModelProvider::Anthropic   => "anthropic",
        ModelProvider::Gemini      => "gemini",
        ModelProvider::AzureOpenAi => "azure-openai",
        ModelProvider::OpenApi3    => "openapi3",
        ModelProvider::Unknown     => return None,
    };
    let a = match action {
        ModelApiAction::ChatCompletions  => "chat",
        ModelApiAction::Completions      => "completions",
        ModelApiAction::Embeddings       => "embeddings",
        ModelApiAction::Messages         => "messages",
        ModelApiAction::GenerateContent  => "generate",
        ModelApiAction::Models           => "models",
        _                                => return None,
    };
    Some(format!("{}-{}-v1", p, a))
}

fn mux_for(action: &ModelApiAction) -> MuxStrategy {
    match action {
        ModelApiAction::ChatCompletions | ModelApiAction::Messages => MuxStrategy::LeastConn,
        ModelApiAction::Embeddings                                  => MuxStrategy::RoundRobin,
        _                                                           => MuxStrategy::Direct,
    }
}

fn classify_action(path: &str, provider: &ModelProvider) -> ModelApiAction {
    if path.contains("/chat/completions") { return ModelApiAction::ChatCompletions; }
    if path.contains("/completions")      { return ModelApiAction::Completions; }
    if path.contains("/embeddings")       { return ModelApiAction::Embeddings; }
    if path.contains("/messages")         { return ModelApiAction::Messages; }
    if path.contains("generateContent")   { return ModelApiAction::GenerateContent; }
    if path.contains("/models")           { return ModelApiAction::Models; }
    if path.contains("/files")            { return ModelApiAction::Files; }
    if path.contains("/fine_tun") || path.contains("/fine-tun") { return ModelApiAction::FineTune; }
    let _ = provider;
    ModelApiAction::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_chat_completions() {
        let data = b"POST /v1/chat/completions HTTP/1.1\r\nHost: api.openai.com\r\n\r\n";
        let decoded = classify_http_request_prefix(data).expect("should decode");
        assert_eq!(decoded.provider, ModelProvider::OpenAi);
        assert!(matches!(decoded.action, ModelApiAction::ChatCompletions));
    }

    #[test]
    fn anthropic_messages() {
        let data = b"POST /v1/messages HTTP/1.1\r\nHost: api.anthropic.com\r\n\r\n";
        let decoded = classify_http_request_prefix(data).expect("should decode");
        assert_eq!(decoded.provider, ModelProvider::Anthropic);
        assert!(matches!(decoded.action, ModelApiAction::Messages));
    }

    #[test]
    fn unknown_returns_none() {
        let data = b"GET /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n";
        assert!(classify_http_request_prefix(data).is_none());
    }
}
