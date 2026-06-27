//! Shared domain types for subscription fetch, parse, and normalize pipelines.

mod credentials;
mod error;
mod node;
mod protocol;
mod redact;
mod subscription;
mod tls;
mod transport;

pub use credentials::Credentials;
pub use error::ConfluxError;
pub use node::{ConfluxNode, NodeMeta, NodeSource, RawPayload};
pub use protocol::Protocol;
pub use subscription::{
    ConfluxSubscription, SubscriptionExtras, SubscriptionHeaders, SubscriptionUserInfo,
};
pub use tls::{RealityConfig, TlsConfig};
pub use transport::{ObfsConfig, Transport, TransportKind};

/// Redact subscription URLs before exposing them over IPC.
pub fn redact_url_for_ipc(url: &Option<String>) -> Option<String> {
    redact::redact_optional_url(url)
}
