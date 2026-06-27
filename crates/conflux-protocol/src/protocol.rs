use serde::{Deserialize, Serialize};

/// Supported proxy protocol kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    Vless,
    Vmess,
    Shadowsocks,
    Trojan,
    Hysteria2,
    Tuic,
    WireGuard,
    Socks,
    /// Native tunnel protocol carried in custom URI envelopes.
    NativeTunnel,
    Unknown,
}

impl Protocol {
    pub fn from_scheme(scheme: &str) -> Self {
        match scheme.to_ascii_lowercase().as_str() {
            "vless" => Self::Vless,
            "vmess" => Self::Vmess,
            "ss" | "shadowsocks" => Self::Shadowsocks,
            "trojan" => Self::Trojan,
            "hysteria2" | "hy2" => Self::Hysteria2,
            "tuic" => Self::Tuic,
            "wireguard" | "wg" => Self::WireGuard,
            "socks" | "socks5" => Self::Socks,
            "himera" => Self::NativeTunnel,
            _ => Self::Unknown,
        }
    }

    pub fn scheme(self) -> &'static str {
        match self {
            Self::Vless => "vless",
            Self::Vmess => "vmess",
            Self::Shadowsocks => "ss",
            Self::Trojan => "trojan",
            Self::Hysteria2 => "hysteria2",
            Self::Tuic => "tuic",
            Self::WireGuard => "wireguard",
            Self::Socks => "socks",
            Self::NativeTunnel => "himera",
            Self::Unknown => "unknown",
        }
    }
}
