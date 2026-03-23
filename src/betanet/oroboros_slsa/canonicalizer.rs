use serde_json::{Map, Value};

pub fn canonicalize(json: &str) -> String {
    // Parse input JSON
    let v: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };

    // Recursively sort object keys
    fn sort_value(v: Value) -> Value {
        match v {
            Value::Object(mut m) => {
                // Sort keys and rebuild map
                let mut entries: Vec<(String, Value)> = m
                    .into_iter()
                    .map(|(k, v)| (k, sort_value(v)))
                    .collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                let mut out = Map::new();
                for (k, v) in entries {
                    out.insert(k, v);
                }
                Value::Object(out)
            }
            Value::Array(arr) => Value::Array(arr.into_iter().map(sort_value).collect()),
            other => other,
        }
    }

    let sorted = sort_value(v);

    // Serialize compactly (no extra spaces)
    serde_json::to_string(&sorted).unwrap_or_default()
}
