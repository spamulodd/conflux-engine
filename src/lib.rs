//! Public facade for the conflux subscription engine.

pub use conflux_backend::{Backend, SingboxBackend};
pub use conflux_core::{
    fetch_and_normalize, fetch_subscription, normalize, parse_and_normalize, parse_body,
    BodyMetadata, ConfluxError, ConfluxNode, ConfluxSubscription, Credentials, FetchResult,
    NodeMeta, NodeSource, ObfsConfig, ParseResult, Protocol, RawNode, RawPayload, RealityConfig,
    SubscriptionExtras, SubscriptionFormat, SubscriptionHeaders, SubscriptionUserInfo, TlsConfig,
    Transport, TransportKind,
};
pub use conflux_protocol;
