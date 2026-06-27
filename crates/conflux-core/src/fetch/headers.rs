use conflux_protocol::{SubscriptionHeaders, SubscriptionUserInfo};
use reqwest::header::{HeaderMap, HeaderName};
use std::str::FromStr;

/// Parse Clash-style subscription response headers from an HTTP response.
pub fn parse_response_headers(headers: &HeaderMap) -> SubscriptionHeaders {
    SubscriptionHeaders {
        profile_title: header_value(headers, "Profile-Title")
            .map(|value| parse_profile_title(Some(value.as_str()))),
        user_info: header_value(headers, "Subscription-Userinfo")
            .and_then(|value| parse_user_info(Some(value.as_str()))),
        update_interval_hours: header_value(headers, "Profile-Update-Interval")
            .map(|value| parse_update_interval_hours(Some(value.as_str())))
            .unwrap_or(0),
        announce: header_value(headers, "Announce")
            .map(|value| parse_announce(Some(value.as_str()))),
        support_url: header_value(headers, "Support-Url")
            .map(|value| parse_support_url(Some(value.as_str()))),
    }
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    HeaderName::from_str(name)
        .ok()
        .and_then(|key| headers.get(key))
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Parse `Subscription-Userinfo: upload=0; download=0; total=0; expire=0`.
pub fn parse_user_info(header_value: Option<&str>) -> Option<SubscriptionUserInfo> {
    let value = header_value?.trim();
    if value.is_empty() {
        return None;
    }

    let mut upload = 0_i64;
    let mut download = 0_i64;
    let mut total = 0_i64;
    let mut expire = None;

    for part in value.split(';') {
        let part = part.trim();
        let Some((key, val)) = part.split_once('=') else {
            continue;
        };
        let Ok(number) = val.trim().parse::<i64>() else {
            continue;
        };
        match key.trim().to_ascii_lowercase().as_str() {
            "upload" => upload = number,
            "download" => download = number,
            "total" => total = number,
            "expire" => expire = Some(number),
            _ => {}
        }
    }

    Some(SubscriptionUserInfo {
        upload_bytes: upload,
        download_bytes: download,
        total_bytes: total,
        expire_unix: expire,
        refill_unix: None,
    })
}

/// Parse `Profile-Update-Interval`, accepting hours or seconds.
pub fn parse_update_interval_hours(header_value: Option<&str>) -> u32 {
    let Some(raw) = header_value
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return 0;
    };

    let Ok(parsed) = raw.parse::<u32>() else {
        return 0;
    };

    if parsed >= 3600 {
        std::cmp::max(1, parsed / 3600)
    } else {
        parsed
    }
}

/// Parse `Profile-Title`, including optional `base64:` prefix.
pub fn parse_profile_title(header_value: Option<&str>) -> String {
    let Some(value) = header_value
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return String::new();
    };

    decode_prefixed_base64(value).unwrap_or_else(|| value.to_string())
}

/// Parse `Announce`, including optional `base64:` prefix.
pub fn parse_announce(header_value: Option<&str>) -> String {
    parse_profile_title(header_value)
}

/// Parse `Support-Url` as plain text.
pub fn parse_support_url(header_value: Option<&str>) -> String {
    header_value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn decode_prefixed_base64(value: &str) -> Option<String> {
    let payload = value
        .strip_prefix("base64:")
        .or_else(|| value.strip_prefix("BASE64:"))?;
    let decoded = base64_decode(payload.trim())?;
    Some(String::from_utf8_lossy(&decoded).trim().to_string())
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let normalized = normalize_base64(input);
    base64::engine::general_purpose::STANDARD
        .decode(normalized)
        .ok()
}

fn normalize_base64(value: &str) -> String {
    let mut b64 = value.replace('-', "+").replace('_', "/");
    match b64.len() % 4 {
        2 => b64.push_str("=="),
        3 => b64.push('='),
        _ => {}
    }
    b64
}
