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

    /// FETCH response payload: summary plus the redacted profile from this fetch.
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
        Self {
            title: self.title.clone(),
            source_url: self.source_url.clone(),
            update_interval_hours: self.update_interval_hours,
            user_info: self.user_info.clone(),
            support_url: self.support_url.clone(),
            announce: self.announce.clone(),
            nodes: self
                .nodes
                .iter()
                .map(ConfluxNode::redacted_for_ipc)
                .collect(),
            extras: self.extras.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_ipc_response_data_includes_profile() {
        let profile = ConfluxSubscription {
            title: "Test".into(),
            source_url: Some("https://example.com/sub".into()),
            update_interval_hours: 12,
            user_info: None,
            support_url: None,
            announce: None,
            nodes: vec![],
            extras: SubscriptionExtras::default(),
        };

        let data = profile.fetch_ipc_response_data();
        assert_eq!(data["node_count"], 0);
        assert_eq!(data["profile"]["title"], "Test");
        assert!(data["profile"]["nodes"].as_array().unwrap().is_empty());
    }
}
