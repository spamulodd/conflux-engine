use serde::{Deserialize, Serialize};

/// Transport layer kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    #[default]
    Tcp,
    Ws,
    Grpc,
    Http,
    HttpUpgrade,
    Kcp,
    Quic,
    H2,
}

/// Transport settings for a proxy node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Transport {
    pub kind: TransportKind,
    pub path: Option<String>,
    pub host: Option<String>,
    pub service_name: Option<String>,
    pub headers: Vec<(String, String)>,
    pub header_type: Option<String>,
}

/// Obfuscation settings (e.g. Hysteria2 salamander).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObfsConfig {
    pub kind: String,
    pub password: Option<String>,
}
