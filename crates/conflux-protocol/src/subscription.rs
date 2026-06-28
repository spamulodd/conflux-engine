use serde::{Deserialize, Serialize};

use crate::ConfluxNode;

/// HTTP response headers commonly attached to proxy subscriptions.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SubscriptionHeaders {
    pub profile_title: Option<String>,
    pub user_info: Option<SubscriptionUserInfo>,
    pub update_interval_hours: u32,
    pub announce: Option<String>,
    pub support_url: Option<String>,
}

/// Clash-style quota snapshot from the Subscription-Userinfo header.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SubscriptionUserInfo {
    pub upload_bytes: i64,
    pub download_bytes: i64,
    /// Zero means unlimited.
    pub total_bytes: i64,
    pub expire_unix: Option<i64>,
    pub refill_unix: Option<i64>,
}

impl SubscriptionUserInfo {
    pub fn used_bytes(&self) -> i64 {
        self.upload_bytes + self.download_bytes
    }
}

/// Passthrough metadata not mapped to individual nodes.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SubscriptionExtras {
    pub clash_proxy_groups: Option<serde_json::Value>,
    pub clash_rules: Option<serde_json::Value>,
}

/// Normalized subscription profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfluxSubscription {
    pub title: String,
    pub source_url: Option<String>,
    pub update_interval_hours: u32,
    pub user_info: Option<SubscriptionUserInfo>,
    pub support_url: Option<String>,
    pub announce: Option<String>,
    pub nodes: Vec<ConfluxNode>,
    pub extras: SubscriptionExtras,
}

impl ConfluxSubscription {
    /// Summary returned by IPC `FETCH` (no node list, no credentials).
    pub fn fetch_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "title": self.title,
            "node_count": self.nodes.len(),
            "update_interval_hours": self.update_interval_hours,
            "user_info": self.user_info,
            "support_url": self.support_url,
            "announce": self.announce,
        })
    }

    /// FETCH IPC payload: summary plus redacted profile for single-round-trip clients.
    pub fn fetch_ipc_response_data(&self) -> serde_json::Value {
        let redacted = self.redacted_for_ipc();
        let mut data = self.fetch_summary();
        if let serde_json::Value::Object(ref mut map) = data {
            map.insert(
                "profile".to_string(),
                serde_json::to_value(&redacted).expect("redacted profile serializes"),
            );
        }
        data
    }

    /// Profile safe for IPC transport: credentials and raw URIs are redacted.
    pub fn redacted_for_ipc(&self) -> Self {
        use crate::redact::{redact_optional_url, redact_sensitive_json};

        Self {
            title: self.title.clone(),
            source_url: redact_optional_url(&self.source_url),
            update_interval_hours: self.update_interval_hours,
            user_info: self.user_info.clone(),
            support_url: self.support_url.clone(),
            announce: self.announce.clone(),
            nodes: self
                .nodes
                .iter()
                .map(ConfluxNode::redacted_for_ipc)
                .collect(),
            extras: SubscriptionExtras {
                clash_proxy_groups: self
                    .extras
                    .clash_proxy_groups
                    .as_ref()
                    .map(redact_sensitive_json),
                clash_rules: self.extras.clash_rules.as_ref().map(redact_sensitive_json),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Credentials, NodeMeta, NodeSource, ObfsConfig, Protocol, RawPayload, Transport,
        TransportKind,
    };
    use serde_json::json;
    use uuid::Uuid;

    fn sample_node() -> ConfluxNode {
        ConfluxNode {
            id: "test".into(),
            tag: "test-node".into(),
            protocol: Protocol::Vless,
            source: NodeSource {
                subscription_url: Some("https://example.com/sub/secret-token".into()),
                raw_uri: Some("vless://uuid@host:443".into()),
                line_index: Some(0),
                parser: Some("uri".into()),
            },
            server: "example.com".into(),
            port: 443,
            ports: None,
            credentials: Credentials::Uuid {
                id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            },
            transport: Transport {
                kind: TransportKind::Tcp,
                ..Default::default()
            },
            tls: None,
            reality: None,
            flow: None,
            encryption: None,
            packet_encoding: None,
            method: None,
            obfs: Some(ObfsConfig {
                kind: "salamander".into(),
                password: Some("obfs-secret".into()),
            }),
            meta: NodeMeta::default(),
            raw: RawPayload::ClashProxy {
                value: json!({
                    "name": "test-node",
                    "type": "trojan",
                    "password": "clash-secret"
                }),
            },
            native_profile: None,
            native_tun_cidr: None,
            usage_url: Some("https://example.com/usage/secret".into()),
        }
    }

    #[test]
    fn redacted_profile_strips_ipc_secrets() {
        let profile = ConfluxSubscription {
            title: "Test".into(),
            source_url: Some("https://example.com/sub/secret-token".into()),
            update_interval_hours: 24,
            user_info: None,
            support_url: None,
            announce: None,
            nodes: vec![sample_node()],
            extras: SubscriptionExtras::default(),
        };

        let redacted = profile.redacted_for_ipc();
        let node = &redacted.nodes[0];

        assert_eq!(
            redacted.source_url,
            Some(crate::redact::IPC_REDACTED.to_string())
        );
        assert_eq!(node.credentials, Credentials::Uuid { id: Uuid::nil() });
        assert_eq!(node.obfs.as_ref().unwrap().password.as_deref(), Some("[redacted]"));
        assert_eq!(
            node.raw,
            RawPayload::ClashProxy {
                value: json!({
                    "name": "test-node",
                    "type": "trojan",
                    "password": "[redacted]"
                })
            }
        );
        assert_eq!(node.usage_url.as_deref(), Some("[redacted]"));
    }
}
