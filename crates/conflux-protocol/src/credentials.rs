use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Authentication material for a proxy node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Credentials {
    Uuid {
        id: Uuid,
    },
    Password {
        password: String,
    },
    Shadowsocks {
        method: String,
        password: String,
    },
    KeyPair {
        private_key: String,
        public_key: Option<String>,
    },
    /// Native tunnel shared secret (hex-encoded).
    NativeKey {
        key_hex: String,
    },
    None,
}
