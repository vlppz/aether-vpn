use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::country;

/// A single connectable endpoint, either imported from a share link, a
/// subscription, or a raw Xray JSON config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_id: Option<String>,
    pub outbound: Outbound,
}

impl Server {
    pub fn new(name: impl Into<String>, outbound: Outbound) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            subscription_id: None,
            outbound,
        }
    }

    /// ISO 3166-1 alpha-2 code guessed from the server name (flag emoji,
    /// `[DE]`-style tags or country names).
    pub fn country(&self) -> Option<String> {
        country::detect(&self.name)
    }

    /// Identity used to deduplicate servers: same protocol, endpoint and credentials.
    pub fn fingerprint(&self) -> String {
        // the display name is deliberately not part of the identity
        serde_json::to_string(&self.outbound).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "protocol", rename_all = "lowercase")]
pub enum Outbound {
    #[serde(rename_all = "camelCase")]
    Vless {
        address: String,
        port: u16,
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        flow: Option<String>,
        #[serde(default = "default_encryption")]
        encryption: String,
        stream: Stream,
    },
    #[serde(rename_all = "camelCase")]
    Vmess {
        address: String,
        port: u16,
        id: String,
        #[serde(default)]
        alter_id: u32,
        #[serde(default = "default_vmess_security")]
        security: String,
        stream: Stream,
    },
    #[serde(rename_all = "camelCase")]
    Trojan {
        address: String,
        port: u16,
        password: String,
        stream: Stream,
    },
    #[serde(rename_all = "camelCase")]
    Shadowsocks {
        address: String,
        port: u16,
        method: String,
        password: String,
        #[serde(default)]
        stream: Stream,
    },
    #[serde(rename_all = "camelCase")]
    Socks {
        address: String,
        port: u16,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        username: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        password: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Hysteria2 {
        address: String,
        port: u16,
        auth: String,
        tls: Tls,
        /// Salamander obfuscation password.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        obfs_password: Option<String>,
        /// Port-hopping range, e.g. `20000-50000`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hop_ports: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        up: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        down: Option<String>,
    },
    /// A complete Xray config supplied by the user or a JSON subscription.
    Custom { config: Value },
}

fn default_encryption() -> String {
    "none".into()
}

fn default_vmess_security() -> String {
    "auto".into()
}

impl Outbound {
    pub fn protocol_name(&self) -> &'static str {
        match self {
            Outbound::Vless { .. } => "VLESS",
            Outbound::Vmess { .. } => "VMess",
            Outbound::Trojan { .. } => "Trojan",
            Outbound::Shadowsocks { .. } => "Shadowsocks",
            Outbound::Socks { .. } => "SOCKS",
            Outbound::Hysteria2 { .. } => "Hysteria2",
            Outbound::Custom { .. } => "JSON",
        }
    }

    /// Host and port of the remote server, if known.
    pub fn endpoint(&self) -> Option<(String, u16)> {
        match self {
            Outbound::Vless { address, port, .. }
            | Outbound::Vmess { address, port, .. }
            | Outbound::Trojan { address, port, .. }
            | Outbound::Shadowsocks { address, port, .. }
            | Outbound::Socks { address, port, .. }
            | Outbound::Hysteria2 { address, port, .. } => Some((address.clone(), *port)),
            Outbound::Custom { config } => custom_endpoint(config),
        }
    }

    pub fn stream(&self) -> Option<&Stream> {
        match self {
            Outbound::Vless { stream, .. }
            | Outbound::Vmess { stream, .. }
            | Outbound::Trojan { stream, .. }
            | Outbound::Shadowsocks { stream, .. } => Some(stream),
            _ => None,
        }
    }

    /// Short transport label for badges, e.g. `ws`, `grpc`, `quic`.
    pub fn transport_name(&self) -> &'static str {
        match self {
            Outbound::Hysteria2 { .. } => "quic",
            Outbound::Socks { .. } => "tcp",
            Outbound::Custom { .. } => "custom",
            o => o.stream().map(|s| s.network.name()).unwrap_or("tcp"),
        }
    }

    /// Short security label for badges.
    pub fn security_name(&self) -> &'static str {
        match self {
            Outbound::Hysteria2 { .. } => "tls",
            Outbound::Shadowsocks { .. } => "aead",
            Outbound::Socks { .. } | Outbound::Custom { .. } => "none",
            o => match o.stream().map(|s| &s.security) {
                Some(Security::Tls(_)) => "tls",
                Some(Security::Reality(_)) => "reality",
                _ => "none",
            },
        }
    }
}

/// Finds the first proxy outbound endpoint inside a raw Xray config.
fn custom_endpoint(config: &Value) -> Option<(String, u16)> {
    let outbounds = config.get("outbounds")?.as_array()?;
    for ob in outbounds {
        let settings = ob.get("settings")?;
        let server = settings
            .get("vnext")
            .or_else(|| settings.get("servers"))
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .unwrap_or(settings);
        if let (Some(addr), Some(port)) = (
            server.get("address").and_then(Value::as_str),
            server.get("port").and_then(Value::as_u64),
        ) {
            return Some((addr.to_string(), port as u16));
        }
    }
    None
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stream {
    #[serde(default)]
    pub network: Network,
    #[serde(default)]
    pub security: Security,
    /// Raw `finalmask` object (share-link parameter `fm`), passed through as-is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finalmask: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Network {
    #[serde(rename_all = "camelCase")]
    Tcp {
        /// `http` enables HTTP header obfuscation.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        header_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        host: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Ws {
        #[serde(default)]
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        host: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Grpc {
        #[serde(default)]
        service_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        authority: Option<String>,
        #[serde(default)]
        multi_mode: bool,
    },
    #[serde(rename_all = "camelCase")]
    HttpUpgrade {
        #[serde(default)]
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        host: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Xhttp {
        #[serde(default)]
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        host: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extra: Option<Value>,
    },
    #[serde(rename_all = "camelCase")]
    Kcp {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        header_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seed: Option<String>,
    },
}

impl Default for Network {
    fn default() -> Self {
        Network::Tcp { header_type: None, host: None, path: None }
    }
}

impl Network {
    pub fn name(&self) -> &'static str {
        match self {
            Network::Tcp { .. } => "tcp",
            Network::Ws { .. } => "ws",
            Network::Grpc { .. } => "grpc",
            Network::HttpUpgrade { .. } => "httpupgrade",
            Network::Xhttp { .. } => "xhttp",
            Network::Kcp { .. } => "kcp",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Security {
    #[default]
    None,
    Tls(Tls),
    Reality(Reality),
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tls {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alpn: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(default)]
    pub allow_insecure: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ech_config_list: Option<String>,
    /// SHA-256 pin of the leaf certificate (hex), used by Hysteria2 links.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_sha256: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reality {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    pub public_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spider_x: Option<String>,
    /// ML-DSA-65 verification key (`pqv` link parameter).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mldsa65_verify: Option<String>,
}

/// Traffic quota published by a subscription via `subscription-userinfo`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub upload: u64,
    pub download: u64,
    /// 0 means unlimited.
    pub total: u64,
    /// Unix timestamp (seconds); `None` means never expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expire: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: String,
    pub name: String,
    /// Set when the user renamed the subscription; `profile-title` then no
    /// longer overrides the name.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub name_locked: bool,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    #[serde(default = "default_true")]
    pub auto_update: bool,
    /// Refresh interval in hours (from `profile-update-interval` or user choice).
    #[serde(default = "default_interval")]
    pub update_interval_hours: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<UserInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub announce: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_page_url: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_interval() -> u32 {
    12
}

impl Subscription {
    pub fn new(url: impl Into<String>) -> Self {
        let url = url.into();
        let name = url::Url::parse(&url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_else(|| "Subscription".into());
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            name_locked: false,
            url,
            user_agent: None,
            auto_update: true,
            update_interval_hours: default_interval(),
            last_updated: None,
            last_error: None,
            info: None,
            announce: None,
            support_url: None,
            web_page_url: None,
        }
    }
}
