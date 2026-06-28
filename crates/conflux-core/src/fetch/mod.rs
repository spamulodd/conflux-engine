//! HTTP subscription fetch and response header parsing.

mod happ;
mod headers;

use conflux_protocol::ConfluxError;
use conflux_protocol::SubscriptionHeaders;
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

/// User-Agents tried in order when a panel returns a stub or unknown format.
const USER_AGENTS: &[&str] = &[
    "clash-verge/v1.3.8",
    "FlClash/v0.8.57",
    "ClashMeta",
    "mihomo",
    "ClashForAndroid/2.5.12",
    "conflux-engine/0.2.2",
    "sing-box",
    "v2rayNG/1.8.29",
];

fn http_client(user_agent: &str) -> Result<Client, ConfluxError> {
    Client::builder()
        .user_agent(user_agent)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|err| ConfluxError::Http(err.to_string()))
}

fn append_query_flag(url: &str, flag: &str) -> String {
    if url.contains('?') {
        format!("{url}&flag={flag}")
    } else {
        format!("{url}?flag={flag}")
    }
}

/// Panels often return a dummy node when the User-Agent is not a known client.
pub fn is_stub_body(body: &str) -> bool {
    let lower = body.to_lowercase();
    if lower.contains("приложение не поддерживается") || lower.contains("not supported")
    {
        return true;
    }

    lower.contains("0.0.0.0")
        && lower.contains("00000000-0000-0000-0000-000000000000")
        && !lower.contains("00000000-0000-0000-0000-000000000001")
}

fn body_quality(body: &str) -> usize {
    if is_stub_body(body) {
        return 0;
    }

    let mut score = body.len();
    if body.contains("proxies:") {
        score += 10_000;
    }
    if body.contains("vless://") || body.contains("vmess://") || body.contains("trojan://") {
        score += 5_000;
    }
    score
}

async fn fetch_once(url: &str, user_agent: &str) -> Result<FetchResult, ConfluxError> {
    let client = http_client(user_agent)?;
    let response = client
        .get(url)
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

/// Download a subscription URL and parse response headers.
pub async fn fetch_subscription(url: &str) -> Result<FetchResult, ConfluxError> {
    let fetch_url = resolve_subscription_url(url)?;
    let trimmed = fetch_url.trim();
    if trimmed.is_empty() {
        return Err(ConfluxError::InvalidUrl("URL is empty".into()));
    }

    let mut best: Option<FetchResult> = None;
    let mut last_err: Option<ConfluxError> = None;

    for user_agent in USER_AGENTS {
        match fetch_once(trimmed, user_agent).await {
            Ok(result) => {
                if !is_stub_body(&result.body) {
                    return Ok(FetchResult {
                        source_url: url.trim().to_string(),
                        ..result
                    });
                }

                let replace = best
                    .as_ref()
                    .map(|current| body_quality(&result.body) > body_quality(&current.body))
                    .unwrap_or(true);
                if replace {
                    best = Some(FetchResult {
                        source_url: url.trim().to_string(),
                        ..result
                    });
                }
            }
            Err(err) => last_err = Some(err),
        }
    }

    if !trimmed.contains("flag=clash") {
        let flagged = append_query_flag(trimmed, "clash");
        for user_agent in USER_AGENTS.iter().take(5) {
            match fetch_once(&flagged, user_agent).await {
                Ok(result) if !is_stub_body(&result.body) => {
                    return Ok(FetchResult {
                        source_url: url.trim().to_string(),
                        ..result
                    });
                }
                Ok(result) => {
                    let replace = best
                        .as_ref()
                        .map(|current| body_quality(&result.body) > body_quality(&current.body))
                        .unwrap_or(true);
                    if replace {
                        best = Some(FetchResult {
                            source_url: url.trim().to_string(),
                            ..result
                        });
                    }
                }
                Err(err) => last_err = Some(err),
            }
        }
    }

    best.ok_or_else(|| {
        last_err.unwrap_or_else(|| {
            ConfluxError::Http("failed to fetch subscription with any User-Agent".into())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::headers::{
        parse_announce, parse_profile_title, parse_support_url, parse_update_interval_hours,
        parse_user_info,
    };
    use super::{body_quality, is_stub_body};

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

    #[test]
    fn detects_stub_subscription_body() {
        let stub =
            "vless://00000000-0000-0000-0000-000000000000@0.0.0.0:1#Приложение не поддерживается";
        assert!(is_stub_body(stub));
        assert_eq!(body_quality(stub), 0);
    }

    #[test]
    fn prefers_clash_yaml_quality() {
        let clash = "proxies:\n  - name: node\n    type: vless\n";
        assert!(body_quality(clash) > body_quality("vless://uuid@1.2.3.4:443#node"));
    }
}
