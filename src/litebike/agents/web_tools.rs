use serde::{Deserialize, Serialize};

/// Web search and JSON processing tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchRequest {
    pub query: String,
    pub provider: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonToolConfig {
    pub enabled: bool,
    pub max_depth: usize,
    pub validate_schema: bool,
    pub pretty_print: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchProvider {
    pub name: String,
    pub api_key: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebTools {
    pub providers: Vec<WebSearchProvider>,
    pub config: JsonToolConfig,
}

impl WebTools {
    pub fn new() -> Self {
        Self {
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
            config: JsonToolConfig {
                enabled: true,
                max_depth: 32,
                validate_schema: true,
                pretty_print: true,
            },
        }
    }

    pub async fn search(&self, request: WebSearchRequest) -> Result<Vec<WebSearchResult>, String> {
        let provider_name = request.provider.as_deref().unwrap_or("brave");
        let provider = self.providers.iter()
            .find(|p| p.name == provider_name)
            .ok_or_else(|| format!("Provider {} not found", provider_name))?;

        // Mock implementation - in real app, would make HTTP request
        Ok(vec![
            WebSearchResult {
                title: format!("{} search for: {}", provider_name, request.query),
                url: format!("https://example.com/search?q={}", request.query),
                snippet: format!("Results for query: {}", request.query),
                score: 0.95,
            },
        ])
    }

    pub fn process_json(&self, json_str: &str) -> Result<serde_json::Value, String> {
        let value: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| format!("Invalid JSON: {}", e))?;

        if self.config.validate_schema {
            // Basic validation - check for common issues
            self.validate_json_structure(&value)?;
        }

        if self.config.pretty_print {
            let pretty = serde_json::to_string_pretty(&value)
                .map_err(|e| format!("Failed to pretty print: {}", e))?;
            return serde_json::from_str(&pretty)
                .map_err(|e| format!("Failed to re-parse pretty JSON: {}", e));
        }

        Ok(value)
    }

    fn validate_json_structure(&self, value: &serde_json::Value) -> Result<(), String> {
        match value {
            serde_json::Value::Object(map) => {
                if map.len() > 1000 {
                    return Err("JSON object too large".to_string());
                }
                for (key, val) in map {
                    if key.len() > 1024 {
                        return Err("Key too long".to_string());
                    }
                    self.validate_json_structure(val)?;
                }
            }
            serde_json::Value::Array(arr) => {
                if arr.len() > 10000 {
                    return Err("JSON array too large".to_string());
                }
                for item in arr {
                    self.validate_json_structure(item)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn create_config(&self) -> serde_json::Value {
        serde_json::json!({
            "web_search": {
                "enabled": true,
                "providers": self.providers.iter().map(|p| serde_json::json!({
                    "name": p.name,
                    "endpoint": p.endpoint,
                })).collect::<Vec<_>>(),
            },
            "json_tools": {
                "enabled": self.config.enabled,
                "max_depth": self.config.max_depth,
                "validate_schema": self.config.validate_schema,
                "pretty_print": self.config.pretty_print,
            }
        })
    }
}

impl Default for WebTools {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_tools_creation() {
        let tools = WebTools::new();
        assert!(tools.providers.len() >= 2);
        assert!(tools.config.enabled);
        assert_eq!(tools.config.max_depth, 32);
    }

    #[test]
    fn test_json_processing() {
        let tools = WebTools::new();
        let json_str = r#"{"key": "value", "number": 42}"#;
        let result = tools.process_json(json_str);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed["key"], "value");
        assert_eq!(parsed["number"], 42);
    }

    #[test]
    fn test_invalid_json() {
        let tools = WebTools::new();
        let result = tools.process_json("invalid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_export() {
        let tools = WebTools::new();
        let config = tools.create_config();
        assert!(config["web_search"].is_object());
        assert!(config["json_tools"].is_object());
    }
}
