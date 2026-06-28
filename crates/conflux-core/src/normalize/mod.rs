use conflux_protocol::{
    stable_node_id, ConfluxError, ConfluxNode, ConfluxSubscription, Credentials, NodeMeta,
    NodeSource, ObfsConfig, Protocol, RawPayload, RealityConfig, SubscriptionHeaders, TlsConfig,
    Transport, TransportKind,
};
use url::Url;
use uuid::Uuid;

use crate::fetch::FetchResult;
use crate::parse::{BodyMetadata, ParseResult, RawNode};

/// Map parsed raw nodes and metadata into canonical protocol types.
pub fn normalize(
    parsed: ParseResult,
    headers: Option<SubscriptionHeaders>,
    source_url: Option<String>,
) -> Result<ConfluxSubscription, ConfluxError> {
    let mut nodes = Vec::with_capacity(parsed.nodes.len());
    let mut skipped = 0usize;
    for raw in parsed.nodes {
        let mut source = NodeSource {
            subscription_url: source_url.clone(),
            raw_uri: Some(raw.raw_uri.clone()),
            line_index: raw.line_index,
            parser: Some(parser_name(&raw)),
        };
        match normalize_node(raw, &mut source) {
            Ok(node) => nodes.push(node),
            Err(err) if is_skippable_node_error(&err) => skipped += 1,
            Err(err) => return Err(err),
        }
    }

    if nodes.is_empty() {
        return Err(ConfluxError::Normalize(if skipped > 0 {
            format!("all {skipped} nodes failed normalization")
        } else {
            "no nodes to normalize".into()
        }));
    }

    let headers = headers.unwrap_or_default();
    let body = parsed.body_metadata;

    Ok(ConfluxSubscription {
        title: pick_title(&headers, &body),
        source_url,
        update_interval_hours: pick_update_interval(&headers, &body),
        user_info: headers.user_info.or(body.user_info),
        support_url: non_empty(headers.support_url),
        announce: non_empty(headers.announce),
        nodes,
        extras: parsed.extras,
    })
}

/// Convenience helper for fetch → parse → normalize.
pub fn normalize_fetch(
    fetch: FetchResult,
    parsed: ParseResult,
) -> Result<ConfluxSubscription, ConfluxError> {
    normalize(parsed, Some(fetch.headers.clone()), Some(fetch.source_url))
}

fn parser_name(raw: &RawNode) -> String {
    if raw.clash_proxy.is_some() {
        "clash".to_string()
    } else {
        "uri".to_string()
    }
}

fn is_skippable_node_error(err: &ConfluxError) -> bool {
    matches!(
        err,
        ConfluxError::UnsupportedProtocol(_)
            | ConfluxError::InvalidUri(_)
            | ConfluxError::Parse(_)
    )
}

fn pick_title(headers: &SubscriptionHeaders, body: &BodyMetadata) -> String {
    headers
        .profile_title
        .clone()
        .filter(|value| !value.is_empty())
        .or_else(|| body.profile_title.clone())
        .unwrap_or_default()
}

fn pick_update_interval(headers: &SubscriptionHeaders, body: &BodyMetadata) -> u32 {
    if headers.update_interval_hours > 0 {
        headers.update_interval_hours
    } else {
        body.update_interval_hours.unwrap_or(0)
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

fn normalize_node(raw: RawNode, source: &mut NodeSource) -> Result<ConfluxNode, ConfluxError> {
    if let Some(clash) = raw.clash_proxy.clone() {
        return normalize_clash_proxy(raw, clash, source);
    }

    let protocol = Protocol::from_scheme(&raw.scheme);
    match protocol {
        Protocol::Vless => normalize_vless(raw, source),
        Protocol::Vmess => normalize_vmess(raw, source),
        Protocol::Shadowsocks => normalize_shadowsocks(raw, source),
        Protocol::Trojan => normalize_trojan(raw, source),
        Protocol::Hysteria2 => normalize_hysteria2(raw, source),
        Protocol::NativeTunnel => normalize_native_tunnel(raw, source),
        Protocol::Unknown => Err(ConfluxError::UnsupportedProtocol(raw.scheme)),
        _ => Err(ConfluxError::UnsupportedProtocol(format!(
            "{} not implemented in v0.1.0",
            raw.scheme
        ))),
    }
}

fn normalize_vless(raw: RawNode, source: &NodeSource) -> Result<ConfluxNode, ConfluxError> {
    let url = parse_url(&raw.raw_uri)?;
    let uuid = parse_user_uuid(&url)?;
    let query = query_pairs(&url);
    let (transport, tls, reality) = build_transport_and_tls(&query);
    let tag = raw.name.clone().unwrap_or_else(|| "vless".to_string());

    Ok(base_node(
        raw,
        source.clone(),
        Protocol::Vless,
        host(&url)?,
        port_or_default(&url, 443)?,
        Credentials::Uuid { id: uuid },
        transport,
        tls,
        reality,
        tag,
        query.get("flow").cloned(),
        query
            .get("encryption")
            .cloned()
            .or(Some("none".to_string())),
        query.get("packetEncoding").cloned(),
        None,
        None,
        None,
    ))
}

fn normalize_trojan(raw: RawNode, source: &NodeSource) -> Result<ConfluxNode, ConfluxError> {
    let url = parse_url(&raw.raw_uri)?;
    let password = if !url.username().is_empty() {
        url.username().to_string()
    } else {
        url.password().unwrap_or("").to_string()
    };
    if password.is_empty() {
        return Err(ConfluxError::InvalidUri("trojan password missing".into()));
    }
    let query = query_pairs(&url);
    let (transport, mut tls, reality) = build_transport_and_tls(&query);
    if tls.is_none() {
        let host_name = host(&url)?;
        tls = Some(TlsConfig {
            enabled: true,
            sni: query.get("sni").cloned().or(Some(host_name)),
            alpn: split_csv(query.get("alpn")),
            fingerprint: query.get("fp").cloned(),
            insecure: query.get("allowInsecure") == Some(&"1".to_string())
                || query.get("insecure") == Some(&"1".to_string()),
        });
    }
    let tag = raw.name.clone().unwrap_or_else(|| "trojan".to_string());

    Ok(base_node(
        raw,
        source.clone(),
        Protocol::Trojan,
        host(&url)?,
        port_or_default(&url, 443)?,
        Credentials::Password { password },
        transport,
        tls,
        reality,
        tag,
        None,
        None,
        None,
        None,
        None,
        None,
    ))
}

fn normalize_shadowsocks(raw: RawNode, source: &NodeSource) -> Result<ConfluxNode, ConfluxError> {
    let (method, password, server, port) = parse_shadowsocks_uri(&raw.raw_uri)?;
    let tag = raw
        .name
        .clone()
        .unwrap_or_else(|| format!("{server}:{port}"));

    Ok(base_node(
        raw,
        source.clone(),
        Protocol::Shadowsocks,
        server,
        port,
        Credentials::Shadowsocks { method, password },
        Transport::default(),
        None,
        None,
        tag,
        None,
        None,
        None,
        None,
        None,
        None,
    ))
}

fn normalize_vmess(raw: RawNode, source: &NodeSource) -> Result<ConfluxNode, ConfluxError> {
    let payload = raw
        .raw_uri
        .split("://")
        .nth(1)
        .ok_or_else(|| ConfluxError::InvalidUri("vmess payload missing".into()))?;
    let json = decode_base64_payload(payload)?;
    let value: serde_json::Value =
        serde_json::from_slice(&json).map_err(|err| ConfluxError::Parse(err.to_string()))?;

    let server = value
        .get("add")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ConfluxError::Parse("vmess add missing".into()))?
        .to_string();
    let port = value
        .get("port")
        .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
        .and_then(|n| u16::try_from(n).ok())
        .ok_or_else(|| ConfluxError::Parse("vmess port missing".into()))?;
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ConfluxError::Parse("vmess id missing".into()))?;
    let uuid = Uuid::parse_str(id).map_err(|err| ConfluxError::Parse(err.to_string()))?;

    let net = value
        .get("net")
        .or_else(|| value.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("tcp");
    let transport = transport_from_type(net, &value);
    let tls_enabled = value.get("tls").and_then(|v| v.as_str()) == Some("tls");
    let tls = tls_enabled.then(|| TlsConfig {
        enabled: true,
        sni: value
            .get("sni")
            .or_else(|| value.get("host"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        alpn: value
            .get("alpn")
            .and_then(|v| v.as_str())
            .map(|s| split_csv(Some(&s.to_string())))
            .unwrap_or_default(),
        fingerprint: value.get("fp").and_then(|v| v.as_str()).map(str::to_string),
        insecure: false,
    });

    let tag = raw
        .name
        .clone()
        .or_else(|| value.get("ps").and_then(|v| v.as_str()).map(str::to_string))
        .unwrap_or_else(|| server.clone());

    Ok(base_node(
        raw,
        source.clone(),
        Protocol::Vmess,
        server,
        port,
        Credentials::Uuid { id: uuid },
        transport,
        tls,
        None,
        tag,
        None,
        value
            .get("scy")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        None,
        None,
        None,
        None,
    ))
}

fn normalize_hysteria2(raw: RawNode, source: &NodeSource) -> Result<ConfluxNode, ConfluxError> {
    let mut uri = raw.raw_uri.clone();
    if uri.starts_with("hy2://") {
        uri = uri.replacen("hy2://", "hysteria2://", 1);
    }
    let url = parse_url(&uri)?;
    let password = if !url.username().is_empty() {
        url.username().to_string()
    } else {
        url.password().unwrap_or("").to_string()
    };
    if password.is_empty() {
        return Err(ConfluxError::InvalidUri(
            "hysteria2 password missing".into(),
        ));
    }
    let query = query_pairs(&url);
    let obfs = query.get("obfs").map(|kind| ObfsConfig {
        kind: kind.clone(),
        password: query.get("obfs-password").cloned(),
    });
    let host_name = host(&url)?;
    let tls = Some(TlsConfig {
        enabled: true,
        sni: query.get("sni").cloned().or(Some(host_name)),
        alpn: split_csv(query.get("alpn")),
        fingerprint: query.get("fp").cloned(),
        insecure: query.get("insecure") == Some(&"1".to_string()),
    });
    let ports = query.get("mport").cloned().map(|value| vec![value]);
    let tag = raw.name.clone().unwrap_or_else(|| "hysteria2".to_string());

    Ok(base_node(
        raw,
        source.clone(),
        Protocol::Hysteria2,
        host(&url)?,
        port_or_default(&url, 443)?,
        Credentials::Password { password },
        Transport {
            kind: TransportKind::Quic,
            ..Transport::default()
        },
        tls,
        None,
        tag,
        None,
        None,
        None,
        None,
        obfs,
        ports,
    ))
}

fn normalize_native_tunnel(raw: RawNode, source: &NodeSource) -> Result<ConfluxNode, ConfluxError> {
    let payload = raw
        .raw_uri
        .split("://")
        .nth(1)
        .and_then(|value| value.split('#').next())
        .ok_or_else(|| ConfluxError::InvalidUri("native tunnel payload missing".into()))?;
    let envelope = decode_base64_url_payload(payload)?;
    let json_bytes = if envelope.first() == Some(&0) {
        envelope[1..].to_vec()
    } else if envelope.first() == Some(&b'{') {
        envelope
    } else {
        return Err(ConfluxError::Parse(
            "unsupported native tunnel envelope".into(),
        ));
    };

    let value: serde_json::Value =
        serde_json::from_slice(&json_bytes).map_err(|err| ConfluxError::Parse(err.to_string()))?;
    let host = value
        .get("host")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ConfluxError::Parse("native host missing".into()))?
        .to_string();
    let port = value
        .get("port")
        .and_then(|v| v.as_u64())
        .and_then(|n| u16::try_from(n).ok())
        .unwrap_or(8443);
    let key_hex = value
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ConfluxError::Parse("native key missing".into()))?
        .to_string();
    let tag = raw
        .name
        .clone()
        .or_else(|| {
            value
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| host.clone());

    let mut node = base_node(
        raw,
        source.clone(),
        Protocol::NativeTunnel,
        host,
        port,
        Credentials::NativeKey { key_hex },
        Transport::default(),
        None,
        None,
        tag,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    node.native_profile = value
        .get("profile")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    node.native_tun_cidr = value
        .get("tun")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    node.usage_url = value
        .get("usage")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    node.meta = NodeMeta {
        country_code: value.get("cc").and_then(|v| v.as_str()).map(str::to_string),
        flag: value
            .get("flag")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        ..NodeMeta::default()
    };
    Ok(node)
}

fn normalize_clash_proxy(
    raw: RawNode,
    clash: serde_json::Value,
    source: &NodeSource,
) -> Result<ConfluxNode, ConfluxError> {
    let obj = clash
        .as_object()
        .ok_or_else(|| ConfluxError::Parse("clash proxy must be an object".into()))?;
    let proxy_type = obj
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let protocol = Protocol::from_scheme(&proxy_type);
    let server = obj
        .get("server")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ConfluxError::Parse("clash server missing".into()))?
        .to_string();
    let port = obj
        .get("port")
        .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
        .and_then(|n| u16::try_from(n).ok())
        .ok_or_else(|| ConfluxError::Parse("clash port missing".into()))?;
    let tag = raw
        .name
        .clone()
        .or_else(|| obj.get("name").and_then(|v| v.as_str()).map(str::to_string))
        .unwrap_or_else(|| server.clone());

    let credentials = match protocol {
        Protocol::Vless => Credentials::Uuid {
            id: Uuid::parse_str(
                obj.get("uuid")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ConfluxError::Parse("clash uuid missing".into()))?,
            )
            .map_err(|err| ConfluxError::Parse(err.to_string()))?,
        },
        Protocol::Shadowsocks => Credentials::Shadowsocks {
            method: obj
                .get("cipher")
                .or_else(|| obj.get("method"))
                .and_then(|v| v.as_str())
                .unwrap_or("aes-256-gcm")
                .to_string(),
            password: obj
                .get("password")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
        Protocol::Trojan | Protocol::Hysteria2 => Credentials::Password {
            password: obj
                .get("password")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
        Protocol::Vmess => Credentials::Uuid {
            id: Uuid::parse_str(
                obj.get("uuid")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ConfluxError::Parse("clash uuid missing".into()))?,
            )
            .map_err(|err| ConfluxError::Parse(err.to_string()))?,
        },
        _ => Credentials::None,
    };

    let mut tls = None;
    let mut reality = None;
    if obj.get("tls").and_then(|v| v.as_bool()) == Some(true)
        || obj.get("tls").and_then(|v| v.as_str()) == Some("true")
    {
        tls = Some(TlsConfig {
            enabled: true,
            sni: obj
                .get("servername")
                .or_else(|| obj.get("sni"))
                .and_then(|v| v.as_str())
                .map(str::to_string),
            alpn: obj
                .get("alpn")
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            fingerprint: obj
                .get("client-fingerprint")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            insecure: obj.get("skip-cert-verify").and_then(|v| v.as_bool()) == Some(true),
        });
    }

    if let Some(opts) = obj.get("reality-opts").and_then(|v| v.as_object()) {
        reality = Some(RealityConfig {
            public_key: opts
                .get("public-key")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            short_id: opts
                .get("short-id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            spider_x: None,
        });
    }

    let network = obj.get("network").and_then(|v| v.as_str()).unwrap_or("tcp");
    let transport = transport_from_type(network, &clash);

    Ok(ConfluxNode {
        id: stable_node_id(protocol, &server, port, &credentials, &transport),
        tag,
        protocol,
        source: source.clone(),
        server,
        port,
        ports: None,
        credentials,
        transport,
        tls,
        reality,
        flow: obj.get("flow").and_then(|v| v.as_str()).map(str::to_string),
        encryption: None,
        packet_encoding: None,
        method: obj
            .get("cipher")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        obfs: None,
        meta: NodeMeta::default(),
        raw: RawPayload::ClashProxy { value: clash },
        native_profile: None,
        native_tun_cidr: None,
        usage_url: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn base_node(
    raw: RawNode,
    source: NodeSource,
    protocol: Protocol,
    server: String,
    port: u16,
    credentials: Credentials,
    transport: Transport,
    tls: Option<TlsConfig>,
    reality: Option<RealityConfig>,
    tag: String,
    flow: Option<String>,
    encryption: Option<String>,
    packet_encoding: Option<String>,
    method: Option<String>,
    obfs: Option<ObfsConfig>,
    ports: Option<Vec<String>>,
) -> ConfluxNode {
    ConfluxNode {
        id: stable_node_id(protocol, &server, port, &credentials, &transport),
        tag,
        protocol,
        source,
        server,
        port,
        ports,
        credentials,
        transport,
        tls,
        reality,
        flow,
        encryption,
        packet_encoding,
        method,
        obfs,
        meta: NodeMeta::default(),
        raw: RawPayload::Uri { value: raw.raw_uri },
        native_profile: None,
        native_tun_cidr: None,
        usage_url: None,
    }
}

fn parse_url(raw: &str) -> Result<Url, ConfluxError> {
    Url::parse(raw).map_err(|err| ConfluxError::InvalidUri(err.to_string()))
}

fn host(url: &Url) -> Result<String, ConfluxError> {
    url.host_str()
        .map(str::to_string)
        .ok_or_else(|| ConfluxError::InvalidUri("host missing".into()))
}

fn port_or_default(url: &Url, default: u16) -> Result<u16, ConfluxError> {
    Ok(url.port_or_known_default().unwrap_or(default))
}

fn parse_user_uuid(url: &Url) -> Result<Uuid, ConfluxError> {
    let id = url.username();
    Uuid::parse_str(id).map_err(|err| ConfluxError::InvalidUri(err.to_string()))
}

fn query_pairs(url: &Url) -> std::collections::HashMap<String, String> {
    url.query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

fn split_csv(value: Option<&String>) -> Vec<String> {
    value
        .map(|text| {
            text.split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn build_transport_and_tls(
    query: &std::collections::HashMap<String, String>,
) -> (Transport, Option<TlsConfig>, Option<RealityConfig>) {
    let network = query.get("type").map(String::as_str).unwrap_or("tcp");
    let mut transport = transport_from_type(network, &serde_json::Value::Null);
    transport.path = query.get("path").cloned();
    transport.host = query
        .get("host")
        .cloned()
        .or_else(|| query.get("sni").cloned());
    transport.service_name = query.get("serviceName").cloned();
    transport.header_type = query.get("headerType").cloned();

    let security = query.get("security").map(String::as_str).unwrap_or("none");
    let mut tls = None;
    let mut reality = None;

    if security == "tls" {
        tls = Some(TlsConfig {
            enabled: true,
            sni: query.get("sni").cloned(),
            alpn: split_csv(query.get("alpn")),
            fingerprint: query.get("fp").cloned(),
            insecure: query.get("allowInsecure") == Some(&"1".to_string()),
        });
    } else if security == "reality" {
        tls = Some(TlsConfig {
            enabled: true,
            sni: query.get("sni").cloned(),
            alpn: split_csv(query.get("alpn")),
            fingerprint: query.get("fp").cloned(),
            insecure: false,
        });
        reality = Some(RealityConfig {
            public_key: query.get("pbk").cloned().unwrap_or_default(),
            short_id: query.get("sid").cloned().unwrap_or_default(),
            spider_x: query.get("spx").cloned(),
        });
    }

    (transport, tls, reality)
}

fn transport_from_type(network: &str, value: &serde_json::Value) -> Transport {
    let kind = match network {
        "ws" => TransportKind::Ws,
        "grpc" => TransportKind::Grpc,
        "http" | "h2" => TransportKind::H2,
        "httpupgrade" => TransportKind::HttpUpgrade,
        "kcp" | "mkcp" => TransportKind::Kcp,
        "quic" => TransportKind::Quic,
        _ => TransportKind::Tcp,
    };

    Transport {
        kind,
        path: value
            .get("path")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        host: value
            .get("host")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        service_name: value
            .get("serviceName")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        headers: Vec::new(),
        header_type: value
            .get("type")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    }
}

fn parse_shadowsocks_uri(raw: &str) -> Result<(String, String, String, u16), ConfluxError> {
    let without_fragment = raw.split('#').next().unwrap_or(raw);
    let payload = without_fragment
        .strip_prefix("ss://")
        .ok_or_else(|| ConfluxError::InvalidUri("invalid ss uri".into()))?;

    if let Some((encoded, hostport)) = payload.split_once('@') {
        let decoded = decode_base64_payload(encoded)?;
        let creds = String::from_utf8_lossy(&decoded);
        let (method, password) = creds
            .split_once(':')
            .ok_or_else(|| ConfluxError::InvalidUri("invalid ss credentials".into()))?;
        let (server, port) = parse_host_port(hostport)?;
        return Ok((method.to_string(), password.to_string(), server, port));
    }

    let decoded = decode_base64_payload(payload)?;
    let creds = String::from_utf8_lossy(&decoded);
    let (method_pass, host_port) = creds
        .split_once('@')
        .ok_or_else(|| ConfluxError::InvalidUri("invalid legacy ss uri".into()))?;
    let (method, password) = method_pass
        .split_once(':')
        .ok_or_else(|| ConfluxError::InvalidUri("invalid legacy ss credentials".into()))?;
    let (server, port) = parse_host_port(host_port)?;
    Ok((method.to_string(), password.to_string(), server, port))
}

fn parse_host_port(hostport: &str) -> Result<(String, u16), ConfluxError> {
    let hostport = hostport
        .trim()
        .trim_start_matches('/')
        .split('?')
        .next()
        .unwrap_or(hostport);
    if hostport.starts_with('[') {
        let end = hostport
            .find(']')
            .ok_or_else(|| ConfluxError::InvalidUri("invalid ipv6 host".into()))?;
        let server = hostport[1..end].to_string();
        let port_text = hostport[end + 1..]
            .strip_prefix(':')
            .ok_or_else(|| ConfluxError::InvalidUri("invalid ipv6 port".into()))?;
        let port: u16 = port_text
            .parse()
            .map_err(|err: std::num::ParseIntError| ConfluxError::InvalidUri(err.to_string()))?;
        return Ok((server, port));
    }

    let (server, port_text) = hostport
        .rsplit_once(':')
        .ok_or_else(|| ConfluxError::InvalidUri("invalid ss host".into()))?;
    let port: u16 = port_text
        .parse()
        .map_err(|err: std::num::ParseIntError| ConfluxError::InvalidUri(err.to_string()))?;
    Ok((server.to_string(), port))
}

fn decode_base64_payload(input: &str) -> Result<Vec<u8>, ConfluxError> {
    use base64::Engine;
    let normalized = normalize_base64(input);
    base64::engine::general_purpose::STANDARD
        .decode(normalized)
        .map_err(|err| ConfluxError::Parse(err.to_string()))
}

fn decode_base64_url_payload(input: &str) -> Result<Vec<u8>, ConfluxError> {
    use base64::Engine;
    let normalized = normalize_base64(input);
    base64::engine::general_purpose::URL_SAFE
        .decode(&normalized)
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(normalized))
        .map_err(|err| ConfluxError::Parse(err.to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{parse_body, SubscriptionFormat};

    #[test]
    fn normalizes_vless_node() {
        let body = "vless://00000000-0000-0000-0000-000000000001@example.com:443?security=reality&type=tcp&flow=xtls-rprx-vision&sni=example.com&fp=chrome&pbk=REDACTED&sid=REDACTED#Test";
        let parsed = parse_body(body, None).expect("parse");
        let sub = normalize(parsed, None, None).expect("normalize");
        assert_eq!(sub.nodes.len(), 1);
        assert_eq!(sub.nodes[0].protocol, Protocol::Vless);
        assert_eq!(sub.nodes[0].server, "example.com");
        assert!(sub.nodes[0].reality.is_some());
    }

    #[test]
    fn normalizes_mixed_uri_list() {
        let body = "\
vless://00000000-0000-0000-0000-000000000001@example.com:443#A
ss://YWVzLTI1Ni1nY206cGFzc3dvcmQ=@example.com:8388#B
trojan://password@example.com:443?sni=example.com#C
hysteria2://password@example.com:8443/?sni=example.com#D
";
        let parsed = parse_body(body, None).expect("parse");
        assert_eq!(parsed.format, SubscriptionFormat::UriList);
        let sub = normalize(parsed, None, None).expect("normalize");
        assert_eq!(sub.nodes.len(), 4);
    }

    #[test]
    fn normalizes_native_tunnel_uri() {
        use base64::Engine;
        let json = r#"{"v":1,"host":"example.com","port":8443,"key":"0000000000000000000000000000000000000000000000000000000000000000","tun":"10.8.0.2/24","profile":"auto","name":"Native Node"}"#;
        let encoded = base64::engine::general_purpose::STANDARD.encode(json.as_bytes());
        let body = format!("himera://{encoded}#Native-Node");
        let parsed = parse_body(&body, None).expect("parse");
        let sub = normalize(parsed, None, None).expect("normalize");
        assert_eq!(sub.nodes.len(), 1);
        assert_eq!(sub.nodes[0].protocol, Protocol::NativeTunnel);
        assert_eq!(sub.nodes[0].native_profile.as_deref(), Some("auto"));
    }

    #[test]
    fn normalizes_vmess_uri() {
        use base64::Engine;
        let json = r#"{"v":"2","ps":"VMess Node","add":"example.com","port":"443","id":"00000000-0000-0000-0000-000000000001","aid":"0","net":"ws","tls":"tls"}"#;
        let encoded = base64::engine::general_purpose::STANDARD.encode(json);
        let body = format!("vmess://{encoded}");
        let parsed = parse_body(&body, None).expect("parse");
        let sub = normalize(parsed, None, None).expect("normalize");
        assert_eq!(sub.nodes.len(), 1);
        assert_eq!(sub.nodes[0].protocol, Protocol::Vmess);
        assert_eq!(sub.nodes[0].tag, "VMess Node");
    }

    #[test]
    fn distinct_credentials_get_distinct_ids() {
        let body = "\
vless://11111111-1111-1111-1111-111111111111@cdn.example.com:443#香港
vless://22222222-2222-2222-2222-222222222222@cdn.example.com:443#香港
";
        let parsed = parse_body(body, None).expect("parse");
        let sub = normalize(parsed, None, None).expect("normalize");
        assert_eq!(sub.nodes.len(), 2);
        assert_ne!(sub.nodes[0].id, sub.nodes[1].id);
    }

    #[test]
    fn merges_header_metadata() {
        let parsed = parse_body(
            "vless://00000000-0000-0000-0000-000000000001@example.com:443#A",
            None,
        )
        .expect("parse");
        let headers = SubscriptionHeaders {
            profile_title: Some("Provider".into()),
            update_interval_hours: 12,
            support_url: Some("https://example.com/support".into()),
            ..SubscriptionHeaders::default()
        };
        let sub = normalize(
            parsed,
            Some(headers),
            Some("https://example.com/sub".into()),
        )
        .expect("normalize");
        assert_eq!(sub.title, "Provider");
        assert_eq!(sub.update_interval_hours, 12);
        assert_eq!(
            sub.support_url.as_deref(),
            Some("https://example.com/support")
        );
    }
}
