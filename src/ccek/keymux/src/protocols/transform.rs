//! Protocol translation logic
//! Ported from cc-switch (MIT License)

use serde_json::{json, Map, Value};

/// Anthropic Request → OpenAI Request
pub fn anthropic_to_openai(body: Value) -> Value {
    let mut result = json!({});

    if let Some(model) = body.get("model").and_then(|m| m.as_str()) {
        result["model"] = json!(model);
    }

    let mut messages = Vec::new();

    // Handle system prompt
    if let Some(system) = body.get("system") {
        if let Some(text) = system.as_str() {
            messages.push(json!({"role": "system", "content": text}));
        } else if let Some(arr) = system.as_array() {
            for msg in arr {
                if let Some(text) = msg.get("text").and_then(|t| t.as_str()) {
                    messages.push(json!({"role": "system", "content": text}));
                }
            }
        }
    }

    // Convert messages
    if let Some(msgs) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in msgs {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content = msg.get("content");

            // Simplified conversion for P0
            if let Some(text) = content.and_then(|c| c.as_str()) {
                messages.push(json!({"role": role, "content": text}));
            } else if let Some(arr) = content.and_then(|c| c.as_array()) {
                // Handle multi-part content
                messages.push(json!({"role": role, "content": arr.clone()}));
            }
        }
    }

    result["messages"] = json!(messages);

    // Pass through common parameters
    for key in ["max_tokens", "temperature", "top_p", "stream"] {
        if let Some(v) = body.get(key) {
            result[key] = v.clone();
        }
    }

    result
}

/// OpenAI Response → Anthropic Response
pub fn openai_to_anthropic(body: Value) -> Value {
    let choices = body.get("choices").and_then(|c| c.as_array());
    let choice = choices.and_then(|c| c.first());
    let message = choice.and_then(|c| c.get("message"));

    let mut content = Vec::new();
    if let Some(text) = message
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
    {
        content.push(json!({"type": "text", "text": text}));
    }

    let usage = body.get("usage");
    let input_tokens = usage
        .and_then(|u| u.get("prompt_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let output_tokens = usage
        .and_then(|u| u.get("completion_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    json!({
        "id": body.get("id").and_then(|i| i.as_str()).unwrap_or(""),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": body.get("model").and_then(|m| m.as_str()).unwrap_or(""),
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens
        }
    })
}
