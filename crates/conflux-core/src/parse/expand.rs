const KNOWN_SCHEMES: [&str; 8] = [
    "vless://",
    "vmess://",
    "ss://",
    "trojan://",
    "hysteria2://",
    "hy2://",
    "himera://",
    "socks://",
];

/// Returns true when the body already contains recognizable proxy URI schemes.
pub fn contains_known_scheme(body: &str) -> bool {
    KNOWN_SCHEMES.iter().any(|scheme| body.contains(scheme))
}

/// Attempt standard or URL-safe Base64 expansion when no schemes are visible.
pub fn expand_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if contains_known_scheme(trimmed) || trimmed.contains('\n') {
        for line in trimmed.lines() {
            if KNOWN_SCHEMES
                .iter()
                .any(|scheme| line.trim().starts_with(scheme))
            {
                return trimmed.to_string();
            }
        }
    }

    if let Some(decoded) = try_decode_base64(trimmed) {
        if contains_known_scheme(&decoded) || decoded.contains('\n') {
            return decoded;
        }
    }

    trimmed.to_string()
}

fn try_decode_base64(value: &str) -> Option<String> {
    use base64::Engine;

    if value.len() < 8 || !looks_like_base64(value) {
        return None;
    }

    let normalized = normalize_base64(value);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(normalized)
        .ok()?;
    Some(String::from_utf8_lossy(&bytes).trim().to_string())
}

fn looks_like_base64(value: &str) -> bool {
    value.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '=' | '-' | '_' | '\r' | '\n')
    })
}

fn normalize_base64(value: &str) -> String {
    let compact = value.replace(['\r', '\n'], "");
    let mut b64 = compact.replace('-', "+").replace('_', "/");
    match b64.len() % 4 {
        2 => b64.push_str("=="),
        3 => b64.push('='),
        _ => {}
    }
    b64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_standard_base64_subscription() {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD
            .encode("vless://00000000-0000-0000-0000-000000000001@example.com:443#A");
        let decoded = expand_body(&encoded);
        assert!(decoded.contains("vless://"));
    }
}
