use serde::{Deserialize, Serialize};

/// TLS settings for a proxy connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TlsConfig {
    pub enabled: bool,
    pub sni: Option<String>,
    pub alpn: Vec<String>,
    pub fingerprint: Option<String>,
    pub insecure: bool,
}

/// REALITY-specific TLS parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RealityConfig {
    pub public_key: String,
    pub short_id: String,
    pub spider_x: Option<String>,
}
