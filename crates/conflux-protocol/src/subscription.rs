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
