use std::collections::HashMap;

use conflux_protocol::{
    ConfluxNode, ConfluxSubscription, Credentials, ObfsConfig, Protocol, RealityConfig, TlsConfig,
    Transport, TransportKind,
};
use serde_json::{json, Map, Value};

use crate::error::BackendError;

/// Options for sing-box config generation.
#[derive(Debug, Clone, Default)]
pub struct GenerateOptions {
    pub selected_node_id: Option<String>,
    pub proxy_tag: String,
    pub tun_tag: String,
    pub interface_name: String,
    pub include_tun_inbound: bool,
}

impl GenerateOptions {
    pub fn for_windows() -> Self {
        Self {
            proxy_tag: "proxy".to_string(),
            tun_tag: "tun-in".to_string(),
            interface_name: "conflux-tun".to_string(),
            include_tun_inbound: true,
            ..Self::default()
        }
    }
}

/// Build a sing-box JSON configuration from a normalized subscription.
pub fn generate_config(
    subscription: &ConfluxSubscription,
    options: &GenerateOptions,
) -> Result<Value, BackendError> {
    let node = select_node(subscription, options.selected_node_id.as_deref())?;
    let proxy_outbound = node_to_outbound(node, &options.proxy_tag)?;
    let mut outbounds = vec![proxy_outbound];
    outbounds.push(json!({ "type": "direct", "tag": "direct" }));
    outbounds.push(json!({ "type": "dns", "tag": "dns-out" }));

    let mut inbounds = Vec::new();
    if options.include_tun_inbound {
        inbounds.push(tun_inbound(&options.tun_tag, &options.interface_name));
    }

    let route_rules = if options.include_tun_inbound {
        vec![
            json!({ "protocol": "dns", "outbound": "dns-out" }),
            json!({ "inbound": options.tun_tag, "outbound": options.proxy_tag }),
        ]
    } else {
        vec![json!({ "protocol": "dns", "outbound": "dns-out" })]
    };

    Ok(json!({
        "log": { "level": "info" },
        "dns": {
            "servers": [{ "tag": "dns-direct", "address": "1.1.1.1" }]
        },
        "inbounds": inbounds,
        "outbounds": outbounds,
        "route": {
            "rules": route_rules,
            "final": options.proxy_tag
        }
    }))
}

/// Replace sensitive values with stable placeholders for snapshot tests.
pub fn redact_config(config: &Value) -> Value {
    redact_value(config)
}

fn select_node<'a>(
    subscription: &'a ConfluxSubscription,
    selected_node_id: Option<&str>,
) -> Result<&'a ConfluxNode, BackendError> {
    if let Some(id) = selected_node_id {
        return subscription
            .nodes
            .iter()
            .find(|node| node.id == id)
            .ok_or_else(|| BackendError::NodeNotFound(id.to_string()));
    }

    subscription
        .nodes
        .iter()
        .find(|node| is_supported_protocol(node.protocol))
        .ok_or(BackendError::NoNode)
}

fn is_supported_protocol(protocol: Protocol) -> bool {
    matches!(
        protocol,
        Protocol::Vless
            | Protocol::Vmess
            | Protocol::Shadowsocks
            | Protocol::Trojan
            | Protocol::Hysteria2
    )
}

fn tun_inbound(tag: &str, interface_name: &str) -> Value {
    json!({
        "type": "tun",
        "tag": tag,
        "interface_name": interface_name,
        "inet4_address": "172.19.0.1/30",
        "auto_route": true,
        "strict_route": true
    })
}

fn node_to_outbound(node: &ConfluxNode, tag: &str) -> Result<Value, BackendError> {
    match node.protocol {
        Protocol::Vless => Ok(vless_outbound(node, tag)),
        Protocol::Vmess => Ok(vmess_outbound(node, tag)),
        Protocol::Shadowsocks => shadowsocks_outbound(node, tag),
        Protocol::Trojan => Ok(trojan_outbound(node, tag)),
        Protocol::Hysteria2 => Ok(hysteria2_outbound(node, tag)),
        other => Err(BackendError::UnsupportedProtocol(
            other.scheme().to_string(),
        )),
    }
}

fn vless_outbound(node: &ConfluxNode, tag: &str) -> Value {
    let mut outbound = json!({
        "type": "vless",
        "tag": tag,
        "server": node.server,
        "server_port": node.port,
        "uuid": uuid_string(&node.credentials),
    });

    if let Some(flow) = &node.flow {
        outbound
            .as_object_mut()
            .expect("vless outbound object")
            .insert("flow".to_string(), json!(flow));
    }

    merge_transport_and_tls(&mut outbound, node);
    outbound
}

fn vmess_outbound(node: &ConfluxNode, tag: &str) -> Value {
    let security = node
        .encryption
        .clone()
        .unwrap_or_else(|| "auto".to_string());

    let mut outbound = json!({
        "type": "vmess",
        "tag": tag,
        "server": node.server,
        "server_port": node.port,
        "uuid": uuid_string(&node.credentials),
        "security": security,
        "alter_id": 0
    });

    if let Some(packet_encoding) = &node.packet_encoding {
        outbound
            .as_object_mut()
            .expect("vmess outbound object")
            .insert("packet_encoding".to_string(), json!(packet_encoding));
    }

    merge_transport_and_tls(&mut outbound, node);
    outbound
}

fn shadowsocks_outbound(node: &ConfluxNode, tag: &str) -> Result<Value, BackendError> {
    let (method, password) = match &node.credentials {
        Credentials::Shadowsocks { method, password } => (method.clone(), password.clone()),
        _ => {
            return Err(BackendError::Config(
                "shadowsocks node missing method/password credentials".into(),
            ))
        }
    };

    Ok(json!({
        "type": "shadowsocks",
        "tag": tag,
        "server": node.server,
        "server_port": node.port,
        "method": method,
        "password": password
    }))
}

fn trojan_outbound(node: &ConfluxNode, tag: &str) -> Value {
    let mut outbound = json!({
        "type": "trojan",
        "tag": tag,
        "server": node.server,
        "server_port": node.port,
        "password": password_string(&node.credentials),
    });

    merge_transport_and_tls(&mut outbound, node);
    outbound
}

fn hysteria2_outbound(node: &ConfluxNode, tag: &str) -> Value {
    let mut outbound = json!({
        "type": "hysteria2",
        "tag": tag,
        "server": node.server,
        "server_port": node.port,
        "password": password_string(&node.credentials),
    });

    if let Some(ports) = &node.ports {
        outbound
            .as_object_mut()
            .expect("hysteria2 outbound object")
            .insert("server_ports".to_string(), json!(ports));
    }

    if let Some(obfs) = &node.obfs {
        if let Some(obfs_value) = hysteria2_obfs(obfs) {
            outbound
                .as_object_mut()
                .expect("hysteria2 outbound object")
                .insert("obfs".to_string(), obfs_value);
        }
    }

    if let Some(tls) = tls_block(&node.tls, &node.reality) {
        outbound
            .as_object_mut()
            .expect("hysteria2 outbound object")
            .insert("tls".to_string(), tls);
    }

    outbound
}

fn hysteria2_obfs(obfs: &ObfsConfig) -> Option<Value> {
    if obfs.kind.is_empty() {
        return None;
    }

    let mut value = json!({ "type": obfs.kind });
    if let Some(password) = &obfs.password {
        value
            .as_object_mut()
            .expect("hysteria2 obfs object")
            .insert("password".to_string(), json!(password));
    }
    Some(value)
}

fn merge_transport_and_tls(outbound: &mut Value, node: &ConfluxNode) {
    if let Some(transport) = transport_block(&node.transport) {
        outbound
            .as_object_mut()
            .expect("outbound object")
            .insert("transport".to_string(), transport);
    }

    if let Some(tls) = tls_block(&node.tls, &node.reality) {
        outbound
            .as_object_mut()
            .expect("outbound object")
            .insert("tls".to_string(), tls);
    }
}

fn transport_block(transport: &Transport) -> Option<Value> {
    let transport_type = match transport.kind {
        TransportKind::Tcp => return None,
        TransportKind::Ws => "ws",
        TransportKind::Grpc => "grpc",
        TransportKind::Http => "http",
        TransportKind::HttpUpgrade => "httpupgrade",
        TransportKind::Kcp => "mkcp",
        TransportKind::Quic => "quic",
        TransportKind::H2 => "http",
    };

    let mut block = json!({ "type": transport_type });

    if let Some(path) = &transport.path {
        block
            .as_object_mut()
            .expect("transport object")
            .insert("path".to_string(), json!(path));
    }

    if let Some(host) = &transport.host {
        block
            .as_object_mut()
            .expect("transport object")
            .insert("host".to_string(), json!(host));
    }

    if let Some(service_name) = &transport.service_name {
        block
            .as_object_mut()
            .expect("transport object")
            .insert("service_name".to_string(), json!(service_name));
    }

    if !transport.headers.is_empty() {
        let headers: HashMap<&str, &str> = transport
            .headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        block
            .as_object_mut()
            .expect("transport object")
            .insert("headers".to_string(), json!(headers));
    }

    if let Some(header_type) = &transport.header_type {
        block
            .as_object_mut()
            .expect("transport object")
            .insert("header".to_string(), json!({ "type": header_type }));
    }

    Some(block)
}

fn tls_block(tls: &Option<TlsConfig>, reality: &Option<RealityConfig>) -> Option<Value> {
    let tls = tls.as_ref()?;
    if !tls.enabled && reality.is_none() {
        return None;
    }

    let mut block = json!({
        "enabled": tls.enabled || reality.is_some(),
    });

    let obj = block.as_object_mut().expect("tls object");

    if let Some(sni) = &tls.sni {
        obj.insert("server_name".to_string(), json!(sni));
    }

    if !tls.alpn.is_empty() {
        obj.insert("alpn".to_string(), json!(tls.alpn));
    }

    if tls.insecure {
        obj.insert("insecure".to_string(), json!(true));
    }

    if let Some(fingerprint) = &tls.fingerprint {
        obj.insert(
            "utls".to_string(),
            json!({
                "enabled": true,
                "fingerprint": fingerprint
            }),
        );
    }

    if let Some(reality) = reality {
        obj.insert("reality".to_string(), reality_block(reality));
    }

    Some(block)
}

fn reality_block(reality: &RealityConfig) -> Value {
    let mut block = json!({
        "enabled": true,
        "public_key": reality.public_key,
        "short_id": reality.short_id,
    });

    if let Some(spider_x) = &reality.spider_x {
        block
            .as_object_mut()
            .expect("reality object")
            .insert("spider_x".to_string(), json!(spider_x));
    }

    block
}

fn uuid_string(credentials: &Credentials) -> String {
    match credentials {
        Credentials::Uuid { id } => id.to_string(),
        _ => String::new(),
    }
}

fn password_string(credentials: &Credentials) -> String {
    match credentials {
        Credentials::Password { password } => password.clone(),
        Credentials::Shadowsocks { password, .. } => password.clone(),
        _ => String::new(),
    }
}

fn redact_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut redacted = Map::new();
            for (key, child) in map {
                redacted.insert(key.clone(), redact_field(key, child));
            }
            Value::Object(redacted)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_value).collect()),
        other => other.clone(),
    }
}

fn redact_field(key: &str, value: &Value) -> Value {
    match key {
        "uuid" | "password" | "private_key" | "public_key" | "short_id" | "key_hex" => {
            json!(format!("<REDACTED_{}>", key.to_ascii_uppercase()))
        }
        "server" => json!("<REDACTED_SERVER>"),
        "server_name" | "host" | "sni" => json!("<REDACTED_HOST>"),
        _ => redact_value(value),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use conflux_protocol::Protocol;

    fn load_fixture_subscription() -> ConfluxSubscription {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/nodes.json");
        let body = fs::read_to_string(path).expect("fixture nodes.json");
        serde_json::from_str(&body).expect("parse fixture subscription")
    }

    #[test]
    fn generate_all_protocol_outbounds_snapshot() {
        let subscription = load_fixture_subscription();
        let protocols = [
            Protocol::Vless,
            Protocol::Vmess,
            Protocol::Shadowsocks,
            Protocol::Trojan,
            Protocol::Hysteria2,
        ];

        for protocol in protocols {
            let node_id = match protocol {
                Protocol::Shadowsocks => "fixture-shadowsocks".to_string(),
                other => format!("fixture-{}", other.scheme()),
            };
            let mut options = GenerateOptions::for_windows();
            options.selected_node_id = Some(node_id);

            let config = generate_config(&subscription, &options).unwrap();
            let redacted = redact_config(&config);

            insta::assert_json_snapshot!(format!("{:?}", protocol).to_ascii_lowercase(), redacted);
        }
    }
}
