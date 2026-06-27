use serde::{Deserialize, Serialize};

use crate::{Credentials, Protocol, RealityConfig, TlsConfig, Transport};

/// Where a node was discovered during parsing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NodeSource {
    pub subscription_url: Option<String>,
    pub raw_uri: Option<String>,
    pub line_index: Option<usize>,
    pub parser: Option<String>,
}

/// Original payload preserved for round-trip or debugging.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RawPayload {
    Uri { value: String },
    ClashProxy { value: serde_json::Value },
    Json { value: serde_json::Value },
}

/// Display and grouping metadata extracted from names or provider hints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NodeMeta {
    pub country_code: Option<String>,
    pub flag: Option<String>,
    pub server_description: Option<String>,
    pub group_hint: Option<String>,
}

/// Canonical normalized proxy node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfluxNode {
    pub id: String,
    pub tag: String,
    pub protocol: Protocol,
    pub source: NodeSource,
    pub server: String,
    pub port: u16,
    pub ports: Option<Vec<String>>,
    pub credentials: Credentials,
    pub transport: Transport,
    pub tls: Option<TlsConfig>,
    pub reality: Option<RealityConfig>,
    pub flow: Option<String>,
    pub encryption: Option<String>,
    pub packet_encoding: Option<String>,
    pub method: Option<String>,
    pub obfs: Option<crate::ObfsConfig>,
    pub meta: NodeMeta,
    pub raw: RawPayload,
    /// Native tunnel profile name when protocol is NativeTunnel.
    pub native_profile: Option<String>,
    /// Native tunnel CIDR when protocol is NativeTunnel.
    pub native_tun_cidr: Option<String>,
    /// Usage endpoint for native tunnel subscriptions.
    pub usage_url: Option<String>,
}

impl ConfluxNode {
    /// Node metadata safe for IPC transport (credentials and raw URI redacted).
    pub fn redacted_for_ipc(&self) -> Self {
        use crate::redact::{redact_optional_url, redact_sensitive_json, IPC_REDACTED};

        let mut node = self.clone();
        node.credentials = node.credentials.redacted_for_ipc();
        node.source.subscription_url = redact_optional_url(&node.source.subscription_url);
        node.source.raw_uri = redact_optional_url(&node.source.raw_uri);
        node.usage_url = redact_optional_url(&node.usage_url);
        node.obfs = node.obfs.as_ref().map(|obfs| crate::ObfsConfig {
            kind: obfs.kind.clone(),
            password: redact_optional_url(&obfs.password),
        });
        node.reality = node.reality.as_ref().map(|reality| crate::RealityConfig {
            public_key: IPC_REDACTED.to_string(),
            short_id: IPC_REDACTED.to_string(),
            spider_x: reality.spider_x.clone(),
        });
        node.raw = match &node.raw {
            RawPayload::Uri { .. } => RawPayload::Uri {
                value: IPC_REDACTED.to_string(),
            },
            RawPayload::ClashProxy { value } => RawPayload::ClashProxy {
                value: redact_sensitive_json(value),
            },
            RawPayload::Json { value } => RawPayload::Json {
                value: redact_sensitive_json(value),
            },
        };
        node
    }
}
