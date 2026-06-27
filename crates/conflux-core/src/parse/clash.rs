use conflux_protocol::ConfluxError;
use serde::Deserialize;

use super::RawNode;

#[derive(Debug, Deserialize)]
struct ClashDocument {
    proxies: Option<Vec<serde_yaml::Value>>,
    #[serde(rename = "proxy-groups")]
    proxy_groups: Option<serde_yaml::Value>,
    rules: Option<serde_yaml::Value>,
}

pub struct ClashParseResult {
    pub nodes: Vec<RawNode>,
    pub proxy_groups: Option<serde_json::Value>,
    pub rules: Option<serde_json::Value>,
}

/// Extract `proxies[]` entries from a Clash-compatible YAML document.
pub fn parse_clash_yaml(body: &str) -> Result<ClashParseResult, ConfluxError> {
    let doc: ClashDocument = serde_yaml::from_str(body)
        .map_err(|err| ConfluxError::Parse(format!("invalid Clash YAML: {err}")))?;

    let proxies = doc.proxies.unwrap_or_default();
    let mut nodes = Vec::new();

    for (index, proxy) in proxies.into_iter().enumerate() {
        let Some(obj) = proxy.as_mapping() else {
            continue;
        };

        let proxy_type = mapping_string(obj, "type").unwrap_or_default();
        let name = mapping_string(obj, "name");
        let server = mapping_string(obj, "server").unwrap_or_default();
        let port = mapping_u16(obj, "port").unwrap_or(0);

        if server.is_empty() || port == 0 {
            continue;
        }

        let scheme = clash_type_to_scheme(&proxy_type);
        let raw_uri = format!(
            "clash://{scheme}/{server}:{port}#{}",
            name.as_deref().unwrap_or("")
        );

        nodes.push(RawNode {
            name,
            scheme,
            raw_uri,
            line_index: Some(index),
            clash_proxy: Some(serde_json::to_value(&proxy).unwrap_or(serde_json::Value::Null)),
        });
    }

    if nodes.is_empty() {
        return Err(ConfluxError::Parse(
            "Clash YAML did not contain usable proxies".into(),
        ));
    }

    Ok(ClashParseResult {
        nodes,
        proxy_groups: doc
            .proxy_groups
            .map(|value| serde_json::to_value(value).unwrap_or(serde_json::Value::Null)),
        rules: doc
            .rules
            .map(|value| serde_json::to_value(value).unwrap_or(serde_json::Value::Null)),
    })
}

fn clash_type_to_scheme(proxy_type: &str) -> String {
    match proxy_type.to_ascii_lowercase().as_str() {
        "ss" | "shadowsocks" => "ss".to_string(),
        "vmess" => "vmess".to_string(),
        "vless" => "vless".to_string(),
        "trojan" => "trojan".to_string(),
        "hysteria2" | "hy2" => "hysteria2".to_string(),
        "tuic" => "tuic".to_string(),
        "socks5" | "socks" => "socks".to_string(),
        other => other.to_string(),
    }
}

fn mapping_string(map: &serde_yaml::Mapping, key: &str) -> Option<String> {
    map.get(serde_yaml::Value::String(key.to_string()))
        .and_then(|value| match value {
            serde_yaml::Value::String(text) => Some(text.clone()),
            serde_yaml::Value::Number(number) => Some(number.to_string()),
            _ => None,
        })
}

fn mapping_u16(map: &serde_yaml::Mapping, key: &str) -> Option<u16> {
    map.get(serde_yaml::Value::String(key.to_string()))
        .and_then(|value| match value {
            serde_yaml::Value::Number(number) => {
                number.as_u64().and_then(|n| u16::try_from(n).ok())
            }
            serde_yaml::Value::String(text) => text.parse().ok(),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_clash_proxies() {
        let yaml = r#"
proxies:
  - name: Test VLESS
    type: vless
    server: example.com
    port: 443
    uuid: 00000000-0000-0000-0000-000000000001
    tls: true
    servername: example.com
  - name: Test SS
    type: ss
    server: example.com
    port: 8388
    cipher: aes-256-gcm
    password: password
"#;
        let parsed = parse_clash_yaml(yaml).expect("parse");
        assert_eq!(parsed.nodes.len(), 2);
        assert_eq!(parsed.nodes[0].scheme, "vless");
        assert_eq!(parsed.nodes[1].scheme, "ss");
    }
}
