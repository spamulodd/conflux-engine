use serde_json::{Map, Value};

pub(crate) const IPC_REDACTED: &str = "[redacted]";

/// Redact credential-like fields from nested JSON (Clash proxies, extras).
pub(crate) fn redact_sensitive_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut redacted = Map::new();
            for (key, child) in map {
                redacted.insert(key.clone(), redact_json_field(key, child));
            }
            Value::Object(redacted)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_sensitive_json).collect()),
        other => other.clone(),
    }
}

fn redact_json_field(key: &str, value: &Value) -> Value {
    match key {
        "uuid"
        | "password"
        | "private_key"
        | "public_key"
        | "short_id"
        | "key_hex"
        | "obfs-password"
        | "obfs_password" => Value::String(IPC_REDACTED.to_string()),
        _ => redact_sensitive_json(value),
    }
}

pub(crate) fn redact_optional_url(url: &Option<String>) -> Option<String> {
    url.as_ref().map(|_| IPC_REDACTED.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_clash_proxy_password_fields() {
        let input = json!({
            "name": "node-1",
            "type": "trojan",
            "server": "example.com",
            "password": "secret-token"
        });
        let output = redact_sensitive_json(&input);
        assert_eq!(output["password"], IPC_REDACTED);
        assert_eq!(output["server"], "example.com");
    }

    #[test]
    fn redacts_nested_proxy_arrays() {
        let input = json!({
            "proxies": [
                { "uuid": "550e8400-e29b-41d4-a716-446655440000" }
            ]
        });
        let output = redact_sensitive_json(&input);
        assert_eq!(output["proxies"][0]["uuid"], IPC_REDACTED);
    }
}
