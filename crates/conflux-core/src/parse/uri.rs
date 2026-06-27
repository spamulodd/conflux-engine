use conflux_protocol::ConfluxError;
use regex::Regex;
use std::sync::LazyLock;

use super::RawNode;

static URI_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(vless|vmess|ss|trojan|hysteria2|hy2|himera|socks5?)://[^\s\)\]>,]+")
        .expect("valid uri regex")
});

const SCHEMES: [&str; 8] = [
    "vless://",
    "vmess://",
    "ss://",
    "trojan://",
    "hysteria2://",
    "hy2://",
    "himera://",
    "socks://",
];

/// Scan expanded body text for supported proxy URI schemes.
pub fn extract_uri_nodes(body: &str) -> Result<Vec<RawNode>, ConfluxError> {
    let mut nodes = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (line_index, line) in body.lines().enumerate() {
        let trimmed = sanitize_line(line);
        if trimmed.is_empty() {
            continue;
        }

        if let Some(node) = parse_single_line(&trimmed, Some(line_index)) {
            if seen.insert(node.raw_uri.clone()) {
                nodes.push(node);
            }
            continue;
        }

        for capture in URI_PATTERN.find_iter(&trimmed) {
            let uri = sanitize_uri(capture.as_str());
            if uri.is_empty() || !seen.insert(uri.clone()) {
                continue;
            }
            if let Some(node) = parse_single_line(&uri, Some(line_index)) {
                nodes.push(node);
            }
        }
    }

    if nodes.is_empty() && SCHEMES.iter().any(|scheme| body.contains(scheme)) {
        if let Some(node) = parse_single_line(body.trim(), None) {
            nodes.push(node);
        }
    }

    Ok(nodes)
}

fn parse_single_line(line: &str, line_index: Option<usize>) -> Option<RawNode> {
    let uri = sanitize_uri(line);
    let scheme_end = uri.find("://")?;
    let scheme = uri[..scheme_end].to_ascii_lowercase();
    if !SCHEMES
        .iter()
        .any(|value| scheme == value.trim_end_matches("://"))
    {
        return None;
    }

    Some(RawNode {
        name: extract_fragment_name(&uri),
        scheme,
        raw_uri: uri,
        line_index,
        clash_proxy: None,
    })
}

fn sanitize_line(line: &str) -> String {
    line.replace('\r', "").trim().to_string()
}

fn sanitize_uri(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches([')', ']', '>', ',', '.', ';']);
    if trimmed.is_empty() {
        return String::new();
    }

    let hash = trimmed.find('#');
    let (base, fragment) = match hash {
        Some(index) => (&trimmed[..index], Some(&trimmed[index + 1..])),
        None => (trimmed, None),
    };

    let mut normalized = base.to_string();
    if let Some(name) = fragment {
        let decoded = percent_encoding::percent_decode_str(name)
            .decode_utf8_lossy()
            .to_string();
        let encoded = urlencoding_fragment(&decoded);
        normalized.push('#');
        normalized.push_str(&encoded);
    }

    normalized
}

fn urlencoding_fragment(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC).to_string()
}

fn extract_fragment_name(uri: &str) -> Option<String> {
    let hash = uri.find('#')?;
    let fragment = uri[hash + 1..].trim();
    if fragment.is_empty() {
        return None;
    }
    Some(
        percent_encoding::percent_decode_str(fragment)
            .decode_utf8_lossy()
            .trim()
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_multiple_schemes() {
        let body = "\
vless://00000000-0000-0000-0000-000000000001@example.com:443?security=tls&sni=example.com#Node-A
ss://YWVzLTI1Ni1nY206cGFzc3dvcmQ=@example.com:8388#Node-B
trojan://password@example.com:443?sni=example.com#Node-C
hy2://password@example.com:8443/?sni=example.com#Node-D
";
        let nodes = extract_uri_nodes(body).expect("extract");
        assert_eq!(nodes.len(), 4);
        assert!(nodes.iter().any(|node| node.scheme == "hy2"));
    }

    #[test]
    fn preserves_url_encoded_fragment() {
        let uri = "vless://00000000-0000-0000-0000-000000000001@example.com:443#Test%20Node";
        let nodes = extract_uri_nodes(uri).expect("extract");
        assert_eq!(nodes[0].name.as_deref(), Some("Test Node"));
    }
}
