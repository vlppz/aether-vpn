//! Generation of complete Xray-core JSON configs.
//!
//! Field names target Xray-core 26.x: `raw` transport, `finalmask` for mKCP
//! headers and Hysteria obfuscation, and `pinnedPeerCertSha256` instead of
//! the removed `allowInsecure`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::error::{Error, Result};
use crate::model::{Network, Outbound, Security, Server, Stream, Tls};
use crate::routing::{self, RoutingSettings, TAG_BLOCK, TAG_DIRECT, TAG_PROXY};

pub const TAG_DNS_OUT: &str = "dns-out";
pub const TAG_TUN_IN: &str = "tun-in";
pub const TAG_SOCKS_IN: &str = "socks-in";
pub const TAG_HTTP_IN: &str = "http-in";
/// Resolver announced to the OS in TUN mode. It is routed into the tunnel,
/// where the port-53 hijack rule hands queries to Xray's DNS module. A public
/// resolver is used on purpose: if restoring DNS ever failed, it still works
/// without the VPN.
pub const TUN_DNS_IP: &str = "1.1.1.1";

/// Routes sent into the TUN. A `0.0.0.0/0` route cannot take over from the
/// existing default route (macOS keeps using the physical interface), so the
/// address space is covered with more specific prefixes instead, which always
/// win over the default route. `0.0.0.0/8` is not routable and left out.
pub const TUN_ROUTES_V4: [&str; 8] = [
    "1.0.0.0/8",
    "2.0.0.0/7",
    "4.0.0.0/6",
    "8.0.0.0/5",
    "16.0.0.0/4",
    "32.0.0.0/3",
    "64.0.0.0/2",
    "128.0.0.0/1",
];
/// Global unicast IPv6 only, so loopback/link-local stay untouched. Always
/// captured, so IPv6 can never bypass the tunnel.
pub const TUN_ROUTES_V6: [&str; 1] = ["2000::/3"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelMode {
    /// Local SOCKS/HTTP inbounds + OS system proxy.
    #[default]
    Proxy,
    /// Virtual interface capturing all traffic (requires elevation).
    Tun,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TunSettings {
    /// Interface name; on macOS it must look like `utunN`.
    pub name: String,
    pub mtu: u32,
    pub ipv6: bool,
}

impl Default for TunSettings {
    fn default() -> Self {
        Self {
            name: if cfg!(target_os = "macos") { "utun233".into() } else { "aether0".into() },
            mtu: 1500,
            ipv6: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DnsSettings {
    /// Resolver used for proxied domains (reached through the proxy).
    pub remote: String,
    /// Plain resolver IP used for direct (region-local) domains.
    pub direct: String,
    /// TUN mode only: resolve every domain with `direct`, outside the tunnel.
    /// Much faster on slow transports (e.g. XHTTP through a CDN), but the
    /// local network sees which domains are looked up.
    pub tun_direct: bool,
}

impl Default for DnsSettings {
    fn default() -> Self {
        Self { remote: "https://1.1.1.1/dns-query".into(), direct: "77.88.8.8".into(), tun_direct: false }
    }
}

fn tun_dns_direct(s: &CoreSettings) -> bool {
    s.mode == TunnelMode::Tun && s.dns.tun_direct
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CoreSettings {
    pub mode: TunnelMode,
    pub socks_port: u16,
    pub http_port: u16,
    /// Local port of Xray's metrics endpoint (traffic counters).
    pub metrics_port: u16,
    /// Listen on 0.0.0.0 so other devices on the LAN can use the proxy.
    pub allow_lan: bool,
    pub sniffing: bool,
    /// `debug`, `info`, `warning`, `error` or `none`.
    pub log_level: String,
    pub access_log: bool,
    pub tun: TunSettings,
    pub dns: DnsSettings,
    pub routing: RoutingSettings,
}

impl Default for CoreSettings {
    fn default() -> Self {
        Self {
            mode: TunnelMode::Proxy,
            // 10808/10809 are taken by Happ/v2rayN on many machines
            socks_port: 11808,
            http_port: 11809,
            metrics_port: 11813,
            allow_lan: false,
            sniffing: true,
            log_level: "warning".into(),
            access_log: true,
            tun: TunSettings::default(),
            dns: DnsSettings::default(),
            routing: RoutingSettings::default(),
        }
    }
}

/// Builds the complete config Xray will run for `server`.
pub fn build_xray_config(server: &Server, s: &CoreSettings) -> Result<Value> {
    if let Outbound::Custom { config } = &server.outbound {
        return build_custom(config, s);
    }

    let mut rules = Vec::new();
    rules.extend(dns_rules(s));
    rules.extend(routing::build_rules(&s.routing));

    Ok(json!({
        "log": log(s),
        "inbounds": inbounds(s),
        "outbounds": [
            outbound_json(&server.outbound, TAG_PROXY)?,
            { "tag": TAG_DIRECT, "protocol": "freedom", "settings": { "domainStrategy": ip_strategy(s) } },
            { "tag": TAG_BLOCK, "protocol": "blackhole" },
            { "tag": TAG_DNS_OUT, "protocol": "dns" },
        ],
        "dns": dns(s),
        "routing": { "domainStrategy": s.routing.domain_strategy, "rules": rules },
        "stats": {},
        "metrics": metrics(s),
        "policy": policy(),
    }))
}

/// Wraps a user-provided full config (e.g. a Remnawave JSON subscription):
/// its outbounds and routing are kept, our inbounds, metrics and DNS
/// handling are injected, and the user's routing preset + custom rules are
/// placed *before* the config's own rules — the user's choice wins, the
/// provider's rules still apply to everything the user did not decide.
fn build_custom(config: &Value, s: &CoreSettings) -> Result<Value> {
    let mut c = config
        .as_object()
        .cloned()
        .ok_or_else(|| Error::InvalidConfig("custom config must be a JSON object".into()))?;
    let outbounds = c
        .get_mut("outbounds")
        .and_then(Value::as_array_mut)
        .filter(|o| !o.is_empty())
        .ok_or_else(|| Error::InvalidConfig("custom config has no outbounds".into()))?;

    // The first outbound is the config's default route = "the proxy".
    if outbounds[0]["tag"].as_str().is_none_or(str::is_empty) {
        outbounds[0]["tag"] = TAG_PROXY.into();
    }
    let proxy_tag = outbounds[0]["tag"].as_str().unwrap_or(TAG_PROXY).to_string();
    for (tag, protocol) in [(TAG_DIRECT, "freedom"), (TAG_BLOCK, "blackhole"), (TAG_DNS_OUT, "dns")] {
        if !outbounds.iter().any(|o| o["tag"] == tag) {
            let mut ob = json!({ "tag": tag, "protocol": protocol });
            if protocol == "freedom" {
                ob["settings"] = json!({ "domainStrategy": ip_strategy(s) });
            }
            outbounds.push(ob);
        }
    }

    c.insert("inbounds".into(), Value::Array(inbounds(s)));
    c.insert("stats".into(), json!({}));
    c.insert("metrics".into(), metrics(s));
    c.insert("policy".into(), policy());
    c.entry("log").or_insert_with(|| log(s));
    // Our `remarks`-style metadata is not an Xray key.
    c.remove("remarks");

    if s.mode == TunnelMode::Tun {
        let dns_cfg = c.entry("dns").or_insert_with(|| dns(s));
        if !s.tun.ipv6 {
            if let Some(d) = dns_cfg.as_object_mut() {
                d.insert("queryStrategy".into(), "UseIPv4".into());
            }
        }
    }

    let mut ours = dns_rules(s);
    ours.extend(routing::build_rules(&s.routing).into_iter().map(|mut r| {
        if r["outboundTag"] == TAG_PROXY {
            r["outboundTag"] = proxy_tag.as_str().into();
        }
        r
    }));
    let routing = c
        .entry("routing")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| Error::InvalidConfig("routing must be an object".into()))?;
    // geoip rules need domains resolved to match
    routing.entry("domainStrategy").or_insert_with(|| s.routing.domain_strategy.clone().into());
    let rules = routing
        .entry("rules")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| Error::InvalidConfig("routing.rules must be an array".into()))?;
    rules.splice(0..0, ours);
    Ok(Value::Object(c))
}

fn ip_strategy(s: &CoreSettings) -> &'static str {
    if s.tun.ipv6 {
        "UseIP"
    } else {
        "UseIPv4"
    }
}

fn log(s: &CoreSettings) -> Value {
    let mut log = json!({ "loglevel": s.log_level, "dnsLog": false });
    if !s.access_log {
        log["access"] = "none".into();
    }
    log
}

fn metrics(s: &CoreSettings) -> Value {
    json!({ "tag": "metrics", "listen": format!("127.0.0.1:{}", s.metrics_port) })
}

fn policy() -> Value {
    json!({
        "system": {
            "statsOutboundUplink": true,
            "statsOutboundDownlink": true,
        }
    })
}

fn sniffing(s: &CoreSettings) -> Value {
    json!({ "enabled": s.sniffing, "destOverride": ["http", "tls", "quic"] })
}

fn inbounds(s: &CoreSettings) -> Vec<Value> {
    let listen = if s.allow_lan { "0.0.0.0" } else { "127.0.0.1" };
    let mut v = vec![
        json!({
            "tag": TAG_SOCKS_IN,
            "listen": listen,
            "port": s.socks_port,
            "protocol": "socks",
            "settings": { "udp": true, "auth": "noauth" },
            "sniffing": sniffing(s),
        }),
        json!({
            "tag": TAG_HTTP_IN,
            "listen": listen,
            "port": s.http_port,
            "protocol": "http",
            "settings": {},
            "sniffing": sniffing(s),
        }),
    ];
    if s.mode == TunnelMode::Tun {
        // Xray 26 only understands name/MTU here; addresses, routes and DNS
        // are configured by aether-helper.
        v.insert(
            0,
            json!({
                "tag": TAG_TUN_IN,
                "port": 0,
                "protocol": "tun",
                "settings": { "name": s.tun.name, "mtu": s.tun.mtu },
                "sniffing": sniffing(s),
            }),
        );
    }
    v
}

/// Extracts a bare IP/host from resolver strings like `https://1.1.1.1/dns-query`.
fn dns_ip(server: &str) -> String {
    let s = server.split("://").last().unwrap_or(server);
    let s = s.split('/').next().unwrap_or(s);
    match s.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) && !h.contains(':') => h.to_string(),
        _ => s.to_string(),
    }
}

fn dns(s: &CoreSettings) -> Value {
    let mut servers = vec![json!(s.dns.remote)];
    if tun_dns_direct(s) {
        servers = vec![json!(s.dns.direct)];
    } else if s.routing.preset != crate::RoutingPreset::Global {
        let (sites, _) = routing::region_lists(&s.routing.region);
        let mut domains = vec!["geosite:private".to_string()];
        domains.extend(sites);
        servers.push(json!({ "address": s.dns.direct, "domains": domains, "skipFallback": true }));
    }
    json!({
        "servers": servers,
        "queryStrategy": if s.tun.ipv6 { "UseIP" } else { "UseIPv4" },
    })
}

/// In TUN mode, system DNS queries arrive on port 53 and are answered by
/// Xray's DNS module; the direct resolver itself must bypass the proxy.
fn dns_rules(s: &CoreSettings) -> Vec<Value> {
    let mut rules = Vec::new();
    if s.mode == TunnelMode::Tun {
        rules.push(json!({ "inboundTag": [TAG_TUN_IN], "port": "53", "outboundTag": TAG_DNS_OUT }));
        if !s.tun.ipv6 {
            // IPv6 is captured by the TUN so it cannot bypass the tunnel. With
            // IPv6 disabled, DNS hands out only A records; sniffed connections
            // are proxied by name, and whatever is left (raw v6 literals) is
            // refused so apps fall back to IPv4 instead of leaking.
            rules.push(json!({ "inboundTag": [TAG_TUN_IN], "ip": ["::/0"], "outboundTag": TAG_BLOCK }));
        }
    }
    if s.routing.preset != crate::RoutingPreset::Global || tun_dns_direct(s) {
        rules.push(json!({ "ip": [dns_ip(&s.dns.direct)], "port": "53", "outboundTag": TAG_DIRECT }));
    }
    rules
}

/// Converts an [`Outbound`] into an Xray outbound object.
pub fn outbound_json(o: &Outbound, tag: &str) -> Result<Value> {
    let v = match o {
        Outbound::Vless { address, port, id, flow, encryption, stream } => {
            let mut user = json!({ "id": id, "encryption": encryption });
            if let Some(flow) = flow {
                user["flow"] = flow.as_str().into();
            }
            json!({
                "tag": tag,
                "protocol": "vless",
                "settings": { "vnext": [{ "address": address, "port": port, "users": [user] }] },
                "streamSettings": stream_json(stream),
            })
        }
        Outbound::Vmess { address, port, id, alter_id, security, stream } => json!({
            "tag": tag,
            "protocol": "vmess",
            "settings": { "vnext": [{
                "address": address,
                "port": port,
                "users": [{ "id": id, "alterId": alter_id, "security": security }],
            }] },
            "streamSettings": stream_json(stream),
        }),
        Outbound::Trojan { address, port, password, stream } => json!({
            "tag": tag,
            "protocol": "trojan",
            "settings": { "servers": [{ "address": address, "port": port, "password": password }] },
            "streamSettings": stream_json(stream),
        }),
        Outbound::Shadowsocks { address, port, method, password, stream } => json!({
            "tag": tag,
            "protocol": "shadowsocks",
            "settings": { "servers": [{
                "address": address, "port": port, "method": method, "password": password,
            }] },
            "streamSettings": stream_json(stream),
        }),
        Outbound::Socks { address, port, username, password } => {
            let mut server = json!({ "address": address, "port": port });
            if let Some(user) = username {
                server["users"] = json!([{ "user": user, "pass": password.clone().unwrap_or_default() }]);
            }
            json!({ "tag": tag, "protocol": "socks", "settings": { "servers": [server] } })
        }
        Outbound::Hysteria2 { address, port, auth, tls, obfs_password, hop_ports, up, down } => {
            let mut stream = json!({
                "network": "hysteria",
                "security": "tls",
                "tlsSettings": tls_json(tls, &["h3"]),
                "hysteriaSettings": { "version": 2, "auth": auth },
            });
            let mut finalmask = Map::new();
            if let Some(pw) = obfs_password {
                finalmask.insert("udp".into(), json!([{ "type": "salamander", "settings": { "password": pw } }]));
            }
            let mut quic = Map::new();
            if let Some(ports) = hop_ports {
                quic.insert("udpHop".into(), json!({ "ports": ports, "interval": 30 }));
            }
            if up.is_some() || down.is_some() {
                quic.insert("congestion".into(), "brutal".into());
                if let Some(up) = up {
                    quic.insert("brutalUp".into(), bandwidth(up).into());
                }
                if let Some(down) = down {
                    quic.insert("brutalDown".into(), bandwidth(down).into());
                }
            }
            if !quic.is_empty() {
                finalmask.insert("quicParams".into(), Value::Object(quic));
            }
            if !finalmask.is_empty() {
                stream["finalmask"] = Value::Object(finalmask);
            }
            json!({
                "tag": tag,
                "protocol": "hysteria",
                "settings": { "version": 2, "address": address, "port": port },
                "streamSettings": stream,
            })
        }
        Outbound::Custom { .. } => {
            return Err(Error::InvalidConfig("custom configs have no single outbound".into()))
        }
    };
    Ok(v)
}

/// Hysteria links carry bare numbers (Mbps) or strings like `100 mbps`.
fn bandwidth(v: &str) -> String {
    if v.chars().all(|c| c.is_ascii_digit()) {
        format!("{v} mbps")
    } else {
        v.to_string()
    }
}

fn tls_json(tls: &Tls, default_alpn: &[&str]) -> Value {
    let mut v = json!({});
    if let Some(sni) = &tls.sni {
        v["serverName"] = sni.as_str().into();
    }
    if !tls.alpn.is_empty() {
        v["alpn"] = tls.alpn.clone().into();
    } else if !default_alpn.is_empty() {
        v["alpn"] = default_alpn.into();
    }
    if let Some(fp) = &tls.fingerprint {
        v["fingerprint"] = fp.as_str().into();
    }
    if let Some(ech) = &tls.ech_config_list {
        v["echConfigList"] = ech.as_str().into();
    }
    // Xray 26 removed `allowInsecure`; certificate pinning is the replacement.
    if let Some(pin) = &tls.pinned_sha256 {
        v["pinnedPeerCertSha256"] = pin.replace(':', "").to_lowercase().into();
    }
    v
}

fn stream_json(stream: &Stream) -> Value {
    let mut v = json!({ "network": stream.network.name() });
    let mut udp_masks: Vec<Value> = Vec::new();
    match &stream.network {
        Network::Tcp { header_type, host, path } => {
            v["network"] = "raw".into();
            if header_type.as_deref() == Some("http") {
                let mut request = json!({ "path": [path.clone().unwrap_or_else(|| "/".into())] });
                if let Some(host) = host {
                    request["headers"] = json!({ "Host": host.split(',').map(str::trim).collect::<Vec<_>>() });
                }
                v["rawSettings"] = json!({ "header": { "type": "http", "request": request } });
            }
        }
        Network::Ws { path, host } => {
            let mut ws = json!({ "path": path });
            if let Some(host) = host {
                ws["host"] = host.as_str().into();
            }
            v["wsSettings"] = ws;
        }
        Network::Grpc { service_name, authority, multi_mode } => {
            let mut g = json!({ "serviceName": service_name, "multiMode": multi_mode });
            if let Some(a) = authority {
                g["authority"] = a.as_str().into();
            }
            v["grpcSettings"] = g;
        }
        Network::HttpUpgrade { path, host } => {
            let mut h = json!({ "path": path });
            if let Some(host) = host {
                h["host"] = host.as_str().into();
            }
            v["httpupgradeSettings"] = h;
        }
        Network::Xhttp { path, host, mode, extra } => {
            let mut x = json!({ "path": path, "mode": mode.clone().unwrap_or_else(|| "auto".into()) });
            if let Some(host) = host {
                x["host"] = host.as_str().into();
            }
            if let Some(extra) = extra {
                x["extra"] = extra.clone();
            }
            v["xhttpSettings"] = x;
        }
        Network::Kcp { header_type, seed } => {
            // Xray 26 moved mKCP header/seed into finalmask udp masks.
            udp_masks.push(match seed {
                Some(seed) => json!({ "type": "mkcp-aes128gcm", "settings": { "password": seed } }),
                None => json!({ "type": "mkcp-original" }),
            });
            if let Some(h) = header_type {
                let h = if h == "wechat-video" { "wechat" } else { h.as_str() };
                udp_masks.push(json!({ "type": format!("header-{h}") }));
            }
        }
    }
    match &stream.security {
        Security::None => {}
        Security::Tls(tls) => {
            v["security"] = "tls".into();
            v["tlsSettings"] = tls_json(tls, &[]);
        }
        Security::Reality(r) => {
            v["security"] = "reality".into();
            let mut rs = json!({
                "publicKey": r.public_key,
                "fingerprint": r.fingerprint.clone().unwrap_or_else(|| "chrome".into()),
            });
            if let Some(sni) = &r.sni {
                rs["serverName"] = sni.as_str().into();
            }
            if let Some(sid) = &r.short_id {
                rs["shortId"] = sid.as_str().into();
            }
            if let Some(spx) = &r.spider_x {
                rs["spiderX"] = spx.as_str().into();
            }
            if let Some(pqv) = &r.mldsa65_verify {
                rs["mldsa65Verify"] = pqv.as_str().into();
            }
            v["realitySettings"] = rs;
        }
    }
    let mut finalmask = stream.finalmask.clone().unwrap_or_else(|| json!({}));
    if !udp_masks.is_empty() {
        let existing = finalmask["udp"].as_array().cloned().unwrap_or_default();
        udp_masks.extend(existing);
        finalmask["udp"] = udp_masks.into();
    }
    if finalmask.as_object().is_some_and(|m| !m.is_empty()) {
        v["finalmask"] = finalmask;
    }
    v
}

#[cfg(test)]
mod tests;
