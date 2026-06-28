//! Shared domain types for subscription fetch, parse, and normalize pipelines.

mod credentials;
mod error;
mod fingerprint;
mod node;
mod protocol;
mod subscription;
mod tls;
mod transport;

pub use credentials::Credentials;
pub use error::ConfluxError;
pub use fingerprint::stable_node_id;
pub use node::{ConfluxNode, NodeMeta, NodeSource, RawPayload};
pub use protocol::Protocol;
pub use subscription::{
    ConfluxSubscription, SubscriptionExtras, SubscriptionHeaders, SubscriptionUserInfo,
};
pub use tls::{RealityConfig, TlsConfig};
pub use transport::{ObfsConfig, Transport, TransportKind};
