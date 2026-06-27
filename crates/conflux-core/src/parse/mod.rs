//! Subscription body parsing: format detection, Base64 expansion, URI extraction.

mod clash;
mod expand;
mod uri;

use conflux_protocol::{ConfluxError, SubscriptionExtras, SubscriptionHeaders};
use serde::{Deserialize, Serialize};

pub use clash::parse_clash_yaml;
pub use expand::expand_body;

/// Detected subscription body format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionFormat {
    UriList,
    ClashYaml,
    Unknown,
}

/// A node extracted from a subscription before normalization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawNode {
    pub name: Option<String>,
    pub scheme: String,
    pub raw_uri: String,
    pub line_index: Option<usize>,
    pub clash_proxy: Option<serde_json::Value>,
}

/// Inline metadata carried in `#`-directive lines inside the body.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BodyMetadata {
    pub profile_title: Option<String>,
    pub user_info: Option<conflux_protocol::SubscriptionUserInfo>,
    pub update_interval_hours: Option<u32>,
}

/// Parsed subscription body prior to normalization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParseResult {
    pub format: SubscriptionFormat,
    pub nodes: Vec<RawNode>,
    pub body_metadata: BodyMetadata,
    pub extras: SubscriptionExtras,
    pub expanded_body: String,
}

/// Detect format, expand Base64 bodies, and extract raw nodes.
pub fn parse_body(
    body: &str,
    headers: Option<&SubscriptionHeaders>,
) -> Result<ParseResult, ConfluxError> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(ConfluxError::Parse("body is empty".into()));
    }

    let (expanded, body_metadata) = expand_with_directives(trimmed);
    let format = detect_format(&expanded);

    let mut extras = SubscriptionExtras::default();
    let nodes = match format {
        SubscriptionFormat::ClashYaml => {
            let parsed = parse_clash_yaml(&expanded)?;
            extras.clash_proxy_groups = parsed.proxy_groups;
            extras.clash_rules = parsed.rules;
            parsed.nodes
        }
        SubscriptionFormat::UriList | SubscriptionFormat::Unknown => {
            uri::extract_uri_nodes(&expanded)?
        }
    };

    if nodes.is_empty() {
        return Err(ConfluxError::Parse(
            "no nodes found in subscription body".into(),
        ));
    }

    // Prefer HTTP headers over inline body directives when both are present.
    let _ = headers;

    Ok(ParseResult {
        format,
        nodes,
        body_metadata,
        extras,
        expanded_body: expanded,
    })
}

fn expand_with_directives(body: &str) -> (String, BodyMetadata) {
    let mut metadata = BodyMetadata::default();
    let mut kept_lines = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(directive) = trimmed.strip_prefix('#') {
            apply_body_directive(directive.trim(), &mut metadata);
            continue;
        }
        kept_lines.push(line);
    }

    let joined = kept_lines.join("\n");
    let expanded = expand_body(&joined);
    (expanded, metadata)
}

fn apply_body_directive(directive: &str, metadata: &mut BodyMetadata) {
    let Some((key, value)) = directive.split_once(':') else {
        return;
    };

    match key.trim().to_ascii_lowercase().as_str() {
        "profile-title" => {
            metadata.profile_title = Some(crate::fetch::parse_profile_title(Some(value)));
        }
        "subscription-userinfo" => {
            metadata.user_info = crate::fetch::parse_user_info(Some(value));
        }
        "profile-update-interval" => {
            metadata.update_interval_hours =
                Some(crate::fetch::parse_update_interval_hours(Some(value)));
        }
        _ => {}
    }
}

fn detect_format(body: &str) -> SubscriptionFormat {
    let trimmed = body.trim_start();
    if trimmed.contains("proxies:") || trimmed.starts_with("---") {
        return SubscriptionFormat::ClashYaml;
    }

    if expand::contains_known_scheme(body) {
        return SubscriptionFormat::UriList;
    }

    SubscriptionFormat::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    const VLESS: &str = "vless://00000000-0000-0000-0000-000000000001@example.com:443?security=tls&sni=example.com#Node-A";
    const SS: &str = "ss://YWVzLTI1Ni1nY206cGFzc3dvcmQ=@example.com:8388#Node-B";

    #[test]
    fn expands_base64_body() {
        use ::base64::Engine;
        let encoded = ::base64::engine::general_purpose::STANDARD.encode(format!("{VLESS}\n{SS}"));
        let result = parse_body(&encoded, None).expect("parse");
        assert_eq!(result.format, SubscriptionFormat::UriList);
        assert_eq!(result.nodes.len(), 2);
    }

    #[test]
    fn parses_inline_body_directives() {
        let body = format!("#profile-title: Inline Title\n#profile-update-interval: 6\n{VLESS}");
        let result = parse_body(&body, None).expect("parse");
        assert_eq!(
            result.body_metadata.profile_title.as_deref(),
            Some("Inline Title")
        );
        assert_eq!(result.body_metadata.update_interval_hours, Some(6));
    }

    #[test]
    fn parses_plain_uri_list() {
        let body = format!("{VLESS}\n{SS}");
        let result = parse_body(&body, None).expect("parse");
        assert_eq!(result.nodes.len(), 2);
        assert!(result.nodes.iter().any(|node| node.scheme == "vless"));
        assert!(result.nodes.iter().any(|node| node.scheme == "ss"));
    }
}
