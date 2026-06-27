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

impl Credentials {
    pub fn redacted_for_ipc(&self) -> Self {
        use crate::redact::IPC_REDACTED;

        match self {
            Self::Uuid { .. } => Self::Uuid {
                id: Uuid::nil(),
            },
            Self::Password { .. } => Self::Password {
                password: IPC_REDACTED.to_string(),
            },
            Self::Shadowsocks { method, .. } => Self::Shadowsocks {
                method: method.clone(),
                password: IPC_REDACTED.to_string(),
            },
            Self::KeyPair { .. } => Self::KeyPair {
                private_key: IPC_REDACTED.to_string(),
                public_key: None,
            },
            Self::NativeKey { .. } => Self::NativeKey {
                key_hex: IPC_REDACTED.to_string(),
            },
            Self::None => Self::None,
        }
    }
}
