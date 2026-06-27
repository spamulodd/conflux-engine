//! Fetch, parse, and normalize proxy subscription content.

pub mod fetch;
pub mod normalize;
pub mod parse;

pub use conflux_protocol::{
    ConfluxError, ConfluxNode, ConfluxSubscription, Credentials, NodeMeta, NodeSource, ObfsConfig,
    Protocol, RawPayload, RealityConfig, SubscriptionExtras, SubscriptionHeaders,
    SubscriptionUserInfo, TlsConfig, Transport, TransportKind,
};
pub use fetch::{fetch_subscription, FetchResult};
pub use normalize::normalize;
pub use parse::{parse_body, BodyMetadata, ParseResult, RawNode, SubscriptionFormat};

/// Download, parse, and normalize a subscription URL end-to-end.
pub async fn fetch_and_normalize(url: &str) -> Result<ConfluxSubscription, ConfluxError> {
    let fetched = fetch_subscription(url).await?;
    let parsed = parse_body(&fetched.body, Some(&fetched.headers))?;
    normalize::normalize_fetch(fetched, parsed)
}

/// Parse a subscription body and normalize it with optional HTTP headers.
pub fn parse_and_normalize(
    body: &str,
    headers: Option<SubscriptionHeaders>,
    source_url: Option<String>,
) -> Result<ConfluxSubscription, ConfluxError> {
    let parsed = parse_body(body, headers.as_ref())?;
    normalize(parsed, headers, source_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_api_parse_and_normalize() {
        let body = "vless://00000000-0000-0000-0000-000000000001@example.com:443#Node";
        let profile = parse_and_normalize(body, None, None).expect("profile");
        assert_eq!(profile.nodes.len(), 1);
        assert_eq!(profile.nodes[0].tag, "Node");
    }
}
