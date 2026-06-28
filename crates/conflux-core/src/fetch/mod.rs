//! HTTP subscription fetch and response header parsing.

mod headers;
mod happ;

use conflux_protocol::{ConfluxError, SubscriptionHeaders};
use reqwest::Client;

pub use happ::resolve_subscription_url;

pub use headers::{
    parse_announce, parse_profile_title, parse_response_headers, parse_support_url,
    parse_update_interval_hours, parse_user_info,
};

/// Result of an HTTP subscription fetch before normalization.
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub body: String,
    pub headers: SubscriptionHeaders,
    pub source_url: String,
}

/// Shared HTTP client configured with rustls.
fn http_client() -> Result<Client, ConfluxError> {
    Client::builder()
        .user_agent("conflux-engine/0.2.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|err| ConfluxError::Http(err.to_string()))
}

/// Download a subscription URL and parse response headers.
pub async fn fetch_subscription(url: &str) -> Result<FetchResult, ConfluxError> {
    let fetch_url = resolve_subscription_url(url)?;
    let trimmed = fetch_url.trim();
    if trimmed.is_empty() {
        return Err(ConfluxError::InvalidUrl("URL is empty".into()));
    }

    let client = http_client()?;
    let response = client
        .get(trimmed)
        .send()
        .await
        .map_err(|err| ConfluxError::Http(err.to_string()))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(ConfluxError::Http("401 Unauthorized".into()));
    }

    if !response.status().is_success() {
        return Err(ConfluxError::Http(format!(
            "unexpected status {}",
            response.status()
        )));
    }

    let headers = parse_response_headers(response.headers());
    let body = response
        .text()
        .await
        .map_err(|err| ConfluxError::Http(err.to_string()))?
        .trim()
        .to_string();

    Ok(FetchResult {
        body,
        headers,
        source_url: url.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::headers::{
        parse_announce, parse_profile_title, parse_support_url, parse_update_interval_hours,
        parse_user_info,
    };

    #[test]
    fn parses_subscription_userinfo_header() {
        let info = parse_user_info(Some("upload=1; download=2; total=100; expire=1700000000"))
            .expect("userinfo");
        assert_eq!(info.upload_bytes, 1);
        assert_eq!(info.download_bytes, 2);
        assert_eq!(info.total_bytes, 100);
        assert_eq!(info.expire_unix, Some(1_700_000_000));
    }

    #[test]
    fn parses_profile_title_base64_prefix() {
        let title = parse_profile_title(Some("base64:SGVsbG8="));
        assert_eq!(title, "Hello");
    }

    #[test]
    fn normalizes_update_interval_from_seconds() {
        assert_eq!(parse_update_interval_hours(Some("86400")), 24);
        assert_eq!(parse_update_interval_hours(Some("12")), 12);
    }

    #[test]
    fn parses_announce_and_support_url() {
        assert_eq!(parse_announce(Some("base64:SGVsbG8=")), "Hello");
        assert_eq!(
            parse_support_url(Some("https://example.com/help")),
            "https://example.com/help"
        );
    }
}
